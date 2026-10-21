#!/usr/bin/env python3
"""Exact finite-state SBC for the conditional hard-multitype pseudoposterior."""

from __future__ import annotations

import itertools
import json
import math
import sys
from pathlib import Path
from typing import Any

import jax

jax.config.update("jax_enable_x64", True)
sys.path.append(str(Path(__file__).resolve().parent))

import numpy as np
import numpyro

import marklab_numpyro_conditional_multitype_mark_worker as fit_worker
from marklab_worker_support import versions
from marklab_worker_support.conditional_mark import (
    affinity_contrast,
    distinct_pairs,
    free_matrices,
    free_pairs,
    policy_met,
    sbc_parameter_summaries,
    validate_single_request,
)
from marklab_worker_support.numpyro_nuts import nuts_diagnostics, sample_nuts
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
    backend_identity,
    derive_seed,
    exact,
    file_sha256,
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
from marklab_worker_support.sbc import calibration_passes, rank_and_coverage

REQUEST_FORMAT = "marklab.numpyro_conditional_multitype_mark_sbc_request"
RESULT_FORMAT = "marklab.numpyro_conditional_multitype_mark_sbc_result"
PYMC_WORKER = "marklab_pymc_conditional_multitype_mark_worker.py"


def validate(request: Any, lock_sha: str, worker_sha: str) -> dict[str, Any]:
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
    exact(request["format"], REQUEST_FORMAT, "request.format")
    exact(request["version"], 1, "request.version")
    validate_backend(
        request["backend"],
        "backend",
        name="numpyro",
        version=versions.NUMPYRO,
        lock_sha256=lock_sha,
        worker_sha256=worker_sha,
    )
    exact(request["jax_version"], versions.JAX, "request.jax_version")
    source_sha = request["source_request_sha256"]
    if not isinstance(source_sha, str) or len(source_sha) != 64:
        raise ContractError("source request digest is invalid")
    pymc_worker_sha = file_sha256(Path(__file__).with_name(PYMC_WORKER))
    config = validate_single_request(request["source_request"], lock_sha, pymc_worker_sha)
    if len(config["observed"]) > 10 or len(config["type_ids"]) > 4:
        raise ContractError("exact SBC requires at most ten sites and four types")

    calibration = obj(
        request["calibration"],
        {
            "replicates",
            "rank_bins",
            "interval_probability",
            "minimum_rank_uniformity_p_value",
            "minimum_coverage",
            "maximum_coverage",
        },
        "calibration",
    )
    config["replicates"] = integer(calibration["replicates"], "calibration.replicates", 20, 100)
    config["rank_bins"] = integer(calibration["rank_bins"], "calibration.rank_bins", 2, 20)
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
    if (
        not 0 <= config["minimum_rank_p"] <= 1
        or not 0 <= config["minimum_coverage"] <= config["maximum_coverage"] <= 1
    ):
        raise ContractError("calibration thresholds are invalid")

    resources = obj(
        request["resources"],
        {
            "maximum_states",
            "state_count",
            "maximum_enumeration_work",
            "enumeration_work",
            "maximum_enumeration_bytes",
            "enumeration_bytes",
            "maximum_total_iterations",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    sites = len(config["observed"])
    state_count = len(config["type_ids"]) ** sites
    exact(resources["state_count"], state_count, "resources.state_count")
    if state_count > integer(resources["maximum_states"], "resources.maximum_states", 1, 1_000_000):
        raise ContractError("state ceiling exceeded")
    enumeration_work = state_count * (sites + len(config["edges"]))
    exact(resources["enumeration_work"], enumeration_work, "resources.enumeration_work")
    maximum_work = integer(
        resources["maximum_enumeration_work"], "resources.maximum_enumeration_work", 1, 100_000_000
    )
    if enumeration_work > maximum_work:
        raise ContractError("enumeration-work ceiling exceeded")
    enumeration_bytes = state_count * sites * 2 + state_count * 16
    exact(resources["enumeration_bytes"], enumeration_bytes, "resources.enumeration_bytes")
    maximum_bytes = integer(
        resources["maximum_enumeration_bytes"], "resources.maximum_enumeration_bytes", 1, 1024**3
    )
    if enumeration_bytes > maximum_bytes:
        raise ContractError("enumeration-byte ceiling exceeded")
    iterations = config["replicates"] * config["chains"] * (config["tune"] + config["draws"])
    maximum_iterations = integer(
        resources["maximum_total_iterations"], "resources.maximum_total_iterations", 1, 10_000_000
    )
    if iterations > maximum_iterations:
        raise ContractError("total-iteration ceiling exceeded")
    config["maximum_output_bytes"] = integer(
        resources["maximum_output_bytes"], "resources.maximum_output_bytes", 1, 2 * 1_048_576
    )
    config["source_request_sha256"] = source_sha
    config["state_count"] = state_count
    config["enumeration_work"] = enumeration_work
    config["enumeration_bytes"] = enumeration_bytes
    return config


def seed_for(seed: int, purpose: str, replicate: int) -> int:
    return derive_seed("marklab-conditional-multitype-sbc-v1", seed, purpose, replicate)


def parameter_names(type_ids: list[str]) -> list[str]:
    types = len(type_ids)
    names = [f"intercept:{identity}" for identity in type_ids[1:]]
    names += [f"potential:{type_ids[left]}|{type_ids[right]}" for left, right in free_pairs(types)]
    names += [
        f"affinity:{type_ids[left]}|{type_ids[right]}" for left, right in distinct_pairs(types)
    ]
    return names


def state_table(config: dict[str, Any]) -> np.ndarray:
    """Every joint labelling of the sites, one row per state."""
    labellings = itertools.product(range(len(config["type_ids"])), repeat=len(config["observed"]))
    return np.asarray(list(labellings), dtype=np.int16)


def simulate(
    config: dict[str, Any], states: np.ndarray, replicate: int
) -> tuple[dict[str, float], np.ndarray, np.ndarray]:
    """Draw parameters from the prior and one exact labelling from the joint Gibbs field.

    The labelling has probability proportional to
    ``exp(sum_i intercept[y_i] + sum_edges potential[y_left, y_right])`` over all states.
    """
    rng = np.random.default_rng(seed_for(config["seed"], "simulate", replicate))
    type_ids = config["type_ids"]
    types = len(type_ids)
    intercept_free = rng.normal(0.0, config["intercept_sd"], types - 1)
    potential_free = rng.normal(0.0, config["interaction_sd"], len(free_pairs(types)))
    intercept, potential = free_matrices(intercept_free, potential_free, types)
    intercept = intercept.reshape(types)
    potential = potential.reshape(types, types)
    energy = intercept[states].sum(axis=1)
    for left, right in config["edges"]:
        energy += potential[states[:, left], states[:, right]]
    weights = np.exp(energy - energy.max())
    probability = weights / weights.sum()
    observed = states[rng.choice(len(states), p=probability)].astype(np.int64)
    counts = np.zeros((len(observed), types), dtype=np.float64)
    for left, right in config["edges"]:
        counts[left, observed[right]] += 1
        counts[right, observed[left]] += 1
    truth: dict[str, float] = {}
    for index, identity in enumerate(type_ids[1:]):
        truth[f"intercept:{identity}"] = float(intercept_free[index])
    for index, (left, right) in enumerate(free_pairs(types)):
        truth[f"potential:{type_ids[left]}|{type_ids[right]}"] = float(potential_free[index])
    for left, right in distinct_pairs(types):
        truth[f"affinity:{type_ids[left]}|{type_ids[right]}"] = float(
            affinity_contrast(potential, left, right)
        )
    return truth, observed, counts


def posterior_parameter_draws(
    samples: dict[str, np.ndarray], type_ids: list[str]
) -> dict[str, np.ndarray]:
    types = len(type_ids)
    _, potential = free_matrices(samples["intercept_free"], samples["potential_free"], types)
    draws: dict[str, np.ndarray] = {}
    for index, identity in enumerate(type_ids[1:]):
        draws[f"intercept:{identity}"] = samples["intercept_free"][..., index].reshape(-1)
    for index, (left, right) in enumerate(free_pairs(types)):
        key = f"potential:{type_ids[left]}|{type_ids[right]}"
        draws[key] = samples["potential_free"][..., index].reshape(-1)
    for left, right in distinct_pairs(types):
        key = f"affinity:{type_ids[left]}|{type_ids[right]}"
        draws[key] = affinity_contrast(potential, left, right).reshape(-1)
    return draws


def run_sbc(
    config: dict[str, Any], states: np.ndarray
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    completed: list[dict[str, Any]] = []
    failures: list[dict[str, Any]] = []
    names = parameter_names(config["type_ids"])
    depth = config["request"]["resources"]["maximum_tree_depth"]
    for replicate in range(config["replicates"]):
        try:
            truth, observed, counts = simulate(config, states, replicate)
            fit_config = dict(config)
            fit_config["observed"] = observed
            fit_config["neighbor_counts"] = counts
            samples, extra = sample_nuts(
                fit_worker.make_model(fit_config),
                config,
                depth,
                seed_for(config["seed"], "fit", replicate),
            )
            draws_by_name = posterior_parameter_draws(samples, config["type_ids"])
            diagnostics = nuts_diagnostics(
                samples, ["intercept_free", "potential_free"], extra, depth
            )
            if not policy_met(config, diagnostics):
                failures.append({"replicate": replicate, "reason": failure_reason(diagnostics)})
                continue
            parameters = []
            for name in names:
                rank, covered = rank_and_coverage(draws_by_name[name], truth[name])
                parameters.append(
                    {"parameter": name, "truth": truth[name], "rank": rank, "covered": covered}
                )
            simulated_counts = np.bincount(observed, minlength=len(config["type_ids"]))
            completed.append(
                {
                    "replicate": replicate,
                    "simulated_type_counts": simulated_counts.tolist(),
                    "parameters": parameters,
                    **diagnostics,
                }
            )
        except Exception as error:
            reason = f"{type(error).__name__}:{str(error)[:400]}"
            failures.append({"replicate": replicate, "reason": reason})
    return completed, failures


def failure_reason(diagnostics: dict[str, Any]) -> str:
    return (
        f"diagnostics_failed:r_hat={diagnostics['r_hat']:.17g},"
        f"bulk={diagnostics['ess_bulk']:.17g},tail={diagnostics['ess_tail']:.17g},"
        f"ebfmi={diagnostics['minimum_ebfmi']:.17g},divergences={diagnostics['divergences']},"
        f"depth_hits={diagnostics['max_tree_depth_hits']}"
    )


def main() -> int:
    require_versions(numpyro=numpyro.__version__, jax=jax.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(16 * MIB)
    config = validate(json.loads(raw), lock_sha, worker_sha)
    states = state_table(config)
    completed, failures = run_sbc(config, states)
    diagnostics = sbc_parameter_summaries(
        completed,
        parameter_names(config["type_ids"]),
        config["chains"] * config["draws"],
        config["rank_bins"],
    )
    passes = calibration_passes(
        diagnostics,
        failures,
        config["minimum_rank_p"],
        config["minimum_coverage"],
        config["maximum_coverage"],
    )
    log_types = math.log(len(config["type_ids"]))
    result = {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": backend_identity("numpyro", numpyro.__version__, lock_sha, worker_sha),
        "jax_version": jax.__version__,
        "input_sha256": config["input_sha"],
        "request_sha256": sha256_hex(raw),
        "source_request_sha256": config["source_request_sha256"],
        "fit_state": "complete" if passes else "nonconverged",
        "enumeration_oracle": {
            "state_count": len(states),
            "zero_parameter_log_normalizer": float(len(config["observed"]) * log_types),
        },
        "replicates": completed,
        "failures": failures,
        "diagnostics": diagnostics,
    }
    write_result(result, max_bytes=config["maximum_output_bytes"])
    return 0


if __name__ == "__main__":
    run_worker(main, "marklab conditional multitype SBC failed")
