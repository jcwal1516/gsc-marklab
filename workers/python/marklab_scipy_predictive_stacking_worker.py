#!/usr/bin/env python3
"""Pinned SciPy worker for patient-grouped predictive stacking."""

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
from scipy.special import logsumexp


SCIPY_VERSION = "1.18.1"


def exact_object(value: Any, keys: set[str], path: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        raise ValueError(f"{path} fields differ")
    return value


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = exact_object(
        request,
        {"format", "version", "backend", "patients", "model_names", "resources"},
        "request",
    )
    if request["format"] != "marklab.scipy_predictive_stacking_request" or request["version"] != 1:
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
    if not isinstance(request["patients"], list) or not 8 <= len(request["patients"]) <= 500:
        raise ValueError("patient count differs")
    if not isinstance(request["model_names"], list) or not 2 <= len(request["model_names"]) <= 16:
        raise ValueError("model count differs")
    for index, raw in enumerate(request["patients"]):
        row = exact_object(raw, {"patient_id", "held_out_unit", "log_predictive_densities"}, f"patients[{index}]")
        if (
            row["held_out_unit"] != "patient"
            or len(row["log_predictive_densities"]) != len(request["model_names"])
            or any(not math.isfinite(float(value)) for value in row["log_predictive_densities"])
        ):
            raise ValueError("patient row differs")
    exact_object(
        request["resources"],
        {"maximum_patients", "maximum_models", "maximum_jackknife_density_visits", "maximum_output_bytes", "timeout_seconds"},
        "resources",
    )
    return request


def optimize_weights(log_densities: np.ndarray) -> np.ndarray:
    model_count = log_densities.shape[1]

    def objective(weights: np.ndarray) -> tuple[float, np.ndarray]:
        positive = np.clip(weights, 1e-300, None)
        log_mixture = logsumexp(log_densities + np.log(positive)[None, :], axis=1)
        ratios = np.exp(log_densities - log_mixture[:, None])
        return -float(np.sum(log_mixture)), -np.sum(ratios, axis=0)

    fitted = minimize(
        lambda weights: objective(weights),
        np.full(model_count, 1.0 / model_count),
        method="SLSQP",
        jac=True,
        bounds=[(0.0, 1.0)] * model_count,
        constraints={"type": "eq", "fun": lambda weights: np.sum(weights) - 1.0, "jac": lambda weights: np.ones(model_count)},
        options={"maxiter": 2_000, "ftol": 1e-12},
    )
    if not fitted.success or not np.all(np.isfinite(fitted.x)):
        raise ValueError(f"stacking optimizer failed: {fitted.message}")
    weights = np.maximum(np.asarray(fitted.x, dtype=np.float64), 0.0)
    return weights / np.sum(weights)


def mixture_log_densities(log_densities: np.ndarray, weights: np.ndarray) -> np.ndarray:
    positive = np.where(weights > 0.0, weights, 0.0)
    with np.errstate(divide="ignore"):
        return logsumexp(log_densities + np.log(positive)[None, :], axis=1)


def run(raw: bytes) -> dict[str, Any]:
    path = Path(__file__)
    request = validate(
        json.loads(raw),
        hashlib.sha256(path.with_name("uv.lock").read_bytes()).hexdigest(),
        hashlib.sha256(path.read_bytes()).hexdigest(),
    )
    matrix = np.asarray([row["log_predictive_densities"] for row in request["patients"]], dtype=np.float64)
    weights = optimize_weights(matrix)
    mixture = mixture_log_densities(matrix, weights)
    jackknife = np.asarray(
        [optimize_weights(np.delete(matrix, index, axis=0)) for index in range(len(matrix))]
    )
    return {
        "format": "marklab.scipy_predictive_stacking_worker_result",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(raw).hexdigest(),
        "weights": [
            {"model": model, "weight": float(weight)}
            for model, weight in zip(request["model_names"], weights, strict=True)
        ],
        "objective_sum_log_predictive_density": float(np.sum(mixture)),
        "grouped_mixture_log_predictive_density": [
            {"patient_id": row["patient_id"], "mixture_log_predictive_density": float(value)}
            for row, value in zip(request["patients"], mixture, strict=True)
        ],
        "leave_one_patient_out_sensitivity": [
            {
                "model": model,
                "leave_one_patient_out_minimum": float(np.min(jackknife[:, index])),
                "leave_one_patient_out_maximum": float(np.max(jackknife[:, index])),
            }
            for index, model in enumerate(request["model_names"])
        ],
    }


def main() -> None:
    raw = sys.stdin.buffer.read()
    try:
        encoded = json.dumps(run(raw), allow_nan=False, separators=(",", ":")).encode()
    except Exception as error:
        print(f"predictive-stacking worker failed: {error}", file=sys.stderr)
        raise SystemExit(2) from error
    sys.stdout.buffer.write(encoded)


if __name__ == "__main__":
    main()
