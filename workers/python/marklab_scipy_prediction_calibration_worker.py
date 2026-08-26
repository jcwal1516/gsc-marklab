#!/usr/bin/env python3
"""Pinned SciPy worker for patient-level probability calibration."""

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


class ContractError(ValueError):
    pass


def exact_object(value: Any, keys: set[str], path: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        raise ContractError(f"{path} fields differ")
    return value


def validate_request(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = exact_object(
        request,
        {"format", "version", "backend", "method", "rows", "bins", "resources"},
        "request",
    )
    if (
        request["format"] != "marklab.scipy_prediction_calibration_request"
        or request["version"] != 1
        or request["method"] != "platt_logistic"
    ):
        raise ContractError("request identity differs")
    backend = exact_object(
        request["backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "request.backend",
    )
    if (
        backend["name"] != "scipy"
        or backend["version"] != SCIPY_VERSION
        or backend["python_version"] != "3.12"
        or backend["environment_lock_sha256"] != lock_digest
        or backend["worker_sha256"] != worker_digest
        or scipy.__version__ != SCIPY_VERSION
    ):
        raise ContractError("backend identity differs")
    rows = request["rows"]
    if not isinstance(rows, list) or not 16 <= len(rows) <= 100_000:
        raise ContractError("row count differs")
    for index, raw in enumerate(rows):
        row = exact_object(raw, {"patient_id", "split", "score", "label"}, f"rows[{index}]")
        if (
            not isinstance(row["patient_id"], str)
            or not row["patient_id"]
            or row["split"] not in {"training_oof", "test"}
            or not math.isfinite(float(row["score"]))
            or row["label"] not in {0, 1}
        ):
            raise ContractError("row differs")
    if not isinstance(request["bins"], int) or not 2 <= request["bins"] <= 20:
        raise ContractError("bin count differs")
    exact_object(
        request["resources"],
        {"maximum_patients", "maximum_bins", "maximum_output_bytes", "timeout_seconds"},
        "request.resources",
    )
    return request


def logistic_objective(parameters: np.ndarray, x: np.ndarray, targets: np.ndarray) -> tuple[float, np.ndarray]:
    z = parameters[0] + parameters[1] * x
    objective = float(np.sum(np.logaddexp(0.0, z) - targets * z))
    residual = expit(z) - targets
    gradient = np.asarray([np.sum(residual), np.dot(residual, x)], dtype=np.float64)
    return objective, gradient


def fit_platt(scores: np.ndarray, labels: np.ndarray) -> np.ndarray:
    positives = int(np.sum(labels))
    negatives = len(labels) - positives
    targets = np.where(labels == 1, (positives + 1.0) / (positives + 2.0), 1.0 / (negatives + 2.0))
    start = np.asarray([math.log((positives + 1.0) / (negatives + 1.0)), 0.0])
    fitted = minimize(
        lambda parameters: logistic_objective(parameters, scores, targets),
        start,
        method="L-BFGS-B",
        jac=True,
        options={"maxiter": 1_000, "ftol": 1e-14, "gtol": 1e-10},
    )
    if not fitted.success or not np.all(np.isfinite(fitted.x)):
        raise ContractError(f"Platt optimizer failed: {fitted.message}")
    return np.asarray(fitted.x, dtype=np.float64)


def calibration_diagnostics(probabilities: np.ndarray, labels: np.ndarray) -> tuple[float, float]:
    logits = np.log(np.clip(probabilities, 1e-12, 1.0 - 1e-12) / np.clip(1.0 - probabilities, 1e-12, 1.0))
    ridge = 1e-8

    def intercept_objective(parameter: np.ndarray) -> tuple[float, np.ndarray]:
        z = logits + parameter[0]
        value = float(np.sum(np.logaddexp(0.0, z) - labels * z) + ridge * parameter[0] ** 2)
        gradient = np.asarray([np.sum(expit(z) - labels) + 2.0 * ridge * parameter[0]])
        return value, gradient

    intercept_fit = minimize(
        lambda parameter: intercept_objective(parameter),
        np.asarray([0.0]),
        method="L-BFGS-B",
        jac=True,
        options={"maxiter": 1_000, "ftol": 1e-14, "gtol": 1e-10},
    )
    if not intercept_fit.success or not np.all(np.isfinite(intercept_fit.x)):
        raise ContractError(f"calibration intercept failed: {intercept_fit.message}")

    def objective(parameters: np.ndarray) -> tuple[float, np.ndarray]:
        z = parameters[0] + parameters[1] * logits
        value = float(np.sum(np.logaddexp(0.0, z) - labels * z) + ridge * np.dot(parameters, parameters))
        residual = expit(z) - labels
        gradient = np.asarray(
            [np.sum(residual), np.dot(residual, logits)], dtype=np.float64
        ) + 2.0 * ridge * parameters
        return value, gradient

    fitted = minimize(
        lambda parameters: objective(parameters),
        np.asarray([0.0, 1.0]),
        method="L-BFGS-B",
        jac=True,
        options={"maxiter": 1_000, "ftol": 1e-14, "gtol": 1e-10},
    )
    if not fitted.success or not np.all(np.isfinite(fitted.x)):
        raise ContractError(f"calibration regression failed: {fitted.message}")
    return float(intercept_fit.x[0]), float(fitted.x[1])


def reliability(probabilities: np.ndarray, labels: np.ndarray, bins: int) -> tuple[list[dict[str, Any]], float]:
    assignments = np.minimum(np.floor(probabilities * bins).astype(np.int64), bins - 1)
    rows: list[dict[str, Any]] = []
    ece = 0.0
    z = 1.959963984540054
    for index in range(bins):
        selected = assignments == index
        count = int(np.count_nonzero(selected))
        if count == 0:
            continue
        mean = float(np.mean(probabilities[selected]))
        rate = float(np.mean(labels[selected]))
        denominator = 1.0 + z * z / count
        center = (rate + z * z / (2.0 * count)) / denominator
        half = z * math.sqrt(rate * (1.0 - rate) / count + z * z / (4.0 * count * count)) / denominator
        rows.append(
            {
                "lower": index / bins,
                "upper": (index + 1) / bins,
                "count": count,
                "mean_probability": mean,
                "observed_rate": rate,
                "wilson_95_lower": max(0.0, center - half),
                "wilson_95_upper": min(1.0, center + half),
            }
        )
        ece += count / len(labels) * abs(mean - rate)
    return rows, ece


def run(raw: bytes) -> dict[str, Any]:
    worker_path = Path(__file__)
    request = validate_request(
        json.loads(raw),
        hashlib.sha256(worker_path.with_name("uv.lock").read_bytes()).hexdigest(),
        hashlib.sha256(worker_path.read_bytes()).hexdigest(),
    )
    training = [row for row in request["rows"] if row["split"] == "training_oof"]
    test = [row for row in request["rows"] if row["split"] == "test"]
    training_scores = np.asarray([row["score"] for row in training], dtype=np.float64)
    training_labels = np.asarray([row["label"] for row in training], dtype=np.float64)
    parameters = fit_platt(training_scores, training_labels)
    test_scores = np.asarray([row["score"] for row in test], dtype=np.float64)
    test_labels = np.asarray([row["label"] for row in test], dtype=np.float64)
    probabilities = expit(parameters[0] + parameters[1] * test_scores)
    calibration_intercept, calibration_slope = calibration_diagnostics(probabilities, test_labels)
    bins, ece = reliability(probabilities, test_labels, request["bins"])
    return {
        "format": "marklab.scipy_prediction_calibration_worker_result",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(raw).hexdigest(),
        "calibrator": {
            "intercept": float(parameters[0]),
            "slope": float(parameters[1]),
            "target_smoothing": "platt_class_count",
        },
        "predictions": [
            {
                "patient_id": row["patient_id"],
                "score": float(row["score"]),
                "label": row["label"],
                "probability": float(probability),
            }
            for row, probability in zip(test, probabilities, strict=True)
        ],
        "metrics": {
            "brier_score": float(np.mean(np.square(probabilities - test_labels))),
            "expected_calibration_error": ece,
            "calibration_in_the_large": calibration_intercept,
            "calibration_slope": calibration_slope,
            "reliability_bins": bins,
        },
    }


def main() -> None:
    raw = sys.stdin.buffer.read()
    try:
        output = json.dumps(run(raw), allow_nan=False, separators=(",", ":")).encode()
    except Exception as error:
        print(f"prediction calibration worker failed: {error}", file=sys.stderr)
        raise SystemExit(2) from error
    sys.stdout.buffer.write(output)


if __name__ == "__main__":
    main()
