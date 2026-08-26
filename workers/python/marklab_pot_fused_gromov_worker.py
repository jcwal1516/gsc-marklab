#!/usr/bin/env python3
"""Pinned POT worker for bounded entropic fused Gromov-Wasserstein alignment."""

from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path
import sys
from typing import Any
import warnings

import numpy as np
import ot


POT_VERSION = "0.9.7.post1"
INITIALIZATIONS = ("independent_mass", "feature_emd", "structure_profile_emd")


def exact_object(value: Any, keys: set[str], path: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        raise ValueError(f"{path} fields differ")
    return value


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = exact_object(
        request,
        {
            "format",
            "version",
            "backend",
            "source",
            "target",
            "source_structure_row_major",
            "target_structure_row_major",
            "alpha",
            "epsilon",
            "feature_scale",
            "structure_scale",
            "tolerance",
            "maximum_iterations",
            "resources",
        },
        "request",
    )
    if request["format"] != "marklab.pot_fused_gromov_wasserstein_request" or request["version"] != 1:
        raise ValueError("request identity differs")
    backend = exact_object(
        request["backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "backend",
    )
    if (
        backend["name"] != "pot"
        or backend["version"] != POT_VERSION
        or backend["python_version"] != "3.12"
        or backend["environment_lock_sha256"] != lock_digest
        or backend["worker_sha256"] != worker_digest
        or ot.__version__ != POT_VERSION
        or sys.version_info[:2] != (3, 12)
    ):
        raise ValueError("backend identity differs")
    for support in ("source", "target"):
        if not isinstance(request[support], list) or not 1 <= len(request[support]) <= 32:
            raise ValueError(f"{support} dimensions differ")
        for index, row in enumerate(request[support]):
            exact_object(row, {"id", "mass", "features"}, f"{support}[{index}]")
    exact_object(
        request["resources"],
        {
            "maximum_source_points",
            "maximum_target_points",
            "maximum_feature_dimension",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    return request


def initialization_plans(
    source: np.ndarray,
    target: np.ndarray,
    feature_cost: np.ndarray,
    source_structure: np.ndarray,
    target_structure: np.ndarray,
) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    independent = np.outer(source, target)
    feature = ot.emd(source, target, feature_cost)
    source_profile = source_structure @ source
    target_profile = target_structure @ target
    profile_cost = (source_profile[:, None] - target_profile[None, :]) ** 2
    structure = ot.emd(source, target, profile_cost)
    return independent, feature, structure


def summarize(
    request: dict[str, Any],
    initialization: str,
    plan: np.ndarray,
    errors: list[float],
    feature_cost: np.ndarray,
    source_structure: np.ndarray,
    target_structure: np.ndarray,
) -> dict[str, Any]:
    source = np.asarray([row["mass"] for row in request["source"]], dtype=np.float64)
    target = np.asarray([row["mass"] for row in request["target"]], dtype=np.float64)
    rows, columns = plan.shape
    source_marginals = np.sum(plan, axis=1)
    target_marginals = np.sum(plan, axis=0)
    residual = max(
        float(np.max(np.abs(source_marginals - source))),
        float(np.max(np.abs(target_marginals - target))),
    )
    feature_objective = float(np.sum(feature_cost * plan))
    difference = (
        source_structure[:, None, :, None]
        - target_structure[None, :, None, :]
    )
    structural_objective = float(
        np.sum((difference * difference) * plan[:, :, None, None] * plan[None, None, :, :])
    )
    positive = plan > 0.0
    entropy = -float(np.sum(plan[positive] * np.log(plan[positive])))
    regularized = (
        float(request["alpha"]) * feature_objective
        + (1.0 - float(request["alpha"])) * structural_objective
        - float(request["epsilon"]) * entropy
    )
    final_change = float(errors[-1])
    iterations = min(int(request["maximum_iterations"]), 10 * (len(errors) - 1) + 1)
    return {
        "initialization": initialization,
        "iterations": iterations,
        "converged": final_change <= float(request["tolerance"]),
        "final_plan_change": final_change,
        "plan": [
            {
                "source_id": request["source"][index // columns]["id"],
                "target_id": request["target"][index % columns]["id"],
                "mass": float(mass),
                "feature_cost": float(feature_cost[index // columns, index % columns]),
            }
            for index, mass in enumerate(plan.ravel())
        ],
        "source_marginals": source_marginals.tolist(),
        "target_marginals": target_marginals.tolist(),
        "maximum_marginal_residual": residual,
        "feature_objective": feature_objective,
        "structural_objective": structural_objective,
        "entropy": entropy,
        "regularized_objective": regularized,
    }


def run(raw: bytes) -> dict[str, Any]:
    path = Path(__file__)
    request = validate(
        json.loads(raw),
        hashlib.sha256(path.with_name("uv.lock").read_bytes()).hexdigest(),
        hashlib.sha256(path.read_bytes()).hexdigest(),
    )
    source = np.asarray([row["mass"] for row in request["source"]], dtype=np.float64)
    target = np.asarray([row["mass"] for row in request["target"]], dtype=np.float64)
    source_features = np.asarray([row["features"] for row in request["source"]], dtype=np.float64)
    target_features = np.asarray([row["features"] for row in request["target"]], dtype=np.float64)
    feature_cost = np.sum(
        ((source_features[:, None, :] - target_features[None, :, :]) / float(request["feature_scale"])) ** 2,
        axis=2,
    )
    rows = len(source)
    columns = len(target)
    source_structure = np.asarray(request["source_structure_row_major"], dtype=np.float64).reshape(rows, rows)
    target_structure = np.asarray(request["target_structure_row_major"], dtype=np.float64).reshape(columns, columns)
    source_structure = source_structure / float(request["structure_scale"])
    target_structure = target_structure / float(request["structure_scale"])
    initial_plans = initialization_plans(
        source, target, feature_cost, source_structure, target_structure
    )
    fits = []
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        for name, initial in zip(INITIALIZATIONS, initial_plans, strict=True):
            plan, log = ot.gromov.entropic_fused_gromov_wasserstein(
                feature_cost,
                source_structure,
                target_structure,
                source,
                target,
                loss_fun="square_loss",
                epsilon=float(request["epsilon"]),
                alpha=1.0 - float(request["alpha"]),
                G0=initial,
                max_iter=int(request["maximum_iterations"]),
                tol=float(request["tolerance"]),
                solver="PGD",
                log=True,
            )
            plan = np.asarray(plan, dtype=np.float64)
            errors = [float(value) for value in log["err"]]
            if not errors or not np.all(np.isfinite(plan)) or not all(math.isfinite(value) for value in errors):
                raise ValueError(f"FGW {name} returned a non-finite or incomplete fit")
            fits.append(
                summarize(
                    request,
                    name,
                    plan,
                    errors,
                    feature_cost,
                    source_structure,
                    target_structure,
                )
            )
    if caught:
        messages = "; ".join(str(item.message) for item in caught)
        raise ValueError(f"POT emitted solver warnings: {messages}")
    best = min(fits, key=lambda fit: float(fit["regularized_objective"]))
    return {
        "format": "marklab.pot_fused_gromov_wasserstein_worker_result",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(raw).hexdigest(),
        "fit_state": "complete" if all(fit["converged"] for fit in fits) else "nonconverged",
        "best_initialization": best["initialization"],
        "best_plan": best["plan"],
        "initialization_sensitivity": fits,
        "claim_status": "descriptive_alignment_not_correspondence",
    }


def main() -> None:
    raw = sys.stdin.buffer.read()
    try:
        encoded = json.dumps(run(raw), allow_nan=False, separators=(",", ":")).encode()
    except Exception as error:
        print(f"FGW worker failed: {error}", file=sys.stderr)
        raise SystemExit(2) from error
    sys.stdout.buffer.write(encoded)


if __name__ == "__main__":
    main()
