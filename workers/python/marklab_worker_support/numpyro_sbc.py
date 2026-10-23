"""Simulation-based calibration harness for NumPyro refits of PyMC source contracts.

The beta-binomial, Dirichlet-multinomial, and Student-t SBC workers share the request contract
(a PyMC source request plus ``calibration`` and ``resources`` blocks), the per-replicate NUTS
fit and diagnostic gate, and the result layout. Each worker supplies the prior simulation, the
model, and the quantities it ranks.
"""

from __future__ import annotations

from collections.abc import Callable, Iterable
from types import ModuleType
from typing import Any

import jax
import numpy as np
import numpyro
from numpyro.infer import NUTS

from .numpyro_nuts import nuts_diagnostics, run_chains, validate_source_request
from .protocol import (
    ContractError,
    backend_identity,
    exact,
    integer,
    number,
    obj,
    sha256_hex,
)
from .sbc import calibration_passes, rank_and_coverage, rank_summary

CALIBRATION_KEYS = frozenset(
    {
        "replicates",
        "rank_bins",
        "interval_probability",
        "maximum_tree_depth",
        "minimum_rank_uniformity_p_value",
        "minimum_coverage",
        "maximum_coverage",
    }
)


def total_trials(config: dict[str, Any]) -> int:
    """Binomial trials simulated per replicate."""
    return int(np.asarray(config["trials"]).sum())


def validate_request(
    request: Any,
    request_format: str,
    lock_sha: str,
    worker_sha: str,
    source_contract: ModuleType,
    *,
    simulated_resource: str = "maximum_simulated_trials",
    simulated_limit: int = 200_000_000,
    simulated_per_replicate: Callable[[dict[str, Any]], int] = total_trials,
) -> dict[str, Any]:
    """Validate an SBC request and return the source configuration extended for SBC.

    ``resources[simulated_resource]`` (at most ``simulated_limit``) bounds the number of
    simulated units over all replicates, ``replicates * simulated_per_replicate(config)``.
    """
    request, config, _ = validate_source_request(
        request,
        request_format,
        lock_sha,
        worker_sha,
        source_contract,
        extra_keys={"calibration", "resources"},
    )
    calibration = obj(request["calibration"], CALIBRATION_KEYS, "calibration")
    config.update(
        {
            "replicates": integer(calibration["replicates"], "calibration.replicates", 20, 100),
            "rank_bins": integer(calibration["rank_bins"], "calibration.rank_bins", 2, 100),
            "maximum_tree_depth": integer(
                calibration["maximum_tree_depth"], "calibration.maximum_tree_depth", 10, 16
            ),
            "minimum_rank_p": number(
                calibration["minimum_rank_uniformity_p_value"], "calibration.rank_p"
            ),
            "minimum_coverage": number(
                calibration["minimum_coverage"], "calibration.minimum_coverage"
            ),
            "maximum_coverage": number(
                calibration["maximum_coverage"], "calibration.maximum_coverage"
            ),
        }
    )
    exact(number(calibration["interval_probability"], "calibration.interval"), 0.9, "interval")
    resources = obj(
        request["resources"],
        {
            "maximum_replicates",
            simulated_resource,
            "maximum_total_iterations",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    replicates = config["replicates"]
    iterations = replicates * config["chains"] * (config["tune"] + config["draws"])
    if (
        replicates > integer(resources["maximum_replicates"], "resources.replicates", 1, 100)
        or replicates * simulated_per_replicate(config)
        > integer(
            resources[simulated_resource], f"resources.{simulated_resource}", 1, simulated_limit
        )
        or iterations
        > integer(resources["maximum_total_iterations"], "resources.iterations", 1, 1_000_000)
    ):
        raise ContractError("SBC resources exceeded")
    config["maximum_output_bytes"] = integer(
        resources["maximum_output_bytes"], "resources.output", 1, 2 * 1_048_576
    )
    return config


def nuts_kernel(model: Callable[..., None], config: dict[str, Any]) -> NUTS:
    """The dense-mass NUTS kernel reused by every replicate."""
    return NUTS(
        model,
        target_accept_prob=config["target_accept"],
        max_tree_depth=config["maximum_tree_depth"],
        dense_mass=True,
    )


def sample_replicate(
    kernel: NUTS,
    config: dict[str, Any],
    rng_key: Any,
    names: list[str],
    arguments: dict[str, Any],
    derive: Callable[[dict[str, np.ndarray]], dict[str, np.ndarray]] | None = None,
) -> tuple[dict[str, np.ndarray], dict[str, Any]]:
    """Fit one simulated data set; return float64 samples by chain and their convergence.

    ``derive`` maps the samples to derived quantities that join (or replace) the sampled sites
    before the convergence of ``names`` is summarized (without MCSE).
    """
    sampler, samples = run_chains(kernel, config, rng_key, arguments)
    if derive is not None:
        samples = {**samples, **derive(samples)}
    extra = sampler.get_extra_fields(group_by_chain=True)
    return samples, nuts_diagnostics(samples, names, extra, config["maximum_tree_depth"])


def diagnostics_failure(diagnostics: dict[str, Any], config: dict[str, Any]) -> str | None:
    """The failure reason when a replicate's fit misses the diagnostic policy, else None."""
    if (
        diagnostics["r_hat"] <= config["maximum_r_hat"]
        and diagnostics["ess_bulk"] >= config["minimum_bulk_ess"]
        and diagnostics["ess_tail"] >= config["minimum_tail_ess"]
        and diagnostics["minimum_ebfmi"] >= config["minimum_ebfmi"]
        and diagnostics["divergences"] <= config["maximum_divergences"]
        and diagnostics["max_tree_depth_hits"] <= config["maximum_tree_depth_hits"]
    ):
        return None
    return (
        f"diagnostics_failed:r_hat={diagnostics['r_hat']:.17g},"
        f"bulk={diagnostics['ess_bulk']:.17g},"
        f"tail={diagnostics['ess_tail']:.17g},"
        f"ebfmi={diagnostics['minimum_ebfmi']:.17g},"
        f"divergences={diagnostics['divergences']},"
        f"depth_hits={diagnostics['max_tree_depth_hits']}"
    )


def run_replicates(
    count: int, replicate: Callable[[int], dict[str, Any] | str]
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Run ``replicate`` for each index; a returned string or a raised exception is a failure."""
    completed: list[dict[str, Any]] = []
    failures: list[dict[str, Any]] = []
    for index in range(count):
        try:
            outcome = replicate(index)
        except Exception as error:
            failures.append(
                {"replicate": index, "reason": f"{type(error).__name__}:{str(error)[:400]}"}
            )
            continue
        if isinstance(outcome, str):
            failures.append({"replicate": index, "reason": outcome})
        else:
            completed.append(outcome)
    return completed, failures


def record_ranks(row: dict[str, Any], values: Iterable[tuple[str, Any, float]]) -> None:
    """Store ``<name>_rank`` and ``<name>_covered`` for each (name, draws, truth)."""
    for name, draws, truth in values:
        row[f"{name}_rank"], row[f"{name}_covered"] = rank_and_coverage(draws, truth)


def summarize(
    completed: list[dict[str, Any]], names: list[str], config: dict[str, Any]
) -> dict[str, dict[str, Any]]:
    """Rank and coverage summaries of the scalar quantities ``names``."""
    total_draws = config["chains"] * config["draws"]
    return {
        name: rank_summary(
            [row[f"{name}_rank"] for row in completed],
            [row[f"{name}_covered"] for row in completed],
            total_draws,
            config["rank_bins"],
        )
        for name in names
    }


def result(
    result_format: str,
    raw: bytes,
    lock_sha: str,
    worker_sha: str,
    config: dict[str, Any],
    completed: list[dict[str, Any]],
    failures: list[dict[str, Any]],
    diagnostics: dict[str, Any],
    summaries: Iterable[dict[str, Any]],
) -> dict[str, Any]:
    """The SBC result; ``summaries`` are the rank summaries the calibration policy covers."""
    passes = calibration_passes(
        summaries,
        failures,
        config["minimum_rank_p"],
        config["minimum_coverage"],
        config["maximum_coverage"],
    )
    return {
        "format": result_format,
        "version": 1,
        "backend": backend_identity("numpyro", numpyro.__version__, lock_sha, worker_sha),
        "jax_version": jax.__version__,
        "request_sha256": sha256_hex(raw),
        "fit_state": "complete" if passes else "nonconverged",
        "replicates": completed,
        "failures": failures,
        "diagnostics": diagnostics,
    }
