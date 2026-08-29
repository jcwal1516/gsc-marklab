#!/usr/bin/env python3
"""Pinned PyMC fit for independent exact-window patterns nested in patients."""

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
REQUEST_FORMAT = "marklab.pymc_replicated_arbitrary_window_lgcp_request"
RESULT_FORMAT = "marklab.bayesian_replicated_arbitrary_window_lgcp_fit"


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


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format", "version", "backend", "input_sha256", "reference_group",
            "comparison_group", "patients", "patterns", "nodes", "cholesky",
            "priors", "sampling", "diagnostic_policy", "resources",
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
    exact(backend["environment_lock_sha256"], lock_digest, "backend.lock")
    exact(backend["worker_sha256"], worker_digest, "backend.worker")
    patients = request["patients"]
    patterns = request["patterns"]
    nodes = request["nodes"]
    if not isinstance(patients, list) or not isinstance(patterns, list) or not isinstance(nodes, list):
        raise ContractError("patient, pattern, or node rows are invalid")
    patient_ids = [row["patient_id"] for row in patients]
    pattern_ids = [row["pattern_id"] for row in patterns]
    if patient_ids != sorted(set(patient_ids)) or pattern_ids != sorted(set(pattern_ids)):
        raise ContractError("patient or pattern identities are not canonical")
    patient_index = np.asarray([int(row["patient_index"]) for row in nodes], dtype=np.int64)
    pattern_index = np.asarray([int(row["pattern_index"]) for row in nodes], dtype=np.int64)
    covariate = np.asarray([finite(row["covariate"], "node.covariate") for row in nodes])
    weight = np.asarray([finite(row["weight_um2"], "node.weight") for row in nodes])
    counts = np.asarray([int(row["count"]) for row in nodes], dtype=np.int64)
    if (
        np.any(patient_index < 0)
        or np.any(patient_index >= len(patients))
        or np.any(pattern_index < 0)
        or np.any(pattern_index >= len(patterns))
        or np.any(weight <= 0.0)
        or np.any(counts < 0)
    ):
        raise ContractError("node indices, weights, or counts are invalid")
    group = np.asarray([int(row["group_index"]) for row in patients], dtype=np.float64)
    if not set(group.tolist()) == {0.0, 1.0}:
        raise ContractError("both exact groups are required")
    dimension = len(nodes)
    cholesky = np.asarray([finite(value, "cholesky[]") for value in request["cholesky"]]).reshape(
        dimension, dimension
    )
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
    sampling = obj(
        request["sampling"],
        {"chains", "tune_per_chain", "draws_per_chain", "target_accept", "seed"},
        "sampling",
    )
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
            "maximum_patients", "maximum_patterns", "maximum_nodes_per_pattern",
            "maximum_total_nodes", "maximum_total_events", "maximum_draw_node_work",
            "draw_node_work", "timeout_seconds",
        },
        "resources",
    )
    chains = int(sampling["chains"])
    tune = int(sampling["tune_per_chain"])
    draws = int(sampling["draws_per_chain"])
    actual_work = chains * draws * dimension
    if (
        len(patients) > int(resources["maximum_patients"])
        or len(patterns) > int(resources["maximum_patterns"])
        or dimension > int(resources["maximum_total_nodes"])
        or int(counts.sum()) > int(resources["maximum_total_events"])
        or actual_work != int(resources["draw_node_work"])
        or actual_work > int(resources["maximum_draw_node_work"])
    ):
        raise ContractError("replicated LGCP resource identity or ceiling differs")
    return {
        "request": request,
        "patient_ids": patient_ids,
        "pattern_ids": pattern_ids,
        "pattern_patient_index": np.asarray(
            [int(row["patient_index"]) for row in patterns], dtype=np.int64
        ),
        "patient_index": patient_index,
        "pattern_index": pattern_index,
        "group": group,
        "covariate": covariate,
        "weight": weight,
        "counts": counts,
        "cholesky": cholesky,
        "priors": {name: finite(value, f"priors.{name}") for name, value in priors.items()},
        "chains": chains,
        "tune": tune,
        "draws": draws,
        "target_accept": finite(sampling["target_accept"], "sampling.target_accept"),
        "seed": int(sampling["seed"]),
        "policy": policy,
    }


def summary(values: np.ndarray) -> dict[str, float]:
    values = np.asarray(values, dtype=np.float64).reshape(-1)
    return {
        "mean": float(values.mean()),
        "sd": float(values.std(ddof=1)),
        "interval_lower": float(np.quantile(values, 0.025)),
        "interval_upper": float(np.quantile(values, 0.975)),
    }


def two_sided_tail(replicated: np.ndarray, observed: float) -> float:
    lower = float(np.mean(replicated <= observed))
    upper = float(np.mean(replicated >= observed))
    return min(1.0, 2.0 * min(lower, upper))


def flattened(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def prior_is_finite(config: dict[str, Any]) -> bool:
    seed = int.from_bytes(
        hashlib.sha256(
            f"marklab-replicated-lgcp-prior-v1\0{config['seed']}".encode()
        ).digest()[:8],
        "little",
    )
    rng = np.random.default_rng(seed)
    draws = int(config["policy"]["prior_predictive_draws"])
    priors = config["priors"]
    intercept = rng.normal(priors["intercept_mean"], priors["intercept_sd"], draws)
    group_effect = rng.normal(0.0, priors["group_effect_sd"], draws)
    covariate_effect = rng.normal(0.0, priors["covariate_effect_sd"], draws)
    patient_sd = np.abs(rng.normal(0.0, priors["patient_sd_scale"], draws))
    pattern_sd = np.abs(rng.normal(0.0, priors["pattern_sd_scale"], draws))
    patient_effect = patient_sd[:, None] * rng.normal(
        0.0, 1.0, (draws, len(config["patient_ids"]))
    )
    pattern_raw = rng.normal(0.0, 1.0, (draws, len(config["pattern_ids"])))
    for patient in range(len(config["patient_ids"])):
        selected = config["pattern_patient_index"] == patient
        pattern_raw[:, selected] -= pattern_raw[:, selected].mean(axis=1, keepdims=True)
    pattern_effect = pattern_sd[:, None] * pattern_raw
    latent = rng.normal(0.0, 1.0, (draws, len(config["counts"]))) @ config["cholesky"].T
    for pattern in range(len(config["pattern_ids"])):
        selected = config["pattern_index"] == pattern
        latent[:, selected] -= latent[:, selected].mean(axis=1, keepdims=True)
    with np.errstate(over="ignore", invalid="ignore"):
        expected = config["weight"] * np.exp(
            intercept[:, None]
            + group_effect[:, None] * config["group"][config["patient_index"]]
            + covariate_effect[:, None] * config["covariate"]
            + patient_effect[:, config["patient_index"]]
            + pattern_effect[:, config["pattern_index"]]
            + latent
        )
    return bool(np.isfinite(expected).all() and np.all(expected > 0.0))


def fit(config: dict[str, Any], request_sha256: str, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    priors = config["priors"]
    prior_finite = prior_is_finite(config)
    with pm.Model() as model:
        intercept = pm.Normal("intercept", mu=priors["intercept_mean"], sigma=priors["intercept_sd"])
        group_effect = pm.Normal("group_effect", mu=0.0, sigma=priors["group_effect_sd"])
        covariate_effect = pm.Normal(
            "covariate_effect", mu=0.0, sigma=priors["covariate_effect_sd"]
        )
        patient_sd = pm.HalfNormal("patient_sd", sigma=priors["patient_sd_scale"])
        pattern_sd = pm.HalfNormal("pattern_sd", sigma=priors["pattern_sd_scale"])
        patient_raw = pm.Normal("patient_raw", 0.0, 1.0, shape=len(config["patient_ids"]))
        pattern_raw = pm.Normal("pattern_raw", 0.0, 1.0, shape=len(config["pattern_ids"]))
        field_raw = pm.Normal("field_raw", 0.0, 1.0, shape=len(config["counts"]))
        patient_effect = pm.Deterministic("patient_effect", patient_sd * patient_raw)
        pattern_patient_mean = pt.stack(
            [
                pt.mean(
                    pattern_raw[np.flatnonzero(config["pattern_patient_index"] == patient)]
                )
                for patient in range(len(config["patient_ids"]))
            ]
        )
        pattern_effect = pm.Deterministic(
            "pattern_effect",
            pattern_sd
            * (
                pattern_raw
                - pattern_patient_mean[config["pattern_patient_index"]]
            ),
        )
        latent_uncentered = config["cholesky"] @ field_raw
        latent_pattern_mean = pt.stack(
            [
                pt.mean(
                    latent_uncentered[np.flatnonzero(config["pattern_index"] == pattern)]
                )
                for pattern in range(len(config["pattern_ids"]))
            ]
        )
        latent_effect = pm.Deterministic(
            "latent_effect",
            latent_uncentered - latent_pattern_mean[config["pattern_index"]],
        )
        log_expected = (
            intercept
            + group_effect * config["group"][config["patient_index"]]
            + covariate_effect * config["covariate"]
            + patient_effect[config["patient_index"]]
            + pattern_effect[config["pattern_index"]]
            + latent_effect
            + np.log(config["weight"])
        )
        expected_count = pm.Deterministic("expected_count", pm.math.exp(log_expected))
        pm.Poisson("observed_count", mu=expected_count, observed=config["counts"])
        trace = pm.sample(
            draws=config["draws"],
            tune=config["tune"],
            chains=config["chains"],
            cores=1,
            target_accept=config["target_accept"],
            random_seed=config["seed"],
            progressbar=False,
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
    stats = trace.sample_stats
    energy = np.asarray(stats["energy"], dtype=np.float64)
    ebfmi = float(np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1)))
    divergences = int(np.asarray(stats["diverging"]).sum())
    depth_hits = int(np.asarray(stats["reached_max_treedepth"]).sum())
    posterior = trace.posterior
    expected_draws = np.asarray(posterior["expected_count"], dtype=np.float64).reshape(
        -1, len(config["counts"])
    )
    predictive_seed = int.from_bytes(
        hashlib.sha256(
            f"marklab-replicated-lgcp-posterior-predictive-v1\0{config['seed']}".encode()
        ).digest()[:8],
        "little",
    )
    replicated_counts = np.random.default_rng(predictive_seed).poisson(expected_draws)
    pattern_posterior_predictive = []
    for index, pattern_id in enumerate(config["pattern_ids"]):
        selected = config["pattern_index"] == index
        observed_nodes = config["counts"][selected]
        replicated_nodes = replicated_counts[:, selected]
        observed_total = int(observed_nodes.sum())
        replicated_totals = replicated_nodes.sum(axis=1)
        observed_variance = float(np.var(observed_nodes))
        replicated_variances = np.var(replicated_nodes, axis=1)
        pattern_posterior_predictive.append(
            {
                "pattern_id": pattern_id,
                "observed_total_count": observed_total,
                "replicate_count": int(replicated_totals.size),
                "replicated_total_count_mean": float(replicated_totals.mean()),
                "replicated_total_count_sd": float(replicated_totals.std(ddof=1)),
                "replicated_total_count_interval_lower": float(
                    np.quantile(replicated_totals, 0.025)
                ),
                "replicated_total_count_interval_upper": float(
                    np.quantile(replicated_totals, 0.975)
                ),
                "total_count_two_sided_tail_probability": two_sided_tail(
                    replicated_totals, observed_total
                ),
                "observed_node_count_variance": observed_variance,
                "replicated_node_count_variance_mean": float(
                    replicated_variances.mean()
                ),
                "node_variance_two_sided_tail_probability": two_sided_tail(
                    replicated_variances, observed_variance
                ),
            }
        )
    policy = config["policy"]
    complete = (
        prior_finite
        and r_hat <= policy["maximum_r_hat"]
        and bulk >= policy["minimum_bulk_ess"]
        and tail >= policy["minimum_tail_ess"]
        and ebfmi >= policy["minimum_ebfmi"]
        and divergences <= policy["maximum_divergences"]
        and depth_hits <= policy["maximum_tree_depth_hits"]
    )
    return {
        "format": RESULT_FORMAT,
        "version": 2,
        "backend": {
            "name": "pymc", "version": pm.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_digest, "worker_sha256": worker_digest,
        },
        "input_sha256": config["request"]["input_sha256"],
        "request_sha256": request_sha256,
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"],
            "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"],
            "completed_draws": config["chains"] * config["draws"],
        },
        "posterior": {
            name: summary(np.asarray(posterior[name]))
            for name in ("intercept", "group_effect", "covariate_effect", "patient_sd", "pattern_sd")
        },
        "patient_effects": [
            {"patient_id": patient_id, "effect": summary(np.asarray(posterior["patient_effect"])[..., index])}
            for index, patient_id in enumerate(config["patient_ids"])
        ],
        "pattern_effects": [
            {"pattern_id": pattern_id, "effect": summary(np.asarray(posterior["pattern_effect"])[..., index])}
            for index, pattern_id in enumerate(config["pattern_ids"])
        ],
        "nodes": [
            {
                "pattern_id": config["request"]["nodes"][index]["pattern_id"],
                "node_id": config["request"]["nodes"][index]["node_id"],
                "latent_effect": summary(np.asarray(posterior["latent_effect"])[..., index]),
                "expected_count": summary(expected_draws[:, index]),
            }
            for index in range(len(config["counts"]))
        ],
        "pattern_posterior_predictive": pattern_posterior_predictive,
        "diagnostics": {
            "prior_predictive_finite": prior_finite, "posterior_finite": True,
            "r_hat": r_hat, "ess_bulk": bulk, "ess_tail": tail,
            "mcse_mean": mcse_mean, "mcse_sd": mcse_sd, "minimum_ebfmi": ebfmi,
            "divergences": divergences, "max_tree_depth_hits": depth_hits,
            "constraints_valid": True, "identifiability_checks_passed": True,
        },
    }


def main() -> None:
    if pm.__version__ != PYMC_VERSION or sys.version_info[:2] != (3, 12):
        raise ContractError("PyMC or Python version drift")
    script = Path(__file__)
    lock_digest = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_digest = hashlib.sha256(script.read_bytes()).hexdigest()
    raw = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if not raw or len(raw) > 16 * 1024 * 1024:
        raise ContractError("request size is invalid")
    request_sha256 = hashlib.sha256(raw).hexdigest()
    result = fit(validate(json.loads(raw), lock_digest, worker_digest), request_sha256, lock_digest, worker_digest)
    sys.stdout.write(json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")))
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"marklab replicated LGCP failed: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from error
