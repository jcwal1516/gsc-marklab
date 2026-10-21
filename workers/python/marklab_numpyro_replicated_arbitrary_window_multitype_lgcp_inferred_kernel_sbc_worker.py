#!/usr/bin/env python3
"""Scenario-stratified SBC of the NumPyro inferred-kernel replicated multitype LGCP."""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path
from typing import Any

import jax

jax.config.update("jax_enable_x64", True)

import numpy as np
import numpyro

sys.path.append(str(Path(__file__).resolve().parent))

from marklab_numpyro_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_worker import (
    MONITORED,
    PYMC_FIXED_WORKER,
    PYMC_INFERRED_WORKER,
    make_model,
)
from marklab_worker_support import multitype_lgcp as lgcp
from marklab_worker_support import versions
from marklab_worker_support.numpyro_nuts import nuts_diagnostics, sample_nuts
from marklab_worker_support.posterior import two_sided_tail
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
    backend_identity,
    derive_seed,
    exact,
    file_sha256,
    integer,
    obj,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    validate_backend,
    worker_digests,
    write_result,
)
from marklab_worker_support.sbc import rank_and_coverage, rank_histogram

REQUEST_FORMAT = (
    "marklab.numpyro_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc_request"
)
RESULT_FORMAT = (
    "marklab.numpyro_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc_result"
)
SCENARIOS = ("positive", "null", "weak_identification", "boundary", "misspecified")
PARAMETERS = (
    "intercept_type_a",
    "group_effect_difference",
    "patient_sd_type_a",
    "pattern_sd_type_a",
    "field_amplitude",
    "field_length_scale_um",
    "latent_node_type_a",
)
RESOURCE_KEYS = frozenset(
    {
        "scenario_replicates",
        "simulated_node_type_work",
        "maximum_simulated_node_type_work",
        "total_iterations",
        "maximum_total_iterations",
        "ppc_pair_work",
        "maximum_ppc_pair_work",
        "estimated_working_bytes",
        "maximum_working_bytes",
        "maximum_output_bytes",
        "timeout_seconds",
    }
)


def validate(
    request: Any, lock_sha: str, worker_sha: str, pymc_inferred_sha: str, pymc_fixed_sha: str
) -> dict[str, Any]:
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
    source_sha = lgcp.source_request_digest(request["source_request_sha256"])
    _, config = lgcp.validate_inferred_kernel_request(
        request["source_request"], lock_sha, pymc_inferred_sha, pymc_fixed_sha
    )
    if len(config["types"]) < 2:
        raise ContractError("multitype SBC requires at least two types")
    calibration = obj(
        request["calibration"],
        {
            "scenarios",
            "replicates_per_scenario",
            "rank_bins",
            "interval_probability",
            "latent_node_index",
        },
        "calibration",
    )
    exact(calibration["scenarios"], list(SCENARIOS), "calibration.scenarios")
    config["replicates_per_scenario"] = integer(
        calibration["replicates_per_scenario"], "calibration.replicates", 1, 20
    )
    config["rank_bins"] = integer(calibration["rank_bins"], "calibration.rank_bins", 2, 20)
    exact(float(calibration["interval_probability"]), 0.9, "calibration.interval")
    nodes = len(config["weights"])
    config["latent_node_index"] = integer(
        calibration["latent_node_index"], "calibration.latent_node_index", 0, nodes - 1
    )
    config["maximum_output_bytes"] = validate_resources(request["resources"], config)
    config["source_request_sha256"] = source_sha
    return config


def validate_resources(value: Any, config: dict[str, Any]) -> int:
    """Check that the declared work matches the request and fits its ceilings; return the
    output limit."""
    resources = obj(value, RESOURCE_KEYS, "resources")
    replicates = len(SCENARIOS) * config["replicates_per_scenario"]
    exact(resources["scenario_replicates"], replicates, "resources.replicates")
    rows = len(config["observed"])
    exact(resources["simulated_node_type_work"], replicates * rows, "resources.simulation_work")
    maximum_simulation = integer(
        resources["maximum_simulated_node_type_work"], "resources.maximum_simulation_work", 1, 10**9
    )
    if resources["simulated_node_type_work"] > maximum_simulation:
        raise ContractError("simulation-work ceiling exceeded")
    iterations = replicates * config["chains"] * (config["tune"] + config["draws"])
    exact(resources["total_iterations"], iterations, "resources.iterations")
    maximum_iterations = integer(
        resources["maximum_total_iterations"], "resources.maximum_iterations", 1, 10**9
    )
    if iterations > maximum_iterations:
        raise ContractError("iteration ceiling exceeded")
    nodes = len(config["weights"])
    ppc_work = replicates * nodes * nodes * config["chains"] * config["draws"]
    exact(resources["ppc_pair_work"], ppc_work, "resources.ppc_work")
    maximum_ppc = integer(
        resources["maximum_ppc_pair_work"], "resources.maximum_ppc_work", 1, 10**12
    )
    if ppc_work > maximum_ppc:
        raise ContractError("PPC-work ceiling exceeded")
    estimated = replicates * rows * 256 + 384 * 1024**2
    exact(resources["estimated_working_bytes"], estimated, "resources.estimated_bytes")
    maximum_bytes = integer(
        resources["maximum_working_bytes"], "resources.maximum_bytes", 1, 8 * 1024**3
    )
    if estimated > maximum_bytes:
        raise ContractError("memory ceiling exceeded")
    return integer(resources["maximum_output_bytes"], "resources.maximum_output", 1, 16 * MIB)


def seed_for(seed: int, purpose: str, scenario: str, replicate: int) -> int:
    return derive_seed("marklab-multitype-lgcp-sbc-v1", seed, purpose, scenario, replicate)


def spatial_latent(
    config: dict[str, Any], amplitude: float, length: float, rng: np.random.Generator
) -> np.ndarray:
    """Draw the ``(type, node)`` Matern-3/2 latent field, centred within each pattern."""
    latent = np.empty((len(config["types"]), len(config["weights"])), dtype=np.float64)
    for pattern in range(len(config["pattern_ids"])):
        selected = np.flatnonzero(config["node_patterns"] == pattern)
        rows = [config["request"]["nodes"][index] for index in selected]
        coordinates = np.asarray([[row["x_um"], row["y_um"]] for row in rows])
        distance = np.linalg.norm(coordinates[:, None, :] - coordinates[None, :, :], axis=2)
        scaled = math.sqrt(3.0) * distance / length
        covariance = amplitude**2 * (1.0 + scaled) * np.exp(-scaled)
        covariance += config["kernel_jitter"] * np.eye(len(selected))
        white = rng.normal(size=(len(config["types"]), len(selected)))
        blocks = white @ np.linalg.cholesky(covariance).T
        latent[:, selected] = blocks - blocks.mean(axis=1, keepdims=True)
    return latent


def draw_truth(
    config: dict[str, Any], scenario: str, replicate: int
) -> tuple[dict[str, Any], np.ndarray]:
    """Draw parameters from the prior, adjusted for the scenario, and simulate counts.

    ``positive`` redraws the group effects until type A's exceeds type B's by a tenth of the
    prior SD; ``null`` zeroes them; ``weak_identification`` shrinks the effects, SDs, and field
    amplitude tenfold; ``boundary`` puts the kernel near zero amplitude and length; and
    ``misspecified`` multiplies the intensities by Gamma(2, 0.5) noise the model omits.
    """
    rng = np.random.default_rng(seed_for(config["seed"], "simulate", scenario, replicate))
    priors = config["priors"]
    types = len(config["types"])
    intercept = rng.normal(priors["intercept_mean"], priors["intercept_sd"], types)
    group = rng.normal(0.0, priors["group_effect_sd"], types)
    covariate = rng.normal(0.0, priors["covariate_effect_sd"], types)
    patient_sd = np.abs(rng.normal(0.0, priors["patient_sd_scale"], types))
    pattern_sd = np.abs(rng.normal(0.0, priors["pattern_sd_scale"], types))
    amplitude = float(abs(rng.normal(0.0, config["amplitude_scale"])))
    length = float(abs(rng.normal(0.0, config["length_scale"])))
    if scenario == "positive":
        while group[0] - group[1] <= 0.1 * priors["group_effect_sd"]:
            group = rng.normal(0.0, priors["group_effect_sd"], types)
    elif scenario == "null":
        group[:] = 0.0
    elif scenario == "weak_identification":
        group *= 0.1
        covariate *= 0.1
        patient_sd *= 0.1
        pattern_sd *= 0.1
        amplitude *= 0.1
    elif scenario == "boundary":
        amplitude = max(config["amplitude_scale"] * 0.01, 1e-6)
        length = max(config["length_scale"] * 0.01, 1e-6)
    amplitude = max(amplitude, 1e-8)
    length = max(length, 1e-8)
    patient_effect = patient_sd[None, :] * rng.normal(size=(len(config["patient_ids"]), types))
    pattern_raw = rng.normal(size=(len(config["pattern_ids"]), types))
    for patient in range(len(config["patient_ids"])):
        selected = config["pattern_patients"] == patient
        pattern_raw[selected] -= pattern_raw[selected].mean(axis=0, keepdims=True)
    pattern_effect = pattern_sd[None, :] * pattern_raw
    latent = spatial_latent(config, amplitude, length, rng)
    log_expected = (
        intercept[:, None]
        + group[:, None] * config["patient_groups"][config["node_patients"]][None, :]
        + covariate[:, None] * config["covariate"][None, :]
        + patient_effect[config["node_patients"], :].T
        + pattern_effect[config["node_patterns"], :].T
        + latent
        + np.log(config["weights"])[None, :]
    )
    expected_matrix = np.exp(log_expected)
    expected = expected_matrix[config["type_index"], config["node_index"]]
    if scenario == "misspecified":
        expected *= rng.gamma(shape=2.0, scale=0.5, size=len(expected))
    counts = rng.poisson(expected).astype(np.int64)
    maximum_events = int(config["request"]["resources"]["maximum_total_events"])
    if int(counts.sum()) > maximum_events:
        raise ContractError("simulated event ceiling exceeded")
    truth = {
        "intercept_type_a": float(intercept[0]),
        "intercept_type_b": float(intercept[1]),
        "group_effect_difference": float(group[0] - group[1]),
        "patient_sd_type_a": float(patient_sd[0]),
        "pattern_sd_type_a": float(pattern_sd[0]),
        "field_amplitude": amplitude,
        "field_length_scale_um": length,
        "latent_node_type_a": float(latent[0, config["latent_node_index"]]),
    }
    return truth, counts


def metrics(config: dict[str, Any], counts: np.ndarray) -> dict[str, float]:
    """Total count, type-A share, A/B covariance across nodes, and variance of node totals."""
    nodes = len(config["weights"])
    types = len(config["types"])
    matrix = np.zeros((nodes, types), dtype=np.float64)
    matrix[config["node_index"], config["type_index"]] = counts
    totals = matrix.sum(axis=1)
    grand = float(totals.sum())
    type_a = matrix[:, 0]
    type_b = matrix[:, 1]
    return {
        "counts": grand,
        "mark_proportions": float(type_a.sum() / grand) if grand > 0 else 0.0,
        "cross_type_enrichment": float(
            np.mean((type_a - type_a.mean()) * (type_b - type_b.mean()))
        ),
        "clustering": float(np.var(totals)),
    }


def ppc(
    config: dict[str, Any], observed: np.ndarray, expected: np.ndarray, rng: np.random.Generator
) -> dict[str, Any]:
    """Posterior predictive checks of ``metrics`` against one Poisson replicate per draw."""
    replicated = rng.poisson(expected).astype(np.int64)
    observed_metrics = metrics(config, observed)
    replicated_metrics = [metrics(config, row) for row in replicated]
    checks = {}
    for name, value in observed_metrics.items():
        replicated_values = np.asarray([row[name] for row in replicated_metrics])
        checks[name] = {
            "observed": value,
            "replicated_mean": float(np.mean([row[name] for row in replicated_metrics])),
            "two_sided_tail_probability": two_sided_tail(replicated_values, value),
        }
    return checks


def fit_one(
    config: dict[str, Any], scenario: str, replicate: int, truth: dict[str, Any], counts: np.ndarray
) -> dict[str, Any]:
    """Refit the simulated counts and rank the true values among the posterior draws."""
    fit_config = {**config, "observed": counts}
    depth = int(config["maximum_tree_depth"])
    samples, extra = sample_nuts(
        make_model(fit_config),
        config,
        depth,
        seed_for(config["seed"], "fit", scenario, replicate),
    )
    flat = {name: value.reshape((-1, *value.shape[2:])) for name, value in samples.items()}
    latent = samples["latent_effect"].reshape((-1, len(config["types"]), len(config["weights"])))
    draws = {
        "intercept_type_a": flat["intercept"][:, 0],
        "group_effect_difference": flat["group_effect"][:, 0] - flat["group_effect"][:, 1],
        "patient_sd_type_a": flat["patient_sd"][:, 0],
        "pattern_sd_type_a": flat["pattern_sd"][:, 0],
        "field_amplitude": flat["field_amplitude"],
        "field_length_scale_um": flat["field_length_scale_um"],
        "latent_node_type_a": latent[:, 0, config["latent_node_index"]],
    }
    ranks: dict[str, int] = {}
    coverage: dict[str, bool] = {}
    for name in PARAMETERS:
        ranks[name], coverage[name] = rank_and_coverage(draws[name], truth[name])
    expected = samples["expected_count"].reshape((-1, len(counts)))
    posterior_predictive = ppc(
        config,
        counts,
        expected,
        np.random.default_rng(seed_for(config["seed"], "ppc", scenario, replicate)),
    )
    return {
        "scenario": scenario,
        "replicate": replicate,
        "status": "complete",
        "failure_reason": None,
        "simulated_total_events": int(counts.sum()),
        "truth": truth,
        "ranks": ranks,
        "coverage_90": coverage,
        "posterior_predictive": posterior_predictive,
        **nuts_diagnostics(samples, MONITORED, extra, depth),
    }


def failed(
    scenario: str, replicate: int, truth: dict[str, Any], count: int, error: Exception
) -> dict[str, Any]:
    return {
        "scenario": scenario,
        "replicate": replicate,
        "status": "failed",
        "failure_reason": f"{type(error).__name__}:{str(error)[:400]}",
        "simulated_total_events": count,
        "truth": truth,
        "ranks": None,
        "coverage_90": None,
        "posterior_predictive": None,
        "r_hat": None,
        "ess_bulk": None,
        "ess_tail": None,
        "minimum_ebfmi": None,
        "divergences": None,
        "max_tree_depth_hits": None,
    }


def aggregate(rows: list[dict[str, Any]], parameter: str, draws: int, bins: int) -> dict[str, Any]:
    """Rank histogram, coverage, and mean normalized rank over the completed replicates."""
    complete = [row for row in rows if row["status"] == "complete"]
    ranks = np.asarray([row["ranks"][parameter] for row in complete], dtype=np.int64)
    coverage = [row["coverage_90"][parameter] for row in complete]
    return {
        "completed_replicates": len(complete),
        "rank_histogram": rank_histogram(ranks, draws, bins).tolist(),
        "coverage_90": float(np.mean(coverage)) if complete else 0.0,
        "mean_normalized_rank": float(ranks.mean() / draws) if complete else 0.0,
    }


def main() -> None:
    require_versions(numpyro=numpyro.__version__, jax=jax.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    worker_directory = Path(__file__).parent
    pymc_inferred_sha = file_sha256(worker_directory / PYMC_INFERRED_WORKER)
    pymc_fixed_sha = file_sha256(worker_directory / PYMC_FIXED_WORKER)
    raw = read_request(64 * MIB)
    config = validate(json.loads(raw), lock_sha, worker_sha, pymc_inferred_sha, pymc_fixed_sha)
    rows: list[dict[str, Any]] = []
    for scenario in SCENARIOS:
        for replicate in range(config["replicates_per_scenario"]):
            truth = {name: 0.0 for name in PARAMETERS}
            count = 0
            try:
                truth, counts = draw_truth(config, scenario, replicate)
                count = int(counts.sum())
                rows.append(fit_one(config, scenario, replicate, truth, counts))
            except Exception as error:
                rows.append(failed(scenario, replicate, truth, count, error))
    draws = config["chains"] * config["draws"]
    result = {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": backend_identity("numpyro", numpyro.__version__, lock_sha, worker_sha),
        "jax_version": jax.__version__,
        "input_sha256": config["input_sha"],
        "request_sha256": sha256_hex(raw),
        "source_request_sha256": config["source_request_sha256"],
        "fit_state": (
            "complete" if all(row["status"] == "complete" for row in rows) else "nonconverged"
        ),
        "scenario_dispositions": rows,
        "rank_diagnostics": {
            parameter: aggregate(rows, parameter, draws, config["rank_bins"])
            for parameter in PARAMETERS
        },
        "pattern_representation": "complete_quadrature_resolved_counting_measure_on_exact_window",
        "cross_type_dependence_estimand": (
            "not_estimated_current_model_uses_conditionally_independent_type_fields"
        ),
    }
    write_result(result, max_bytes=config["maximum_output_bytes"])


if __name__ == "__main__":
    run_worker(main, "marklab multitype LGCP SBC failed")
