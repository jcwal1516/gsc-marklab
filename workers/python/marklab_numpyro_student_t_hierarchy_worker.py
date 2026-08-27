#!/usr/bin/env python3
"""Independent NumPyro worker for Student-t hierarchy agreement."""

from __future__ import annotations

import hashlib
import importlib.util
import json
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


REQUEST_FORMAT = "marklab.numpyro_student_t_hierarchy_worker_request"
RESULT_FORMAT = "marklab.numpyro_student_t_hierarchy_worker_result"


class ContractError(ValueError):
    pass


def load_contract() -> Any:
    path = Path(__file__).with_name("marklab_pymc_student_t_hierarchy_worker.py")
    specification = importlib.util.spec_from_file_location("marklab_pymc_student_t_contract", path)
    if specification is None or specification.loader is None:
        raise ContractError("cannot load the PyMC Student-t contract")
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


def exact_object(value: Any, keys: set[str], path: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        actual = set(value) if isinstance(value, dict) else set()
        raise ContractError(f"{path} fields differ: missing={sorted(keys-actual)}, unknown={sorted(actual-keys)}")
    return value


def exact(value: Any, expected: Any, path: str) -> None:
    if value != expected:
        raise ContractError(f"{path} must equal {expected!r}")


def validate(request: Any, lock_digest: str, worker_digest: str) -> tuple[dict[str, Any], dict[str, Any]]:
    request = exact_object(request, {"format", "version", "backend", "jax_version", "source_request_sha256", "source_request"}, "request")
    exact(request["format"], REQUEST_FORMAT, "request.format")
    exact(request["version"], 1, "request.version")
    backend = exact_object(request["backend"], {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"}, "backend")
    exact(backend["name"], "numpyro", "backend.name")
    exact(backend["version"], "0.21.0", "backend.version")
    exact(backend["python_version"], "3.12", "backend.python")
    exact(backend["environment_lock_sha256"], lock_digest, "backend.lock")
    exact(backend["worker_sha256"], worker_digest, "backend.worker")
    exact(request["jax_version"], "0.11.1", "request.jax_version")
    source_digest = request["source_request_sha256"]
    if not isinstance(source_digest, str) or len(source_digest) != 64 or any(character not in "0123456789abcdef" for character in source_digest):
        raise ContractError("source request digest is invalid")
    pymc_digest = hashlib.sha256(Path(__file__).with_name("marklab_pymc_student_t_hierarchy_worker.py").read_bytes()).hexdigest()
    config = load_contract().validate(request["source_request"], lock_digest, pymc_digest)
    return request, config


def seed_for(seed: int, purpose: str) -> int:
    digest = hashlib.sha256(f"marklab-numpyro-student-t-v1\0{seed}\0{purpose}".encode()).digest()
    return int.from_bytes(digest[:4], "little")


def model(patient_index: jnp.ndarray, observations: jnp.ndarray, patient_count: int, global_mean: float, global_sd: float, between_sd: float, observation_sd: float, df_rate: float) -> None:
    population = numpyro.sample("global_mean", dist.Normal(global_mean, global_sd))
    between = numpyro.sample("between_patient_sd", dist.HalfNormal(between_sd))
    patient_z = numpyro.sample("patient_z", dist.Normal(0.0, 1.0).expand([patient_count]).to_event(1))
    patient_mean = numpyro.deterministic("patient_mean", population + between * patient_z)
    sigma = numpyro.sample("observation_sd", dist.HalfNormal(observation_sd))
    df_excess = numpyro.sample("degrees_of_freedom_excess", dist.Exponential(df_rate))
    degrees = numpyro.deterministic("degrees_of_freedom", 2.0 + df_excess)
    numpyro.sample("observation", dist.StudentT(degrees, patient_mean[patient_index], sigma), obs=observations)


def summary(values: np.ndarray) -> dict[str, float]:
    values = values.reshape(-1)
    return {"mean": float(values.mean()), "sd": float(values.std(ddof=1)), "interval_lower": float(np.quantile(values, 0.025)), "interval_upper": float(np.quantile(values, 0.975))}


def flattened(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def fit(config: dict[str, Any], request_sha: str, source_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    patient_index = np.concatenate([np.full(len(values), index, dtype=np.int32) for index, values in enumerate(config["patient_values"])])
    observations = np.concatenate(config["patient_values"])
    kernel = NUTS(
        model,
        target_accept_prob=config["target_accept"],
        max_tree_depth=config["maximum_tree_depth"],
        dense_mass=True,
    )
    sampler = MCMC(kernel, num_warmup=config["tune"], num_samples=config["draws"], num_chains=config["chains"], chain_method="sequential", progress_bar=False)
    sampler.run(jax.random.PRNGKey(seed_for(config["seed"], "nuts")), patient_index=jnp.asarray(patient_index), observations=jnp.asarray(observations), patient_count=len(config["patient_ids"]), global_mean=config["global_mean"], global_sd=config["global_sd"], between_sd=config["between_sd"], observation_sd=config["observation_sd"], df_rate=config["df_rate"], extra_fields=("diverging", "num_steps", "energy"))
    samples = {name: np.asarray(value, dtype=np.float64) for name, value in sampler.get_samples(group_by_chain=True).items()}
    degrees = 2.0 + samples["degrees_of_freedom_excess"]
    monitored_samples = {"global_mean": samples["global_mean"], "between_patient_sd": samples["between_patient_sd"], "patient_mean": samples["patient_mean"], "observation_sd": samples["observation_sd"], "degrees_of_freedom": degrees}
    posterior = az.from_dict({"posterior": monitored_samples})
    names = list(monitored_samples)
    r_hat = float(flattened(az.rhat(posterior, var_names=names, method="rank"), names).max())
    bulk = float(flattened(az.ess(posterior, var_names=names, method="bulk"), names).min())
    tail = float(flattened(az.ess(posterior, var_names=names, method="tail"), names).min())
    mcse_mean = float(flattened(az.mcse(posterior, var_names=names, method="mean"), names).max())
    mcse_sd = float(flattened(az.mcse(posterior, var_names=names, method="sd"), names).max())
    extra = sampler.get_extra_fields(group_by_chain=True)
    energy = np.asarray(extra["energy"], dtype=np.float64)
    ebfmi = float(np.min(np.mean(np.diff(energy, axis=1)**2, axis=1) / np.var(energy, axis=1)))
    divergences = int(np.asarray(extra["diverging"]).sum())
    depth_hits = int((np.asarray(extra["num_steps"]) >= 2**config["maximum_tree_depth"] - 1).sum())
    rng = np.random.default_rng(seed_for(config["seed"], "predictive"))
    patient_draws = samples["patient_mean"]
    predictive = patient_draws[..., patient_index] + samples["observation_sd"][..., None] * rng.standard_t(degrees[..., None], size=patient_draws[..., patient_index].shape)
    prior_rng = np.random.default_rng(seed_for(config["seed"], "prior"))
    prior_population = prior_rng.normal(config["global_mean"], config["global_sd"], config["prior_draws"])
    prior_between = np.abs(prior_rng.normal(0.0, config["between_sd"], config["prior_draws"]))
    prior_sigma = np.abs(prior_rng.normal(0.0, config["observation_sd"], config["prior_draws"]))
    prior_df = 2.0 + prior_rng.exponential(1.0 / config["df_rate"], config["prior_draws"])
    prior_finite = bool(np.isfinite(prior_population).all() and np.isfinite(prior_between).all() and np.isfinite(prior_sigma).all() and np.isfinite(prior_df).all())
    posterior_finite = bool(all(np.isfinite(value).all() for value in monitored_samples.values()) and np.isfinite(predictive).all())
    constraints = bool(np.all(samples["between_patient_sd"] > 0.0) and np.all(samples["observation_sd"] > 0.0) and np.all(degrees > 2.0))
    complete = prior_finite and posterior_finite and constraints and r_hat <= config["maximum_r_hat"] and bulk >= config["minimum_bulk_ess"] and tail >= config["minimum_tail_ess"] and ebfmi >= config["minimum_ebfmi"] and divergences <= config["maximum_divergences"] and depth_hits <= config["maximum_tree_depth_hits"]
    flat_patient = patient_draws.reshape(-1, patient_draws.shape[-1])
    population_mean = float(samples["global_mean"].mean())
    partial = []
    for index, (patient_id, values) in enumerate(zip(config["patient_ids"], config["patient_values"], strict=True)):
        raw_mean = float(values.mean())
        denominator = population_mean - raw_mean
        shrinkage = 0.0 if denominator == 0.0 else float((flat_patient[:, index].mean() - raw_mean) / denominator)
        item = summary(flat_patient[:, index])
        partial.append({"patient_id": patient_id, "observation_count": len(values), "raw_mean": raw_mean, "posterior_mean": item["mean"], "posterior_sd": item["sd"], "interval_lower": item["interval_lower"], "interval_upper": item["interval_upper"], "shrinkage": shrinkage, "warning": "shrinkage_is_model_dependent_not_a_quality_score"})
    predictive_flat = predictive.reshape(-1, observations.size)
    observed_patient_means = np.asarray([values.mean() for values in config["patient_values"]])
    predictive_patient_means = np.stack([predictive_flat[:, patient_index == index].mean(axis=1) for index in range(len(config["patient_ids"]))], axis=1)
    observed_residual = max(float(np.max(np.abs(values-values.mean()))) for values in config["patient_values"])
    predictive_residual = np.stack([np.max(np.abs(predictive_flat[:, patient_index == index] - predictive_patient_means[:, index, None]), axis=1) for index in range(len(config["patient_ids"]))], axis=1).max(axis=1)
    return {"format": RESULT_FORMAT, "version": 1, "backend": {"name": "numpyro", "version": numpyro.__version__, "python_version": f"{sys.version_info.major}.{sys.version_info.minor}", "environment_lock_sha256": lock_sha, "worker_sha256": worker_sha}, "jax_version": jax.__version__, "request_sha256": request_sha, "source_request_sha256": source_sha, "fit_state": "complete" if complete else "nonconverged", "sampling": {"chains": config["chains"], "tune_per_chain": config["tune"], "draws_per_chain": config["draws"], "completed_draws": config["chains"]*config["draws"]}, "posterior": {"global_mean": summary(samples["global_mean"]), "between_patient_sd": summary(samples["between_patient_sd"]), "observation_sd": summary(samples["observation_sd"]), "degrees_of_freedom": summary(degrees)}, "partial_pooling": partial, "diagnostics": {"prior_predictive_finite": prior_finite, "posterior_finite": posterior_finite, "r_hat": r_hat, "ess_bulk": bulk, "ess_tail": tail, "mcse_mean": mcse_mean, "mcse_sd": mcse_sd, "minimum_ebfmi": ebfmi, "divergences": divergences, "max_tree_depth_hits": depth_hits, "constraints_valid": constraints, "identifiability_checks_passed": True}, "posterior_predictive": {"observed_global_mean": float(observations.mean()), "replicated_global_mean_mean": float(predictive_flat.mean(axis=1).mean()), "replicated_global_mean_sd": float(predictive_flat.mean(axis=1).std(ddof=1)), "observed_patient_mean_sd": float(observed_patient_means.std(ddof=1)), "replicated_patient_mean_sd_mean": float(predictive_patient_means.std(axis=1, ddof=1).mean()), "observed_maximum_absolute_residual": observed_residual, "replicated_maximum_absolute_residual_mean": float(predictive_residual.mean()), "probability_replicated_maximum_absolute_residual_at_least_observed": float(np.mean(predictive_residual >= observed_residual))}}


def main() -> None:
    if numpyro.__version__ != "0.21.0" or jax.__version__ != "0.11.1" or sys.version_info[:2] != (3, 12):
        raise ContractError("NumPyro, JAX, or Python version drift")
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    raw = sys.stdin.buffer.read(2*1024*1024+1)
    if not raw or len(raw) > 2*1024*1024:
        raise ContractError("request size is invalid")
    request, config = validate(json.loads(raw), lock_sha, worker_sha)
    result = fit(config, hashlib.sha256(raw).hexdigest(), request["source_request_sha256"], lock_sha, worker_sha)
    sys.stdout.write(json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")))
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"Marklab NumPyro Student-t worker failed: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from error
