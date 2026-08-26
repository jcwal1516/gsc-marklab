#!/usr/bin/env python3
"""Pinned SciPy worker for calibrated patient-level late fusion."""

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
        {"format", "version", "backend", "base_prediction_source", "patients", "modalities", "l2_penalty", "resources"},
        "request",
    )
    if (
        request["format"] != "marklab.scipy_late_fusion_request"
        or request["version"] != 1
        or request["base_prediction_source"] != "patient_level_out_of_fold"
    ):
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
    if not isinstance(request["modalities"], list) or not 2 <= len(request["modalities"]) <= 16:
        raise ValueError("modalities differ")
    for index, raw in enumerate(request["patients"]):
        row = exact_object(raw, {"patient_id", "split", "base_prediction_source", "label", "modality_probabilities"}, f"patients[{index}]")
        if (
            row["split"] not in {"meta_train", "calibration", "test"}
            or row["base_prediction_source"] != "patient_level_out_of_fold"
            or row["label"] not in {0, 1}
        ):
            raise ValueError("patient differs")
    exact_object(
        request["resources"],
        {"maximum_patients", "maximum_modalities", "maximum_output_bytes", "timeout_seconds"},
        "resources",
    )
    return request


def features(row: dict[str, Any]) -> np.ndarray:
    return np.asarray(
        [item for value in row["modality_probabilities"] for item in ((0.5 if value is None else value), float(value is not None))],
        dtype=np.float64,
    )


def fit_logistic(x: np.ndarray, y: np.ndarray, penalty: float) -> np.ndarray:
    def objective(parameters: np.ndarray) -> tuple[float, np.ndarray]:
        z = parameters[0] + x @ parameters[1:]
        residual = expit(z) - y
        value = float(np.sum(np.logaddexp(0.0, z) - y * z) + 0.5 * penalty * np.dot(parameters[1:], parameters[1:]))
        gradient = np.concatenate([np.asarray([np.sum(residual)]), x.T @ residual + penalty * parameters[1:]])
        return value, gradient

    fitted = minimize(lambda p: objective(p), np.zeros(x.shape[1] + 1), method="BFGS", jac=True, options={"maxiter": 1_000, "gtol": 1e-8})
    if not fitted.success or not np.all(np.isfinite(fitted.x)):
        raise ValueError(f"fusion optimizer failed: {fitted.message}")
    return np.asarray(fitted.x, dtype=np.float64)


def fit_platt(scores: np.ndarray, labels: np.ndarray) -> np.ndarray:
    positives = int(np.sum(labels))
    negatives = len(labels) - positives
    targets = np.where(labels == 1, (positives + 1.0) / (positives + 2.0), 1.0 / (negatives + 2.0))
    logits = np.log(np.clip(scores, 1e-12, 1.0 - 1e-12) / np.clip(1.0 - scores, 1e-12, 1.0))

    def objective(parameters: np.ndarray) -> tuple[float, np.ndarray]:
        z = parameters[0] + parameters[1] * logits
        residual = expit(z) - targets
        return float(np.sum(np.logaddexp(0.0, z) - targets * z)), np.asarray([np.sum(residual), np.dot(residual, logits)])

    fitted = minimize(lambda p: objective(p), np.asarray([0.0, 1.0]), method="BFGS", jac=True, options={"maxiter": 1_000, "gtol": 1e-8})
    if not fitted.success or not np.all(np.isfinite(fitted.x)):
        raise ValueError(f"fusion calibration failed: {fitted.message}")
    return np.asarray(fitted.x, dtype=np.float64)


def run(raw: bytes) -> dict[str, Any]:
    path = Path(__file__)
    request = validate(
        json.loads(raw),
        hashlib.sha256(path.with_name("uv.lock").read_bytes()).hexdigest(),
        hashlib.sha256(path.read_bytes()).hexdigest(),
    )
    meta = [row for row in request["patients"] if row["split"] == "meta_train"]
    calibration = [row for row in request["patients"] if row["split"] == "calibration"]
    test = [row for row in request["patients"] if row["split"] == "test"]
    model = fit_logistic(
        np.asarray([features(row) for row in meta]),
        np.asarray([row["label"] for row in meta], dtype=np.float64),
        float(request["l2_penalty"]),
    )

    def raw_probability(row: dict[str, Any], ablate: int | None = None) -> float:
        values = features(row)
        if ablate is not None:
            values[2 * ablate] = 0.5
            values[2 * ablate + 1] = 0.0
        return float(expit(model[0] + np.dot(values, model[1:])))

    calibration_raw = np.asarray([raw_probability(row) for row in calibration])
    calibrator = fit_platt(calibration_raw, np.asarray([row["label"] for row in calibration], dtype=np.float64))

    def calibrated(raw_value: float) -> float:
        logit = math.log(min(max(raw_value, 1e-12), 1.0 - 1e-12) / (1.0 - min(max(raw_value, 1e-12), 1.0 - 1e-12)))
        return float(expit(calibrator[0] + calibrator[1] * logit))

    predictions = []
    for row in test:
        raw_value = raw_probability(row)
        predictions.append(
            {
                "patient_id": row["patient_id"],
                "label": row["label"],
                "availability": [value is not None for value in row["modality_probabilities"]],
                "raw_probability": raw_value,
                "probability": calibrated(raw_value),
            }
        )
    full_brier = float(np.mean([(row["probability"] - row["label"]) ** 2 for row in predictions]))
    scenarios: dict[str, list[dict[str, Any]]] = {}
    for row in predictions:
        missing = [name for name, available in zip(request["modalities"], row["availability"], strict=True) if not available]
        name = "complete" if not missing else "missing_" + "_and_".join(missing)
        scenarios.setdefault(name, []).append(row)
    missing_scenarios = [
        {
            "scenario": name,
            "count": len(rows),
            "brier_score": float(np.mean([(row["probability"] - row["label"]) ** 2 for row in rows])),
        }
        for name, rows in sorted(scenarios.items())
    ]
    ablations = []
    for index, modality in enumerate(request["modalities"]):
        probabilities = [calibrated(raw_probability(row, index)) for row in test]
        brier = float(np.mean([(value - row["label"]) ** 2 for value, row in zip(probabilities, test, strict=True)]))
        ablations.append(
            {"modality": modality, "brier_score": brier, "brier_difference_ablated_minus_full": brier - full_brier}
        )
    names = [item for modality in request["modalities"] for item in (f"{modality}_probability", f"{modality}_available")]
    return {
        "format": "marklab.scipy_late_fusion_worker_result",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(raw).hexdigest(),
        "model": {"intercept": float(model[0]), "coefficients": model[1:].tolist(), "feature_names": names, "l2_penalty": request["l2_penalty"]},
        "calibrator": {"intercept": float(calibrator[0]), "slope": float(calibrator[1])},
        "predictions": predictions,
        "metrics": {"brier_score": full_brier},
        "missing_scenarios": missing_scenarios,
        "modality_ablations": ablations,
    }


def main() -> None:
    raw = sys.stdin.buffer.read()
    try:
        encoded = json.dumps(run(raw), allow_nan=False, separators=(",", ":")).encode()
    except Exception as error:
        print(f"late-fusion worker failed: {error}", file=sys.stderr)
        raise SystemExit(2) from error
    sys.stdout.buffer.write(encoded)


if __name__ == "__main__":
    main()
