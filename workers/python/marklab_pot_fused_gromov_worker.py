#!/usr/bin/env python3
"""Pinned POT worker for bounded entropic fused Gromov-Wasserstein alignment."""

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
    initialization: str,
    plan: np.ndarray,
    errors: list[float],
    feature_cost: np.ndarray,
    source_structure: np.ndarray,
    target_structure: np.ndarray,
) -> dict[str, Any]:
    source = np.asarray([row["mass"] for row in request["source"]], dtype=np.float64)
    target = np.asarray([row["mass"] for row in request["target"]], dtype=np.float64)
    source_marginals = np.sum(plan, axis=1)
    target_marginals = np.sum(plan, axis=0)
    residual = max(
        float(np.max(np.abs(source_marginals - source))),
        float(np.max(np.abs(target_marginals - target))),
    )
    feature_objective, structural_objective, entropy = fused_gromov.plan_objectives(
        plan, feature_cost, source_structure, target_structure
    )
    alpha = float(request["alpha"])
    regularized = (
        alpha * feature_objective
        + (1.0 - alpha) * structural_objective
        - float(request["epsilon"]) * entropy
    )
    final_change = float(errors[-1])
    # POT logs the plan change every tenth iteration, starting with the first.
    iterations = min(int(request["maximum_iterations"]), 10 * (len(errors) - 1) + 1)
    return {
        "initialization": initialization,
        "iterations": iterations,
        "converged": final_change <= float(request["tolerance"]),
        "final_plan_change": final_change,
        "plan": fused_gromov.plan_entries(request, plan, feature_cost),
        "source_marginals": source_marginals.tolist(),
        "target_marginals": target_marginals.tolist(),
        "maximum_marginal_residual": residual,
        "feature_objective": feature_objective,
        "structural_objective": structural_objective,
        "entropy": entropy,
        "regularized_objective": regularized,
    }


def fit(request: dict[str, Any]) -> list[dict[str, Any]]:
    """Fit entropic FGW from each starting plan by projected gradient descent (POT ``PGD``).

    Minimizes ``alpha * <C, P> + (1 - alpha) * sum_ijkl (S_ik - T_jl)^2 P_ij P_kl
    - epsilon * H(P)`` over couplings ``P`` with the request's source and target marginals,
    where ``C`` is the scaled squared feature distance and ``S``, ``T`` the scaled structures.
    """
    source, target, feature_cost, source_structure, target_structure = fused_gromov.problem(request)
    starting_plans = fused_gromov.initial_plans(
        source, target, feature_cost, source_structure, target_structure
    )
    fits = []
    with fused_gromov.no_solver_warnings():
        for name, initial in zip(fused_gromov.INITIALIZATIONS, starting_plans, strict=True):
            plan, log = ot.gromov.entropic_fused_gromov_wasserstein(
                feature_cost,
                source_structure,
                target_structure,
                source,
                target,
                loss_fun="square_loss",
                epsilon=float(request["epsilon"]),
                # POT weights the feature term by 1 - alpha; Marklab's alpha weights features.
                alpha=1.0 - float(request["alpha"]),
                G0=initial,
                max_iter=int(request["maximum_iterations"]),
                tol=float(request["tolerance"]),
                solver="PGD",
                log=True,
            )
            plan = np.asarray(plan, dtype=np.float64)
            errors = [float(value) for value in log["err"]]
            if (
                not errors
                or not np.all(np.isfinite(plan))
                or not all(math.isfinite(value) for value in errors)
            ):
                raise ContractError(f"FGW {name} returned a non-finite or incomplete fit")
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
    return fits


def main() -> None:
    require_versions(pot=ot.__version__)
    lock_digest, worker_digest = worker_digests(__file__)
    raw = read_request(MAXIMUM_REQUEST_BYTES)
    request = fused_gromov.validate_request(
        json.loads(raw),
        request_format="marklab.pot_fused_gromov_wasserstein_request",
        extra_request_keys=(),
        extra_resource_keys=(),
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )
    fits = fit(request)
    best = min(fits, key=lambda fit: float(fit["regularized_objective"]))
    result = {
        "format": "marklab.pot_fused_gromov_wasserstein_worker_result",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(raw),
        "fit_state": "complete" if all(fit["converged"] for fit in fits) else "nonconverged",
        "best_initialization": best["initialization"],
        "best_plan": best["plan"],
        "initialization_sensitivity": fits,
        "claim_status": "descriptive_alignment_not_correspondence",
    }
    write_result(result, sort_keys=False, newline=False)


if __name__ == "__main__":
    run_worker(main, "FGW worker failed")
