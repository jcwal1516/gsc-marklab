#!/usr/bin/env python3
"""Pinned NumPyro agreement worker for the replicated exact-window patient LGCP."""

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
import jax.numpy as jnp
import numpy as np
import numpyro
import numpyro.distributions as dist
from numpyro.infer import MCMC, NUTS


REQUEST_FORMAT = "marklab.numpyro_replicated_arbitrary_window_lgcp_request"
RESULT_FORMAT = "marklab.numpyro_replicated_arbitrary_window_lgcp_result"
NUMPYRO_VERSION = "0.21.0"
JAX_VERSION = "0.11.1"


class ContractError(ValueError):
    pass


def load_pymc_contract() -> Any:
    path = Path(__file__).with_name(
        "marklab_pymc_replicated_arbitrary_window_lgcp_worker.py"
    )
    specification = importlib.util.spec_from_file_location(
        "marklab_pymc_replicated_lgcp_contract", path
    )
    if specification is None or specification.loader is None:
        raise ContractError("cannot load the PyMC replicated LGCP contract")
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


def validate(
    request: Any, lock_digest: str, worker_digest: str, pymc_worker_digest: str
) -> tuple[dict[str, Any], dict[str, Any]]:
    request = obj(
        request,
        {
            "format", "version", "backend", "jax_version",
            "source_request_sha256", "source_request", "maximum_tree_depth",
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
    exact(backend["environment_lock_sha256"], lock_digest, "backend.lock")
    exact(backend["worker_sha256"], worker_digest, "backend.worker")
    exact(request["jax_version"], JAX_VERSION, "request.jax_version")
    source_sha = request["source_request_sha256"]
    if (
        not isinstance(source_sha, str)
        or len(source_sha) != 64
        or any(character not in "0123456789abcdef" for character in source_sha)
    ):
        raise ContractError("source request digest is invalid")
    config = load_pymc_contract().validate(
        request["source_request"], lock_digest, pymc_worker_digest
    )
    maximum_tree_depth = request["maximum_tree_depth"]
    if (
        isinstance(maximum_tree_depth, bool)
        or not isinstance(maximum_tree_depth, int)
        or not 10 <= maximum_tree_depth <= 14
    ):
        raise ContractError("maximum_tree_depth is outside [10, 14]")
    config["numpyro_maximum_tree_depth"] = maximum_tree_depth
    return request, config


def seed_for(seed: int, purpose: str) -> int:
    digest = hashlib.sha256(
        f"marklab-numpyro-replicated-lgcp-v1\0{seed}\0{purpose}".encode()
    ).digest()
    return int.from_bytes(digest[:4], "little")


def model(
    patient_index: jnp.ndarray,
    pattern_index: jnp.ndarray,
    pattern_patient_index: jnp.ndarray,
    group: jnp.ndarray,
    covariate: jnp.ndarray,
    weight: jnp.ndarray,
    counts: jnp.ndarray,
    cholesky: jnp.ndarray,
    priors: dict[str, float],
) -> None:
    patient_count = group.shape[0]
    pattern_count = pattern_patient_index.shape[0]
    node_count = counts.shape[0]
    intercept = numpyro.sample(
        "intercept", dist.Normal(priors["intercept_mean"], priors["intercept_sd"])
    )
    group_effect = numpyro.sample(
        "group_effect", dist.Normal(0.0, priors["group_effect_sd"])
    )
    covariate_effect = numpyro.sample(
        "covariate_effect", dist.Normal(0.0, priors["covariate_effect_sd"])
    )
    patient_sd = numpyro.sample(
        "patient_sd", dist.HalfNormal(priors["patient_sd_scale"])
    )
    pattern_sd = numpyro.sample(
        "pattern_sd", dist.HalfNormal(priors["pattern_sd_scale"])
    )
    patient_raw = numpyro.sample(
        "patient_raw", dist.Normal(0.0, 1.0).expand([patient_count]).to_event(1)
    )
    pattern_raw = numpyro.sample(
        "pattern_raw", dist.Normal(0.0, 1.0).expand([pattern_count]).to_event(1)
    )
    field_raw = numpyro.sample(
        "field_raw", dist.Normal(0.0, 1.0).expand([node_count]).to_event(1)
    )
    patient_effect = numpyro.deterministic("patient_effect", patient_sd * patient_raw)
    pattern_sums = jnp.zeros(patient_count).at[pattern_patient_index].add(pattern_raw)
    patterns_per_patient = jnp.zeros(patient_count).at[pattern_patient_index].add(1.0)
    pattern_means = pattern_sums / patterns_per_patient
    pattern_effect = numpyro.deterministic(
        "pattern_effect",
        pattern_sd * (pattern_raw - pattern_means[pattern_patient_index]),
    )
    latent_uncentered = cholesky @ field_raw
    latent_sums = jnp.zeros(pattern_count).at[pattern_index].add(latent_uncentered)
    nodes_per_pattern = jnp.zeros(pattern_count).at[pattern_index].add(1.0)
    latent_means = latent_sums / nodes_per_pattern
    latent_effect = numpyro.deterministic(
        "latent_effect", latent_uncentered - latent_means[pattern_index]
    )
    expected_count = numpyro.deterministic(
        "expected_count",
        weight
        * jnp.exp(
            intercept
            + group_effect * group[patient_index]
            + covariate_effect * covariate
            + patient_effect[patient_index]
            + pattern_effect[pattern_index]
            + latent_effect
        ),
    )
    numpyro.sample("observed_count", dist.Poisson(expected_count), obs=counts)


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


def fit(
    config: dict[str, Any],
    request_sha256: str,
    source_request_sha256: str,
    lock_digest: str,
    worker_digest: str,
) -> dict[str, Any]:
    kernel = NUTS(
        model,
        target_accept_prob=config["target_accept"],
        max_tree_depth=config["numpyro_maximum_tree_depth"],
    )
    sampler = MCMC(
        kernel,
        num_warmup=config["tune"],
        num_samples=config["draws"],
        num_chains=config["chains"],
        chain_method="sequential",
        progress_bar=False,
    )
    sampler.run(
        jax.random.PRNGKey(seed_for(config["seed"], "nuts")),
        patient_index=jnp.asarray(config["patient_index"]),
        pattern_index=jnp.asarray(config["pattern_index"]),
        pattern_patient_index=jnp.asarray(config["pattern_patient_index"]),
        group=jnp.asarray(config["group"]),
        covariate=jnp.asarray(config["covariate"]),
        weight=jnp.asarray(config["weight"]),
        counts=jnp.asarray(config["counts"]),
        cholesky=jnp.asarray(config["cholesky"]),
        priors=config["priors"],
        extra_fields=("diverging", "num_steps", "energy"),
    )
    samples = {
        name: np.asarray(value, dtype=np.float64)
        for name, value in sampler.get_samples(group_by_chain=True).items()
    }
    patient_effect = samples["patient_sd"][..., None] * samples["patient_raw"]
    pattern_centered = samples["pattern_raw"].copy()
    for patient in range(len(config["patient_ids"])):
        selected = config["pattern_patient_index"] == patient
        pattern_centered[..., selected] -= pattern_centered[..., selected].mean(
            axis=-1, keepdims=True
        )
    pattern_effect = samples["pattern_sd"][..., None] * pattern_centered
    latent = np.einsum("ij,cdj->cdi", config["cholesky"], samples["field_raw"])
    for pattern in range(len(config["pattern_ids"])):
        selected = config["pattern_index"] == pattern
        latent[..., selected] -= latent[..., selected].mean(axis=-1, keepdims=True)
    expected = config["weight"] * np.exp(
        samples["intercept"][..., None]
        + samples["group_effect"][..., None]
        * config["group"][config["patient_index"]]
        + samples["covariate_effect"][..., None] * config["covariate"]
        + patient_effect[..., config["patient_index"]]
        + pattern_effect[..., config["pattern_index"]]
        + latent
    )
    posterior = az.from_dict(
        {
            "posterior": {
                name: samples[name]
                for name in (
                    "intercept", "group_effect", "covariate_effect", "patient_sd",
                    "pattern_sd", "patient_raw", "pattern_raw", "field_raw",
                )
            }
        }
    )
    monitored = [
        "intercept", "group_effect", "covariate_effect", "patient_sd", "pattern_sd",
        "patient_raw", "pattern_raw", "field_raw",
    ]
    r_hat = float(flattened(az.rhat(posterior, var_names=monitored, method="rank"), monitored).max())
    bulk = float(flattened(az.ess(posterior, var_names=monitored, method="bulk"), monitored).min())
    tail = float(flattened(az.ess(posterior, var_names=monitored, method="tail"), monitored).min())
    mcse_mean = float(flattened(az.mcse(posterior, var_names=monitored, method="mean"), monitored).max())
    mcse_sd = float(flattened(az.mcse(posterior, var_names=monitored, method="sd"), monitored).max())
    extra = sampler.get_extra_fields(group_by_chain=True)
    energy = np.asarray(extra["energy"], dtype=np.float64)
    ebfmi = float(np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1)))
    divergences = int(np.asarray(extra["diverging"]).sum())
    maximum_steps = 2 ** config["numpyro_maximum_tree_depth"] - 1
    depth_hits = int((np.asarray(extra["num_steps"]) >= maximum_steps).sum())
    prior_finite = load_pymc_contract().prior_is_finite(config)
    posterior_finite = bool(
        all(np.isfinite(value).all() for value in samples.values())
        and np.isfinite(patient_effect).all()
        and np.isfinite(pattern_effect).all()
        and np.isfinite(latent).all()
        and np.isfinite(expected).all()
        and np.all(expected > 0.0)
    )
    policy = config["policy"]
    complete = (
        prior_finite
        and posterior_finite
        and r_hat <= policy["maximum_r_hat"]
        and bulk >= policy["minimum_bulk_ess"]
        and tail >= policy["minimum_tail_ess"]
        and ebfmi >= policy["minimum_ebfmi"]
        and divergences <= policy["maximum_divergences"]
        and depth_hits <= policy["maximum_tree_depth_hits"]
    )
    flat_expected = expected.reshape(-1, expected.shape[-1])
    flat_latent = latent.reshape(-1, latent.shape[-1])
    replicated = np.random.default_rng(seed_for(config["seed"], "posterior_predictive")).poisson(
        flat_expected
    )
    pattern_predictive = []
    for index, pattern_id in enumerate(config["pattern_ids"]):
        selected = config["pattern_index"] == index
        observed_nodes = config["counts"][selected]
        replicated_nodes = replicated[:, selected]
        observed_total = int(observed_nodes.sum())
        replicated_totals = replicated_nodes.sum(axis=1)
        observed_variance = float(np.var(observed_nodes))
        replicated_variances = np.var(replicated_nodes, axis=1)
        pattern_predictive.append(
            {
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
            }
        )
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": {
            "name": "numpyro", "version": numpyro.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_digest, "worker_sha256": worker_digest,
        },
        "jax_version": jax.__version__,
        "input_sha256": config["request"]["input_sha256"],
        "request_sha256": request_sha256,
        "source_request_sha256": source_request_sha256,
        "maximum_tree_depth": config["numpyro_maximum_tree_depth"],
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"], "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"],
            "completed_draws": config["chains"] * config["draws"],
        },
        "posterior": {
            name: summary(samples[name])
            for name in ("intercept", "group_effect", "covariate_effect", "patient_sd", "pattern_sd")
        },
        "patient_effects": [
            {"patient_id": identity, "effect": summary(patient_effect[..., index])}
            for index, identity in enumerate(config["patient_ids"])
        ],
        "pattern_effects": [
            {"pattern_id": identity, "effect": summary(pattern_effect[..., index])}
            for index, identity in enumerate(config["pattern_ids"])
        ],
        "nodes": [
            {
                "pattern_id": config["request"]["nodes"][index]["pattern_id"],
                "node_id": config["request"]["nodes"][index]["node_id"],
                "latent_effect": summary(flat_latent[:, index]),
                "expected_count": summary(flat_expected[:, index]),
            }
            for index in range(len(config["counts"]))
        ],
        "pattern_posterior_predictive": pattern_predictive,
        "diagnostics": {
            "prior_predictive_finite": prior_finite, "posterior_finite": posterior_finite,
            "r_hat": r_hat, "ess_bulk": bulk, "ess_tail": tail,
            "mcse_mean": mcse_mean, "mcse_sd": mcse_sd, "minimum_ebfmi": ebfmi,
            "divergences": divergences, "max_tree_depth_hits": depth_hits,
            "constraints_valid": posterior_finite, "identifiability_checks_passed": True,
        },
    }


def main() -> None:
    if (
        numpyro.__version__ != NUMPYRO_VERSION
        or jax.__version__ != JAX_VERSION
        or sys.version_info[:2] != (3, 12)
    ):
        raise ContractError("NumPyro, JAX, or Python version drift")
    script = Path(__file__)
    lock_digest = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_digest = hashlib.sha256(script.read_bytes()).hexdigest()
    pymc_worker_digest = hashlib.sha256(
        script.with_name("marklab_pymc_replicated_arbitrary_window_lgcp_worker.py").read_bytes()
    ).hexdigest()
    raw = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if not raw or len(raw) > 16 * 1024 * 1024:
        raise ContractError("request size is invalid")
    request_sha256 = hashlib.sha256(raw).hexdigest()
    request, config = validate(
        json.loads(raw), lock_digest, worker_digest, pymc_worker_digest
    )
    result = fit(
        config, request_sha256, request["source_request_sha256"],
        lock_digest, worker_digest,
    )
    sys.stdout.write(json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")))
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(
            f"marklab NumPyro replicated LGCP failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
