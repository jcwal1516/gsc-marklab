#!/usr/bin/env python3
"""Pinned SciPy worker for finite entropic partial optimal transport."""

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


SCIPY_VERSION = "1.18.1"


def exact_object(value: Any, keys: set[str], path: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        raise ValueError(f"{path} fields differ")
    return value


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = exact_object(
        request,
        {"format", "version", "backend", "source", "target", "costs_row_major", "transported_mass", "epsilon", "resources"},
        "request",
    )
    if request["format"] != "marklab.scipy_partial_transport_request" or request["version"] != 1:
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
    for support in ("source", "target"):
        if not isinstance(request[support], list) or not 1 <= len(request[support]) <= 64:
            raise ValueError(f"{support} dimensions differ")
        for index, row in enumerate(request[support]):
            exact_object(row, {"id", "mass"}, f"{support}[{index}]")
    exact_object(
        request["resources"],
        {"maximum_source_points", "maximum_target_points", "maximum_plan_variables", "feasibility_tolerance", "maximum_output_bytes", "timeout_seconds"},
        "resources",
    )
    return request


def run(raw: bytes) -> dict[str, Any]:
    path = Path(__file__)
    request = validate(
        json.loads(raw),
        hashlib.sha256(path.with_name("uv.lock").read_bytes()).hexdigest(),
        hashlib.sha256(path.read_bytes()).hexdigest(),
    )
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
        value = float(np.dot(cost, plan) + epsilon * np.sum(np.where(plan > 0.0, plan * (np.log(positive) - 1.0), 0.0)))
        gradient = cost + epsilon * np.log(positive)
        return value, gradient

    row_matrix = np.zeros((rows, rows * columns))
    column_matrix = np.zeros((columns, rows * columns))
    for row in range(rows):
        row_matrix[row, row * columns : (row + 1) * columns] = 1.0
    for column in range(columns):
        column_matrix[column, column::columns] = 1.0
    fitted = minimize(
        lambda plan: objective(plan),
        initial,
        method="SLSQP",
        jac=True,
        bounds=[(0.0, min(source[index // columns], target[index % columns], transported)) for index in range(rows * columns)],
        constraints=[
            {"type": "eq", "fun": lambda plan: np.sum(plan) - transported, "jac": lambda plan: np.ones(rows * columns)},
            {"type": "ineq", "fun": lambda plan: source - row_matrix @ plan, "jac": lambda plan: -row_matrix},
            {"type": "ineq", "fun": lambda plan: target - column_matrix @ plan, "jac": lambda plan: -column_matrix},
        ],
        options={"maxiter": 2_000, "ftol": 1e-12},
    )
    if not fitted.success or not np.all(np.isfinite(fitted.x)):
        raise ValueError(f"partial transport optimizer failed: {fitted.message}")
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
        raise ValueError("partial transport constraints exceed tolerance")
    positive = plan > 0.0
    entropy = -float(np.sum(plan[positive] * np.log(plan[positive])))
    transport_cost = float(np.dot(cost, plan))
    regularized = transport_cost + epsilon * float(np.sum(plan[positive] * (np.log(plan[positive]) - 1.0)))
    return {
        "format": "marklab.scipy_partial_transport_worker_result",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(raw).hexdigest(),
        "optimizer": {"method": "SLSQP", "success": True, "iterations": int(fitted.nit), "message": str(fitted.message)},
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
    raw = sys.stdin.buffer.read()
    try:
        encoded = json.dumps(run(raw), allow_nan=False, separators=(",", ":")).encode()
    except Exception as error:
        print(f"partial transport worker failed: {error}", file=sys.stderr)
        raise SystemExit(2) from error
    sys.stdout.buffer.write(encoded)


if __name__ == "__main__":
    main()
