#!/usr/bin/env python3
"""Scenario-stratified SBC for the replicated exact-window multitype LGCP."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import math
from pathlib import Path
import sys
from typing import Any

import arviz as az
import jax

jax.config.update("jax_enable_x64", True)
import numpy as np
import numpyro
from numpyro.infer import MCMC, NUTS


REQUEST_FORMAT = "marklab.numpyro_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc_request"
RESULT_FORMAT = "marklab.numpyro_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc_result"
NUMPYRO_VERSION = "0.21.0"
JAX_VERSION = "0.11.1"
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


class ContractError(ValueError):
    pass


def load(filename: str, name: str) -> Any:
    path = Path(__file__).with_name(filename)
    specification = importlib.util.spec_from_file_location(name, path)
    if specification is None or specification.loader is None:
        raise ContractError(f"cannot load {filename}")
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


def obj(value: Any, keys: set[str], path: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        actual = set(value) if isinstance(value, dict) else set()
        raise ContractError(
            f"{path} fields differ: missing={sorted(keys - actual)}, unknown={sorted(actual - keys)}"
        )
    return value


def exact(value: Any, expected: Any, path: str) -> None:
    if value != expected:
        raise ContractError(f"{path} must equal {expected!r}")


def integer(value: Any, path: str, low: int, high: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not low <= value <= high:
        raise ContractError(f"{path} must be an integer in [{low}, {high}]")
    return value


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
    backend = obj(
        request["backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "backend",
    )
    exact(backend["name"], "numpyro", "backend.name")
    exact(backend["version"], NUMPYRO_VERSION, "backend.version")
    exact(backend["python_version"], "3.12", "backend.python_version")
    exact(backend["environment_lock_sha256"], lock_sha, "backend.lock")
    exact(backend["worker_sha256"], worker_sha, "backend.worker")
    exact(request["jax_version"], JAX_VERSION, "request.jax_version")
    source_digest = request["source_request_sha256"]
    if not isinstance(source_digest, str) or len(source_digest) != 64:
        raise ContractError("source request digest is invalid")

    pymc = load(
        "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_worker.py",
        "marklab_pymc_multitype_sbc_contract",
    )
    pymc_inferred_sha = hashlib.sha256(
        Path(__file__).with_name(
            "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_worker.py"
        ).read_bytes()
    ).hexdigest()
    pymc_fixed_sha = hashlib.sha256(
        Path(__file__).with_name(
            "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_worker.py"
        ).read_bytes()
    ).hexdigest()
    _, config = pymc.validate(
        request["source_request"], lock_sha, pymc_inferred_sha, pymc_fixed_sha
    )
    if len(config["types"]) < 2:
        raise ContractError("multitype SBC requires at least two types")
    calibration = obj(
        request["calibration"],
        {"scenarios", "replicates_per_scenario", "rank_bins", "interval_probability", "latent_node_index"},
        "calibration",
    )
    exact(calibration["scenarios"], list(SCENARIOS), "calibration.scenarios")
    config["replicates_per_scenario"] = integer(
        calibration["replicates_per_scenario"], "calibration.replicates", 1, 20
    )
    config["rank_bins"] = integer(calibration["rank_bins"], "calibration.rank_bins", 2, 20)
    exact(float(calibration["interval_probability"]), 0.9, "calibration.interval")
    config["latent_node_index"] = integer(
        calibration["latent_node_index"], "calibration.latent_node_index", 0, len(config["weights"]) - 1
    )
    resources = obj(
        request["resources"],
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
        },
        "resources",
    )
    replicates = len(SCENARIOS) * config["replicates_per_scenario"]
    exact(resources["scenario_replicates"], replicates, "resources.replicates")
    rows = len(config["observed"])
    exact(resources["simulated_node_type_work"], replicates * rows, "resources.simulation_work")
    if resources["simulated_node_type_work"] > integer(
        resources["maximum_simulated_node_type_work"], "resources.maximum_simulation_work", 1, 10**9
    ):
        raise ContractError("simulation-work ceiling exceeded")
    iterations = replicates * config["chains"] * (config["tune"] + config["draws"])
    exact(resources["total_iterations"], iterations, "resources.iterations")
    if iterations > integer(resources["maximum_total_iterations"], "resources.maximum_iterations", 1, 10**9):
        raise ContractError("iteration ceiling exceeded")
    nodes = len(config["weights"])
    ppc_work = replicates * nodes * nodes * config["chains"] * config["draws"]
    exact(resources["ppc_pair_work"], ppc_work, "resources.ppc_work")
    if ppc_work > integer(resources["maximum_ppc_pair_work"], "resources.maximum_ppc_work", 1, 10**12):
        raise ContractError("PPC-work ceiling exceeded")
    estimated = replicates * rows * 256 + 384 * 1024**2
    exact(resources["estimated_working_bytes"], estimated, "resources.estimated_bytes")
    if estimated > integer(resources["maximum_working_bytes"], "resources.maximum_bytes", 1, 8 * 1024**3):
        raise ContractError("memory ceiling exceeded")
    config["maximum_output_bytes"] = integer(
        resources["maximum_output_bytes"], "resources.maximum_output", 1, 16 * 1024**2
    )
    config["source_request_sha256"] = source_digest
    return config


def seed_for(seed: int, purpose: str, scenario: str, replicate: int) -> int:
    digest = hashlib.sha256(
        f"marklab-multitype-lgcp-sbc-v1\0{seed}\0{purpose}\0{scenario}\0{replicate}".encode()
    ).digest()
    return int.from_bytes(digest[:4], "little")


def spatial_latent(config: dict[str, Any], amplitude: float, length: float, rng: np.random.Generator) -> np.ndarray:
    latent = np.empty((len(config["types"]), len(config["weights"])), dtype=np.float64)
    for pattern in range(len(config["pattern_ids"])):
        selected = np.flatnonzero(config["node_patterns"] == pattern)
        rows = [config["request"]["nodes"][index] for index in selected]
        coordinates = np.asarray([[row["x_um"], row["y_um"]] for row in rows])
        distance = np.linalg.norm(coordinates[:, None, :] - coordinates[None, :, :], axis=2)
        scaled = math.sqrt(3.0) * distance / length
        covariance = amplitude**2 * (1.0 + scaled) * np.exp(-scaled)
        covariance += config["kernel_jitter"] * np.eye(len(selected))
        blocks = rng.normal(size=(len(config["types"]), len(selected))) @ np.linalg.cholesky(covariance).T
        latent[:, selected] = blocks - blocks.mean(axis=1, keepdims=True)
    return latent


def draw_truth(config: dict[str, Any], scenario: str, replicate: int) -> tuple[dict[str, Any], np.ndarray]:
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


def rank_and_coverage(values: np.ndarray, truth: float) -> tuple[int, bool]:
    flat = np.asarray(values, dtype=np.float64).reshape(-1)
    return int(np.sum(flat < truth)), bool(
        np.quantile(flat, 0.05) <= truth <= np.quantile(flat, 0.95)
    )


def metrics(config: dict[str, Any], counts: np.ndarray) -> dict[str, float]:
    nodes = len(config["weights"])
    types = len(config["types"])
    matrix = np.zeros((nodes, types), dtype=np.float64)
    matrix[config["node_index"], config["type_index"]] = counts
    totals = matrix.sum(axis=1)
    grand = float(totals.sum())
    return {
        "counts": grand,
        "mark_proportions": float(matrix[:, 0].sum() / grand) if grand > 0 else 0.0,
        "cross_type_enrichment": float(
            np.mean((matrix[:, 0] - matrix[:, 0].mean()) * (matrix[:, 1] - matrix[:, 1].mean()))
        ),
        "clustering": float(np.var(totals)),
    }


def tail(values: np.ndarray, observed: float) -> float:
    low = float(np.mean(values <= observed))
    high = float(np.mean(values >= observed))
    return min(1.0, 2.0 * min(low, high))


def ppc(config: dict[str, Any], observed: np.ndarray, expected: np.ndarray, rng: np.random.Generator) -> dict[str, Any]:
    replicated = rng.poisson(expected).astype(np.int64)
    observed_metrics = metrics(config, observed)
    replicated_metrics = [metrics(config, row) for row in replicated]
    return {
        name: {
            "observed": value,
            "replicated_mean": float(np.mean([row[name] for row in replicated_metrics])),
            "two_sided_tail_probability": tail(
                np.asarray([row[name] for row in replicated_metrics]), value
            ),
        }
        for name, value in observed_metrics.items()
    }


def fit_one(config: dict[str, Any], scenario: str, replicate: int, truth: dict[str, Any], counts: np.ndarray) -> dict[str, Any]:
    dynamic = load(
        "marklab_numpyro_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_worker.py",
        "marklab_numpyro_multitype_sbc_model",
    )
    fit_config = dict(config)
    fit_config["observed"] = counts
    sampler = MCMC(
        NUTS(dynamic.make_model(fit_config), target_accept_prob=config["target_accept"],
             max_tree_depth=int(config["maximum_tree_depth"])),
        num_warmup=config["tune"],
        num_samples=config["draws"],
        num_chains=config["chains"],
        chain_method="sequential",
        progress_bar=False,
    )
    sampler.run(
        jax.random.PRNGKey(seed_for(config["seed"], "fit", scenario, replicate)),
        extra_fields=("diverging", "num_steps", "energy"),
    )
    samples = {
        name: np.asarray(value, dtype=np.float64)
        for name, value in sampler.get_samples(group_by_chain=True).items()
    }
    flat = {name: value.reshape((-1, *value.shape[2:])) for name, value in samples.items()}
    draws = {
        "intercept_type_a": flat["intercept"][:, 0],
        "group_effect_difference": flat["group_effect"][:, 0] - flat["group_effect"][:, 1],
        "patient_sd_type_a": flat["patient_sd"][:, 0],
        "pattern_sd_type_a": flat["pattern_sd"][:, 0],
        "field_amplitude": flat["field_amplitude"],
        "field_length_scale_um": flat["field_length_scale_um"],
        "latent_node_type_a": np.asarray(samples["latent_effect"]).reshape(
            (-1, len(config["types"]), len(config["weights"]))
        )[:, 0, config["latent_node_index"]],
    }
    ranks: dict[str, int] = {}
    coverage: dict[str, bool] = {}
    for name in PARAMETERS:
        ranks[name], coverage[name] = rank_and_coverage(draws[name], truth[name])
    expected = np.asarray(samples["expected_count"]).reshape((-1, len(counts)))
    posterior_predictive = ppc(
        config,
        counts,
        expected,
        np.random.default_rng(seed_for(config["seed"], "ppc", scenario, replicate)),
    )
    names = [
        "intercept", "group_effect", "covariate_effect", "patient_sd", "pattern_sd",
        "field_amplitude", "field_length_scale_um", "patient_raw", "pattern_raw", "field_raw",
    ]
    posterior = az.from_dict({"posterior": {name: samples[name] for name in names}})
    flattened = lambda tree: np.concatenate(
        [np.asarray(tree[name].values).reshape(-1) for name in names]
    )
    r_hat = float(flattened(az.rhat(posterior, var_names=names, method="rank")).max())
    bulk = float(flattened(az.ess(posterior, var_names=names, method="bulk")).min())
    tail_ess = float(flattened(az.ess(posterior, var_names=names, method="tail")).min())
    extra = sampler.get_extra_fields(group_by_chain=True)
    energy = np.asarray(extra["energy"], dtype=np.float64)
    ebfmi = float(np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1)))
    divergences = int(np.asarray(extra["diverging"]).sum())
    depth = int(config["maximum_tree_depth"])
    depth_hits = int((np.asarray(extra["num_steps"]) >= 2**depth - 1).sum())
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
        "r_hat": r_hat,
        "ess_bulk": bulk,
        "ess_tail": tail_ess,
        "minimum_ebfmi": ebfmi,
        "divergences": divergences,
        "max_tree_depth_hits": depth_hits,
    }


def failed(scenario: str, replicate: int, truth: dict[str, Any], count: int, error: Exception) -> dict[str, Any]:
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
    complete = [row for row in rows if row["status"] == "complete"]
    ranks = np.asarray([row["ranks"][parameter] for row in complete], dtype=np.int64)
    histogram = np.zeros(bins, dtype=np.int64)
    for value in ranks:
        histogram[min(int(value) * bins // (draws + 1), bins - 1)] += 1
    return {
        "completed_replicates": len(complete),
        "rank_histogram": histogram.tolist(),
        "coverage_90": float(np.mean([row["coverage_90"][parameter] for row in complete])) if complete else 0.0,
        "mean_normalized_rank": float(ranks.mean() / draws) if complete else 0.0,
    }


def main() -> int:
    if numpyro.__version__ != NUMPYRO_VERSION or jax.__version__ != JAX_VERSION or sys.version_info[:2] != (3, 12):
        raise ContractError("NumPyro, JAX, or Python version drift")
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    raw = sys.stdin.buffer.read(64 * 1024 * 1024 + 1)
    if not raw or len(raw) > 64 * 1024 * 1024:
        raise ContractError("request size is invalid")
    request_sha = hashlib.sha256(raw).hexdigest()
    config = validate(json.loads(raw), lock_sha, worker_sha)
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
    diagnostics = {
        parameter: aggregate(rows, parameter, draws, config["rank_bins"])
        for parameter in PARAMETERS
    }
    result = {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": {
            "name": "numpyro",
            "version": numpyro.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha,
            "worker_sha256": worker_sha,
        },
        "jax_version": jax.__version__,
        "input_sha256": config["input_sha"],
        "request_sha256": request_sha,
        "source_request_sha256": config["source_request_sha256"],
        "fit_state": "complete" if all(row["status"] == "complete" for row in rows) else "nonconverged",
        "scenario_dispositions": rows,
        "rank_diagnostics": diagnostics,
        "pattern_representation": "complete_quadrature_resolved_counting_measure_on_exact_window",
        "cross_type_dependence_estimand": "not_estimated_current_model_uses_conditionally_independent_type_fields",
    }
    output = json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")) + "\n"
    if len(output.encode()) > config["maximum_output_bytes"]:
        raise ContractError("SBC output exceeds ceiling")
    sys.stdout.write(output)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"marklab multitype LGCP SBC failed: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from error
