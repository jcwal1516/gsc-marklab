#!/usr/bin/env python3
"""SciPy worker for the nested-grid Strauss pseudolikelihood contract."""

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
RESOLUTION_KEYS = {
    "grid_x",
    "grid_y",
    "cell_area_um2",
    "weight_sum_um2",
    "observed_nodes",
    "dummy_nodes",
    "neighbor_visits",
    "rows",
}
ROW_KEYS = {
    "node_id",
    "node_kind",
    "x_um",
    "y_um",
    "weight_um2",
    "response",
    "neighbor_count",
    "block",
}
SPATIAL_BLOCKS = 4
FINITE_FIT_FIELDS = [
    "beta_per_um2",
    "gamma",
    "objective",
    "model_se_log_beta",
    "model_se_log_gamma",
    "robust_se_log_beta",
    "robust_se_log_gamma",
]


def resolution(raw: Any, path: str) -> dict[str, Any]:
    """Validate one Berman-Turner quadrature table (data points plus a dummy grid)."""
    table = obj(raw, RESOLUTION_KEYS, path)
    grid_x = integer(table["grid_x"], path + ".grid_x", 1, 100_000)
    grid_y = integer(table["grid_y"], path + ".grid_y", 1, 100_000)
    area = number(table["cell_area_um2"], path + ".cell_area_um2")
    weight_sum = number(table["weight_sum_um2"], path + ".weight_sum_um2")
    observed = integer(table["observed_nodes"], path + ".observed_nodes", 1, 10_000)
    dummy = integer(table["dummy_nodes"], path + ".dummy_nodes", 1, 100_000)
    rows = table["rows"]
    if not isinstance(rows, list) or len(rows) != observed + dummy or dummy != grid_x * grid_y:
        raise ContractError(path + " dimensions")
    responses = []
    weights = []
    neighbor_counts = []
    blocks = []
    for index, raw_row in enumerate(rows):
        row = obj(raw_row, ROW_KEYS, f"{path}.rows[{index}]")
        if (
            not isinstance(row["node_id"], str)
            or not row["node_id"]
            or row["node_kind"] not in {"observed", "dummy"}
        ):
            raise ContractError("node identity")
        number(row["x_um"], "x")
        number(row["y_um"], "y")
        weight = number(row["weight_um2"], "weight")
        response = number(row["response"], "response")
        neighbor_count = integer(row["neighbor_count"], "neighbors", 0, 10_000)
        block = integer(row["block"], "block", 0, SPATIAL_BLOCKS - 1)
        if weight <= 0 or response < 0 or (row["node_kind"] == "observed") != (response > 0):
            raise ContractError("node values")
        responses.append(response)
        weights.append(weight)
        neighbor_counts.append(neighbor_count)
        blocks.append(block)
    if abs(sum(weights) - weight_sum) > 1e-9 * max(1, weight_sum):
        raise ContractError("weight sum")
    integer(table["neighbor_visits"], path + ".neighbor_visits", 1, 100_000_000)
    return {
        "gx": grid_x,
        "gy": grid_y,
        "area": area,
        "weight_sum": weight_sum,
        "y": np.array(responses),
        "w": np.array(weights),
        "z": np.array(neighbor_counts, dtype=float),
        "block": np.array(blocks),
    }


def validate(request: Any, lock: str, worker: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "model",
            "window",
            "interaction_radius_um",
            "bounds",
            "maximum_iterations",
            "coarse",
            "fine",
            "resources",
        },
        "request",
    )
    exact(request["format"], "marklab.pymc_worker_request", "format")
    integer(request["version"], "version", 1, 1)
    validate_backend(
        request["backend"],
        "backend",
        name="scipy",
        version=BACKEND_VERSION,
        lock_sha256=lock,
        worker_sha256=worker,
    )
    model = obj(
        request["model"],
        {
            "family",
            "conditional_intensity",
            "quadrature",
            "pair_boundary",
            "uncertainty",
            "backend_capability",
            "maturity",
        },
        "model",
    )
    exact(model["family"], "strauss_inhibitory_point_process", "model.family")
    exact(
        model["backend_capability"],
        "bounded_weighted_poisson_optimization",
        "model.backend_capability",
    )
    bounds = obj(
        request["bounds"],
        {"beta_min_per_um2", "beta_max_per_um2", "gamma_min", "gamma_max"},
        "bounds",
    )
    beta_min = number(bounds["beta_min_per_um2"], "bounds.beta_min_per_um2")
    gamma_min = number(bounds["gamma_min"], "bounds.gamma_min")
    lower = np.log([beta_min, gamma_min])
    beta_max = number(bounds["beta_max_per_um2"], "bounds.beta_max_per_um2")
    gamma_max = number(bounds["gamma_max"], "bounds.gamma_max")
    upper = np.log([beta_max, gamma_max])
    # gamma <= 1 keeps the process inhibitory.
    if np.any(lower >= upper) or upper[1] > 1e-15:
        raise ContractError("bounds")
    iterations = integer(request["maximum_iterations"], "maximum_iterations", 10, 100_000)
    resources = obj(
        request["resources"],
        {
            "maximum_table_rows",
            "maximum_iterations",
            "maximum_neighbor_visits",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    integer(resources["maximum_table_rows"], "resources.maximum_table_rows", 1, 100_000)
    integer(resources["maximum_iterations"], "resources.maximum_iterations", 10, 100_000)
    integer(
        resources["maximum_neighbor_visits"], "resources.maximum_neighbor_visits", 1, 100_000_000
    )
    maximum_output_bytes = integer(
        resources["maximum_output_bytes"], "resources.maximum_output_bytes", 1, MIB
    )
    integer(resources["timeout_seconds"], "resources.timeout_seconds", 1, 3_600)
    return {
        "lower": lower,
        "upper": upper,
        "iterations": iterations,
        "coarse": resolution(request["coarse"], "coarse"),
        "fine": resolution(request["fine"], "fine"),
        "maximum_output_bytes": maximum_output_bytes,
    }


def fit_one(
    table: dict[str, Any], lower: np.ndarray, upper: np.ndarray, iterations: int
) -> dict[str, Any]:
    """Fit the Strauss model by Berman-Turner weighted Poisson pseudolikelihood.

    The conditional intensity at node ``i`` is ``beta * gamma**z_i`` with ``z_i`` the number of
    neighbours within the interaction radius, so ``eta_i = log(beta) + z_i * log(gamma)``. With
    quadrature weights ``w_i`` and responses ``y_i`` (``1/w_i`` at data points, 0 at dummy
    nodes) the minimized objective is ``sum_i w_i * (exp(eta_i) - y_i * eta_i)`` over the box
    ``[lower, upper]`` on ``(log(beta), log(gamma))`` with ``gamma <= 1``. No prior is used.
    Model standard errors come from the inverse Fisher information; robust ones from a sandwich
    over the four spatial blocks.
    """
    design = np.column_stack([np.ones(table["y"].size), table["z"]])
    weights = table["w"]
    responses = table["y"]

    def objective(theta: np.ndarray) -> float:
        eta = design @ theta
        return float(np.sum(weights * (np.exp(eta) - responses * eta)))

    def gradient(theta: np.ndarray) -> np.ndarray:
        eta = design @ theta
        return design.T @ (weights * (np.exp(eta) - responses))

    initial = (lower + upper) / 2
    initial_objective = objective(initial)
    result = optimize.minimize(
        objective,
        initial,
        jac=gradient,
        bounds=list(zip(lower, upper, strict=True)),
        method="L-BFGS-B",
        options={"maxiter": iterations, "gtol": 1e-12, "ftol": 1e-15, "maxls": 50},
    )
    theta = np.asarray(result.x)
    eta = design @ theta
    mu = np.exp(eta)
    hessian = design.T @ (design * (weights * mu)[:, None])
    covariance = np.linalg.inv(hessian)
    residual_scores = (weights * (mu - responses))[:, None] * design
    block_scores = np.array(
        [
            np.sum(residual_scores * (table["block"] == block)[:, None], axis=0)
            for block in range(SPATIAL_BLOCKS)
        ]
    )
    robust = covariance @ (block_scores.T @ block_scores) @ covariance
    final_gradient = gradient(theta)
    projected_gradient = final_gradient.copy()
    for index in range(2):
        if theta[index] <= lower[index] + 1e-10 and final_gradient[index] > 0:
            projected_gradient[index] = 0
        if theta[index] >= upper[index] - 1e-10 and final_gradient[index] < 0:
            projected_gradient[index] = 0
    tiny = np.finfo(float).tiny
    return {
        "grid_x": table["gx"],
        "grid_y": table["gy"],
        "beta_per_um2": float(np.exp(theta[0])),
        "gamma": float(np.exp(theta[1])),
        "log_beta": float(theta[0]),
        "log_gamma": float(theta[1]),
        "objective": objective(theta),
        "initial_objective": initial_objective,
        "gradient_norm": float(np.linalg.norm(projected_gradient, np.inf)),
        "hessian_condition": float(np.linalg.cond(hessian)),
        "model_se_log_beta": float(np.sqrt(covariance[0, 0])),
        "model_se_log_gamma": float(np.sqrt(covariance[1, 1])),
        "robust_se_log_beta": float(np.sqrt(max(robust[0, 0], tiny))),
        "robust_se_log_gamma": float(np.sqrt(max(robust[1, 1], tiny))),
        "weight_sum_um2": table["weight_sum"],
        "converged": bool(result.success),
        "evaluations": int(result.nfev),
        "message": str(result.message),
    }


def fit_is_complete(fit: dict[str, Any]) -> bool:
    return (
        fit["converged"]
        and fit["gradient_norm"] <= 1e-6
        and all(math.isfinite(float(fit[key])) for key in FINITE_FIT_FIELDS)
    )


def main() -> None:
    require_versions(scipy=scipy.__version__)
    lock, worker = worker_digests(__file__)
    raw = read_request(16 * MIB)
    config = validate(json.loads(raw), lock, worker)
    coarse = fit_one(config["coarse"], config["lower"], config["upper"], config["iterations"])
    fine = fit_one(config["fine"], config["lower"], config["upper"], config["iterations"])
    complete = all(fit_is_complete(fit) for fit in [coarse, fine])
    result = {
        "format": "marklab.scipy_strauss_pseudolikelihood_worker_result",
        "version": 1,
        "backend": backend_identity("scipy", BACKEND_VERSION, lock, worker),
        "request_sha256": sha256_hex(raw),
        "fit_state": "complete" if complete else "nonconverged",
        "coarse_fit": coarse,
        "fine_fit": fine,
        "refinement": {
            "absolute_beta_difference": abs(coarse["beta_per_um2"] - fine["beta_per_um2"]),
            "absolute_gamma_difference": abs(coarse["gamma"] - fine["gamma"]),
            "absolute_objective_difference": abs(coarse["objective"] - fine["objective"]),
        },
    }
    write_result(result, max_bytes=config["maximum_output_bytes"])


if __name__ == "__main__":
    run_worker(main, "marklab SciPy Strauss pseudolikelihood worker failed")
