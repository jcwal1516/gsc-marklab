#!/usr/bin/env python3
"""Static SciPy worker for bounded Thomas-process minimum contrast."""

from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path
import sys
from typing import Any

import numpy as np
from scipy import optimize

SCIPY_VERSION = "1.18.1"


class ContractError(ValueError):
    pass


def obj(value: Any, keys: set[str], path: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        actual = set(value) if isinstance(value, dict) else set()
        raise ContractError(
            f"{path} fields differ: missing={sorted(keys - actual)}, unknown={sorted(actual - keys)}"
        )
    return value


def exact(value: Any, expected: Any, path: str) -> None:
    if value != expected:
        raise ContractError(f"{path} must equal {expected!r}")


def number(value: Any, path: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ContractError(f"{path} must be numeric")
    result = float(value)
    if not math.isfinite(result):
        raise ContractError(f"{path} must be finite")
    return result


def integer(value: Any, path: str, low: int, high: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not low <= value <= high:
        raise ContractError(f"{path} must be an integer in [{low}, {high}]")
    return value


def validate(request: Any, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format", "version", "backend", "model", "curve",
            "observed_intensity_per_um2", "bounds", "maximum_iterations", "resources",
        },
        "request",
    )
    exact(request["format"], "marklab.pymc_worker_request", "request.format")
    integer(request["version"], "request.version", 1, 1)
    backend = obj(
        request["backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "backend",
    )
    exact(backend["name"], "scipy", "backend.name")
    exact(backend["version"], f"scipy-{SCIPY_VERSION}", "backend.version")
    exact(backend["python_version"], "3.12", "backend.python")
    exact(backend["environment_lock_sha256"], lock_sha, "backend.lock")
    exact(backend["worker_sha256"], worker_sha, "backend.worker")
    model = obj(
        request["model"],
        {
            "family", "summary", "theoretical_curve", "contrast_transform",
            "primary_weights", "sensitivity_fits", "offspring_mean_identification",
            "backend_capability", "maturity",
        },
        "model",
    )
    exact(model["family"], "thomas_cluster_process", "model.family")
    exact(model["summary"], "observed_isotropic_K_curve", "model.summary")
    exact(
        model["theoretical_curve"],
        "pi_r_squared_plus_inverse_kappa_times_one_minus_exp_negative_r_squared_over_four_sigma_squared",
        "model.curve",
    )
    exact(model["contrast_transform"], "fourth_root", "model.transform")
    exact(model["primary_weights"], "caller_supplied_positive", "model.weights")
    exact(
        model["sensitivity_fits"],
        ["unit_weights_full_range", "caller_weights_interior_range"],
        "model.sensitivity",
    )
    exact(
        model["offspring_mean_identification"],
        "observed_intensity_divided_by_parent_intensity",
        "model.mu",
    )
    exact(model["backend_capability"], "bounded_nonlinear_least_squares", "model.backend")
    exact(model["maturity"], "experimental_minimum_contrast", "model.maturity")
    curve = request["curve"]
    if not isinstance(curve, list) or not 8 <= len(curve) <= 1_000:
        raise ContractError("curve must contain 8-1000 rows")
    radii = []
    observed = []
    weights = []
    for index, raw in enumerate(curve):
        row = obj(raw, {"radius_um", "observed_k_um2", "weight"}, f"curve[{index}]")
        radius = number(row["radius_um"], "curve.radius")
        value = number(row["observed_k_um2"], "curve.K")
        weight = number(row["weight"], "curve.weight")
        if radius <= 0.0 or value < 0.0 or weight <= 0.0 or (radii and radius <= radii[-1]):
            raise ContractError("curve rows are invalid or unordered")
        radii.append(radius)
        observed.append(value)
        weights.append(weight)
    intensity = number(request["observed_intensity_per_um2"], "intensity")
    if intensity <= 0.0:
        raise ContractError("observed intensity must be positive")
    bounds = obj(
        request["bounds"],
        {"kappa_min_per_um2", "kappa_max_per_um2", "sigma_min_um", "sigma_max_um"},
        "bounds",
    )
    kappa_min = number(bounds["kappa_min_per_um2"], "bounds.kappa_min")
    kappa_max = number(bounds["kappa_max_per_um2"], "bounds.kappa_max")
    sigma_min = number(bounds["sigma_min_um"], "bounds.sigma_min")
    sigma_max = number(bounds["sigma_max_um"], "bounds.sigma_max")
    if min(kappa_min, sigma_min) <= 0.0 or kappa_min >= kappa_max or sigma_min >= sigma_max:
        raise ContractError("parameter bounds are invalid")
    maximum_iterations = integer(request["maximum_iterations"], "iterations", 10, 100_000)
    resources = obj(
        request["resources"],
        {"maximum_curve_rows", "maximum_iterations", "maximum_output_bytes", "timeout_seconds"},
        "resources",
    )
    if len(curve) > integer(resources["maximum_curve_rows"], "resources.rows", 8, 1_000):
        raise ContractError("curve row cap exceeded")
    if maximum_iterations > integer(
        resources["maximum_iterations"], "resources.iterations", 10, 100_000
    ):
        raise ContractError("iteration cap exceeded")
    integer(resources["maximum_output_bytes"], "resources.output", 1, 1_048_576)
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3_600)
    return {
        "radii": np.asarray(radii, dtype=np.float64),
        "observed": np.asarray(observed, dtype=np.float64),
        "weights": np.asarray(weights, dtype=np.float64),
        "intensity": intensity,
        "lower": np.log([kappa_min, sigma_min]),
        "upper": np.log([kappa_max, sigma_max]),
        "maximum_iterations": maximum_iterations,
    }


def thomas_k(radii: np.ndarray, kappa: float, sigma: float) -> np.ndarray:
    return np.pi * radii**2 + (1.0 / kappa) * (
        1.0 - np.exp(-(radii**2) / (4.0 * sigma**2))
    )


def run_fit(
    config: dict[str, Any], name: str, weight_rule: str, first: int, last: int, weights: np.ndarray
) -> dict[str, Any]:
    radii = config["radii"][first:last]
    observed = config["observed"][first:last]
    selected_weights = weights[first:last]

    def residual(log_parameters: np.ndarray) -> np.ndarray:
        kappa, sigma = np.exp(log_parameters)
        fitted = thomas_k(radii, float(kappa), float(sigma))
        return np.sqrt(selected_weights) * (np.power(observed, 0.25) - np.power(fitted, 0.25))

    result = optimize.least_squares(
        residual,
        x0=(config["lower"] + config["upper"]) / 2.0,
        bounds=(config["lower"], config["upper"]),
        max_nfev=config["maximum_iterations"],
        ftol=1e-13,
        xtol=1e-13,
        gtol=1e-13,
        method="trf",
    )
    kappa, sigma = np.exp(result.x)
    residuals = residual(result.x)
    return {
        "fit_name": name,
        "weight_rule": weight_rule,
        "first_row": first,
        "last_row_exclusive": last,
        "row_count": last - first,
        "kappa_parent_per_um2": float(kappa),
        "sigma_um": float(sigma),
        "mu_offspring": float(config["intensity"] / kappa),
        "objective": float(np.dot(residuals, residuals)),
        "converged": bool(result.success),
        "status": int(result.status),
        "message": str(result.message),
        "evaluations": int(result.nfev),
        "optimality": float(result.optimality),
    }


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    count = config["radii"].size
    primary = run_fit(
        config, "primary_weighted_full_range", "caller_supplied", 0, count, config["weights"]
    )
    unweighted = run_fit(
        config,
        "sensitivity_unweighted_full_range",
        "unit",
        0,
        count,
        np.ones(count, dtype=np.float64),
    )
    interior = run_fit(
        config,
        "sensitivity_weighted_interior_range",
        "caller_supplied",
        1,
        count - 1,
        config["weights"],
    )
    fitted = thomas_k(
        config["radii"], primary["kappa_parent_per_um2"], primary["sigma_um"]
    )
    transformed = np.power(config["observed"], 0.25) - np.power(fitted, 0.25)
    curve = [
        {
            "radius_um": float(config["radii"][index]),
            "observed_k_um2": float(config["observed"][index]),
            "weight": float(config["weights"][index]),
            "fitted_k_um2": float(fitted[index]),
            "transformed_residual": float(transformed[index]),
            "weighted_squared_contribution": float(
                config["weights"][index] * transformed[index] ** 2
            ),
        }
        for index in range(count)
    ]
    fits = [primary, unweighted, interior]
    finite = all(
        math.isfinite(float(value))
        for result in fits
        for key, value in result.items()
        if key in {"kappa_parent_per_um2", "sigma_um", "mu_offspring", "objective", "optimality"}
    ) and np.isfinite(fitted).all()
    complete = finite and all(result["converged"] and result["optimality"] <= 1e-6 for result in fits)
    return {
        "format": "marklab.scipy_thomas_minimum_contrast_worker_result",
        "version": 1,
        "backend": {
            "name": "scipy",
            "version": f"scipy-{SCIPY_VERSION}",
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha,
            "worker_sha256": worker_sha,
        },
        "request_sha256": request_sha,
        "fit_state": "complete" if complete else "nonconverged",
        "primary_fit": primary,
        "sensitivity_fits": [unweighted, interior],
        "curve": curve,
    }


def main() -> None:
    import scipy

    if scipy.__version__ != SCIPY_VERSION or sys.version_info[:2] != (3, 12):
        raise ContractError("SciPy or Python version drift")
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    raw = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if len(raw) > 16 * 1024 * 1024:
        raise ContractError("request exceeds 16 MiB")
    request_sha = hashlib.sha256(raw).hexdigest()
    result = fit(validate(json.loads(raw), lock_sha, worker_sha), request_sha, lock_sha, worker_sha)
    sys.stdout.write(json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")))
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(
            f"marklab SciPy Thomas minimum-contrast worker failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
