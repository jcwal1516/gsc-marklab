#!/usr/bin/env python3
"""Pinned POT worker for fused unbalanced Gromov-Wasserstein with KL-relaxed marginals.

POT 0.9.7.post1 ``ot.gromov.fused_unbalanced_gromov_wasserstein`` minimizes the lower bound of
fused unbalanced Gromov-Wasserstein (Thual et al. 2022; Sejourne, Vialard and Peyre 2021) over two
sample couplings ``P`` and ``Q`` of equal mass:

    sum_ijkl (S_ik - T_jl)^2 P_ij Q_kl + (a / 2) (<C, P> + <C, Q>)
    + rho KL(P_1 Q_1^T | u u^T) + rho KL(P_2 Q_2^T | v v^T)
    + epsilon KL(P (x) Q | (u v^T) (x) (u v^T)),

where ``S`` and ``T`` are the scaled structure matrices, ``C`` is the squared Euclidean feature
cost, ``u`` and ``v`` are the source and target masses, ``KL(x | y) = sum x log(x / y) - x + y``
and ``_1``/``_2`` denote row and column sums. Unlike partial transport, marginals may fall below
or exceed the supplied masses; ``rho`` sets how strongly they are held to them. With
``epsilon = 0`` the KL majorization-minimization (MM) solver is used, otherwise log-domain
Sinkhorn. Each of three starting plans is fitted; the lowest objective is reported.
"""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path
from typing import Any

sys.path.append(str(Path(__file__).resolve().parent))

import numpy as np
import ot

from marklab_worker_support import fused_gromov, versions
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
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

# The same 16 MiB input bound and growth argument as the partial FGW worker.
MAXIMUM_REQUEST_BYTES = 17 * MIB
REQUEST_KEYS = {
    "format",
    "version",
    "backend",
    "source",
    "target",
    "source_structure_row_major",
    "target_structure_row_major",
    "linear_weight",
    "marginal_relaxation",
    "epsilon",
    "feature_scale",
    "structure_scale",
    "tolerance",
    "maximum_iterations",
    "inner_tolerance",
    "maximum_inner_iterations",
    "resources",
}
OBJECTIVE_RELATIVE_TOLERANCE = 1e-8


def validate(request: Any, lock_sha256: str, worker_sha256: str) -> dict[str, Any]:
    request = obj(request, REQUEST_KEYS, "request")
    exact(request["format"], "marklab.pot_unbalanced_fused_gromov_wasserstein_request", "format")
    exact(request["version"], 1, "request.version")
    validate_backend(
        request["backend"],
        "request.backend",
        name="pot",
        version=versions.POT,
        lock_sha256=lock_sha256,
        worker_sha256=worker_sha256,
    )
    for support in ("source", "target"):
        rows = request[support]
        if not isinstance(rows, list) or not 1 <= len(rows) <= fused_gromov.MAXIMUM_SUPPORT_POINTS:
            raise ContractError(f"{support} must contain 1-32 support points")
        for index, row in enumerate(rows):
            obj(row, fused_gromov.SUPPORT_KEYS, f"{support}[{index}]")
    obj(
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
    number(request["linear_weight"], "linear_weight", 0.0, 1e6)
    number(request["marginal_relaxation"], "marginal_relaxation", 1e-6, 1e6)
    number(request["epsilon"], "epsilon", 0.0, 1e3)
    number(request["tolerance"], "tolerance", 1e-15, 1.0)
    number(request["inner_tolerance"], "inner_tolerance", 1e-15, 1.0)
    integer(request["maximum_iterations"], "maximum_iterations", 1, 10_000)
    integer(request["maximum_inner_iterations"], "maximum_inner_iterations", 1, 100_000)
    integer(request["resources"]["maximum_output_bytes"], "maximum_output_bytes", 1, 16 * MIB)
    return request


def kl(values: np.ndarray, reference: np.ndarray) -> float:
    """``sum x log(x / y) - x + y`` with ``0 log 0 = 0``."""
    positive = values > 0.0
    log_terms = values[positive] * np.log(values[positive] / reference[positive])
    return float(np.sum(log_terms) - np.sum(values) + np.sum(reference))


def objective_terms(
    first: np.ndarray,
    second: np.ndarray,
    source: np.ndarray,
    target: np.ndarray,
    feature_cost: np.ndarray,
    source_structure: np.ndarray,
    target_structure: np.ndarray,
    linear_weight: float,
    relaxation: float,
    epsilon: float,
) -> dict[str, float]:
    """Each term of the documented lower-bound objective, evaluated directly."""
    difference = source_structure[:, None, :, None] - target_structure[None, :, None, :]
    structural = float(np.einsum("ijkl,ij,kl->", difference**2, first, second, optimize=True))
    linear = 0.5 * linear_weight * float(np.sum(feature_cost * (first + second)))
    source_marginal = kl(np.outer(first.sum(axis=1), second.sum(axis=1)), np.outer(source, source))
    target_marginal = kl(np.outer(first.sum(axis=0), second.sum(axis=0)), np.outer(target, target))
    reference = np.outer(source, target)
    entropic = kl(np.outer(first, second), np.outer(reference, reference)) if epsilon else 0.0
    total = (
        structural + linear + relaxation * (source_marginal + target_marginal) + epsilon * entropic
    )
    return {
        "structural": structural,
        "linear": linear,
        "source_marginal_kl": source_marginal,
        "target_marginal_kl": target_marginal,
        "entropic_kl": entropic,
        "total": total,
    }


def fit_pot(
    request: dict[str, Any], initial: np.ndarray, problem: tuple[np.ndarray, ...]
) -> tuple[Any, Any, dict[str, Any]]:
    """One POT fit from ``initial``: MM for ``epsilon = 0``, otherwise log-domain Sinkhorn."""
    source, target, feature_cost, source_structure, target_structure = problem
    linear_weight = float(request["linear_weight"])
    relaxation = float(request["marginal_relaxation"])
    epsilon = float(request["epsilon"])
    return ot.gromov.fused_unbalanced_gromov_wasserstein(
        source_structure,
        target_structure,
        wx=source,
        wy=target,
        reg_marginals=relaxation,
        epsilon=epsilon,
        divergence="kl",
        unbalanced_solver="mm" if epsilon == 0.0 else "sinkhorn_log",
        alpha=linear_weight,
        M=feature_cost,
        init_pi=initial,
        max_iter=int(request["maximum_iterations"]),
        tol=float(request["tolerance"]),
        max_iter_ot=int(request["maximum_inner_iterations"]),
        tol_ot=float(request["inner_tolerance"]),
        log=True,
    )


def solve(
    request: dict[str, Any],
    initialization: str,
    initial: np.ndarray,
    problem: tuple[np.ndarray, ...],
) -> dict[str, Any]:
    source, target, feature_cost, source_structure, target_structure = problem
    linear_weight = float(request["linear_weight"])
    relaxation = float(request["marginal_relaxation"])
    epsilon = float(request["epsilon"])
    try:
        first, second, log = fit_pot(request, initial, problem)
    except ValueError as error:
        # POT reports a NaN coupling when the MM update divides a vanished plan by itself,
        # for example when the optimum transports no mass at all.
        raise ContractError(
            f"unbalanced FGW {initialization}: POT coupling collapsed ({error}); "
            "the objective may favour transporting no mass"
        ) from error
    first = np.asarray(first, dtype=np.float64)
    second = np.asarray(second, dtype=np.float64)
    errors = [float(value) for value in log["error"]]
    if (
        not errors
        or not np.isfinite(first).all()
        or not np.isfinite(second).all()
        or np.any(first < 0.0)
        or np.any(second < 0.0)
    ):
        raise ContractError(f"unbalanced FGW {initialization} returned an incomplete fit")
    terms = objective_terms(
        first,
        second,
        source,
        target,
        feature_cost,
        source_structure,
        target_structure,
        linear_weight,
        relaxation,
        epsilon,
    )
    reported = float(log["fugw_cost"])
    if abs(terms["total"] - reported) > OBJECTIVE_RELATIVE_TOLERANCE * max(1.0, abs(reported)):
        raise ContractError(
            f"recomputed unbalanced FGW objective {terms['total']} differs from POT {reported}"
        )
    source_marginals = first.sum(axis=1)
    target_marginals = first.sum(axis=0)
    return {
        "initialization": initialization,
        "iterations": len(errors),
        "final_plan_change": errors[-1],
        "converged": errors[-1] < float(request["tolerance"]),
        "plan": fused_gromov.plan_entries(request, first, feature_cost),
        "coupling_difference_maximum": float(np.max(np.abs(first - second))),
        "source_marginals": source_marginals.tolist(),
        "target_marginals": target_marginals.tolist(),
        "transported_mass": float(first.sum()),
        "source_marginal_minus_mass": (source_marginals - source).tolist(),
        "target_marginal_minus_mass": (target_marginals - target).tolist(),
        "objective": terms,
    }


def fit_all(request: dict[str, Any], problem: tuple[np.ndarray, ...]) -> list[dict[str, Any]]:
    """Fit from each shared FGW starting plan, rejecting any POT solver warning.

    The shared starts need equal total masses, so they are formed from the normalized masses
    and scaled to carry ``sqrt(sum(u) * sum(v))``, the geometric mean of the two totals.
    """
    source, target, feature_cost, source_structure, target_structure = problem
    scale = math.sqrt(float(source.sum()) * float(target.sum()))
    starting_plans = [
        scale * plan
        for plan in fused_gromov.initial_plans(
            source / source.sum(),
            target / target.sum(),
            feature_cost,
            source_structure,
            target_structure,
        )
    ]
    with fused_gromov.no_solver_warnings():
        return [
            solve(request, name, initial, problem)
            for name, initial in zip(fused_gromov.INITIALIZATIONS, starting_plans, strict=True)
        ]


def main() -> None:
    require_versions(pot=ot.__version__)
    lock_digest, worker_digest = worker_digests(__file__)
    raw = read_request(MAXIMUM_REQUEST_BYTES)
    request = validate(json.loads(raw), lock_digest, worker_digest)
    fits = fit_all(request, fused_gromov.problem(request))
    best = min(fits, key=lambda fit: fit["objective"]["total"])
    result = {
        "format": "marklab.pot_unbalanced_fused_gromov_wasserstein_worker_result",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(raw),
        "fit_state": "complete" if best["converged"] else "nonconverged",
        "best_initialization": best["initialization"],
        "initialization_sensitivity": fits,
        "objective_spread": float(
            max(fit["objective"]["total"] for fit in fits) - best["objective"]["total"]
        ),
        "claim_status": "ensemble_descriptive_alignment_not_correspondence",
    }
    if not all(math.isfinite(fit["objective"]["total"]) for fit in fits):
        raise ContractError("unbalanced FGW objective is not finite")
    write_result(
        result,
        max_bytes=int(request["resources"]["maximum_output_bytes"]),
        sort_keys=False,
        newline=False,
    )


if __name__ == "__main__":
    run_worker(main, "unbalanced FGW worker failed")
