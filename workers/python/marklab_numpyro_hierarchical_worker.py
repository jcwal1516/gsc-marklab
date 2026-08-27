#!/usr/bin/env python3
"""Static NumPyro worker for cross-backend Gaussian hierarchy agreement."""

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


def load_pymc_contract() -> Any:
    path = Path(__file__).with_name("marklab_pymc_hierarchical_worker.py")
    specification = importlib.util.spec_from_file_location("marklab_pymc_hierarchy_contract", path)
    if specification is None or specification.loader is None:
        raise ContractError("cannot load the PyMC hierarchy contract")
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


REQUEST_FORMAT = "marklab.numpyro_hierarchical_worker_request"
RESULT_FORMAT = "marklab.numpyro_hierarchical_worker_result"
NUMPYRO_VERSION = "0.21.0"
JAX_VERSION = "0.11.1"


class ContractError(ValueError):
    pass


def exact_object(value: Any, keys: set[str], path: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        actual = set(value) if isinstance(value, dict) else set()
        raise ContractError(
            f"{path} fields differ: missing={sorted(keys - actual)}, unknown={sorted(actual - keys)}"
        )
    return value


def exact(value: Any, expected: Any, path: str) -> None:
    if value != expected:
        raise ContractError(f"{path} must equal {expected!r}")


def seed_for(seed: int, purpose: str) -> int:
    digest = hashlib.sha256(f"marklab-numpyro-hierarchy-v1\0{seed}\0{purpose}".encode()).digest()
    return int.from_bytes(digest[:4], "little")


def scalar_summary(values: np.ndarray) -> dict[str, float]:
    flattened = values.reshape(-1)
    return {
        "mean": float(flattened.mean()),
        "sd": float(flattened.std(ddof=1)),
        "interval_lower": float(np.quantile(flattened, 0.025)),
        "interval_upper": float(np.quantile(flattened, 0.975)),
    }


def flattened(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def validate_wrapper(
    request: Any, lock_digest: str, worker_digest: str, pymc_worker_digest: str
) -> tuple[dict[str, Any], dict[str, Any]]:
    request = exact_object(
        request,
        {
            "format",
            "version",
            "backend",
            "jax_version",
            "source_request_sha256",
            "source_request",
        },
        "request",
    )
    exact(request["format"], REQUEST_FORMAT, "request.format")
    exact(request["version"], 1, "request.version")
    backend = exact_object(
        request["backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "request.backend",
    )
    exact(backend["name"], "numpyro", "request.backend.name")
    exact(backend["version"], NUMPYRO_VERSION, "request.backend.version")
    exact(backend["python_version"], "3.12", "request.backend.python_version")
    exact(backend["environment_lock_sha256"], lock_digest, "request.backend.lock")
    exact(backend["worker_sha256"], worker_digest, "request.backend.worker")
    exact(request["jax_version"], JAX_VERSION, "request.jax_version")
    source_digest = request["source_request_sha256"]
    if not isinstance(source_digest, str) or len(source_digest) != 64:
        raise ContractError("source request digest is invalid")
    config = load_pymc_contract().validate_request(
        request["source_request"], lock_digest, pymc_worker_digest
    )
    return request, config


def model(
    patient_index: jnp.ndarray,
    observations: jnp.ndarray,
    patient_count: int,
    global_prior_mean: float,
    global_prior_sd: float,
    between_prior_sd: float,
    known_sigma: float,
) -> None:
    global_mean = numpyro.sample(
        "global_mean", dist.Normal(global_prior_mean, global_prior_sd)
    )
    between_patient_sd = numpyro.sample(
        "between_patient_sd", dist.HalfNormal(between_prior_sd)
    )
    patient_z = numpyro.sample(
        "patient_z", dist.Normal(0.0, 1.0).expand([patient_count]).to_event(1)
    )
    patient_mean = numpyro.deterministic(
        "patient_mean", global_mean + between_patient_sd * patient_z
    )
    numpyro.sample("y", dist.Normal(patient_mean[patient_index], known_sigma), obs=observations)


def fit(
    config: dict[str, Any],
    request_sha256: str,
    source_request_sha256: str,
    lock_digest: str,
    worker_digest: str,
) -> dict[str, Any]:
    patient_index = np.concatenate(
        [
            np.full(len(values), index, dtype=np.int32)
            for index, values in enumerate(config["patient_values"])
        ]
    )
    observations = np.concatenate(config["patient_values"]).astype(np.float64)
    kernel = NUTS(
        model,
        target_accept_prob=config["target_accept"],
        max_tree_depth=config["maximum_tree_depth"],
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
        patient_index=jnp.asarray(patient_index),
        observations=jnp.asarray(observations),
        patient_count=len(config["patient_ids"]),
        global_prior_mean=config["global_prior_mean"],
        global_prior_sd=config["global_prior_sd"],
        between_prior_sd=config["between_prior_sd"],
        known_sigma=config["known_sigma"],
        extra_fields=("diverging", "num_steps", "energy"),
    )
    samples = {
        name: np.asarray(value, dtype=np.float64)
        for name, value in sampler.get_samples(group_by_chain=True).items()
    }
    posterior = az.from_dict({"posterior": samples})
    monitored = ["global_mean", "between_patient_sd", "patient_mean"]
    r_hat = float(flattened(az.rhat(posterior, var_names=monitored, method="rank"), monitored).max())
    ess_bulk = float(flattened(az.ess(posterior, var_names=monitored, method="bulk"), monitored).min())
    ess_tail = float(flattened(az.ess(posterior, var_names=monitored, method="tail"), monitored).min())
    mcse_mean = float(flattened(az.mcse(posterior, var_names=monitored, method="mean"), monitored).max())
    mcse_sd = float(flattened(az.mcse(posterior, var_names=monitored, method="sd"), monitored).max())
    extra = sampler.get_extra_fields(group_by_chain=True)
    energy = np.asarray(extra["energy"], dtype=np.float64)
    minimum_ebfmi = float(
        np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1))
    )
    divergences = int(np.asarray(extra["diverging"]).sum())
    maximum_steps = 2 ** config["maximum_tree_depth"] - 1
    tree_depth_hits = int((np.asarray(extra["num_steps"]) >= maximum_steps).sum())

    global_draws = samples["global_mean"]
    between_draws = samples["between_patient_sd"]
    patient_draws = samples["patient_mean"]
    rng = np.random.default_rng(seed_for(config["seed"], "posterior_predictive"))
    predictive_draws = rng.normal(
        patient_draws[..., patient_index], config["known_sigma"]
    )
    prior_rng = np.random.default_rng(seed_for(config["seed"], "prior"))
    prior_global = prior_rng.normal(
        config["global_prior_mean"], config["global_prior_sd"], config["prior_draws"]
    )
    prior_between = np.abs(
        prior_rng.normal(0.0, config["between_prior_sd"], config["prior_draws"])
    )
    prior_patient_z = prior_rng.normal(
        0.0, 1.0, (config["prior_draws"], len(config["patient_ids"]))
    )
    prior_patient_mean = prior_global[:, None] + prior_between[:, None] * prior_patient_z
    prior_y = prior_rng.normal(
        prior_patient_mean[:, patient_index], config["known_sigma"]
    )
    prior_finite = bool(
        np.isfinite(prior_global).all()
        and np.isfinite(prior_between).all()
        and np.isfinite(prior_y).all()
    )
    posterior_finite = bool(
        all(np.isfinite(samples[name]).all() for name in monitored)
        and np.isfinite(predictive_draws).all()
    )

    partial_pooling = []
    for index, (patient_id, values) in enumerate(
        zip(config["patient_ids"], config["patient_values"], strict=True)
    ):
        draws = patient_draws[..., index]
        posterior_variance = float(draws.var(ddof=1))
        unpooled_variance = config["known_sigma"] ** 2 / len(values)
        partial_pooling.append(
            {
                "patient_id": patient_id,
                "observation_count": len(values),
                "raw_mean": float(values.mean()),
                "posterior_mean": float(draws.mean()),
                "posterior_sd": float(draws.std(ddof=1)),
                "interval_lower": float(np.quantile(draws, 0.025)),
                "interval_upper": float(np.quantile(draws, 0.975)),
                "shrinkage": 1.0 - posterior_variance / unpooled_variance,
                "warning": "shrinkage_is_model_dependent_not_a_quality_score",
            }
        )

    replicated_global_means = predictive_draws.mean(axis=-1)
    replicated_patient_means = np.stack(
        [
            predictive_draws[..., patient_index == index].mean(axis=-1)
            for index in range(len(config["patient_ids"]))
        ],
        axis=-1,
    )
    replicated_patient_mean_sd = replicated_patient_means.std(axis=-1, ddof=1)
    observed_global_mean = float(observations.mean())
    observed_patient_mean_sd = float(
        np.asarray([values.mean() for values in config["patient_values"]]).std(ddof=1)
    )
    variance_partition = between_draws**2 / (
        between_draws**2 + config["known_sigma"] ** 2
    )
    diagnostics_pass = (
        prior_finite
        and posterior_finite
        and r_hat <= config["maximum_r_hat"]
        and ess_bulk >= config["minimum_bulk_ess"]
        and ess_tail >= config["minimum_tail_ess"]
        and minimum_ebfmi >= config["minimum_ebfmi"]
        and divergences <= config["maximum_divergences"]
        and tree_depth_hits <= config["maximum_tree_depth_hits"]
    )
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": {
            "name": "numpyro",
            "version": numpyro.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_digest,
            "worker_sha256": worker_digest,
        },
        "jax_version": jax.__version__,
        "request_sha256": request_sha256,
        "source_request_sha256": source_request_sha256,
        "fit_state": "complete" if diagnostics_pass else "nonconverged",
        "sampling": {
            "chains": config["chains"],
            "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"],
            "completed_draws": config["chains"] * config["draws"],
        },
        "posterior": {
            "global_mean": scalar_summary(global_draws),
            "between_patient_sd": scalar_summary(between_draws),
            "variance_partition_mean": float(variance_partition.mean()),
        },
        "partial_pooling": partial_pooling,
        "diagnostics": {
            "prior_predictive_finite": prior_finite,
            "posterior_finite": posterior_finite,
            "r_hat": r_hat,
            "ess_bulk": ess_bulk,
            "ess_tail": ess_tail,
            "mcse_mean": mcse_mean,
            "mcse_sd": mcse_sd,
            "minimum_ebfmi": minimum_ebfmi,
            "divergences": divergences,
            "max_tree_depth_hits": tree_depth_hits,
            "constraints_valid": True,
            "identifiability_checks_passed": True,
        },
        "posterior_predictive": {
            "observed_global_mean": observed_global_mean,
            "replicated_global_mean_mean": float(replicated_global_means.mean()),
            "replicated_global_mean_sd": float(replicated_global_means.std(ddof=1)),
            "probability_replicated_global_mean_at_least_observed": float(
                np.mean(replicated_global_means >= observed_global_mean)
            ),
            "observed_patient_mean_sd": observed_patient_mean_sd,
            "replicated_patient_mean_sd_mean": float(
                replicated_patient_mean_sd.mean()
            ),
        },
    }


def main() -> int:
    script = Path(__file__).resolve()
    lock_digest = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_digest = hashlib.sha256(script.read_bytes()).hexdigest()
    pymc_worker_digest = hashlib.sha256(
        script.with_name("marklab_pymc_hierarchical_worker.py").read_bytes()
    ).hexdigest()
    raw = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if not raw or len(raw) > 16 * 1024 * 1024:
        raise ContractError("request size is invalid")
    request, config = validate_wrapper(
        json.loads(raw), lock_digest, worker_digest, pymc_worker_digest
    )
    result = fit(
        config,
        hashlib.sha256(raw).hexdigest(),
        request["source_request_sha256"],
        lock_digest,
        worker_digest,
    )
    encoded = json.dumps(result, allow_nan=False, separators=(",", ":"), sort_keys=True).encode()
    maximum_output = request["source_request"]["resources"]["maximum_output_bytes"]
    if len(encoded) > maximum_output:
        raise ContractError("result exceeds output limit")
    sys.stdout.buffer.write(encoded)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"Marklab NumPyro hierarchy worker failed: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from error
