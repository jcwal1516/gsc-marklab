#!/usr/bin/env python3
"""SciPy worker for bounded Thomas-process minimum-contrast fits."""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path
from typing import Any

sys.path.append(str(Path(__file__).resolve().parent))

import numpy as np
import scipy
from scipy import optimize

from marklab_worker_support import versions
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
    backend_identity,
    exact,
    integer,
    number,
    obj,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    validate_backend,
    worker_digests,
    write_result,
)

BACKEND_VERSION = f"scipy-{versions.SCIPY}"
FINITE_FIT_FIELDS = {"kappa_parent_per_um2", "sigma_um", "mu_offspring", "objective", "optimality"}


def validate(request: Any, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "model",
            "curve",
            "observed_intensity_per_um2",
            "bounds",
            "maximum_iterations",
            "resources",
        },
        "request",
    )
    exact(request["format"], "marklab.pymc_worker_request", "request.format")
    integer(request["version"], "request.version", 1, 1)
    validate_backend(
        request["backend"],
        "backend",
        name="scipy",
        version=BACKEND_VERSION,
        lock_sha256=lock_sha,
        worker_sha256=worker_sha,
    )
    model = obj(
        request["model"],
        {
            "family",
            "summary",
            "theoretical_curve",
            "contrast_transform",
            "primary_weights",
            "sensitivity_fits",
            "offspring_mean_identification",
            "backend_capability",
            "maturity",
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
    maximum_output_bytes = integer(
        resources["maximum_output_bytes"], "resources.maximum_output_bytes", 1, MIB
    )
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3_600)
    return {
        "radii": np.asarray(radii, dtype=np.float64),
        "observed": np.asarray(observed, dtype=np.float64),
        "weights": np.asarray(weights, dtype=np.float64),
        "intensity": intensity,
        "lower": np.log([kappa_min, sigma_min]),
        "upper": np.log([kappa_max, sigma_max]),
        "maximum_iterations": maximum_iterations,
        "maximum_output_bytes": maximum_output_bytes,
    }


def thomas_k(radii: np.ndarray, kappa: float, sigma: float) -> np.ndarray:
    """Ripley's K of a Thomas process with parent intensity ``kappa`` and offspring SD ``sigma``.

    ``K(r) = pi r^2 + (1 - exp(-r^2 / (4 sigma^2))) / kappa``.
    """
    return np.pi * radii**2 + (1.0 / kappa) * (1.0 - np.exp(-(radii**2) / (4.0 * sigma**2)))


def run_fit(
    config: dict[str, Any], name: str, weight_rule: str, first: int, last: int, weights: np.ndarray
) -> dict[str, Any]:
    """Fit ``(kappa, sigma)`` by bounded minimum contrast on rows ``first:last``.

    Minimizes ``sum_r w_r * (K_obs(r)**0.25 - K(r; kappa, sigma)**0.25)**2`` over
    ``(log kappa, log sigma)`` within the request bounds (trust-region reflective least squares).
    The mean offspring count is identified as ``observed intensity / kappa``.
    """
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
    fitted = thomas_k(config["radii"], primary["kappa_parent_per_um2"], primary["sigma_um"])
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
    fits_finite = all(
        math.isfinite(float(value))
        for result in fits
        for key, value in result.items()
        if key in FINITE_FIT_FIELDS
    )
    finite = fits_finite and np.isfinite(fitted).all()
    complete = finite and all(
        result["converged"] and result["optimality"] <= 1e-6 for result in fits
    )
    return {
        "format": "marklab.scipy_thomas_minimum_contrast_worker_result",
        "version": 1,
        "backend": backend_identity("scipy", BACKEND_VERSION, lock_sha, worker_sha),
        "request_sha256": request_sha,
        "fit_state": "complete" if complete else "nonconverged",
        "primary_fit": primary,
        "sensitivity_fits": [unweighted, interior],
        "curve": curve,
    }


def main() -> None:
    require_versions(scipy=scipy.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(16 * MIB)
    config = validate(json.loads(raw), lock_sha, worker_sha)
    result = fit(config, sha256_hex(raw), lock_sha, worker_sha)
    write_result(result, max_bytes=config["maximum_output_bytes"])


if __name__ == "__main__":
    run_worker(main, "marklab SciPy Thomas minimum-contrast worker failed")
