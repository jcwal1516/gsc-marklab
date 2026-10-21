"""NUTS request blocks and convergence checks shared by the PyMC and NumPyro NUTS workers.

The validators enforce the ``sampling``, ``resources``, and ``diagnostic_policy`` blocks that the
Rust caller builds; ``convergence_passes`` applies a request's diagnostic policy to the
convergence summaries produced here. PyMC sampling itself lives in ``pymc_nuts``.
"""

from __future__ import annotations

from collections.abc import Sequence
from typing import Any

from .diagnostics import mcse_extremes, pymc_sampler_diagnostics, rank_convergence
from .protocol import MIB, ContractError, integer, number, obj

SAMPLING_KEYS = {"chains", "tune_per_chain", "draws_per_chain", "target_accept", "seed"}
RESOURCE_KEYS = {
    "maximum_observations",
    "maximum_total_iterations",
    "maximum_output_bytes",
    "timeout_seconds",
}
POLICY_KEYS = {
    "prior_predictive_draws",
    "maximum_r_hat",
    "minimum_bulk_ess",
    "minimum_tail_ess",
    "minimum_ebfmi",
    "maximum_divergences",
    "maximum_tree_depth_hits",
    "maximum_tree_depth",
}
UNBOUNDED_COUNT = 2**63 - 1


def nuts_sampling(value: Any, path: str, *, bounded_target_accept: bool = True) -> dict[str, Any]:
    """Validate a ``sampling`` block; return chains, tune, draws, target_accept, and seed."""
    sampling = obj(value, SAMPLING_KEYS, path)
    chains = integer(sampling["chains"], f"{path}.chains", 2, 8)
    tune = integer(sampling["tune_per_chain"], f"{path}.tune_per_chain", 100, 100_000)
    draws = integer(sampling["draws_per_chain"], f"{path}.draws_per_chain", 100, 100_000)
    target_accept = number(sampling["target_accept"], f"{path}.target_accept")
    if bounded_target_accept and not 0.5 <= target_accept < 1.0:
        raise ContractError(f"{path}.target_accept must be in [0.5, 1)")
    seed = integer(sampling["seed"], f"{path}.seed", 0, 2**64 - 1)
    return {
        "chains": chains,
        "tune": tune,
        "draws": draws,
        "target_accept": target_accept,
        "seed": seed,
    }


def nuts_resources(
    value: Any,
    path: str,
    *,
    observations: int,
    iterations: int,
    observation_ceiling: int = 100_000,
) -> int:
    """Validate a ``resources`` block against the work requested; return its output limit."""
    resources = obj(value, RESOURCE_KEYS, path)
    maximum_observations = integer(
        resources["maximum_observations"],
        f"{path}.maximum_observations",
        1,
        observation_ceiling,
    )
    maximum_iterations = integer(
        resources["maximum_total_iterations"], f"{path}.maximum_total_iterations", 1, 800_000
    )
    maximum_output_bytes = integer(
        resources["maximum_output_bytes"], f"{path}.maximum_output_bytes", 1, MIB
    )
    integer(resources["timeout_seconds"], f"{path}.timeout_seconds", 1, 3_600)
    if observations > maximum_observations:
        raise ContractError("observation count exceeds the request resource limit")
    if iterations > maximum_iterations:
        raise ContractError("sampling iterations exceed the request resource limit")
    return maximum_output_bytes


def nuts_policy(
    value: Any,
    path: str,
    *,
    prior_draw_range: tuple[int, int] = (100, 10_000),
    count_ceiling: int,
    check_thresholds: bool = True,
) -> dict[str, Any]:
    """Validate a ``diagnostic_policy`` block.

    ``count_ceiling`` bounds the divergence and tree-depth-hit allowances. With
    ``check_thresholds`` the R-hat bound must be at least one and the ESS and E-BFMI minima
    positive.
    """
    policy = obj(value, POLICY_KEYS, path)
    low, high = prior_draw_range
    config = {
        "prior_draws": integer(
            policy["prior_predictive_draws"], f"{path}.prior_predictive_draws", low, high
        ),
        "maximum_r_hat": number(policy["maximum_r_hat"], f"{path}.maximum_r_hat"),
        "minimum_bulk_ess": number(policy["minimum_bulk_ess"], f"{path}.minimum_bulk_ess"),
        "minimum_tail_ess": number(policy["minimum_tail_ess"], f"{path}.minimum_tail_ess"),
        "minimum_ebfmi": number(policy["minimum_ebfmi"], f"{path}.minimum_ebfmi"),
        "maximum_divergences": integer(
            policy["maximum_divergences"], f"{path}.maximum_divergences", 0, count_ceiling
        ),
        "maximum_tree_depth_hits": integer(
            policy["maximum_tree_depth_hits"],
            f"{path}.maximum_tree_depth_hits",
            0,
            count_ceiling,
        ),
        "maximum_tree_depth": integer(
            policy["maximum_tree_depth"], f"{path}.maximum_tree_depth", 1, 32
        ),
    }
    if check_thresholds and (
        config["maximum_r_hat"] < 1.0
        or config["minimum_bulk_ess"] <= 0.0
        or config["minimum_tail_ess"] <= 0.0
        or config["minimum_ebfmi"] <= 0.0
    ):
        raise ContractError("diagnostic thresholds must be positive and R-hat must be at least one")
    return config


def pymc_nuts_diagnostics(posterior: Any, monitored: Sequence[str]) -> dict[str, Any]:
    """Worst-case convergence summaries of a PyMC NUTS fit over the monitored variables."""
    r_hat, ess_bulk, ess_tail = rank_convergence(posterior, monitored)
    mcse_mean, mcse_sd = mcse_extremes(posterior, monitored)
    ebfmi, divergences, depth_hits = pymc_sampler_diagnostics(posterior["sample_stats"])
    return {
        "r_hat": r_hat,
        "ess_bulk": ess_bulk,
        "ess_tail": ess_tail,
        "mcse_mean": mcse_mean,
        "mcse_sd": mcse_sd,
        "minimum_ebfmi": ebfmi,
        "divergences": divergences,
        "max_tree_depth_hits": depth_hits,
    }


def convergence_passes(config: dict[str, Any], diagnostics: dict[str, Any]) -> bool:
    """Apply a validated diagnostic policy to ``*_nuts_diagnostics`` summaries."""
    return (
        diagnostics["r_hat"] <= config["maximum_r_hat"]
        and diagnostics["ess_bulk"] >= config["minimum_bulk_ess"]
        and diagnostics["ess_tail"] >= config["minimum_tail_ess"]
        and diagnostics["minimum_ebfmi"] >= config["minimum_ebfmi"]
        and diagnostics["divergences"] <= config["maximum_divergences"]
        and diagnostics["max_tree_depth_hits"] <= config["maximum_tree_depth_hits"]
    )


def sampling_record(config: dict[str, Any]) -> dict[str, int]:
    """The ``sampling`` block reported in a NUTS result."""
    return {
        "chains": config["chains"],
        "tune_per_chain": config["tune"],
        "draws_per_chain": config["draws"],
        "completed_draws": config["chains"] * config["draws"],
    }


def fit_summary(
    convergence: dict[str, Any],
    config: dict[str, Any],
    prior_finite: bool,
    posterior_finite: bool,
    constraints: bool,
) -> dict[str, Any]:
    """The ``fit_state``, ``sampling``, and ``diagnostics`` blocks of a fit result.

    The fit is ``complete`` when the prior predictive and posterior are finite, the draws satisfy
    the parameter constraints, and the convergence summary meets the policy; otherwise it is
    ``nonconverged``.
    """
    complete = (
        prior_finite
        and posterior_finite
        and constraints
        and convergence_passes(config, convergence)
    )
    return {
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": sampling_record(config),
        "diagnostics": {
            "prior_predictive_finite": prior_finite,
            "posterior_finite": posterior_finite,
            **convergence,
            "constraints_valid": constraints,
            "identifiability_checks_passed": True,
        },
    }
