#!/usr/bin/env python3
"""SciPy worker for finite entropic partial optimal transport."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import numpy as np
import scipy
from scipy.optimize import minimize

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

# Rust re-serializes a source and a target CSV file of at most 16 MiB each (at most 64 rows each)
# and at most 4,096 costs. Each CSV byte becomes at most 6 JSON bytes (\u00XX escapes), each cost
# at most 25 bytes, and each support row adds fewer than 32 bytes of keys: 192 MiB + 100 KiB +
# 4 KiB + fixed fields, rounded up.
MAXIMUM_REQUEST_BYTES = 193 * MIB


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "source",
            "target",
            "costs_row_major",
            "transported_mass",
            "epsilon",
            "resources",
        },
        "request",
    )
    if request["format"] != "marklab.scipy_partial_transport_request" or request["version"] != 1:
        raise ContractError("request identity differs")
    validate_backend(
        request["backend"],
        "backend",
        name="scipy",
        version=versions.SCIPY,
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )
    for support in ("source", "target"):
        if not isinstance(request[support], list) or not 1 <= len(request[support]) <= 64:
            raise ContractError(f"{support} dimensions differ")
        for index, row in enumerate(request[support]):
            obj(row, {"id", "mass"}, f"{support}[{index}]")
    obj(
        request["resources"],
        {
            "maximum_source_points",
            "maximum_target_points",
            "maximum_plan_variables",
            "feasibility_tolerance",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    return request


def run(raw: bytes, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    """Solve the entropic partial transport problem by SLSQP and report the plan.

    The plan ``P`` minimizes ``<C, P> + epsilon * sum(P log P - P)`` subject to ``sum(P) = m``,
    row sums at most the source masses, column sums at most the target masses, and
    ``0 <= P_ij <= min(a_i, b_j, m)``, starting from the scaled product of the normalized
    marginals.
    """
    request = validate(json.loads(raw), lock_digest, worker_digest)
    source = np.asarray([row["mass"] for row in request["source"]], dtype=np.float64)
    target = np.asarray([row["mass"] for row in request["target"]], dtype=np.float64)
    rows = len(source)
    columns = len(target)
    cost = np.asarray(request["costs_row_major"], dtype=np.float64)
    transported = float(request["transported_mass"])
    epsilon = float(request["epsilon"])
    initial = transported * np.outer(source / np.sum(source), target / np.sum(target)).ravel()

    def objective(plan: np.ndarray) -> tuple[float, np.ndarray]:
        positive = np.clip(plan, 1e-300, None)
        entropy_terms = np.where(plan > 0.0, plan * (np.log(positive) - 1.0), 0.0)
        value = float(np.dot(cost, plan) + epsilon * np.sum(entropy_terms))
        gradient = cost + epsilon * np.log(positive)
        return value, gradient

    row_matrix = np.zeros((rows, rows * columns))
    column_matrix = np.zeros((columns, rows * columns))
    for row in range(rows):
        row_matrix[row, row * columns : (row + 1) * columns] = 1.0
    for column in range(columns):
        column_matrix[column, column::columns] = 1.0
    bounds = [
        (0.0, min(source[index // columns], target[index % columns], transported))
        for index in range(rows * columns)
    ]
    constraints = [
        {
            "type": "eq",
            "fun": lambda plan: np.sum(plan) - transported,
            "jac": lambda plan: np.ones(rows * columns),
        },
        {
            "type": "ineq",
            "fun": lambda plan: source - row_matrix @ plan,
            "jac": lambda plan: -row_matrix,
        },
        {
            "type": "ineq",
            "fun": lambda plan: target - column_matrix @ plan,
            "jac": lambda plan: -column_matrix,
        },
    ]
    fitted = minimize(
        objective,
        initial,
        method="SLSQP",
        jac=True,
        bounds=bounds,
        constraints=constraints,
        options={"maxiter": 2_000, "ftol": 1e-12},
    )
    if not fitted.success or not np.all(np.isfinite(fitted.x)):
        raise ContractError(f"partial transport optimizer failed: {fitted.message}")
    plan = np.maximum(np.asarray(fitted.x, dtype=np.float64), 0.0)
    source_marginals = row_matrix @ plan
    target_marginals = column_matrix @ plan
    violation = max(
        abs(float(np.sum(plan)) - transported),
        float(np.max(np.maximum(source_marginals - source, 0.0))),
        float(np.max(np.maximum(target_marginals - target, 0.0))),
    )
    tolerance = float(request["resources"]["feasibility_tolerance"])
    if violation > tolerance:
        raise ContractError("partial transport constraints exceed tolerance")
    positive = plan > 0.0
    entropy = -float(np.sum(plan[positive] * np.log(plan[positive])))
    transport_cost = float(np.dot(cost, plan))
    entropy_terms = plan[positive] * (np.log(plan[positive]) - 1.0)
    regularized = transport_cost + epsilon * float(np.sum(entropy_terms))
    return {
        "format": "marklab.scipy_partial_transport_worker_result",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(raw),
        "optimizer": {
            "method": "SLSQP",
            "success": True,
            "iterations": int(fitted.nit),
            "message": str(fitted.message),
        },
        "plan": [
            {
                "source_id": request["source"][index // columns]["id"],
                "target_id": request["target"][index % columns]["id"],
                "mass": float(mass),
                "cost": float(cost[index]),
            }
            for index, mass in enumerate(plan)
        ],
        "source_marginals": source_marginals.tolist(),
        "target_marginals": target_marginals.tolist(),
        "transported_mass": float(np.sum(plan)),
        "unmatched_source_mass": (source - source_marginals).tolist(),
        "unmatched_target_mass": (target - target_marginals).tolist(),
        "transport_cost": transport_cost,
        "entropy": entropy,
        "regularized_objective": regularized,
        "maximum_constraint_violation": violation,
        "constraint_status": "feasible_within_tolerance",
    }


def main() -> None:
    require_versions(scipy=scipy.__version__)
    lock_digest, worker_digest = worker_digests(__file__)
    raw = read_request(MAXIMUM_REQUEST_BYTES)
    result = run(raw, lock_digest, worker_digest)
    write_result(result, sort_keys=False, newline=False)


if __name__ == "__main__":
    run_worker(main, "partial transport worker failed")
