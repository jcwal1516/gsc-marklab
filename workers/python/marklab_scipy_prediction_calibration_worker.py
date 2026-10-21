#!/usr/bin/env python3
"""SciPy worker for patient-level probability calibration (Platt scaling)."""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path
from typing import Any

import numpy as np
import scipy
from scipy.optimize import minimize
from scipy.special import expit

sys.path.append(str(Path(__file__).resolve().parent))

from marklab_worker_support import versions
from marklab_worker_support.logistic_calibration import clipped_logit, platt_targets
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
    obj,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    validate_backend,
    worker_digests,
    write_result,
)

# Rust re-serializes one CSV file of at most 16 MiB with at most 100,000 patient rows. Each CSV
# byte becomes at most 6 JSON bytes (\u00XX escapes; numbers grow less), and each row adds at
# most 256 bytes of keys: 96 MiB + 24.4 MiB + fixed fields, rounded up.
MAXIMUM_REQUEST_BYTES = 122 * MIB
OPTIMIZER_OPTIONS = {"maxiter": 1_000, "ftol": 1e-14, "gtol": 1e-10}


def validate_request(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
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
    validate_backend(
        request["backend"],
        "request.backend",
        name="scipy",
        version=versions.SCIPY,
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )
    rows = request["rows"]
    if not isinstance(rows, list) or not 16 <= len(rows) <= 100_000:
        raise ContractError("row count differs")
    for index, raw in enumerate(rows):
        row = obj(raw, {"patient_id", "split", "score", "label"}, f"rows[{index}]")
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
    obj(
        request["resources"],
        {"maximum_patients", "maximum_bins", "maximum_output_bytes", "timeout_seconds"},
        "request.resources",
    )
    return request


def logistic_objective(
    parameters: np.ndarray, x: np.ndarray, targets: np.ndarray
) -> tuple[float, np.ndarray]:
    """Bernoulli cross-entropy of ``expit(a + b * x)`` against ``targets`` and its gradient."""
    z = parameters[0] + parameters[1] * x
    objective = float(np.sum(np.logaddexp(0.0, z) - targets * z))
    residual = expit(z) - targets
    gradient = np.asarray([np.sum(residual), np.dot(residual, x)], dtype=np.float64)
    return objective, gradient


def fit_platt(scores: np.ndarray, labels: np.ndarray) -> np.ndarray:
    """Platt scaling ``P(y = 1) = expit(a + b * score)`` fitted to the smoothed targets by
    L-BFGS-B from the smoothed prior log-odds and zero slope."""
    positives = int(np.sum(labels))
    negatives = len(labels) - positives
    targets = platt_targets(labels)
    start = np.asarray([math.log((positives + 1.0) / (negatives + 1.0)), 0.0])
    fitted = minimize(
        logistic_objective,
        start,
        args=(scores, targets),
        method="L-BFGS-B",
        jac=True,
        options=OPTIMIZER_OPTIONS,
    )
    if not fitted.success or not np.all(np.isfinite(fitted.x)):
        raise ContractError(f"Platt optimizer failed: {fitted.message}")
    return np.asarray(fitted.x, dtype=np.float64)


def calibration_diagnostics(probabilities: np.ndarray, labels: np.ndarray) -> tuple[float, float]:
    """Calibration-in-the-large and calibration slope of test probabilities.

    The intercept is fitted with the logit of the probabilities as an offset; the slope comes
    from regressing the labels on that logit with a free intercept. Both fits add a ``1e-8``
    ridge for stability.
    """
    logits = clipped_logit(probabilities)
    ridge = 1e-8

    def intercept_objective(parameter: np.ndarray) -> tuple[float, np.ndarray]:
        z = logits + parameter[0]
        value = float(np.sum(np.logaddexp(0.0, z) - labels * z) + ridge * parameter[0] ** 2)
        gradient = np.asarray([np.sum(expit(z) - labels) + 2.0 * ridge * parameter[0]])
        return value, gradient

    intercept_fit = minimize(
        intercept_objective,
        np.asarray([0.0]),
        method="L-BFGS-B",
        jac=True,
        options=OPTIMIZER_OPTIONS,
    )
    if not intercept_fit.success or not np.all(np.isfinite(intercept_fit.x)):
        raise ContractError(f"calibration intercept failed: {intercept_fit.message}")

    def objective(parameters: np.ndarray) -> tuple[float, np.ndarray]:
        z = parameters[0] + parameters[1] * logits
        penalty = ridge * np.dot(parameters, parameters)
        value = float(np.sum(np.logaddexp(0.0, z) - labels * z) + penalty)
        residual = expit(z) - labels
        score = np.asarray([np.sum(residual), np.dot(residual, logits)], dtype=np.float64)
        return value, score + 2.0 * ridge * parameters

    fitted = minimize(
        objective,
        np.asarray([0.0, 1.0]),
        method="L-BFGS-B",
        jac=True,
        options=OPTIMIZER_OPTIONS,
    )
    if not fitted.success or not np.all(np.isfinite(fitted.x)):
        raise ContractError(f"calibration regression failed: {fitted.message}")
    return float(intercept_fit.x[0]), float(fitted.x[1])


def reliability(
    probabilities: np.ndarray, labels: np.ndarray, bins: int
) -> tuple[list[dict[str, Any]], float]:
    """Equal-width reliability bins with Wilson 95% intervals, and the expected calibration
    error (the count-weighted mean absolute gap between mean probability and observed rate)."""
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
        spread = math.sqrt(rate * (1.0 - rate) / count + z * z / (4.0 * count * count))
        half = z * spread / denominator
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


def run(raw: bytes, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = validate_request(json.loads(raw), lock_digest, worker_digest)
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
        "request_sha256": sha256_hex(raw),
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
    require_versions(scipy=scipy.__version__)
    lock_digest, worker_digest = worker_digests(__file__)
    raw = read_request(MAXIMUM_REQUEST_BYTES)
    result = run(raw, lock_digest, worker_digest)
    write_result(result, sort_keys=False, newline=False)


if __name__ == "__main__":
    run_worker(main, "prediction calibration worker failed")
