#!/usr/bin/env python3
"""Pinned POT worker for bounded entropic partial FGW sensitivity."""

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
            "transported_mass",
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
    if request["format"] != "marklab.pot_partial_fused_gromov_wasserstein_request" or request["version"] != 1:
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
            "maximum_solver_fits",
            "feasibility_tolerance",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    return request


def make_problem(request: dict[str, Any]) -> tuple[np.ndarray, ...]:
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
    return (
        source,
        target,
        feature_cost,
        source_structure / float(request["structure_scale"]),
        target_structure / float(request["structure_scale"]),
    )


def initial_plans(
    source: np.ndarray,
    target: np.ndarray,
    feature_cost: np.ndarray,
    source_structure: np.ndarray,
    target_structure: np.ndarray,
    mass: float,
) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    independent = mass * np.outer(source, target)
    feature = mass * ot.emd(source, target, feature_cost)
    source_profile = source_structure @ source
    target_profile = target_structure @ target
    profile_cost = (source_profile[:, None] - target_profile[None, :]) ** 2
    structure = mass * ot.emd(source, target, profile_cost)
    return independent, feature, structure


def summarize(
    request: dict[str, Any],
    scenario: str,
    initialization: str,
    mass_control: float,
    alpha: float,
    epsilon: float,
    plan: np.ndarray,
    errors: list[float],
    source: np.ndarray,
    target: np.ndarray,
    feature_cost: np.ndarray,
    source_structure: np.ndarray,
    target_structure: np.ndarray,
) -> dict[str, Any]:
    rows, columns = plan.shape
    source_marginals = np.sum(plan, axis=1)
    target_marginals = np.sum(plan, axis=0)
    transported = float(np.sum(plan))
    unmatched_source = source - source_marginals
    unmatched_target = target - target_marginals
    violation = max(
        abs(transported - mass_control),
        float(np.max(np.maximum(-unmatched_source, 0.0))),
        float(np.max(np.maximum(-unmatched_target, 0.0))),
    )
    feature_objective = float(np.sum(feature_cost * plan))
    difference = source_structure[:, None, :, None] - target_structure[None, :, None, :]
    structural_objective = float(
        np.sum((difference * difference) * plan[:, :, None, None] * plan[None, None, :, :])
    )
    positive = plan > 0.0
    entropy = -float(np.sum(plan[positive] * np.log(plan[positive])))
    regularized = alpha * feature_objective + (1.0 - alpha) * structural_objective - epsilon * entropy
    final_change = float(errors[-1])
    return {
        "scenario": scenario,
        "initialization": initialization,
        "transported_mass_control": mass_control,
        "alpha": alpha,
        "epsilon": epsilon,
        "iterations": len(errors),
        "converged": final_change <= float(request["tolerance"]),
        "final_plan_change": final_change,
        "plan": [
            {
                "source_id": request["source"][index // columns]["id"],
                "target_id": request["target"][index % columns]["id"],
                "mass": float(value),
                "feature_cost": float(feature_cost[index // columns, index % columns]),
            }
            for index, value in enumerate(plan.ravel())
        ],
        "source_marginals": source_marginals.tolist(),
        "target_marginals": target_marginals.tolist(),
        "transported_mass": transported,
        "unmatched_source_mass": unmatched_source.tolist(),
        "unmatched_target_mass": unmatched_target.tolist(),
        "maximum_constraint_violation": violation,
        "feature_objective": feature_objective,
        "structural_objective": structural_objective,
        "entropy": entropy,
        "regularized_objective": regularized,
    }


def solve(
    request: dict[str, Any],
    scenario: str,
    initialization: str,
    mass: float,
    alpha: float,
    epsilon: float,
    initial: np.ndarray,
    source: np.ndarray,
    target: np.ndarray,
    feature_cost: np.ndarray,
    source_structure: np.ndarray,
    target_structure: np.ndarray,
) -> dict[str, Any]:
    plan = np.asarray(initial, dtype=np.float64)
    structural_loss = (
        source_structure[:, None, :, None]
        - target_structure[None, :, None, :]
    ) ** 2
    errors = []
    for _ in range(int(request["maximum_iterations"])):
        structural_gradient = 2.0 * np.einsum(
            "ijkl,kl->ij", structural_loss, plan, optimize=True
        )
        linearized_cost = alpha * feature_cost + (1.0 - alpha) * structural_gradient
        updated = ot.partial.entropic_partial_wasserstein(
            source,
            target,
            linearized_cost,
            reg=epsilon,
            m=mass,
            method="sinkhorn_log",
            numItermax=1000,
            stopThr=min(float(request["tolerance"]) / 10.0, 1e-12),
        )
        updated = np.asarray(updated, dtype=np.float64)
        error = float(np.linalg.norm(updated - plan))
        errors.append(error)
        plan = updated
        if error <= float(request["tolerance"]):
            break
    if not errors or not np.all(np.isfinite(plan)) or not all(math.isfinite(value) for value in errors):
        raise ValueError(f"partial FGW {scenario}/{initialization} returned an incomplete fit")
    return summarize(
        request,
        scenario,
        initialization,
        mass,
        alpha,
        epsilon,
        plan,
        errors,
        source,
        target,
        feature_cost,
        source_structure,
        target_structure,
    )


def run(raw: bytes) -> dict[str, Any]:
    path = Path(__file__)
    request = validate(
        json.loads(raw),
        hashlib.sha256(path.with_name("uv.lock").read_bytes()).hexdigest(),
        hashlib.sha256(path.read_bytes()).hexdigest(),
    )
    source, target, feature_cost, source_structure, target_structure = make_problem(request)
    mass = float(request["transported_mass"])
    alpha = float(request["alpha"])
    epsilon = float(request["epsilon"])
    bases = initial_plans(
        source, target, feature_cost, source_structure, target_structure, mass
    )
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        initialization_sensitivity = [
            solve(
                request,
                "baseline",
                name,
                mass,
                alpha,
                epsilon,
                initial,
                source,
                target,
                feature_cost,
                source_structure,
                target_structure,
            )
            for name, initial in zip(
                ("independent_mass", "feature_emd", "structure_profile_emd"),
                bases,
                strict=True,
            )
        ]
        baseline = initialization_sensitivity[0]
        parameter_controls = [
            ("alpha_lower", mass, alpha / 2.0, epsilon),
            ("alpha_upper", mass, (1.0 + alpha) / 2.0, epsilon),
            ("mass_lower", mass * 0.8, alpha, epsilon),
            ("mass_upper", min(mass * 1.2, 0.999999), alpha, epsilon),
            ("epsilon_lower", mass, alpha, epsilon / 2.0),
            ("epsilon_upper", mass, alpha, epsilon * 2.0),
        ]
        additional = []
        for scenario, scenario_mass, scenario_alpha, scenario_epsilon in parameter_controls:
            independent = scenario_mass * np.outer(source, target)
            additional.append(
                solve(
                    request,
                    scenario,
                    "independent_mass",
                    scenario_mass,
                    scenario_alpha,
                    scenario_epsilon,
                    independent,
                    source,
                    target,
                    feature_cost,
                    source_structure,
                    target_structure,
                )
            )
        parameter_sensitivity = [
            additional[0],
            baseline,
            additional[1],
            additional[2],
            additional[3],
            additional[4],
            additional[5],
        ]
    if caught:
        messages = "; ".join(str(item.message) for item in caught)
        raise ValueError(f"POT emitted solver warnings: {messages}")
    best = min(initialization_sensitivity, key=lambda fit: float(fit["regularized_objective"]))
    all_fits = initialization_sensitivity + parameter_sensitivity
    return {
        "format": "marklab.pot_partial_fused_gromov_wasserstein_worker_result",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(raw).hexdigest(),
        "fit_state": "complete" if all(fit["converged"] for fit in all_fits) else "nonconverged",
        "best_initialization": best["initialization"],
        "best_plan": best["plan"],
        "source_marginals": best["source_marginals"],
        "target_marginals": best["target_marginals"],
        "transported_mass": best["transported_mass"],
        "unmatched_source_mass": best["unmatched_source_mass"],
        "unmatched_target_mass": best["unmatched_target_mass"],
        "initialization_sensitivity": initialization_sensitivity,
        "parameter_sensitivity": parameter_sensitivity,
        "claim_status": "ensemble_descriptive_alignment_not_correspondence",
    }


def main() -> None:
    raw = sys.stdin.buffer.read()
    try:
        encoded = json.dumps(run(raw), allow_nan=False, separators=(",", ":")).encode()
    except Exception as error:
        print(f"partial FGW worker failed: {error}", file=sys.stderr)
        raise SystemExit(2) from error
    sys.stdout.buffer.write(encoded)


if __name__ == "__main__":
    main()
