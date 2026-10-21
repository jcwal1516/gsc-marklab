#!/usr/bin/env python3
"""Exact finite-state SBC for patient/pattern hierarchical conditional marks."""

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

import marklab_numpyro_replicated_conditional_multitype_mark_worker as fit_worker
from marklab_worker_support import versions
from marklab_worker_support.conditional_mark import (
    HIERARCHY_PARAMETERS,
    affinity_contrast,
    distinct_pairs,
    policy_met,
    potential_design,
    sbc_parameter_summaries,
    validate_replicated_request,
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
    sha256_digest,
    sha256_hex,
    validate_backend,
    worker_digests,
    write_result,
)
from marklab_worker_support.sbc import calibration_passes, rank_and_coverage

REQUEST_FORMAT = "marklab.numpyro_replicated_conditional_multitype_mark_sbc_request"
RESULT_FORMAT = "marklab.numpyro_replicated_conditional_multitype_mark_sbc_result"
PYMC_WORKER = "marklab_pymc_replicated_conditional_multitype_mark_worker.py"
SCALE_PARAMETERS = [
    "patient_intercept_sd",
    "patient_potential_sd",
    "pattern_intercept_sd",
    "pattern_potential_sd",
]


def validate(request: Any, lock_sha: str, worker_sha: str, pymc_worker_sha: str) -> dict[str, Any]:
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
    source_sha = sha256_digest(request["source_request_sha256"], "source_request_sha256")
    config = validate_replicated_request(request["source_request"], lock_sha, pymc_worker_sha)
    source_patterns = request["source_request"]["patterns"]
    point_counts = [row["point_count"] for row in source_patterns]
    points_per_pattern = point_counts[0]
    if (
        not 6 <= points_per_pattern <= 8
        or any(value != points_per_pattern for value in point_counts)
        or len(config["types"]) > 4
    ):
        raise ContractError("exact SBC pattern dimensions are invalid")

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
            "maximum_states_per_pattern",
            "states_per_pattern",
            "points_per_pattern",
            "pattern_count",
            "maximum_enumeration_work",
            "enumeration_work",
            "maximum_enumeration_bytes",
            "enumeration_bytes",
            "maximum_total_iterations",
            "total_iterations",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    states_per_pattern = len(config["types"]) ** points_per_pattern
    exact(resources["states_per_pattern"], states_per_pattern, "resources.states_per_pattern")
    exact(resources["points_per_pattern"], points_per_pattern, "resources.points_per_pattern")
    exact(resources["pattern_count"], len(config["pattern_ids"]), "resources.pattern_count")
    maximum_states = integer(
        resources["maximum_states_per_pattern"], "resources.maximum_states", 1, 1_000_000
    )
    if states_per_pattern > maximum_states:
        raise ContractError("state ceiling exceeded")
    edge_counts = [row["edge_count"] for row in source_patterns]
    sites_and_edges = sum(
        points + edges for points, edges in zip(point_counts, edge_counts, strict=True)
    )
    enumeration_work = config["replicates"] * states_per_pattern * sites_and_edges
    exact(resources["enumeration_work"], enumeration_work, "resources.enumeration_work")
    maximum_work = integer(
        resources["maximum_enumeration_work"],
        "resources.maximum_enumeration_work",
        1,
        1_000_000_000,
    )
    if enumeration_work > maximum_work:
        raise ContractError("enumeration-work ceiling exceeded")
    enumeration_bytes = states_per_pattern * points_per_pattern * 2 + states_per_pattern * 16
    exact(resources["enumeration_bytes"], enumeration_bytes, "resources.enumeration_bytes")
    maximum_bytes = integer(
        resources["maximum_enumeration_bytes"], "resources.maximum_enumeration_bytes", 1, 1024**3
    )
    if enumeration_bytes > maximum_bytes:
        raise ContractError("enumeration-byte ceiling exceeded")
    total_iterations = config["replicates"] * config["chains"] * (config["tune"] + config["draws"])
    exact(resources["total_iterations"], total_iterations, "resources.total_iterations")
    maximum_iterations = integer(
        resources["maximum_total_iterations"], "resources.maximum_total_iterations", 1, 10_000_000
    )
    if total_iterations > maximum_iterations:
        raise ContractError("iteration ceiling exceeded")
    config["maximum_output_bytes"] = integer(
        resources["maximum_output_bytes"], "resources.maximum_output_bytes", 1, 8 * 1_048_576
    )
    integer(resources["timeout_seconds"], "resources.timeout_seconds", 1, 86_400)
    config["source_request_sha256"] = source_sha
    config["states_per_pattern"] = states_per_pattern
    config["points_per_pattern"] = points_per_pattern
    config["numpyro_maximum_tree_depth"] = config["maximum_tree_depth"]
    return config


def seed_for(seed: int, purpose: str, replicate: int) -> int:
    return derive_seed("marklab-replicated-conditional-multitype-sbc-v1", seed, purpose, replicate)


def state_table(config: dict[str, Any]) -> np.ndarray:
    """Every joint labelling of one pattern's sites, one row per state."""
    labellings = itertools.product(range(len(config["types"])), repeat=config["points_per_pattern"])
    return np.asarray(list(labellings), dtype=np.int16)


def parameter_names(type_ids: list[str]) -> list[str]:
    names = [
        f"{prefix}:{type_ids[left]}|{type_ids[right]}"
        for prefix in ["baseline_affinity", "group_affinity_shift"]
        for left, right in distinct_pairs(len(type_ids))
    ]
    return names + SCALE_PARAMETERS


def simulate(
    config: dict[str, Any], states: np.ndarray, replicate: int
) -> tuple[dict[str, float], np.ndarray, np.ndarray, list[list[int]]]:
    """Draw hierarchy parameters from the prior and exact labellings for every pattern.

    Each pattern's labelling has probability proportional to
    ``exp(sum_i intercept_p[y_i] + sum_edges potential_p[y_left, y_right])`` over all states.
    """
    rng = np.random.default_rng(seed_for(config["seed"], "simulate", replicate))
    types = len(config["types"])
    patients = len(config["patient_ids"])
    patterns = len(config["pattern_ids"])
    free_intercepts = types - 1
    free_potentials = types * (types + 1) // 2 - 1
    design = potential_design(types)

    baseline_intercept = rng.normal(0.0, config["intercept_sd"], free_intercepts)
    group_intercept = rng.normal(0.0, config["group_effect_sd"], free_intercepts)
    baseline_potential_free = rng.normal(0.0, config["interaction_sd"], free_potentials)
    group_potential_free = rng.normal(0.0, config["group_effect_sd"], free_potentials)
    patient_intercept_sd = abs(float(rng.normal(0.0, config["patient_sd_scale"])))
    patient_potential_sd = abs(float(rng.normal(0.0, config["patient_sd_scale"])))
    pattern_intercept_sd = abs(float(rng.normal(0.0, config["pattern_sd_scale"])))
    pattern_potential_sd = abs(float(rng.normal(0.0, config["pattern_sd_scale"])))
    patient_intercept_z = rng.normal(size=(patients, free_intercepts))
    patient_potential_z = rng.normal(size=(patients, free_potentials))
    pattern_intercept_z = rng.normal(size=(patterns, free_intercepts))
    pattern_potential_z = rng.normal(size=(patterns, free_potentials))
    pattern_groups = config["pattern_groups"][:, None]
    pattern_patients = config["pattern_patients"]
    pattern_intercept_free = (
        baseline_intercept[None, :]
        + pattern_groups * group_intercept[None, :]
        + patient_intercept_sd * patient_intercept_z[pattern_patients]
        + pattern_intercept_sd * pattern_intercept_z
    )
    pattern_potential_free = (
        baseline_potential_free[None, :]
        + pattern_groups * group_potential_free[None, :]
        + patient_potential_sd * patient_potential_z[pattern_patients]
        + pattern_potential_sd * pattern_potential_z
    )
    pattern_intercept = np.zeros((patterns, types), dtype=np.float64)
    pattern_intercept[:, 1:] = pattern_intercept_free
    pattern_potential = np.einsum("pf,fkl->pkl", pattern_potential_free, design)

    observed = np.empty(len(config["observed"]), dtype=np.int64)
    pattern_counts: list[list[int]] = []
    for pattern in range(patterns):
        indices = np.flatnonzero(config["point_patterns"] == pattern)
        local = {int(index): position for position, index in enumerate(indices)}
        endpoints = config["edges"][config["edge_patterns"] == pattern]
        local_edges = [(local[int(left)], local[int(right)]) for left, right in endpoints]
        energy = pattern_intercept[pattern, states].sum(axis=1)
        for left, right in local_edges:
            energy += pattern_potential[pattern, states[:, left], states[:, right]]
        weights = np.exp(energy - energy.max())
        labels = states[rng.choice(len(states), p=weights / weights.sum())].astype(np.int64)
        observed[indices] = labels
        pattern_counts.append(np.bincount(labels, minlength=types).astype(int).tolist())

    counts = np.zeros((len(observed), types), dtype=np.float64)
    for left, right in config["edges"]:
        counts[left, observed[right]] += 1.0
        counts[right, observed[left]] += 1.0
    potentials = {
        "baseline_affinity": np.einsum("f,fkl->kl", baseline_potential_free, design),
        "group_affinity_shift": np.einsum("f,fkl->kl", group_potential_free, design),
    }
    truth: dict[str, float] = {}
    for prefix, potential in potentials.items():
        for left, right in distinct_pairs(types):
            key = f"{prefix}:{config['types'][left]}|{config['types'][right]}"
            truth[key] = float(affinity_contrast(potential, left, right))
    truth.update(
        {
            "patient_intercept_sd": patient_intercept_sd,
            "patient_potential_sd": patient_potential_sd,
            "pattern_intercept_sd": pattern_intercept_sd,
            "pattern_potential_sd": pattern_potential_sd,
        }
    )
    return truth, observed, counts, pattern_counts


def posterior_parameter_draws(
    samples: dict[str, np.ndarray], config: dict[str, Any]
) -> dict[str, np.ndarray]:
    types = len(config["types"])
    design = potential_design(types)
    flat_baseline = samples["baseline_potential"].reshape((-1, design.shape[0]))
    flat_group = samples["group_potential"].reshape((-1, design.shape[0]))
    potentials = {
        "baseline_affinity": np.einsum("df,fkl->dkl", flat_baseline, design),
        "group_affinity_shift": np.einsum("df,fkl->dkl", flat_group, design),
    }
    result: dict[str, np.ndarray] = {}
    for prefix, potential in potentials.items():
        for left, right in distinct_pairs(types):
            key = f"{prefix}:{config['types'][left]}|{config['types'][right]}"
            result[key] = affinity_contrast(potential, left, right)
    for name in SCALE_PARAMETERS:
        result[name] = samples[name].reshape(-1)
    return result


def failure_reason(diagnostics: dict[str, Any]) -> str:
    return (
        f"diagnostics_failed:r_hat={diagnostics['r_hat']:.17g},"
        f"bulk={diagnostics['ess_bulk']:.17g},tail={diagnostics['ess_tail']:.17g},"
        f"ebfmi={diagnostics['minimum_ebfmi']:.17g},divergences={diagnostics['divergences']},"
        f"depth_hits={diagnostics['max_tree_depth_hits']}"
    )


def run_sbc(
    config: dict[str, Any], states: np.ndarray
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    completed: list[dict[str, Any]] = []
    failures: list[dict[str, Any]] = []
    names = parameter_names(config["types"])
    depth = config["numpyro_maximum_tree_depth"]
    for replicate in range(config["replicates"]):
        try:
            truth, observed, counts, pattern_counts = simulate(config, states, replicate)
            fit_config = dict(config)
            fit_config["observed"] = observed
            fit_config["neighbor_counts"] = counts
            samples, extra = sample_nuts(
                fit_worker.make_model(fit_config),
                config,
                depth,
                seed_for(config["seed"], "fit", replicate),
            )
            diagnostics = nuts_diagnostics(samples, HIERARCHY_PARAMETERS, extra, depth)
            convergence = [diagnostics[key] for key in ("r_hat", "ess_bulk", "ess_tail")]
            finite = np.isfinite([*convergence, diagnostics["minimum_ebfmi"]]).all()
            if not (finite and policy_met(config, diagnostics)):
                failures.append({"replicate": replicate, "reason": failure_reason(diagnostics)})
                continue
            draws = posterior_parameter_draws(samples, config)
            parameters = []
            for name in names:
                rank, covered = rank_and_coverage(draws[name], truth[name])
                parameters.append(
                    {"parameter": name, "truth": truth[name], "rank": rank, "covered": covered}
                )
            completed.append(
                {
                    "replicate": replicate,
                    "simulated_pattern_type_counts": pattern_counts,
                    "parameters": parameters,
                    **diagnostics,
                }
            )
        except Exception as error:
            reason = f"{type(error).__name__}:{str(error)[:400]}"
            failures.append({"replicate": replicate, "reason": reason})
    return completed, failures


def main() -> int:
    require_versions(numpyro=numpyro.__version__, jax=jax.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    pymc_worker_sha = file_sha256(Path(__file__).with_name(PYMC_WORKER))
    raw = read_request(64 * MIB)
    config = validate(json.loads(raw), lock_sha, worker_sha, pymc_worker_sha)
    states = state_table(config)
    completed, failures = run_sbc(config, states)
    diagnostics = sbc_parameter_summaries(
        completed,
        parameter_names(config["types"]),
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
    log_types = math.log(len(config["types"]))
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
            "states_per_pattern": len(states),
            "zero_parameter_log_normalizer_per_pattern": float(
                config["points_per_pattern"] * log_types
            ),
        },
        "replicates": completed,
        "failures": failures,
        "diagnostics": diagnostics,
    }
    write_result(result, max_bytes=config["maximum_output_bytes"])
    return 0


if __name__ == "__main__":
    run_worker(main, "marklab replicated conditional multitype SBC failed")
