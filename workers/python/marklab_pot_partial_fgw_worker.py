#!/usr/bin/env python3
"""Pinned POT worker for bounded entropic partial FGW sensitivity."""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path
from typing import Any

sys.path.append(str(Path(__file__).resolve().parent))

import numpy as np
import ot

from marklab_worker_support import fused_gromov
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    worker_digests,
    write_result,
)

# Rust re-serializes one JSON input file of at most 16 MiB. JSON strings never grow, and each of
# the at most 10,304 numbers (2 x 32 masses, 64 x 128 features, 2 x 32 x 32 structure entries)
# grows to at most 24 bytes, so a request stays below 16 MiB + 0.25 MiB + 1 KiB of fixed fields.
MAXIMUM_REQUEST_BYTES = 17 * MIB


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
    feature_objective, structural_objective, entropy = fused_gromov.plan_objectives(
        plan, feature_cost, source_structure, target_structure
    )
    regularized = (
        alpha * feature_objective + (1.0 - alpha) * structural_objective - epsilon * entropy
    )
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
        "plan": fused_gromov.plan_entries(request, plan, feature_cost),
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
    """Fit entropic partial FGW by successive linearization from ``initial``.

    Minimizes ``alpha * <C, P> + (1 - alpha) * sum_ijkl (S_ik - T_jl)^2 P_ij P_kl
    - epsilon * H(P)`` over plans with total mass ``mass`` and marginals bounded by the source
    and target masses. Each step solves the entropic partial transport problem for the
    linearized cost ``alpha * C + (1 - alpha) * 2 * sum_kl (S_ik - T_jl)^2 P_kl`` (log-domain
    Sinkhorn) and stops once the Frobenius plan change falls to the tolerance.
    """
    plan = np.asarray(initial, dtype=np.float64)
    structural_loss = (source_structure[:, None, :, None] - target_structure[None, :, None, :]) ** 2
    tolerance = float(request["tolerance"])
    errors = []
    for _ in range(int(request["maximum_iterations"])):
        structural_gradient = 2.0 * np.einsum("ijkl,kl->ij", structural_loss, plan, optimize=True)
        linearized_cost = alpha * feature_cost + (1.0 - alpha) * structural_gradient
        updated = ot.partial.entropic_partial_wasserstein(
            source,
            target,
            linearized_cost,
            reg=epsilon,
            m=mass,
            method="sinkhorn_log",
            numItermax=1000,
            stopThr=min(tolerance / 10.0, 1e-12),
        )
        updated = np.asarray(updated, dtype=np.float64)
        error = float(np.linalg.norm(updated - plan))
        errors.append(error)
        plan = updated
        if error <= tolerance:
            break
    if (
        not errors
        or not np.all(np.isfinite(plan))
        or not all(math.isfinite(value) for value in errors)
    ):
        raise ContractError(f"partial FGW {scenario}/{initialization} returned an incomplete fit")
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


def run(request: dict[str, Any]) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Fit the baseline from each starting plan, then one-at-a-time parameter controls.

    Returns the initialization sensitivity (three starts at the requested controls) and the
    parameter sensitivity (alpha, transported mass, and epsilon each moved down and up, from the
    independent coupling, with the baseline in second position).
    """
    problem = fused_gromov.problem(request)
    source, target = problem[0], problem[1]
    mass = float(request["transported_mass"])
    alpha = float(request["alpha"])
    epsilon = float(request["epsilon"])
    starting_plans = fused_gromov.initial_plans(*problem, mass)
    with fused_gromov.no_solver_warnings():
        initialization_sensitivity = [
            solve(request, "baseline", name, mass, alpha, epsilon, initial, *problem)
            for name, initial in zip(fused_gromov.INITIALIZATIONS, starting_plans, strict=True)
        ]
        parameter_controls = [
            ("alpha_lower", mass, alpha / 2.0, epsilon),
            ("alpha_upper", mass, (1.0 + alpha) / 2.0, epsilon),
            ("mass_lower", mass * 0.8, alpha, epsilon),
            ("mass_upper", min(mass * 1.2, 0.999999), alpha, epsilon),
            ("epsilon_lower", mass, alpha, epsilon / 2.0),
            ("epsilon_upper", mass, alpha, epsilon * 2.0),
        ]
        controls = []
        for scenario, scenario_mass, scenario_alpha, scenario_epsilon in parameter_controls:
            independent = scenario_mass * np.outer(source, target)
            controls.append(
                solve(
                    request,
                    scenario,
                    "independent_mass",
                    scenario_mass,
                    scenario_alpha,
                    scenario_epsilon,
                    independent,
                    *problem,
                )
            )
    parameter_sensitivity = [controls[0], initialization_sensitivity[0], *controls[1:]]
    return initialization_sensitivity, parameter_sensitivity


def main() -> None:
    require_versions(pot=ot.__version__)
    lock_digest, worker_digest = worker_digests(__file__)
    raw = read_request(MAXIMUM_REQUEST_BYTES)
    request = fused_gromov.validate_request(
        json.loads(raw),
        request_format="marklab.pot_partial_fused_gromov_wasserstein_request",
        extra_request_keys=("transported_mass",),
        extra_resource_keys=("maximum_solver_fits", "feasibility_tolerance"),
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )
    initialization_sensitivity, parameter_sensitivity = run(request)
    best = min(initialization_sensitivity, key=lambda fit: float(fit["regularized_objective"]))
    all_fits = initialization_sensitivity + parameter_sensitivity
    result = {
        "format": "marklab.pot_partial_fused_gromov_wasserstein_worker_result",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(raw),
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
    write_result(result, sort_keys=False, newline=False)


if __name__ == "__main__":
    run_worker(main, "partial FGW worker failed")
