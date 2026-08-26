#!/usr/bin/env python3
"""Pinned SciPy worker for context-gated mixture-of-experts fusion."""

from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path
import sys
from typing import Any

import numpy as np
import scipy
from scipy.optimize import minimize
from scipy.special import expit


SCIPY_VERSION = "1.18.1"


def exact_object(value: Any, keys: set[str], path: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        raise ValueError(f"{path} fields differ")
    return value


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = exact_object(
        request,
        {"format", "version", "backend", "patients", "context_names", "expert_names", "l2_penalty", "entropy_regularization", "ood_validation_quantile", "resources"},
        "request",
    )
    if request["format"] != "marklab.scipy_mixture_of_experts_request" or request["version"] != 1:
        raise ValueError("request identity differs")
    backend = exact_object(
        request["backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "backend",
    )
    if (
        backend["name"] != "scipy"
        or backend["version"] != SCIPY_VERSION
        or backend["python_version"] != "3.12"
        or backend["environment_lock_sha256"] != lock_digest
        or backend["worker_sha256"] != worker_digest
        or scipy.__version__ != SCIPY_VERSION
    ):
        raise ValueError("backend identity differs")
    keys = {"patient_id", "split", "expert_prediction_source", "label", "context", "expert_probabilities"}
    for index, raw in enumerate(request["patients"]):
        row = exact_object(raw, keys, f"patients[{index}]")
        if (
            row["split"] not in {"gate_train", "calibration", "test"}
            or row["expert_prediction_source"] != "patient_level_out_of_fold"
            or row["label"] not in {0, 1}
        ):
            raise ValueError("patient differs")
    exact_object(
        request["resources"],
        {"maximum_patients", "maximum_context_features", "maximum_experts", "maximum_output_bytes", "timeout_seconds"},
        "resources",
    )
    return request


def gate_features(row: dict[str, Any], mean: np.ndarray, sd: np.ndarray) -> np.ndarray:
    context = (np.asarray(row["context"], dtype=np.float64) - mean) / sd
    available = np.asarray([value is not None for value in row["expert_probabilities"]], dtype=np.float64)
    return np.concatenate([context, available])


def gate_weights(coefficients: np.ndarray, features: np.ndarray, probabilities: list[float | None]) -> np.ndarray:
    logits = coefficients[:, 0] + coefficients[:, 1:] @ features
    available = np.asarray([value is not None for value in probabilities])
    logits = np.where(available, logits, -np.inf)
    maximum = np.max(logits)
    weights = np.where(available, np.exp(logits - maximum), 0.0)
    return weights / np.sum(weights)


def fit_gate(rows: list[dict[str, Any]], mean: np.ndarray, sd: np.ndarray, experts: int, l2: float, entropy_regularization: float) -> np.ndarray:
    columns = 1 + len(mean) + experts

    def objective(flat: np.ndarray) -> float:
        coefficients = flat.reshape(experts, columns)
        loss = 0.0
        entropy = 0.0
        for row in rows:
            weights = gate_weights(coefficients, gate_features(row, mean, sd), row["expert_probabilities"])
            probabilities = np.asarray([0.0 if value is None else value for value in row["expert_probabilities"]])
            mixture = float(np.clip(np.dot(weights, probabilities), 1e-12, 1.0 - 1e-12))
            label = row["label"]
            loss -= label * math.log(mixture) + (1 - label) * math.log(1.0 - mixture)
            positive = weights[weights > 0.0]
            entropy -= float(np.sum(positive * np.log(positive)))
        return loss + 0.5 * l2 * float(np.dot(flat, flat)) - entropy_regularization * entropy

    fitted = minimize(
        objective,
        np.zeros(experts * columns),
        method="L-BFGS-B",
        options={"maxiter": 2_000, "ftol": 1e-12, "gtol": 1e-7, "maxls": 50},
    )
    if not fitted.success or not np.all(np.isfinite(fitted.x)):
        raise ValueError(f"mixture gate optimizer failed: {fitted.message}")
    return np.asarray(fitted.x, dtype=np.float64).reshape(experts, columns)


def fit_platt(raw_probabilities: np.ndarray, labels: np.ndarray) -> np.ndarray:
    positives = int(np.sum(labels))
    negatives = len(labels) - positives
    targets = np.where(labels == 1, (positives + 1.0) / (positives + 2.0), 1.0 / (negatives + 2.0))
    logits = np.log(np.clip(raw_probabilities, 1e-12, 1.0 - 1e-12) / np.clip(1.0 - raw_probabilities, 1e-12, 1.0))

    def objective(parameters: np.ndarray) -> tuple[float, np.ndarray]:
        z = parameters[0] + parameters[1] * logits
        residual = expit(z) - targets
        return float(np.sum(np.logaddexp(0.0, z) - targets * z)), np.asarray([np.sum(residual), np.dot(residual, logits)])

    fitted = minimize(lambda p: objective(p), np.asarray([0.0, 1.0]), method="BFGS", jac=True, options={"maxiter": 1_000, "gtol": 1e-8})
    if not fitted.success or not np.all(np.isfinite(fitted.x)):
        raise ValueError(f"mixture calibration failed: {fitted.message}")
    return np.asarray(fitted.x, dtype=np.float64)


def run(raw: bytes) -> dict[str, Any]:
    path = Path(__file__)
    request = validate(
        json.loads(raw),
        hashlib.sha256(path.with_name("uv.lock").read_bytes()).hexdigest(),
        hashlib.sha256(path.read_bytes()).hexdigest(),
    )
    train = [row for row in request["patients"] if row["split"] == "gate_train"]
    calibration = [row for row in request["patients"] if row["split"] == "calibration"]
    test = [row for row in request["patients"] if row["split"] == "test"]
    train_context = np.asarray([row["context"] for row in train], dtype=np.float64)
    mean = np.mean(train_context, axis=0)
    sd = np.std(train_context, axis=0, ddof=0)
    if np.any(sd <= 1e-14):
        raise ValueError("every gating context feature must vary on gate_train")
    expert_count = len(request["expert_names"])
    coefficients = fit_gate(
        train,
        mean,
        sd,
        expert_count,
        float(request["l2_penalty"]),
        float(request["entropy_regularization"]),
    )

    def raw_probability(row: dict[str, Any]) -> tuple[float, np.ndarray]:
        weights = gate_weights(coefficients, gate_features(row, mean, sd), row["expert_probabilities"])
        probabilities = np.asarray([0.0 if value is None else value for value in row["expert_probabilities"]])
        return float(np.dot(weights, probabilities)), weights

    calibration_raw = np.asarray([raw_probability(row)[0] for row in calibration])
    calibrator = fit_platt(calibration_raw, np.asarray([row["label"] for row in calibration], dtype=np.float64))

    def calibrated(raw_value: float) -> float:
        clipped = min(max(raw_value, 1e-12), 1.0 - 1e-12)
        return float(expit(calibrator[0] + calibrator[1] * math.log(clipped / (1.0 - clipped))))

    calibration_ood = sorted(float(np.linalg.norm((np.asarray(row["context"]) - mean) / sd)) for row in calibration)
    rank = math.ceil(float(request["ood_validation_quantile"]) * len(calibration_ood))
    threshold = calibration_ood[rank - 1]
    predictions = []
    for row in test:
        raw_value, weights = raw_probability(row)
        ood = float(np.linalg.norm((np.asarray(row["context"]) - mean) / sd))
        predictions.append(
            {
                "patient_id": row["patient_id"],
                "label": row["label"],
                "availability": [value is not None for value in row["expert_probabilities"]],
                "gating_weights": weights.tolist(),
                "raw_probability": raw_value,
                "probability": calibrated(raw_value),
                "ood_score": ood,
                "exceeds_ood_threshold": ood > threshold,
            }
        )
    train_entropies = []
    for row in train:
        weights = raw_probability(row)[1]
        positive = weights[weights > 0.0]
        train_entropies.append(-float(np.sum(positive * np.log(positive))))
    names = [*request["context_names"], *[f"{name}_available" for name in request["expert_names"]]]
    return {
        "format": "marklab.scipy_mixture_of_experts_worker_result",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(raw).hexdigest(),
        "model": {
            "context_training_mean": mean.tolist(),
            "context_training_population_sd": sd.tolist(),
            "gating_feature_names": names,
            "coefficients_row_major": coefficients.ravel().tolist(),
            "coefficient_columns": coefficients.shape[1],
            "l2_penalty": request["l2_penalty"],
            "entropy_regularization": request["entropy_regularization"],
        },
        "calibrator": {"intercept": float(calibrator[0]), "slope": float(calibrator[1])},
        "ood_threshold": threshold,
        "predictions": predictions,
        "metrics": {
            "brier_score": float(np.mean([(row["probability"] - row["label"]) ** 2 for row in predictions])),
            "mean_gate_entropy": float(np.mean(train_entropies)),
        },
    }


def main() -> None:
    raw = sys.stdin.buffer.read()
    try:
        encoded = json.dumps(run(raw), allow_nan=False, separators=(",", ":")).encode()
    except Exception as error:
        print(f"mixture-of-experts worker failed: {error}", file=sys.stderr)
        raise SystemExit(2) from error
    sys.stdout.buffer.write(encoded)


if __name__ == "__main__":
    main()
