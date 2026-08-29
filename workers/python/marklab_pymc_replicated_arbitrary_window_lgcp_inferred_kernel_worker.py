#!/usr/bin/env python3
"""Pinned PyMC replicated exact-window LGCP with shared inferred Matérn scales."""

from __future__ import annotations

import hashlib
import importlib.util
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
REQUEST_FORMAT = "marklab.pymc_replicated_arbitrary_window_lgcp_inferred_kernel_request"
RESULT_FORMAT = "marklab.bayesian_replicated_arbitrary_window_lgcp_inferred_kernel_fit"


class ContractError(ValueError):
    pass


def load_source_contract() -> Any:
    path = Path(__file__).with_name(
        "marklab_pymc_replicated_arbitrary_window_lgcp_worker.py"
    )
    specification = importlib.util.spec_from_file_location(
        "marklab_replicated_lgcp_source_contract", path
    )
    if specification is None or specification.loader is None:
        raise ContractError("cannot load replicated LGCP source contract")
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


def finite(value: Any, path: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ContractError(f"{path} must be numeric")
    result = float(value)
    if not math.isfinite(result):
        raise ContractError(f"{path} must be finite")
    return result


def validate(
    request: Any, lock_digest: str, worker_digest: str, source_worker_digest: str
) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format", "version", "backend", "source_backend", "source_request_sha256",
            "source_request", "kernel_priors", "resources",
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
    source_backend = obj(
        request["source_backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "source_backend",
    )
    exact(source_backend, request["source_request"]["backend"], "source backend")
    source_sha = request["source_request_sha256"]
    if (
        not isinstance(source_sha, str)
        or len(source_sha) != 64
        or any(character not in "0123456789abcdef" for character in source_sha)
    ):
        raise ContractError("source request digest is invalid")
    config = load_source_contract().validate(
        request["source_request"], lock_digest, source_worker_digest
    )
    priors = obj(
        request["kernel_priors"],
        {"field_amplitude_scale", "field_length_scale_scale_um", "jitter"},
        "kernel_priors",
    )
    amplitude_scale = finite(priors["field_amplitude_scale"], "kernel amplitude scale")
    length_scale = finite(priors["field_length_scale_scale_um"], "kernel length scale")
    jitter = finite(priors["jitter"], "kernel jitter")
    if min(amplitude_scale, length_scale, jitter) <= 0.0:
        raise ContractError("kernel priors must be positive")
    exact(config["priors"]["field_amplitude"], amplitude_scale, "source amplitude reference")
    exact(
        config["priors"]["field_length_scale_um"], length_scale,
        "source length-scale reference",
    )
    exact(config["priors"]["jitter"], jitter, "source jitter")
    resources = obj(
        request["resources"],
        {
            "maximum_kernel_cube_work", "kernel_cube_work", "maximum_tree_depth",
            "timeout_seconds",
        },
        "resources",
    )
    pattern_sizes = np.bincount(
        config["pattern_index"], minlength=len(config["pattern_ids"])
    )
    cube_work = int(np.sum(pattern_sizes.astype(object) ** 3))
    exact(int(resources["kernel_cube_work"]), cube_work, "kernel cube work")
    if cube_work > int(resources["maximum_kernel_cube_work"]):
        raise ContractError("kernel cube work exceeds maximum")
    maximum_tree_depth = resources["maximum_tree_depth"]
    if (
        isinstance(maximum_tree_depth, bool)
        or not isinstance(maximum_tree_depth, int)
        or not 10 <= maximum_tree_depth <= 14
    ):
        raise ContractError("maximum tree depth is outside [10, 14]")
    config.update(
        {
            "outer_request": request,
            "field_amplitude_scale": amplitude_scale,
            "field_length_scale_scale_um": length_scale,
            "kernel_jitter": jitter,
            "kernel_cube_work": cube_work,
            "maximum_tree_depth": maximum_tree_depth,
        }
    )
    return config


def seed_for(seed: int, purpose: str) -> int:
    digest = hashlib.sha256(
        f"marklab-replicated-lgcp-inferred-kernel-v1\0{seed}\0{purpose}".encode()
    ).digest()
    return int.from_bytes(digest[:4], "little")


def summary(values: np.ndarray) -> dict[str, float]:
    values = np.asarray(values, dtype=np.float64).reshape(-1)
    return {
        "mean": float(values.mean()),
        "sd": float(values.std(ddof=1)),
        "interval_lower": float(np.quantile(values, 0.025)),
        "interval_upper": float(np.quantile(values, 0.975)),
    }


def flattened(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def two_sided_tail(replicated: np.ndarray, observed: float) -> float:
    lower = float(np.mean(replicated <= observed))
    upper = float(np.mean(replicated >= observed))
    return min(1.0, 2.0 * min(lower, upper))


def prior_is_finite(config: dict[str, Any]) -> bool:
    rng = np.random.default_rng(seed_for(config["seed"], "prior"))
    draws = int(config["policy"]["prior_predictive_draws"])
    priors = config["priors"]
    for _ in range(draws):
        amplitude = abs(rng.normal(0.0, config["field_amplitude_scale"]))
        length = abs(rng.normal(0.0, config["field_length_scale_scale_um"]))
        if amplitude <= 0.0 or length <= 0.0:
            return False
        latent = np.empty(len(config["counts"]), dtype=np.float64)
        for pattern in range(len(config["pattern_ids"])):
            selected = np.flatnonzero(config["pattern_index"] == pattern)
            nodes = [config["request"]["nodes"][index] for index in selected]
            coordinates = np.asarray([[row["x_um"], row["y_um"]] for row in nodes])
            distance = np.linalg.norm(coordinates[:, None, :] - coordinates[None, :, :], axis=2)
            scaled = math.sqrt(3.0) * distance / length
            covariance = amplitude**2 * (1.0 + scaled) * np.exp(-scaled)
            covariance += config["kernel_jitter"] * np.eye(len(selected))
            block = np.linalg.cholesky(covariance) @ rng.normal(0.0, 1.0, len(selected))
            latent[selected] = block - block.mean()
        if not np.isfinite(latent).all():
            return False
    return True


def fit(
    config: dict[str, Any], request_sha256: str, lock_digest: str, worker_digest: str
) -> dict[str, Any]:
    priors = config["priors"]
    with pm.Model():
        intercept = pm.Normal("intercept", priors["intercept_mean"], priors["intercept_sd"])
        group_effect = pm.Normal("group_effect", 0.0, priors["group_effect_sd"])
        covariate_effect = pm.Normal("covariate_effect", 0.0, priors["covariate_effect_sd"])
        patient_sd = pm.HalfNormal("patient_sd", priors["patient_sd_scale"])
        pattern_sd = pm.HalfNormal("pattern_sd", priors["pattern_sd_scale"])
        field_amplitude = pm.HalfNormal("field_amplitude", config["field_amplitude_scale"])
        field_length_scale_um = pm.HalfNormal(
            "field_length_scale_um", config["field_length_scale_scale_um"]
        )
        patient_raw = pm.Normal("patient_raw", 0.0, 1.0, shape=len(config["patient_ids"]))
        pattern_raw = pm.Normal("pattern_raw", 0.0, 1.0, shape=len(config["pattern_ids"]))
        field_raw = pm.Normal("field_raw", 0.0, 1.0, shape=len(config["counts"]))
        patient_effect = pm.Deterministic("patient_effect", patient_sd * patient_raw)
        pattern_patient_mean = pt.stack([
            pt.mean(pattern_raw[np.flatnonzero(config["pattern_patient_index"] == patient)])
            for patient in range(len(config["patient_ids"]))
        ])
        pattern_effect = pm.Deterministic(
            "pattern_effect",
            pattern_sd * (pattern_raw - pattern_patient_mean[config["pattern_patient_index"]]),
        )
        latent_blocks = []
        for pattern in range(len(config["pattern_ids"])):
            selected = np.flatnonzero(config["pattern_index"] == pattern)
            nodes = [config["request"]["nodes"][index] for index in selected]
            coordinates = np.asarray([[row["x_um"], row["y_um"]] for row in nodes])
            distance = np.linalg.norm(coordinates[:, None, :] - coordinates[None, :, :], axis=2)
            scaled = math.sqrt(3.0) * distance / field_length_scale_um
            covariance = field_amplitude**2 * (1.0 + scaled) * pt.exp(-scaled)
            covariance += config["kernel_jitter"] * np.eye(len(selected))
            block = pt.linalg.cholesky(covariance) @ field_raw[selected]
            latent_blocks.append(block - pt.mean(block))
        latent_effect = pm.Deterministic("latent_effect", pt.concatenate(latent_blocks))
        expected_count = pm.Deterministic(
            "expected_count",
            config["weight"]
            * pt.exp(
                intercept
                + group_effect * config["group"][config["patient_index"]]
                + covariate_effect * config["covariate"]
                + patient_effect[config["patient_index"]]
                + pattern_effect[config["pattern_index"]]
                + latent_effect
            ),
        )
        pm.Poisson("observed_count", expected_count, observed=config["counts"])
        trace = pm.sample(
            draws=config["draws"], tune=config["tune"], chains=config["chains"], cores=1,
            target_accept=config["target_accept"], random_seed=config["seed"], progressbar=False,
            compute_convergence_checks=False, return_inferencedata=True,
            nuts={"max_treedepth": config["maximum_tree_depth"]},
        )
    names = [
        "intercept", "group_effect", "covariate_effect", "patient_sd", "pattern_sd",
        "field_amplitude", "field_length_scale_um", "patient_raw", "pattern_raw", "field_raw",
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
    expected = np.asarray(posterior["expected_count"], dtype=np.float64).reshape(
        -1, len(config["counts"])
    )
    latent = np.asarray(posterior["latent_effect"], dtype=np.float64).reshape(
        -1, len(config["counts"])
    )
    replicated = np.random.default_rng(seed_for(config["seed"], "predictive")).poisson(expected)
    predictive = []
    for index, pattern_id in enumerate(config["pattern_ids"]):
        selected = config["pattern_index"] == index
        observed_nodes = config["counts"][selected]
        replicated_nodes = replicated[:, selected]
        observed_total = int(observed_nodes.sum())
        replicated_totals = replicated_nodes.sum(axis=1)
        observed_variance = float(np.var(observed_nodes))
        replicated_variances = np.var(replicated_nodes, axis=1)
        predictive.append({
            "pattern_id": pattern_id,
            "observed_total_count": observed_total,
            "replicate_count": int(replicated_totals.size),
            "replicated_total_count_mean": float(replicated_totals.mean()),
            "replicated_total_count_sd": float(replicated_totals.std(ddof=1)),
            "replicated_total_count_interval_lower": float(np.quantile(replicated_totals, 0.025)),
            "replicated_total_count_interval_upper": float(np.quantile(replicated_totals, 0.975)),
            "total_count_two_sided_tail_probability": two_sided_tail(replicated_totals, observed_total),
            "observed_node_count_variance": observed_variance,
            "replicated_node_count_variance_mean": float(replicated_variances.mean()),
            "node_variance_two_sided_tail_probability": two_sided_tail(
                replicated_variances, observed_variance
            ),
        })
    policy = config["policy"]
    prior_finite = prior_is_finite(config)
    complete = (
        prior_finite and r_hat <= policy["maximum_r_hat"]
        and bulk >= policy["minimum_bulk_ess"] and tail >= policy["minimum_tail_ess"]
        and ebfmi >= policy["minimum_ebfmi"] and divergences <= policy["maximum_divergences"]
        and depth_hits <= policy["maximum_tree_depth_hits"]
    )
    return {
        "format": RESULT_FORMAT, "version": 1,
        "backend": {
            "name": "pymc", "version": pm.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_digest, "worker_sha256": worker_digest,
        },
        "source_backend": config["request"]["backend"],
        "input_sha256": config["request"]["input_sha256"],
        "request_sha256": request_sha256,
        "source_request_sha256": config["outer_request"]["source_request_sha256"],
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"], "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"],
            "completed_draws": config["chains"] * config["draws"],
        },
        "posterior": {
            name: summary(np.asarray(posterior[name]))
            for name in (
                "intercept", "group_effect", "covariate_effect", "patient_sd", "pattern_sd",
                "field_amplitude", "field_length_scale_um",
            )
        },
        "patient_effects": [
            {"patient_id": identity, "effect": summary(np.asarray(posterior["patient_effect"])[..., index])}
            for index, identity in enumerate(config["patient_ids"])
        ],
        "pattern_effects": [
            {"pattern_id": identity, "effect": summary(np.asarray(posterior["pattern_effect"])[..., index])}
            for index, identity in enumerate(config["pattern_ids"])
        ],
        "nodes": [
            {
                "pattern_id": config["request"]["nodes"][index]["pattern_id"],
                "node_id": config["request"]["nodes"][index]["node_id"],
                "latent_effect": summary(latent[:, index]),
                "expected_count": summary(expected[:, index]),
            }
            for index in range(len(config["counts"]))
        ],
        "pattern_posterior_predictive": predictive,
        "diagnostics": {
            "prior_predictive_finite": prior_finite, "posterior_finite": True,
            "r_hat": r_hat, "ess_bulk": bulk, "ess_tail": tail,
            "mcse_mean": mcse_mean, "mcse_sd": mcse_sd, "minimum_ebfmi": ebfmi,
            "divergences": divergences, "max_tree_depth_hits": depth_hits,
            "constraints_valid": True, "identifiability_checks_passed": True,
        },
        "kernel_cube_work": config["kernel_cube_work"],
    }


def main() -> None:
    if pm.__version__ != PYMC_VERSION or sys.version_info[:2] != (3, 12):
        raise ContractError("PyMC or Python version drift")
    script = Path(__file__)
    lock_digest = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_digest = hashlib.sha256(script.read_bytes()).hexdigest()
    source_worker_digest = hashlib.sha256(
        script.with_name("marklab_pymc_replicated_arbitrary_window_lgcp_worker.py").read_bytes()
    ).hexdigest()
    raw = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if not raw or len(raw) > 16 * 1024 * 1024:
        raise ContractError("request size is invalid")
    request_sha256 = hashlib.sha256(raw).hexdigest()
    result = fit(
        validate(json.loads(raw), lock_digest, worker_digest, source_worker_digest),
        request_sha256, lock_digest, worker_digest,
    )
    sys.stdout.write(json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")))
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(
            f"marklab inferred-kernel replicated LGCP failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
