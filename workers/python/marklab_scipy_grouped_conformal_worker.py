#!/usr/bin/env python3
"""Pinned SciPy worker for patient-level split-conformal classification."""

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
        {"format", "version", "backend", "patients", "feature_names", "alpha", "l2_penalty", "resources"},
        "request",
    )
    if request["format"] != "marklab.scipy_grouped_conformal_request" or request["version"] != 1:
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
    if not isinstance(request["patients"], list) or not 30 <= len(request["patients"]) <= 100_000:
        raise ValueError("patient count differs")
    if not isinstance(request["feature_names"], list) or not 2 <= len(request["feature_names"]) <= 128:
        raise ValueError("feature count differs")
    keys = {"patient_id", "split", "site", "subgroup", "label", "features"}
    for index, raw in enumerate(request["patients"]):
        patient = exact_object(raw, keys, f"patients[{index}]")
        if (
            patient["split"] not in {"train", "calibration", "test"}
            or patient["label"] not in {0, 1}
            or len(patient["features"]) != len(request["feature_names"])
            or any(not math.isfinite(float(value)) for value in patient["features"])
        ):
            raise ValueError("patient differs")
    exact_object(
        request["resources"],
        {"maximum_patients", "maximum_features", "maximum_output_bytes", "timeout_seconds"},
        "resources",
    )
    return request


def fit_logistic(x: np.ndarray, y: np.ndarray, penalty: float) -> np.ndarray:
    def objective(parameters: np.ndarray) -> tuple[float, np.ndarray]:
        z = parameters[0] + x @ parameters[1:]
        value = float(np.sum(np.logaddexp(0.0, z) - y * z) + 0.5 * penalty * np.dot(parameters[1:], parameters[1:]))
        residual = expit(z) - y
        gradient = np.concatenate(
            [np.asarray([np.sum(residual)]), x.T @ residual + penalty * parameters[1:]]
        )
        return value, gradient

    fitted = minimize(
        lambda parameters: objective(parameters),
        np.zeros(x.shape[1] + 1),
        method="BFGS",
        jac=True,
        options={"maxiter": 1_000, "gtol": 1e-8},
    )
    if not fitted.success or not np.all(np.isfinite(fitted.x)):
        raise ValueError(f"logistic optimizer failed: {fitted.message}")
    return np.asarray(fitted.x, dtype=np.float64)


def coverage(rows: list[dict[str, Any]], key: str | None) -> list[dict[str, Any]]:
    groups = ["all"] if key is None else sorted({row[key] for row in rows})
    output = []
    for group in groups:
        selected = rows if key is None else [row for row in rows if row[key] == group]
        covered = sum(bool(row["covered"]) for row in selected)
        output.append(
            {"group": group, "count": len(selected), "covered": covered, "coverage": covered / len(selected)}
        )
    return output


def run(raw: bytes) -> dict[str, Any]:
    path = Path(__file__)
    request = validate(
        json.loads(raw),
        hashlib.sha256(path.with_name("uv.lock").read_bytes()).hexdigest(),
        hashlib.sha256(path.read_bytes()).hexdigest(),
    )
    patients = request["patients"]
    train = [row for row in patients if row["split"] == "train"]
    calibration = [row for row in patients if row["split"] == "calibration"]
    test = [row for row in patients if row["split"] == "test"]
    train_x = np.asarray([row["features"] for row in train], dtype=np.float64)
    mean = np.mean(train_x, axis=0)
    sd = np.std(train_x, axis=0, ddof=0)
    if np.any(sd <= 1e-14):
        raise ValueError("every training feature must vary")
    standardized = (train_x - mean) / sd
    parameters = fit_logistic(
        standardized,
        np.asarray([row["label"] for row in train], dtype=np.float64),
        float(request["l2_penalty"]),
    )

    def probability(row: dict[str, Any]) -> float:
        values = (np.asarray(row["features"], dtype=np.float64) - mean) / sd
        return float(expit(parameters[0] + np.dot(values, parameters[1:])))

    calibration_scores = sorted(
        1.0 - probability(row) if row["label"] == 1 else probability(row)
        for row in calibration
    )
    rank = math.ceil((len(calibration) + 1) * (1.0 - float(request["alpha"])))
    threshold = calibration_scores[rank - 1]
    predictions = []
    for row in test:
        probability_one = probability(row)
        prediction_set = []
        if probability_one <= threshold:
            prediction_set.append(0)
        if 1.0 - probability_one <= threshold:
            prediction_set.append(1)
        predictions.append(
            {
                "patient_id": row["patient_id"],
                "site": row["site"],
                "subgroup": row["subgroup"],
                "label": row["label"],
                "probability_one": probability_one,
                "prediction_set": prediction_set,
                "covered": row["label"] in prediction_set,
            }
        )
    return {
        "format": "marklab.scipy_grouped_conformal_worker_result",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(raw).hexdigest(),
        "model": {
            "training_mean": mean.tolist(),
            "training_population_sd": sd.tolist(),
            "intercept": float(parameters[0]),
            "coefficients": parameters[1:].tolist(),
            "l2_penalty": request["l2_penalty"],
        },
        "calibration_count": len(calibration),
        "corrected_rank": rank,
        "nonconformity_threshold": threshold,
        "predictions": predictions,
        "coverage": {
            "overall": coverage(predictions, None)[0],
            "by_site": coverage(predictions, "site"),
            "by_subgroup": coverage(predictions, "subgroup"),
        },
    }


def main() -> None:
    raw = sys.stdin.buffer.read()
    try:
        encoded = json.dumps(run(raw), allow_nan=False, separators=(",", ":")).encode()
    except Exception as error:
        print(f"grouped conformal worker failed: {error}", file=sys.stderr)
        raise SystemExit(2) from error
    sys.stdout.buffer.write(encoded)


if __name__ == "__main__":
    main()
