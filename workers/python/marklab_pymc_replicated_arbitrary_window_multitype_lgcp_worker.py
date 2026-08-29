#!/usr/bin/env python3
"""Pinned PyMC fit for patient-replicated exact-window multitype LGCPs."""

from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path
import sys
from typing import Any

import arviz as az
import numpy as np
import pymc as pm
import pytensor.tensor as pt


PYMC_VERSION = "6.3.0"
REQUEST_FORMAT = "marklab.pymc_replicated_arbitrary_window_multitype_lgcp_request"
RESULT_FORMAT = "marklab.bayesian_replicated_arbitrary_window_multitype_lgcp_fit"


class ContractError(ValueError):
    pass


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


def finite(value: Any, path: str) -> float:
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


def identities(value: Any, path: str, low: int, high: int) -> list[str]:
    if (
        not isinstance(value, list)
        or not low <= len(value) <= high
        or any(not isinstance(item, str) or not item or len(item) > 256 for item in value)
        or len(set(value)) != len(value)
    ):
        raise ContractError(f"{path} identities are invalid")
    return value


def validate(request: Any, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format", "version", "backend", "input_sha256", "reference_group",
            "comparison_group", "type_ids", "reference_type", "patients", "patterns",
            "nodes", "node_type_counts", "cholesky", "priors", "sampling",
            "diagnostic_policy", "resources",
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
    exact(backend["name"], "pymc", "backend.name")
    exact(backend["version"], PYMC_VERSION, "backend.version")
    exact(backend["python_version"], "3.12", "backend.python_version")
    exact(backend["environment_lock_sha256"], lock_sha, "backend.lock")
    exact(backend["worker_sha256"], worker_sha, "backend.worker")
    input_sha = request["input_sha256"]
    if not isinstance(input_sha, str) or len(input_sha) != 64:
        raise ContractError("input digest is invalid")
    types = identities(request["type_ids"], "type_ids", 3, 8)
    exact(request["reference_type"], types[0], "reference_type")
    patients = request["patients"]
    patterns = request["patterns"]
    nodes = request["nodes"]
    counts = request["node_type_counts"]
    if not all(isinstance(rows, list) for rows in (patients, patterns, nodes, counts)):
        raise ContractError("hierarchy rows are invalid")
    patient_ids = [row["patient_id"] for row in patients]
    pattern_ids = [row["pattern_id"] for row in patterns]
    if patient_ids != sorted(set(patient_ids)) or pattern_ids != sorted(set(pattern_ids)):
        raise ContractError("patient or pattern identities are not canonical")
    patient_groups = np.asarray(
        [integer(row["group_index"], "patient.group_index", 0, 1) for row in patients],
        dtype=np.float64,
    )
    if set(patient_groups.tolist()) != {0.0, 1.0}:
        raise ContractError("both groups are required")
    pattern_patients = np.asarray(
        [integer(row["patient_index"], "pattern.patient_index", 0, len(patients) - 1) for row in patterns],
        dtype=np.int64,
    )
    node_patterns = np.asarray(
        [integer(row["pattern_index"], "node.pattern_index", 0, len(patterns) - 1) for row in nodes],
        dtype=np.int64,
    )
    node_patients = np.asarray(
        [integer(row["patient_index"], "node.patient_index", 0, len(patients) - 1) for row in nodes],
        dtype=np.int64,
    )
    if np.any(node_patients != pattern_patients[node_patterns]):
        raise ContractError("node hierarchy differs")
    weights = np.asarray([finite(row["weight_um2"], "node.weight") for row in nodes])
    covariate = np.asarray([finite(row["covariate"], "node.covariate") for row in nodes])
    if np.any(weights <= 0.0):
        raise ContractError("node weights must be positive")
    if len(counts) != len(nodes) * len(types):
        raise ContractError("node-type table is incomplete")
    node_index = np.empty(len(counts), dtype=np.int64)
    type_index = np.empty(len(counts), dtype=np.int64)
    observed = np.empty(len(counts), dtype=np.int64)
    for index, row in enumerate(counts):
        row = obj(row, {"node_index", "type_index", "count"}, f"counts[{index}]")
        node_index[index] = integer(row["node_index"], "count.node_index", 0, len(nodes) - 1)
        type_index[index] = integer(row["type_index"], "count.type_index", 0, len(types) - 1)
        observed[index] = integer(row["count"], "count", 0, 2**63 - 1)
        exact(node_index[index], index // len(types), "count node order")
        exact(type_index[index], index % len(types), "count type order")
    for pattern_index, pattern in enumerate(patterns):
        selected_nodes = node_patterns == pattern_index
        if int(selected_nodes.sum()) != int(pattern["node_count"]):
            raise ContractError("pattern node count differs")
        selected_rows = selected_nodes[node_index]
        if int(observed[selected_rows].sum()) != int(pattern["event_count"]):
            raise ContractError("pattern event count differs")
        if abs(float(weights[selected_nodes].sum()) - finite(pattern["window_area_um2"], "pattern.area")) > 1e-10 * max(1.0, abs(float(pattern["window_area_um2"]))):
            raise ContractError("pattern weights do not conserve area")
    dimension = len(nodes)
    cholesky = np.asarray(
        [finite(value, "cholesky[]") for value in request["cholesky"]], dtype=np.float64
    )
    if cholesky.size != dimension * dimension:
        raise ContractError("Cholesky dimensions differ")
    cholesky = cholesky.reshape(dimension, dimension)
    if np.max(np.abs(np.triu(cholesky, 1))) > 1e-14 or np.any(np.diag(cholesky) <= 0.0):
        raise ContractError("physical block Cholesky is invalid")
    priors = obj(
        request["priors"],
        {
            "intercept_mean", "intercept_sd", "group_effect_sd", "covariate_effect_sd",
            "patient_sd_scale", "pattern_sd_scale", "field_amplitude",
            "field_length_scale_um", "jitter",
        },
        "priors",
    )
    prior = {name: finite(value, f"priors.{name}") for name, value in priors.items()}
    if min(value for name, value in prior.items() if name != "intercept_mean") <= 0.0:
        raise ContractError("prior scales must be positive")
    sampling = obj(
        request["sampling"],
        {"chains", "tune_per_chain", "draws_per_chain", "target_accept", "seed"},
        "sampling",
    )
    chains = integer(sampling["chains"], "sampling.chains", 2, 8)
    tune = integer(sampling["tune_per_chain"], "sampling.tune", 100, 100_000)
    draws = integer(sampling["draws_per_chain"], "sampling.draws", 100, 100_000)
    target_accept = finite(sampling["target_accept"], "sampling.target_accept")
    if not 0.5 <= target_accept < 1.0:
        raise ContractError("target acceptance is invalid")
    seed = integer(sampling["seed"], "sampling.seed", 0, 2**64 - 1)
    policy = obj(
        request["diagnostic_policy"],
        {
            "prior_predictive_draws", "maximum_r_hat", "minimum_bulk_ess",
            "minimum_tail_ess", "minimum_ebfmi", "maximum_divergences",
            "maximum_tree_depth_hits", "maximum_tree_depth",
        },
        "diagnostic_policy",
    )
    resources = obj(
        request["resources"],
        {
            "maximum_patients", "maximum_patterns", "maximum_types",
            "maximum_nodes_per_pattern", "maximum_total_nodes",
            "maximum_total_node_type_rows", "maximum_total_events",
            "maximum_draw_node_type_work", "draw_node_type_work", "timeout_seconds",
        },
        "resources",
    )
    work = chains * draws * len(counts)
    exact(resources["draw_node_type_work"], work, "resources.draw_work")
    maximum_nodes_per_pattern = integer(
        resources["maximum_nodes_per_pattern"], "resources.nodes_per_pattern", 4, 36
    )
    if (
        len(patients) > integer(resources["maximum_patients"], "resources.patients", 8, 32)
        or len(patterns) > integer(resources["maximum_patterns"], "resources.patterns", 16, 64)
        or len(types) > integer(resources["maximum_types"], "resources.types", 3, 8)
        or any(int(pattern["node_count"]) > maximum_nodes_per_pattern for pattern in patterns)
        or len(nodes) > integer(resources["maximum_total_nodes"], "resources.nodes", 64, 512)
        or len(counts) > integer(resources["maximum_total_node_type_rows"], "resources.rows", 1, 4096)
        or int(observed.sum()) > integer(resources["maximum_total_events"], "resources.events", 1, 2**63 - 1)
        or work > integer(resources["maximum_draw_node_type_work"], "resources.work", 1, 2**63 - 1)
    ):
        raise ContractError("resource ceiling exceeded")
    return {
        "request": request,
        "input_sha": input_sha,
        "types": types,
        "patient_ids": patient_ids,
        "pattern_ids": pattern_ids,
        "patient_groups": patient_groups,
        "pattern_patients": pattern_patients,
        "node_patterns": node_patterns,
        "node_patients": node_patients,
        "weights": weights,
        "covariate": covariate,
        "node_index": node_index,
        "type_index": type_index,
        "observed": observed,
        "cholesky": cholesky,
        "priors": prior,
        "chains": chains,
        "tune": tune,
        "draws": draws,
        "target_accept": target_accept,
        "seed": seed,
        "policy": policy,
    }


def summary(values: np.ndarray) -> dict[str, float]:
    flat = np.asarray(values, dtype=np.float64).reshape(-1)
    return {
        "mean": float(flat.mean()),
        "sd": float(flat.std(ddof=1)),
        "interval_lower": float(np.quantile(flat, 0.025)),
        "interval_upper": float(np.quantile(flat, 0.975)),
    }


def flattened(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def two_sided_tail(replicated: np.ndarray, observed: float) -> float:
    lower = float(np.mean(replicated <= observed))
    upper = float(np.mean(replicated >= observed))
    return min(1.0, 2.0 * min(lower, upper))


def seed_for(seed: int, purpose: str) -> int:
    value = hashlib.sha256(
        f"marklab-replicated-arbitrary-window-multitype-lgcp-v1\0{seed}\0{purpose}".encode()
    ).digest()
    return int.from_bytes(value[:8], "little")


def prior_is_finite(config: dict[str, Any]) -> bool:
    rng = np.random.default_rng(seed_for(config["seed"], "prior"))
    draws = int(config["policy"]["prior_predictive_draws"])
    types = len(config["types"])
    patients = len(config["patient_ids"])
    patterns = len(config["pattern_ids"])
    nodes = len(config["weights"])
    priors = config["priors"]
    intercept = rng.normal(priors["intercept_mean"], priors["intercept_sd"], (draws, types))
    group = rng.normal(0.0, priors["group_effect_sd"], (draws, types))
    covariate = rng.normal(0.0, priors["covariate_effect_sd"], (draws, types))
    patient_sd = np.abs(rng.normal(0.0, priors["patient_sd_scale"], (draws, types)))
    pattern_sd = np.abs(rng.normal(0.0, priors["pattern_sd_scale"], (draws, types)))
    patient_effect = patient_sd[:, None, :] * rng.normal(size=(draws, patients, types))
    pattern_raw = rng.normal(size=(draws, patterns, types))
    for patient in range(patients):
        selected = config["pattern_patients"] == patient
        pattern_raw[:, selected, :] -= pattern_raw[:, selected, :].mean(axis=1, keepdims=True)
    pattern_effect = pattern_sd[:, None, :] * pattern_raw
    field_raw = rng.normal(size=(draws, types, nodes))
    latent = np.einsum("dkn,mn->dkm", field_raw, config["cholesky"])
    for pattern in range(patterns):
        selected = config["node_patterns"] == pattern
        latent[:, :, selected] -= latent[:, :, selected].mean(axis=2, keepdims=True)
    with np.errstate(over="ignore", invalid="ignore"):
        log_expected = (
            intercept[:, :, None]
            + group[:, :, None] * config["patient_groups"][config["node_patients"]][None, None, :]
            + covariate[:, :, None] * config["covariate"][None, None, :]
            + np.transpose(patient_effect[:, config["node_patients"], :], (0, 2, 1))
            + np.transpose(pattern_effect[:, config["node_patterns"], :], (0, 2, 1))
            + latent
            + np.log(config["weights"])[None, None, :]
        )
        expected = np.exp(log_expected)
    return bool(np.isfinite(expected).all() and np.all(expected > 0.0))


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    types = len(config["types"])
    patients = len(config["patient_ids"])
    patterns = len(config["pattern_ids"])
    nodes = len(config["weights"])
    priors = config["priors"]
    prior_finite = prior_is_finite(config)
    with pm.Model():
        intercept = pm.Normal(
            "intercept", priors["intercept_mean"], priors["intercept_sd"], shape=types
        )
        group_effect = pm.Normal(
            "group_effect", 0.0, priors["group_effect_sd"], shape=types
        )
        covariate_effect = pm.Normal(
            "covariate_effect", 0.0, priors["covariate_effect_sd"], shape=types
        )
        patient_sd = pm.HalfNormal(
            "patient_sd", priors["patient_sd_scale"], shape=types
        )
        pattern_sd = pm.HalfNormal(
            "pattern_sd", priors["pattern_sd_scale"], shape=types
        )
        patient_raw = pm.Normal("patient_raw", 0.0, 1.0, shape=(patients, types))
        pattern_raw = pm.Normal("pattern_raw", 0.0, 1.0, shape=(patterns, types))
        field_raw = pm.Normal("field_raw", 0.0, 1.0, shape=(types, nodes))
        patient_effect = pm.Deterministic(
            "patient_effect", patient_sd[None, :] * patient_raw
        )
        pattern_patient_mean = pt.stack(
            [
                pt.mean(
                    pattern_raw[np.flatnonzero(config["pattern_patients"] == patient), :],
                    axis=0,
                )
                for patient in range(patients)
            ]
        )
        pattern_effect = pm.Deterministic(
            "pattern_effect",
            pattern_sd[None, :]
            * (pattern_raw - pattern_patient_mean[config["pattern_patients"], :]),
        )
        latent_uncentered = field_raw @ config["cholesky"].T
        latent_pattern_mean = pt.stack(
            [
                pt.mean(
                    latent_uncentered[:, np.flatnonzero(config["node_patterns"] == pattern)],
                    axis=1,
                )
                for pattern in range(patterns)
            ],
            axis=1,
        )
        latent_effect = pm.Deterministic(
            "latent_effect",
            latent_uncentered - latent_pattern_mean[:, config["node_patterns"]],
        )
        log_expected_matrix = (
            intercept[:, None]
            + group_effect[:, None]
            * config["patient_groups"][config["node_patients"]][None, :]
            + covariate_effect[:, None] * config["covariate"][None, :]
            + patient_effect[config["node_patients"], :].T
            + pattern_effect[config["node_patterns"], :].T
            + latent_effect
            + np.log(config["weights"])[None, :]
        )
        expected_matrix = pm.Deterministic(
            "expected_matrix", pm.math.exp(log_expected_matrix)
        )
        expected_count = pm.Deterministic(
            "expected_count",
            expected_matrix[config["type_index"], config["node_index"]],
        )
        pm.Poisson("observed_count", mu=expected_count, observed=config["observed"])
        trace = pm.sample(
            draws=config["draws"],
            tune=config["tune"],
            chains=config["chains"],
            cores=1,
            blas_cores=1,
            target_accept=config["target_accept"],
            random_seed=config["seed"],
            progressbar=False,
            quiet=True,
            compute_convergence_checks=False,
            return_inferencedata=True,
            nuts={"max_treedepth": int(config["policy"]["maximum_tree_depth"])},
        )
    names = [
        "intercept", "group_effect", "covariate_effect", "patient_sd", "pattern_sd",
        "patient_raw", "pattern_raw", "field_raw",
    ]
    r_hat = float(flattened(az.rhat(trace, var_names=names, method="rank"), names).max())
    bulk = float(flattened(az.ess(trace, var_names=names, method="bulk"), names).min())
    tail = float(flattened(az.ess(trace, var_names=names, method="tail"), names).min())
    mcse_mean = float(flattened(az.mcse(trace, var_names=names, method="mean"), names).max())
    mcse_sd = float(flattened(az.mcse(trace, var_names=names, method="sd"), names).max())
    stats_tree = trace.sample_stats
    energy = np.asarray(stats_tree["energy"], dtype=np.float64)
    ebfmi = float(np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1)))
    divergences = int(np.asarray(stats_tree["diverging"]).sum())
    depth_hits = int(np.asarray(stats_tree["reached_max_treedepth"]).sum())
    posterior = trace.posterior
    flat = {
        name: np.asarray(posterior[name], dtype=np.float64).reshape(
            (-1, *np.asarray(posterior[name]).shape[2:])
        )
        for name in [
            "intercept", "group_effect", "covariate_effect", "patient_sd", "pattern_sd",
            "patient_effect", "pattern_effect", "latent_effect", "expected_count",
        ]
    }
    type_posteriors = [
        {
            "type_id": identity,
            "intercept": summary(flat["intercept"][:, index]),
            "group_effect": summary(flat["group_effect"][:, index]),
            "covariate_effect": summary(flat["covariate_effect"][:, index]),
            "patient_sd": summary(flat["patient_sd"][:, index]),
            "pattern_sd": summary(flat["pattern_sd"][:, index]),
        }
        for index, identity in enumerate(config["types"])
    ]
    group_differences = [
        {
            "type_a": config["types"][left],
            "type_b": config["types"][right],
            "difference": summary(flat["group_effect"][:, left] - flat["group_effect"][:, right]),
        }
        for left in range(types)
        for right in range(left + 1, types)
    ]
    patient_type_effects = [
        {
            "owner_id": patient_id,
            "type_id": type_id,
            "effect": summary(flat["patient_effect"][:, patient_index, type_index]),
        }
        for patient_index, patient_id in enumerate(config["patient_ids"])
        for type_index, type_id in enumerate(config["types"])
    ]
    pattern_type_effects = [
        {
            "owner_id": pattern_id,
            "type_id": type_id,
            "effect": summary(flat["pattern_effect"][:, pattern_index, type_index]),
        }
        for pattern_index, pattern_id in enumerate(config["pattern_ids"])
        for type_index, type_id in enumerate(config["types"])
    ]
    node_type_posteriors = [
        {
            "pattern_id": config["request"]["nodes"][int(node)]["pattern_id"],
            "node_id": config["request"]["nodes"][int(node)]["node_id"],
            "type_id": config["types"][int(type_index)],
            "latent_effect": summary(flat["latent_effect"][:, int(type_index), int(node)]),
            "expected_count": summary(flat["expected_count"][:, row]),
        }
        for row, (node, type_index) in enumerate(zip(config["node_index"], config["type_index"], strict=True))
    ]
    predictive_rng = np.random.default_rng(seed_for(config["seed"], "posterior_predictive"))
    replicated = predictive_rng.poisson(flat["expected_count"])
    predictive = []
    for pattern_index, pattern_id in enumerate(config["pattern_ids"]):
        for type_index, type_id in enumerate(config["types"]):
            selected = (
                (config["node_patterns"][config["node_index"]] == pattern_index)
                & (config["type_index"] == type_index)
            )
            observed_nodes = config["observed"][selected]
            replicated_nodes = replicated[:, selected]
            observed_total = int(observed_nodes.sum())
            replicated_totals = replicated_nodes.sum(axis=1)
            observed_variance = float(np.var(observed_nodes))
            replicated_variances = np.var(replicated_nodes, axis=1)
            predictive.append(
                {
                    "pattern_id": pattern_id,
                    "type_id": type_id,
                    "observed_total_count": observed_total,
                    "replicate_count": int(len(replicated_totals)),
                    "replicated_total_count_mean": float(replicated_totals.mean()),
                    "replicated_total_count_sd": float(replicated_totals.std(ddof=1)),
                    "replicated_total_count_interval_lower": float(np.quantile(replicated_totals, 0.025)),
                    "replicated_total_count_interval_upper": float(np.quantile(replicated_totals, 0.975)),
                    "total_count_two_sided_tail_probability": two_sided_tail(replicated_totals, observed_total),
                    "observed_node_count_variance": observed_variance,
                    "replicated_node_count_variance_mean": float(replicated_variances.mean()),
                    "node_variance_two_sided_tail_probability": two_sided_tail(replicated_variances, observed_variance),
                }
            )
    constraints_valid = bool(
        all(
            np.max(np.abs(flat["pattern_effect"][:, config["pattern_patients"] == patient, :].sum(axis=1))) < 1e-10
            for patient in range(patients)
        )
        and all(
            np.max(np.abs(flat["latent_effect"][:, :, config["node_patterns"] == pattern].sum(axis=2))) < 1e-10
            for pattern in range(patterns)
        )
    )
    posterior_finite = bool(
        all(np.isfinite(value).all() for value in flat.values())
        and all(np.isfinite(value).all() for value in (replicated,))
    )
    policy = config["policy"]
    complete = bool(
        prior_finite
        and posterior_finite
        and constraints_valid
        and r_hat <= policy["maximum_r_hat"]
        and bulk >= policy["minimum_bulk_ess"]
        and tail >= policy["minimum_tail_ess"]
        and ebfmi >= policy["minimum_ebfmi"]
        and divergences <= policy["maximum_divergences"]
        and depth_hits <= policy["maximum_tree_depth_hits"]
    )
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": {
            "name": "pymc", "version": pm.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha, "worker_sha256": worker_sha,
        },
        "input_sha256": config["input_sha"],
        "request_sha256": request_sha,
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"], "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"],
            "completed_draws": config["chains"] * config["draws"],
        },
        "type_posteriors": type_posteriors,
        "group_effect_differences": group_differences,
        "patient_type_effects": patient_type_effects,
        "pattern_type_effects": pattern_type_effects,
        "node_type_posteriors": node_type_posteriors,
        "pattern_type_posterior_predictive": predictive,
        "diagnostics": {
            "prior_predictive_finite": prior_finite,
            "posterior_finite": posterior_finite,
            "r_hat": r_hat, "ess_bulk": bulk, "ess_tail": tail,
            "mcse_mean": mcse_mean, "mcse_sd": mcse_sd,
            "minimum_ebfmi": ebfmi, "divergences": divergences,
            "max_tree_depth_hits": depth_hits,
            "constraints_valid": constraints_valid,
            "identifiability_checks_passed": constraints_valid,
        },
    }


def main() -> None:
    if pm.__version__ != PYMC_VERSION or sys.version_info[:2] != (3, 12):
        raise ContractError("PyMC or Python version drift")
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    raw = sys.stdin.buffer.read(64 * 1_048_576 + 1)
    if not raw or len(raw) > 64 * 1_048_576:
        raise ContractError("request size is invalid")
    request_sha = hashlib.sha256(raw).hexdigest()
    result = fit(validate(json.loads(raw), lock_sha, worker_sha), request_sha, lock_sha, worker_sha)
    output = json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")) + "\n"
    if len(output.encode()) > 16 * 1_048_576:
        raise ContractError("result exceeds output ceiling")
    sys.stdout.write(output)


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(
            f"marklab replicated arbitrary-window multitype LGCP failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
