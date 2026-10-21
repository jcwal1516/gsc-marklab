#!/usr/bin/env python3
"""Pinned SciPy worker for context-gated mixture-of-experts fusion."""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path
from typing import Any

sys.path.append(str(Path(__file__).resolve().parent))

import numpy as np
import scipy
from scipy.optimize import minimize

from marklab_worker_support import versions
from marklab_worker_support.logistic_calibration import apply_platt, fit_platt_to_probabilities
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
    exact,
    obj,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    validate_backend,
    worker_digests,
    write_result,
)

# Rust re-serializes one CSV file of at most 16 MiB with at most 10,000 patient rows. Each CSV
# byte becomes at most 6 JSON bytes (\u00XX escapes; numbers and nulls grow less), and each row
# adds at most 256 bytes of keys: 96 MiB + 2.5 MiB + fixed fields, rounded up.
MAXIMUM_REQUEST_BYTES = 100 * MIB


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "patients",
            "context_names",
            "expert_names",
            "l2_penalty",
            "entropy_regularization",
            "ood_validation_quantile",
            "resources",
        },
        "request",
    )
    exact(request["format"], "marklab.scipy_mixture_of_experts_request", "request.format")
    exact(request["version"], 1, "request.version")
    validate_backend(
        request["backend"],
        "request.backend",
        name="scipy",
        version=versions.SCIPY,
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )
    keys = {
        "patient_id",
        "split",
        "expert_prediction_source",
        "label",
        "context",
        "expert_probabilities",
    }
    for index, raw in enumerate(request["patients"]):
        row = obj(raw, keys, f"patients[{index}]")
        if (
            row["split"] not in {"gate_train", "calibration", "test"}
            or row["expert_prediction_source"] != "patient_level_out_of_fold"
            or row["label"] not in {0, 1}
        ):
            raise ContractError("patient differs")
    obj(
        request["resources"],
        {
            "maximum_patients",
            "maximum_context_features",
            "maximum_experts",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    return request


def gate_features(row: dict[str, Any], mean: np.ndarray, sd: np.ndarray) -> np.ndarray:
    """Standardized context followed by one availability flag per expert."""
    context = (np.asarray(row["context"], dtype=np.float64) - mean) / sd
    available = np.asarray(
        [value is not None for value in row["expert_probabilities"]], dtype=np.float64
    )
    return np.concatenate([context, available])


def gate_weights(
    coefficients: np.ndarray, features: np.ndarray, probabilities: list[float | None]
) -> np.ndarray:
    """Softmax gate over the available experts; unavailable experts get zero weight."""
    logits = coefficients[:, 0] + coefficients[:, 1:] @ features
    available = np.asarray([value is not None for value in probabilities])
    logits = np.where(available, logits, -np.inf)
    maximum = np.max(logits)
    weights = np.where(available, np.exp(logits - maximum), 0.0)
    return weights / np.sum(weights)


def expert_probabilities(row: dict[str, Any]) -> np.ndarray:
    return np.asarray([0.0 if value is None else value for value in row["expert_probabilities"]])


def gate_entropy(weights: np.ndarray) -> float:
    positive = weights[weights > 0.0]
    return -float(np.sum(positive * np.log(positive)))


def fit_gate(
    rows: list[dict[str, Any]],
    mean: np.ndarray,
    sd: np.ndarray,
    experts: int,
    l2: float,
    entropy_regularization: float,
) -> np.ndarray:
    """Fit the softmax gating network of the mixture of experts by L-BFGS-B from zero.

    Model: ``P(y = 1) = sum_e w_e(x) p_e`` where ``p_e`` are the (fixed) out-of-fold expert
    probabilities and ``w`` is a softmax over available experts of linear functions of the
    standardized context and the availability flags. The objective is the Bernoulli negative
    log-likelihood (mixture clipped to ``[1e-12, 1 - 1e-12]``) plus ``l2 / 2 * ||coefficients||^2``
    minus ``entropy_regularization`` times the summed gate entropy. Gradients are numerical.
    """
    columns = 1 + len(mean) + experts

    def objective(flat: np.ndarray) -> float:
        coefficients = flat.reshape(experts, columns)
        loss = 0.0
        entropy = 0.0
        for row in rows:
            weights = gate_weights(
                coefficients, gate_features(row, mean, sd), row["expert_probabilities"]
            )
            mixture = float(np.clip(np.dot(weights, expert_probabilities(row)), 1e-12, 1.0 - 1e-12))
            label = row["label"]
            loss -= label * math.log(mixture) + (1 - label) * math.log(1.0 - mixture)
            entropy -= float(np.sum(weights[weights > 0.0] * np.log(weights[weights > 0.0])))
        return loss + 0.5 * l2 * float(np.dot(flat, flat)) - entropy_regularization * entropy

    fitted = minimize(
        objective,
        np.zeros(experts * columns),
        method="L-BFGS-B",
        options={"maxiter": 2_000, "ftol": 1e-12, "gtol": 1e-7, "maxls": 50},
    )
    if not fitted.success or not np.all(np.isfinite(fitted.x)):
        raise ContractError(f"mixture gate optimizer failed: {fitted.message}")
    return np.asarray(fitted.x, dtype=np.float64).reshape(experts, columns)


def run(request: dict[str, Any]) -> dict[str, Any]:
    patients = request["patients"]
    train = [row for row in patients if row["split"] == "gate_train"]
    calibration = [row for row in patients if row["split"] == "calibration"]
    test = [row for row in patients if row["split"] == "test"]
    train_context = np.asarray([row["context"] for row in train], dtype=np.float64)
    mean = np.mean(train_context, axis=0)
    sd = np.std(train_context, axis=0, ddof=0)
    if np.any(sd <= 1e-14):
        raise ContractError("every gating context feature must vary on gate_train")
    coefficients = fit_gate(
        train,
        mean,
        sd,
        len(request["expert_names"]),
        float(request["l2_penalty"]),
        float(request["entropy_regularization"]),
    )

    def raw_probability(row: dict[str, Any]) -> tuple[float, np.ndarray]:
        weights = gate_weights(
            coefficients, gate_features(row, mean, sd), row["expert_probabilities"]
        )
        return float(np.dot(weights, expert_probabilities(row))), weights

    def ood_score(row: dict[str, Any]) -> float:
        return float(np.linalg.norm((np.asarray(row["context"]) - mean) / sd))

    calibration_raw = np.asarray([raw_probability(row)[0] for row in calibration])
    calibrator = fit_platt_to_probabilities(
        calibration_raw, np.asarray([row["label"] for row in calibration], dtype=np.float64)
    )
    calibration_ood = sorted(ood_score(row) for row in calibration)
    rank = math.ceil(float(request["ood_validation_quantile"]) * len(calibration_ood))
    threshold = calibration_ood[rank - 1]
    predictions = []
    for row in test:
        raw_value, weights = raw_probability(row)
        ood = ood_score(row)
        predictions.append(
            {
                "patient_id": row["patient_id"],
                "label": row["label"],
                "availability": [value is not None for value in row["expert_probabilities"]],
                "gating_weights": weights.tolist(),
                "raw_probability": raw_value,
                "probability": apply_platt(calibrator, raw_value),
                "ood_score": ood,
                "exceeds_ood_threshold": ood > threshold,
            }
        )
    train_entropies = [gate_entropy(raw_probability(row)[1]) for row in train]
    names = [*request["context_names"], *[f"{name}_available" for name in request["expert_names"]]]
    brier = float(np.mean([(row["probability"] - row["label"]) ** 2 for row in predictions]))
    return {
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
            "brier_score": brier,
            "mean_gate_entropy": float(np.mean(train_entropies)),
        },
    }


def main() -> None:
    require_versions(scipy=scipy.__version__)
    lock_digest, worker_digest = worker_digests(__file__)
    raw = read_request(MAXIMUM_REQUEST_BYTES)
    request = validate(json.loads(raw), lock_digest, worker_digest)
    result = {
        "format": "marklab.scipy_mixture_of_experts_worker_result",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(raw),
        **run(request),
    }
    write_result(result, sort_keys=False, newline=False)


if __name__ == "__main__":
    run_worker(main, "mixture-of-experts worker failed")
