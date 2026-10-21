"""Prior-generative SBC harness for the fixed- and inferred-kernel replicated patient LGCP.

Each replicate draws the regression coefficients, hierarchy scales, and (for the inferred
kernel) the Matérn amplitude and length scale from their priors, simulates node counts on the
physical source design, refits the NumPyro model by NUTS, and records the SBC rank and 90%
coverage of the monitored parameters and of one physical latent node.
"""

from __future__ import annotations

from collections.abc import Callable, Sequence
from typing import Any

import arviz as az
import jax
import numpy as np
import numpyro

from . import versions
from .diagnostics import divergence_count, minimum_ebfmi, rank_convergence, tree_depth_hits
from .protocol import (
    ContractError,
    backend_identity,
    exact,
    integer,
    number,
    obj,
    sha256_digest,
    validate_backend,
)
from .replicated_lgcp import center_within_groups, expected_counts
from .sbc import calibration_passes, rank_and_coverage, rank_summary


def validate_sbc_request(
    request: Any,
    request_format: str,
    lock_digest: str,
    worker_digest: str,
    validate_source: Callable[[Any], dict[str, Any]],
) -> dict[str, Any]:
    """Validate an SBC request whose source request ``validate_source`` turns into a config."""
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "jax_version",
            "source_request_sha256",
            "source_request",
            "calibration",
            "resources",
        },
        "request",
    )
    exact(request["format"], request_format, "request.format")
    exact(request["version"], 1, "request.version")
    validate_backend(
        request["backend"],
        "backend",
        name="numpyro",
        version=versions.NUMPYRO,
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )
    exact(request["jax_version"], versions.JAX, "request.jax_version")
    source_digest = sha256_digest(request["source_request_sha256"], "source request digest")
    config = validate_source(request["source_request"])
    calibration = obj(
        request["calibration"],
        {
            "replicates",
            "rank_bins",
            "interval_probability",
            "latent_node_index",
            "minimum_rank_uniformity_p_value",
            "minimum_coverage",
            "maximum_coverage",
        },
        "calibration",
    )
    config["replicates"] = integer(calibration["replicates"], "calibration.replicates", 20, 100)
    config["rank_bins"] = integer(calibration["rank_bins"], "calibration.rank_bins", 2, 20)
    config["latent_node_index"] = integer(
        calibration["latent_node_index"],
        "calibration.latent_node_index",
        0,
        len(config["counts"]) - 1,
    )
    exact(number(calibration["interval_probability"], "calibration.interval"), 0.9, "interval")
    config["minimum_rank_p"] = number(
        calibration["minimum_rank_uniformity_p_value"], "calibration.minimum_rank_p"
    )
    config["minimum_coverage"] = number(
        calibration["minimum_coverage"], "calibration.minimum_coverage"
    )
    config["maximum_coverage"] = number(
        calibration["maximum_coverage"], "calibration.maximum_coverage"
    )
    if not (
        0.0 <= config["minimum_rank_p"] <= 1.0
        and 0.0 <= config["minimum_coverage"] <= config["maximum_coverage"] <= 1.0
    ):
        raise ContractError("calibration thresholds are invalid")
    resources = obj(
        request["resources"],
        {
            "maximum_replicates",
            "maximum_simulated_nodes",
            "maximum_total_iterations",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    maximum_replicates = integer(
        resources["maximum_replicates"], "resources.maximum_replicates", 20, 100
    )
    if config["replicates"] > maximum_replicates:
        raise ContractError("replicate ceiling exceeded")
    maximum_nodes = integer(
        resources["maximum_simulated_nodes"], "resources.maximum_simulated_nodes", 1, 51_200
    )
    if config["replicates"] * len(config["counts"]) > maximum_nodes:
        raise ContractError("simulated-node ceiling exceeded")
    iterations = config["replicates"] * config["chains"] * (config["tune"] + config["draws"])
    maximum_iterations = integer(
        resources["maximum_total_iterations"], "resources.maximum_total_iterations", 1, 2_000_000
    )
    if iterations > maximum_iterations:
        raise ContractError("iteration ceiling exceeded")
    config["maximum_output_bytes"] = integer(
        resources["maximum_output_bytes"], "resources.maximum_output_bytes", 1, 2 * 1_048_576
    )
    config["source_request_sha256"] = source_digest
    return config


def draw_hierarchy_truth(config: dict[str, Any], rng: np.random.Generator) -> dict[str, float]:
    """Draw the intercept, regression effects, and hierarchy scales from their priors."""
    priors = config["priors"]
    return {
        "intercept": float(rng.normal(priors["intercept_mean"], priors["intercept_sd"])),
        "group_effect": float(rng.normal(0.0, priors["group_effect_sd"])),
        "covariate_effect": float(rng.normal(0.0, priors["covariate_effect_sd"])),
        "patient_sd": float(abs(rng.normal(0.0, priors["patient_sd_scale"]))),
        "pattern_sd": float(abs(rng.normal(0.0, priors["pattern_sd_scale"]))),
    }


def draw_random_effects(
    config: dict[str, Any], rng: np.random.Generator, truth: dict[str, float]
) -> tuple[np.ndarray, np.ndarray]:
    """Draw patient effects and within-patient-centred pattern effects."""
    patient_effect = truth["patient_sd"] * rng.normal(0.0, 1.0, len(config["patient_ids"]))
    pattern_raw = rng.normal(0.0, 1.0, len(config["pattern_ids"]))
    center_within_groups(pattern_raw, config["pattern_patient_index"], len(config["patient_ids"]))
    return patient_effect, truth["pattern_sd"] * pattern_raw


def simulate_counts(
    config: dict[str, Any],
    rng: np.random.Generator,
    truth: dict[str, float],
    patient_effect: np.ndarray,
    pattern_effect: np.ndarray,
    latent: np.ndarray,
) -> np.ndarray:
    """Draw Poisson node counts and record the true latent value of the monitored node."""
    expected = expected_counts(
        config,
        truth["intercept"],
        truth["group_effect"],
        truth["covariate_effect"],
        patient_effect,
        pattern_effect,
        latent,
    )
    if not np.isfinite(expected).all() or np.any(expected <= 0.0):
        raise ContractError("nonfinite prior-generative expected count")
    counts = rng.poisson(expected).astype(np.int64)
    if int(counts.sum()) > int(config["request"]["resources"]["maximum_total_events"]):
        raise ContractError("prior-generative event ceiling exceeded")
    truth["latent_node"] = float(latent[config["latent_node_index"]])
    return counts


def run_sbc(
    config: dict[str, Any],
    *,
    simulate: Callable[[int], tuple[dict[str, float], np.ndarray]],
    sample_posterior: Callable[[int, np.ndarray], Any],
    monitored: Sequence[str],
    calibrated: Sequence[str],
    latent_draws: Callable[[dict[str, np.ndarray]], np.ndarray],
    maximum_tree_depth: int,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Run every replicate; a replicate that fails or misses the diagnostic policy is a failure.

    ``sample_posterior(replicate, counts)`` returns the finished sampler, ``calibrated`` names
    the sampled parameters whose ranks are recorded, and ``latent_draws`` maps the samples to
    ``(chain, draw, node)`` latent-field draws.
    """
    policy = config["policy"]
    completed: list[dict[str, Any]] = []
    failures: list[dict[str, Any]] = []
    for replicate in range(config["replicates"]):
        try:
            truth, counts = simulate(replicate)
            sampler = sample_posterior(replicate, counts)
            samples = {
                name: np.asarray(value, dtype=np.float64)
                for name, value in sampler.get_samples(group_by_chain=True).items()
            }
            posterior = az.from_dict({"posterior": {name: samples[name] for name in monitored}})
            r_hat, bulk, tail = rank_convergence(posterior, monitored)
            extra = sampler.get_extra_fields(group_by_chain=True)
            ebfmi = minimum_ebfmi(np.asarray(extra["energy"], dtype=np.float64))
            divergences = divergence_count(extra["diverging"])
            depth_hits = tree_depth_hits(extra["num_steps"], maximum_tree_depth)
            if not (
                r_hat <= policy["maximum_r_hat"]
                and bulk >= policy["minimum_bulk_ess"]
                and tail >= policy["minimum_tail_ess"]
                and ebfmi >= policy["minimum_ebfmi"]
                and divergences <= policy["maximum_divergences"]
                and depth_hits <= policy["maximum_tree_depth_hits"]
            ):
                reason = (
                    f"diagnostics_failed:r_hat={r_hat:.17g},bulk={bulk:.17g},"
                    f"tail={tail:.17g},ebfmi={ebfmi:.17g},"
                    f"divergences={divergences},depth_hits={depth_hits}"
                )
                failures.append({"replicate": replicate, "reason": reason})
                continue
            draws = {name: samples[name].reshape(-1) for name in calibrated}
            latent = latent_draws(samples)
            draws["latent_node"] = latent[..., config["latent_node_index"]].reshape(-1)
            row: dict[str, Any] = {
                "replicate": replicate,
                "simulated_total_events": int(counts.sum()),
                "r_hat": r_hat,
                "ess_bulk": bulk,
                "ess_tail": tail,
                "minimum_ebfmi": ebfmi,
                "divergences": divergences,
                "max_tree_depth_hits": depth_hits,
            }
            for parameter, values in draws.items():
                rank, covered = rank_and_coverage(values, truth[parameter])
                row[f"true_{parameter}"] = truth[parameter]
                row[f"{parameter}_rank"] = rank
                row[f"{parameter}_covered"] = covered
            completed.append(row)
        except Exception as error:
            reason = f"{type(error).__name__}:{str(error)[:400]}"
            failures.append({"replicate": replicate, "reason": reason})
    return completed, failures


def sbc_result(
    config: dict[str, Any],
    completed: list[dict[str, Any]],
    failures: list[dict[str, Any]],
    parameters: Sequence[str],
    result_format: str,
    request_sha256: str,
    lock_digest: str,
    worker_digest: str,
) -> dict[str, Any]:
    """Assemble the SBC result with per-parameter rank summaries and the overall verdict."""
    draws = config["chains"] * config["draws"]
    diagnostics = {}
    for parameter in parameters:
        ranks = [row[f"{parameter}_rank"] for row in completed]
        covered = [row[f"{parameter}_covered"] for row in completed]
        diagnostics[parameter] = rank_summary(ranks, covered, draws, config["rank_bins"])
    passes = calibration_passes(
        diagnostics.values(),
        failures,
        config["minimum_rank_p"],
        config["minimum_coverage"],
        config["maximum_coverage"],
    )
    node = config["request"]["nodes"][config["latent_node_index"]]
    return {
        "format": result_format,
        "version": 1,
        "backend": backend_identity("numpyro", numpyro.__version__, lock_digest, worker_digest),
        "jax_version": jax.__version__,
        "input_sha256": config["request"]["input_sha256"],
        "request_sha256": request_sha256,
        "source_request_sha256": config["source_request_sha256"],
        "fit_state": "complete" if passes else "nonconverged",
        "replicates": completed,
        "failures": failures,
        "diagnostics": diagnostics,
        "physical_latent_pattern_id": node["pattern_id"],
        "physical_latent_node_id": node["node_id"],
    }
