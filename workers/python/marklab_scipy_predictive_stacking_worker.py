#!/usr/bin/env python3
"""SciPy worker for patient-grouped predictive stacking of model log predictive densities."""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path
from typing import Any

import numpy as np
import scipy
from scipy.optimize import minimize
from scipy.special import logsumexp

sys.path.append(str(Path(__file__).resolve().parent))

from marklab_worker_support import versions
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

# Rust re-serializes one CSV file of at most 16 MiB with at most 500 patient rows and 16 models.
# Each CSV byte becomes at most 6 JSON bytes (\u00XX escapes; numbers grow less), and each row
# adds fewer than 128 bytes of keys: 96 MiB + 64 KiB + fixed fields, rounded up.
MAXIMUM_REQUEST_BYTES = 100 * MIB


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {"format", "version", "backend", "patients", "model_names", "resources"},
        "request",
    )
    if request["format"] != "marklab.scipy_predictive_stacking_request" or request["version"] != 1:
        raise ContractError("request identity differs")
    validate_backend(
        request["backend"],
        "backend",
        name="scipy",
        version=versions.SCIPY,
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )
    patients = request["patients"]
    model_names = request["model_names"]
    if not isinstance(patients, list) or not 8 <= len(patients) <= 500:
        raise ContractError("patient count differs")
    if not isinstance(model_names, list) or not 2 <= len(model_names) <= 16:
        raise ContractError("model count differs")
    for index, raw in enumerate(patients):
        row = obj(
            raw,
            {"patient_id", "held_out_unit", "log_predictive_densities"},
            f"patients[{index}]",
        )
        densities = row["log_predictive_densities"]
        if (
            row["held_out_unit"] != "patient"
            or len(densities) != len(model_names)
            or any(not math.isfinite(float(value)) for value in densities)
        ):
            raise ContractError("patient row differs")
    obj(
        request["resources"],
        {
            "maximum_patients",
            "maximum_models",
            "maximum_jackknife_density_visits",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    return request


def optimize_weights(log_densities: np.ndarray) -> np.ndarray:
    """Stacking weights maximizing the summed log density of the weighted model mixture.

    ``log_densities`` is ``(patient, model)``. The weights lie on the simplex; SLSQP solves the
    constrained problem from equal weights with the analytic gradient.
    """
    model_count = log_densities.shape[1]

    def objective(weights: np.ndarray) -> tuple[float, np.ndarray]:
        positive = np.clip(weights, 1e-300, None)
        log_mixture = logsumexp(log_densities + np.log(positive)[None, :], axis=1)
        ratios = np.exp(log_densities - log_mixture[:, None])
        return -float(np.sum(log_mixture)), -np.sum(ratios, axis=0)

    simplex = {
        "type": "eq",
        "fun": lambda weights: np.sum(weights) - 1.0,
        "jac": lambda weights: np.ones(model_count),
    }
    fitted = minimize(
        objective,
        np.full(model_count, 1.0 / model_count),
        method="SLSQP",
        jac=True,
        bounds=[(0.0, 1.0)] * model_count,
        constraints=simplex,
        options={"maxiter": 2_000, "ftol": 1e-12},
    )
    if not fitted.success or not np.all(np.isfinite(fitted.x)):
        raise ContractError(f"stacking optimizer failed: {fitted.message}")
    weights = np.maximum(np.asarray(fitted.x, dtype=np.float64), 0.0)
    return weights / np.sum(weights)


def mixture_log_densities(log_densities: np.ndarray, weights: np.ndarray) -> np.ndarray:
    positive = np.where(weights > 0.0, weights, 0.0)
    with np.errstate(divide="ignore"):
        return logsumexp(log_densities + np.log(positive)[None, :], axis=1)


def run(raw: bytes, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = validate(json.loads(raw), lock_digest, worker_digest)
    matrix = np.asarray(
        [row["log_predictive_densities"] for row in request["patients"]], dtype=np.float64
    )
    weights = optimize_weights(matrix)
    mixture = mixture_log_densities(matrix, weights)
    jackknife = np.asarray(
        [optimize_weights(np.delete(matrix, index, axis=0)) for index in range(len(matrix))]
    )
    return {
        "format": "marklab.scipy_predictive_stacking_worker_result",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(raw),
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
    require_versions(scipy=scipy.__version__)
    lock_digest, worker_digest = worker_digests(__file__)
    raw = read_request(MAXIMUM_REQUEST_BYTES)
    result = run(raw, lock_digest, worker_digest)
    write_result(result, sort_keys=False, newline=False)


if __name__ == "__main__":
    run_worker(main, "predictive-stacking worker failed")
