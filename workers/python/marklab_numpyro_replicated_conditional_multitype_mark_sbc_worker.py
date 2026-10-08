#!/usr/bin/env python3
"""Exact finite-state SBC for patient/pattern hierarchical conditional marks."""

from __future__ import annotations

import hashlib
import importlib.util
import itertools
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
from scipy import stats


REQUEST_FORMAT = "marklab.numpyro_replicated_conditional_multitype_mark_sbc_request"
RESULT_FORMAT = "marklab.numpyro_replicated_conditional_multitype_mark_sbc_result"
NUMPYRO_VERSION = "0.21.0"
JAX_VERSION = "0.11.1"


class ContractError(ValueError):
    pass


def load_module(filename: str, name: str) -> Any:
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


def number(value: Any, path: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ContractError(f"{path} must be numeric")
    result = float(value)
    if not math.isfinite(result):
        raise ContractError(f"{path} must be finite")
    return result


def integer(value: Any, path: str, low: int, high: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not low <= value <= high:
        raise ContractError(f"{path} must be an integer in [{low}, {high}]")
    return value


def validate(
    request: Any, lock_sha: str, worker_sha: str, pymc_worker_sha: str
) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format", "version", "backend", "jax_version", "source_request_sha256",
            "source_request", "calibration", "resources",
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
    source_sha = request["source_request_sha256"]
    if (
        not isinstance(source_sha, str)
        or len(source_sha) != 64
        or any(character not in "0123456789abcdef" for character in source_sha)
    ):
        raise ContractError("source request digest is invalid")
    pymc = load_module(
        "marklab_pymc_replicated_conditional_multitype_mark_worker.py",
        "marklab_pymc_replicated_conditional_sbc_contract",
    )
    config = pymc.validate(request["source_request"], lock_sha, pymc_worker_sha)
    point_counts = [row["point_count"] for row in request["source_request"]["patterns"]]
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
            "replicates", "rank_bins", "interval_probability",
            "minimum_rank_uniformity_p_value", "minimum_coverage", "maximum_coverage",
        },
        "calibration",
    )
    config["replicates"] = integer(calibration["replicates"], "calibration.replicates", 20, 100)
    config["rank_bins"] = integer(calibration["rank_bins"], "calibration.rank_bins", 2, 20)
    exact(number(calibration["interval_probability"], "calibration.interval"), 0.9, "interval")
    config["minimum_rank_p"] = number(
        calibration["minimum_rank_uniformity_p_value"], "calibration.minimum_rank_p"
    )
    config["minimum_coverage"] = number(calibration["minimum_coverage"], "calibration.minimum_coverage")
    config["maximum_coverage"] = number(calibration["maximum_coverage"], "calibration.maximum_coverage")
    if not 0 <= config["minimum_rank_p"] <= 1 or not 0 <= config["minimum_coverage"] <= config["maximum_coverage"] <= 1:
        raise ContractError("calibration thresholds are invalid")

    resources = obj(
        request["resources"],
        {
            "maximum_states_per_pattern", "states_per_pattern", "points_per_pattern",
            "pattern_count", "maximum_enumeration_work", "enumeration_work",
            "maximum_enumeration_bytes", "enumeration_bytes", "maximum_total_iterations",
            "total_iterations", "maximum_output_bytes", "timeout_seconds",
        },
        "resources",
    )
    states_per_pattern = len(config["types"]) ** points_per_pattern
    exact(resources["states_per_pattern"], states_per_pattern, "resources.states_per_pattern")
    exact(resources["points_per_pattern"], points_per_pattern, "resources.points_per_pattern")
    exact(resources["pattern_count"], len(config["pattern_ids"]), "resources.pattern_count")
    if states_per_pattern > integer(
        resources["maximum_states_per_pattern"], "resources.maximum_states", 1, 1_000_000
    ):
        raise ContractError("state ceiling exceeded")
    edge_counts = [row["edge_count"] for row in request["source_request"]["patterns"]]
    enumeration_work = config["replicates"] * states_per_pattern * sum(
        points + edges for points, edges in zip(point_counts, edge_counts, strict=True)
    )
    exact(resources["enumeration_work"], enumeration_work, "resources.enumeration_work")
    if enumeration_work > integer(
        resources["maximum_enumeration_work"], "resources.maximum_enumeration_work", 1, 1_000_000_000
    ):
        raise ContractError("enumeration-work ceiling exceeded")
    enumeration_bytes = states_per_pattern * points_per_pattern * 2 + states_per_pattern * 16
    exact(resources["enumeration_bytes"], enumeration_bytes, "resources.enumeration_bytes")
    if enumeration_bytes > integer(
        resources["maximum_enumeration_bytes"], "resources.maximum_enumeration_bytes", 1, 1024**3
    ):
        raise ContractError("enumeration-byte ceiling exceeded")
    total_iterations = config["replicates"] * config["chains"] * (config["tune"] + config["draws"])
    exact(resources["total_iterations"], total_iterations, "resources.total_iterations")
    if total_iterations > integer(
        resources["maximum_total_iterations"], "resources.maximum_total_iterations", 1, 10_000_000
    ):
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
    value = hashlib.sha256(
        f"marklab-replicated-conditional-multitype-sbc-v1\0{seed}\0{purpose}\0{replicate}".encode()
    ).digest()
    return int.from_bytes(value[:4], "little")


def state_table(config: dict[str, Any]) -> np.ndarray:
    return np.asarray(
        list(
            itertools.product(
                range(len(config["types"])), repeat=config["points_per_pattern"]
            )
        ),
        dtype=np.int16,
    )


def potential_design(types: int) -> np.ndarray:
    result = np.zeros((types * (types + 1) // 2 - 1, types, types), dtype=np.float64)
    index = 0
    for left in range(types):
        for right in range(left, types):
            if left == 0 and right == 0:
                continue
            result[index, left, right] = 1.0
            result[index, right, left] = 1.0
            index += 1
    return result


def affinity_values(potential: np.ndarray) -> list[float]:
    return [
        float(potential[left, right] - 0.5 * (potential[left, left] + potential[right, right]))
        for left in range(potential.shape[0])
        for right in range(left + 1, potential.shape[0])
    ]


def parameter_names(type_ids: list[str]) -> list[str]:
    names: list[str] = []
    for prefix in ["baseline_affinity", "group_affinity_shift"]:
        names.extend(
            f"{prefix}:{type_ids[left]}|{type_ids[right]}"
            for left in range(len(type_ids))
            for right in range(left + 1, len(type_ids))
        )
    names.extend(
        [
            "patient_intercept_sd", "patient_potential_sd",
            "pattern_intercept_sd", "pattern_potential_sd",
        ]
    )
    return names


def simulate(
    config: dict[str, Any], states: np.ndarray, replicate: int
) -> tuple[dict[str, float], np.ndarray, np.ndarray, list[list[int]]]:
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
    pattern_intercept_free = (
        baseline_intercept[None, :]
        + config["pattern_groups"][:, None] * group_intercept[None, :]
        + patient_intercept_sd * patient_intercept_z[config["pattern_patients"]]
        + pattern_intercept_sd * pattern_intercept_z
    )
    pattern_potential_free = (
        baseline_potential_free[None, :]
        + config["pattern_groups"][:, None] * group_potential_free[None, :]
        + patient_potential_sd * patient_potential_z[config["pattern_patients"]]
        + pattern_potential_sd * pattern_potential_z
    )
    pattern_intercept = np.zeros((patterns, types), dtype=np.float64)
    pattern_intercept[:, 1:] = pattern_intercept_free
    pattern_potential = np.einsum("pf,fkl->pkl", pattern_potential_free, design)

    observed = np.empty(len(config["observed"]), dtype=np.int64)
    pattern_counts: list[list[int]] = []
    for pattern in range(patterns):
        indices = np.flatnonzero(config["point_patterns"] == pattern)
        local = {int(global_index): local_index for local_index, global_index in enumerate(indices)}
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
    baseline_potential = np.einsum("f,fkl->kl", baseline_potential_free, design)
    group_potential = np.einsum("f,fkl->kl", group_potential_free, design)
    truth: dict[str, float] = {}
    pairs = [
        (left, right)
        for left in range(types)
        for right in range(left + 1, types)
    ]
    for prefix, values in [
        ("baseline_affinity", affinity_values(baseline_potential)),
        ("group_affinity_shift", affinity_values(group_potential)),
    ]:
        for (left, right), value in zip(pairs, values, strict=True):
            truth[f"{prefix}:{config['types'][left]}|{config['types'][right]}"] = value
    truth.update(
        {
            "patient_intercept_sd": patient_intercept_sd,
            "patient_potential_sd": patient_potential_sd,
            "pattern_intercept_sd": pattern_intercept_sd,
            "pattern_potential_sd": pattern_potential_sd,
        }
    )
    return truth, observed, counts, pattern_counts


def flattened(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def posterior_parameter_draws(
    samples: dict[str, np.ndarray], config: dict[str, Any]
) -> dict[str, np.ndarray]:
    types = len(config["types"])
    design = potential_design(types)
    flat_baseline = samples["baseline_potential"].reshape((-1, design.shape[0]))
    flat_group = samples["group_potential"].reshape((-1, design.shape[0]))
    baseline = np.einsum("df,fkl->dkl", flat_baseline, design)
    group = np.einsum("df,fkl->dkl", flat_group, design)
    result: dict[str, np.ndarray] = {}
    for prefix, potential in [("baseline_affinity", baseline), ("group_affinity_shift", group)]:
        for left in range(types):
            for right in range(left + 1, types):
                result[f"{prefix}:{config['types'][left]}|{config['types'][right]}"] = (
                    potential[:, left, right]
                    - 0.5 * (potential[:, left, left] + potential[:, right, right])
                )
    for name in [
        "patient_intercept_sd", "patient_potential_sd",
        "pattern_intercept_sd", "pattern_potential_sd",
    ]:
        result[name] = samples[name].reshape(-1)
    return result


def run_sbc(
    config: dict[str, Any], states: np.ndarray
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    model_module = load_module(
        "marklab_numpyro_replicated_conditional_multitype_mark_worker.py",
        "marklab_numpyro_replicated_conditional_sbc_model",
    )
    completed: list[dict[str, Any]] = []
    failures: list[dict[str, Any]] = []
    names = parameter_names(config["types"])
    diagnostic_names = [
        "baseline_intercept", "group_intercept", "baseline_potential", "group_potential",
        "patient_intercept_sd", "patient_potential_sd", "pattern_intercept_sd",
        "pattern_potential_sd", "patient_intercept_z", "patient_potential_z",
        "pattern_intercept_z", "pattern_potential_z",
    ]
    for replicate in range(config["replicates"]):
        try:
            truth, observed, counts, pattern_counts = simulate(config, states, replicate)
            fit_config = dict(config)
            fit_config["observed"] = observed
            fit_config["neighbor_counts"] = counts
            sampler = MCMC(
                NUTS(
                    model_module.make_model(fit_config),
                    target_accept_prob=config["target_accept"],
                    max_tree_depth=config["numpyro_maximum_tree_depth"],
                ),
                num_warmup=config["tune"],
                num_samples=config["draws"],
                num_chains=config["chains"],
                chain_method="sequential",
                progress_bar=False,
            )
            sampler.run(
                jax.random.PRNGKey(seed_for(config["seed"], "fit", replicate)),
                extra_fields=("diverging", "num_steps", "energy"),
            )
            samples = {
                key: np.asarray(value, dtype=np.float64)
                for key, value in sampler.get_samples(group_by_chain=True).items()
            }
            posterior = az.from_dict({"posterior": samples})
            r_hat = float(flattened(az.rhat(posterior, var_names=diagnostic_names, method="rank"), diagnostic_names).max())
            bulk = float(flattened(az.ess(posterior, var_names=diagnostic_names, method="bulk"), diagnostic_names).min())
            tail = float(flattened(az.ess(posterior, var_names=diagnostic_names, method="tail"), diagnostic_names).min())
            extra = sampler.get_extra_fields(group_by_chain=True)
            energy = np.asarray(extra["energy"], dtype=np.float64)
            variance = np.var(energy, axis=1)
            ebfmi = float(np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / variance))
            divergences = int(np.asarray(extra["diverging"]).sum())
            maximum_steps = 2 ** config["numpyro_maximum_tree_depth"] - 1
            depth_hits = int((np.asarray(extra["num_steps"]) >= maximum_steps).sum())
            if not (
                np.isfinite([r_hat, bulk, tail, ebfmi]).all()
                and r_hat <= config["maximum_r_hat"]
                and bulk >= config["minimum_bulk_ess"]
                and tail >= config["minimum_tail_ess"]
                and ebfmi >= config["minimum_ebfmi"]
                and divergences <= config["maximum_divergences"]
                and depth_hits <= config["maximum_tree_depth_hits"]
            ):
                failures.append(
                    {
                        "replicate": replicate,
                        "reason": (
                            f"diagnostics_failed:r_hat={r_hat:.17g},bulk={bulk:.17g},"
                            f"tail={tail:.17g},ebfmi={ebfmi:.17g},divergences={divergences},"
                            f"depth_hits={depth_hits}"
                        ),
                    }
                )
                continue
            draws = posterior_parameter_draws(samples, config)
            parameters = [
                {
                    "parameter": name,
                    "truth": truth[name],
                    "rank": int(np.sum(draws[name] < truth[name])),
                    "covered": bool(
                        np.quantile(draws[name], 0.05)
                        <= truth[name]
                        <= np.quantile(draws[name], 0.95)
                    ),
                }
                for name in names
            ]
            completed.append(
                {
                    "replicate": replicate,
                    "simulated_pattern_type_counts": pattern_counts,
                    "parameters": parameters,
                    "r_hat": r_hat,
                    "ess_bulk": bulk,
                    "ess_tail": tail,
                    "minimum_ebfmi": ebfmi,
                    "divergences": divergences,
                    "max_tree_depth_hits": depth_hits,
                }
            )
        except Exception as error:
            failures.append(
                {"replicate": replicate, "reason": f"{type(error).__name__}:{str(error)[:400]}"}
            )
    return completed, failures


def aggregate(completed: list[dict[str, Any]], name: str, draws: int, bins: int) -> dict[str, Any]:
    rows = [next(value for value in row["parameters"] if value["parameter"] == name) for row in completed]
    ranks = np.asarray([row["rank"] for row in rows], dtype=np.int64)
    histogram = np.zeros(bins, dtype=np.int64)
    for value in ranks:
        histogram[min(int(value) * bins // (draws + 1), bins - 1)] += 1
    return {
        "parameter": name,
        "rank_histogram": histogram.tolist(),
        "rank_uniformity_p_value": float(stats.chisquare(histogram).pvalue) if rows else 0.0,
        "coverage_90": float(np.mean([row["covered"] for row in rows])) if rows else 0.0,
        "mean_normalized_rank": float(ranks.mean() / draws) if rows else 0.0,
    }


def main() -> int:
    if numpyro.__version__ != NUMPYRO_VERSION or jax.__version__ != JAX_VERSION or sys.version_info[:2] != (3, 12):
        raise ContractError("NumPyro, JAX, or Python version drift")
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    pymc_worker_sha = hashlib.sha256(
        script.with_name("marklab_pymc_replicated_conditional_multitype_mark_worker.py").read_bytes()
    ).hexdigest()
    raw = sys.stdin.buffer.read(64 * 1_048_576 + 1)
    if not raw or len(raw) > 64 * 1_048_576:
        raise ContractError("request size is invalid")
    request_sha = hashlib.sha256(raw).hexdigest()
    config = validate(json.loads(raw), lock_sha, worker_sha, pymc_worker_sha)
    states = state_table(config)
    completed, failures = run_sbc(config, states)
    total_draws = config["chains"] * config["draws"]
    diagnostics = [
        aggregate(completed, name, total_draws, config["rank_bins"])
        for name in parameter_names(config["types"])
    ]
    passes = not failures and all(
        config["minimum_rank_p"] <= row["rank_uniformity_p_value"]
        and config["minimum_coverage"] <= row["coverage_90"] <= config["maximum_coverage"]
        for row in diagnostics
    )
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
        "fit_state": "complete" if passes else "nonconverged",
        "enumeration_oracle": {
            "states_per_pattern": len(states),
            "zero_parameter_log_normalizer_per_pattern": float(
                config["points_per_pattern"] * math.log(len(config["types"]))
            ),
        },
        "replicates": completed,
        "failures": failures,
        "diagnostics": diagnostics,
    }
    output = json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")) + "\n"
    if len(output.encode()) > config["maximum_output_bytes"]:
        raise ContractError("result exceeds output ceiling")
    sys.stdout.write(output)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(
            f"marklab replicated conditional multitype SBC failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
