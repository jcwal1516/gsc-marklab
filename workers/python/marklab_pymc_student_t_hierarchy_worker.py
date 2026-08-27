#!/usr/bin/env python3
"""Static PyMC worker for a robust Student-t patient hierarchy."""

from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path
import sys
from typing import Any

import numpy as np
import pymc as pm


PYMC_VERSION = "6.3.0"


class ContractError(ValueError):
    pass


def obj(value: Any, keys: set[str], path: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        actual = set(value) if isinstance(value, dict) else set()
        raise ContractError(
            f"{path} fields differ: missing={sorted(keys - actual)}, unknown={sorted(actual - keys)}"
        )
    return value


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


def exact(value: Any, expected: Any, path: str) -> None:
    if value != expected:
        raise ContractError(f"{path} must equal {expected!r}")


def prior(value: Any, path: str) -> tuple[float, float]:
    value = obj(value, {"family", "mean", "sd", "rationale"}, path)
    exact(value["family"], "normal", f"{path}.family")
    exact(value["rationale"], "user_supplied", f"{path}.rationale")
    mean = number(value["mean"], f"{path}.mean")
    sd = number(value["sd"], f"{path}.sd")
    if sd <= 0.0:
        raise ContractError(f"{path}.sd must be positive")
    return mean, sd


def half_prior(value: Any, path: str) -> float:
    value = obj(value, {"family", "sd", "support", "rationale"}, path)
    exact(value["family"], "half_normal", f"{path}.family")
    exact(value["support"], "positive", f"{path}.support")
    exact(value["rationale"], "user_supplied", f"{path}.rationale")
    sd = number(value["sd"], f"{path}.sd")
    if sd <= 0.0:
        raise ContractError(f"{path}.sd must be positive")
    return sd


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(request, {"format", "version", "backend", "model", "patients", "sampling", "resources", "diagnostic_policy"}, "request")
    exact(request["format"], "marklab.pymc_worker_request", "request.format")
    exact(request["version"], 1, "request.version")
    backend = obj(request["backend"], {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"}, "backend")
    exact(backend["name"], "pymc", "backend.name")
    exact(backend["version"], PYMC_VERSION, "backend.version")
    exact(backend["python_version"], "3.12", "backend.python")
    exact(backend["environment_lock_sha256"], lock_digest, "backend.lock")
    exact(backend["worker_sha256"], worker_digest, "backend.worker")
    model = obj(request["model"], {"format", "version", "family", "global_mean_prior", "between_patient_sd_prior", "patient_effect_parameterization", "likelihood", "observation_unit", "biological_unit", "hierarchy", "spatial_component", "posterior_predictive_statistics", "backend_capability", "maturity"}, "model")
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    exact(model["version"], 1, "model.version")
    exact(model["family"], "student_t_patient_varying_intercept", "model.family")
    global_mean, global_sd = prior(model["global_mean_prior"], "model.global_mean_prior")
    between_sd = half_prior(model["between_patient_sd_prior"], "model.between_prior")
    exact(model["patient_effect_parameterization"], "noncentered", "model.parameterization")
    likelihood = obj(model["likelihood"], {"family", "observation_sd_prior", "degrees_of_freedom", "degrees_of_freedom_excess_prior", "degrees_of_freedom_excess_rate", "finite_variance_floor", "link"}, "model.likelihood")
    exact(likelihood["family"], "student_t_inferred_scale_and_degrees_of_freedom", "likelihood.family")
    observation_sd = half_prior(likelihood["observation_sd_prior"], "likelihood.observation_sd_prior")
    exact(likelihood["degrees_of_freedom"], "two_plus_positive_excess", "likelihood.df")
    exact(likelihood["degrees_of_freedom_excess_prior"], "exponential_rate", "likelihood.df_prior")
    df_rate = number(likelihood["degrees_of_freedom_excess_rate"], "likelihood.df_rate")
    exact(number(likelihood["finite_variance_floor"], "likelihood.floor"), 2.0, "likelihood.floor")
    if df_rate <= 0.0:
        raise ContractError("degrees-of-freedom excess rate must be positive")
    exact(model["observation_unit"], "patient_nested_scalar_observation", "model.observation_unit")
    exact(model["biological_unit"], "patient", "model.biological_unit")
    exact(model["hierarchy"], ["patient"], "model.hierarchy")
    exact(model["spatial_component"], "none", "model.spatial_component")
    exact(model["posterior_predictive_statistics"], ["global_mean", "patient_mean_sd", "maximum_absolute_residual"], "model.ppc")
    exact(model["backend_capability"], "nuts", "model.backend_capability")
    exact(model["maturity"], "experimental", "model.maturity")
    patients = request["patients"]
    if not isinstance(patients, list) or len(patients) < 3:
        raise ContractError("at least three patients are required")
    patient_ids: list[str] = []
    patient_values: list[np.ndarray] = []
    for index, raw in enumerate(patients):
        row = obj(raw, {"patient_id", "observations"}, f"patients[{index}]")
        patient_id = row["patient_id"]
        if not isinstance(patient_id, str) or not patient_id or patient_id.strip() != patient_id or (patient_ids and patient_id <= patient_ids[-1]):
            raise ContractError("patient identities must be nonempty and strictly increasing")
        values = row["observations"]
        if not isinstance(values, list) or len(values) < 2:
            raise ContractError("each patient requires at least two observations")
        patient_ids.append(patient_id)
        patient_values.append(np.asarray([number(value, "patient.observation") for value in values], dtype=np.float64))
    sampling = obj(request["sampling"], {"chains", "tune_per_chain", "draws_per_chain", "target_accept", "seed"}, "sampling")
    chains = integer(sampling["chains"], "sampling.chains", 2, 8)
    tune = integer(sampling["tune_per_chain"], "sampling.tune", 100, 100_000)
    draws = integer(sampling["draws_per_chain"], "sampling.draws", 100, 100_000)
    target_accept = number(sampling["target_accept"], "sampling.target_accept")
    seed = integer(sampling["seed"], "sampling.seed", 0, 2**64 - 1)
    if not 0.5 <= target_accept < 1.0:
        raise ContractError("target acceptance is invalid")
    resources = obj(request["resources"], {"maximum_observations", "maximum_total_iterations", "maximum_output_bytes", "timeout_seconds"}, "resources")
    if sum(map(len, patient_values)) > integer(resources["maximum_observations"], "resources.observations", 1, 100_000) or chains * (tune + draws) > integer(resources["maximum_total_iterations"], "resources.iterations", 1, 800_000):
        raise ContractError("Student-t resources are exceeded")
    policy = obj(request["diagnostic_policy"], {"prior_predictive_draws", "maximum_r_hat", "minimum_bulk_ess", "minimum_tail_ess", "minimum_ebfmi", "maximum_divergences", "maximum_tree_depth_hits", "maximum_tree_depth"}, "policy")
    return {"global_mean": global_mean, "global_sd": global_sd, "between_sd": between_sd, "observation_sd": observation_sd, "df_rate": df_rate, "patient_ids": patient_ids, "patient_values": patient_values, "chains": chains, "tune": tune, "draws": draws, "target_accept": target_accept, "seed": seed, "prior_draws": integer(policy["prior_predictive_draws"], "policy.prior_draws", 1, 100_000), "maximum_r_hat": number(policy["maximum_r_hat"], "policy.rhat"), "minimum_bulk_ess": number(policy["minimum_bulk_ess"], "policy.bulk"), "minimum_tail_ess": number(policy["minimum_tail_ess"], "policy.tail"), "minimum_ebfmi": number(policy["minimum_ebfmi"], "policy.ebfmi"), "maximum_divergences": integer(policy["maximum_divergences"], "policy.divergences", 0, 2**63-1), "maximum_tree_depth_hits": integer(policy["maximum_tree_depth_hits"], "policy.depth_hits", 0, 2**63-1), "maximum_tree_depth": integer(policy["maximum_tree_depth"], "policy.depth", 1, 32)}


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    digest = hashlib.sha256(f"marklab-pymc-student-t-hierarchy-v1\0{seed}\0{purpose}\0{index}".encode()).digest()
    return int.from_bytes(digest[:4], "little")


def summary(values: np.ndarray) -> dict[str, float]:
    values = values.reshape(-1)
    return {"mean": float(values.mean()), "sd": float(values.std(ddof=1)), "interval_lower": float(np.quantile(values, 0.025)), "interval_upper": float(np.quantile(values, 0.975))}


def flattened(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    patient_index = np.concatenate([np.full(len(values), index, dtype=np.int64) for index, values in enumerate(config["patient_values"])])
    observations = np.concatenate(config["patient_values"])
    with pm.Model():
        global_mean = pm.Normal("global_mean", config["global_mean"], config["global_sd"])
        between_sd = pm.HalfNormal("between_patient_sd", config["between_sd"])
        patient_z = pm.Normal("patient_z", 0.0, 1.0, shape=len(config["patient_ids"]))
        patient_mean = pm.Deterministic("patient_mean", global_mean + between_sd * patient_z)
        observation_sd = pm.HalfNormal("observation_sd", config["observation_sd"])
        df_excess = pm.Exponential("degrees_of_freedom_excess", config["df_rate"])
        degrees_of_freedom = pm.Deterministic("degrees_of_freedom", 2.0 + df_excess)
        pm.StudentT("observation", nu=degrees_of_freedom, mu=patient_mean[patient_index], sigma=observation_sd, observed=observations)
        prior = pm.sample_prior_predictive(draws=config["prior_draws"], random_seed=seed_for(config["seed"], "prior"))
        posterior = pm.sample(draws=config["draws"], tune=config["tune"], chains=config["chains"], cores=1, blas_cores=1, random_seed=[seed_for(config["seed"], "chain", index) for index in range(config["chains"])], target_accept=config["target_accept"], nuts_sampler="pymc", nuts={"max_treedepth": config["maximum_tree_depth"]}, progressbar=False, quiet=True, compute_convergence_checks=False)
        predictive = pm.sample_posterior_predictive(posterior, var_names=["observation"], random_seed=seed_for(config["seed"], "predictive"), progressbar=False)
    draws = {name: np.asarray(posterior["posterior"][name].values, dtype=np.float64) for name in ["global_mean", "between_patient_sd", "patient_mean", "observation_sd", "degrees_of_freedom"]}
    replicated = np.asarray(predictive["posterior_predictive"]["observation"].values, dtype=np.float64).reshape(-1, observations.size)
    prior_finite = bool(all(np.isfinite(np.asarray(value.values)).all() for value in prior["prior"].values()))
    posterior_finite = bool(all(np.isfinite(value).all() for value in draws.values()) and np.isfinite(replicated).all())
    monitored = ["global_mean", "between_patient_sd", "patient_mean", "observation_sd", "degrees_of_freedom"]
    r_hat = float(flattened(pm.stats.rhat(posterior, var_names=monitored, method="rank"), monitored).max())
    bulk = float(flattened(pm.stats.ess(posterior, var_names=monitored, method="bulk"), monitored).min())
    tail = float(flattened(pm.stats.ess(posterior, var_names=monitored, method="tail"), monitored).min())
    mcse_mean = float(flattened(pm.stats.mcse(posterior, var_names=monitored, method="mean"), monitored).max())
    mcse_sd = float(flattened(pm.stats.mcse(posterior, var_names=monitored, method="sd"), monitored).max())
    energy = np.asarray(posterior["sample_stats"]["energy"].values, dtype=np.float64)
    ebfmi = float(np.min(np.mean(np.diff(energy, axis=1)**2, axis=1) / np.var(energy, axis=1)))
    divergences = int(np.asarray(posterior["sample_stats"]["divergences"].values).sum())
    depth_hits = int(np.asarray(posterior["sample_stats"]["reached_max_treedepth"].values).sum())
    constraints = bool(np.all(draws["between_patient_sd"] > 0.0) and np.all(draws["observation_sd"] > 0.0) and np.all(draws["degrees_of_freedom"] > 2.0))
    complete = prior_finite and posterior_finite and constraints and r_hat <= config["maximum_r_hat"] and bulk >= config["minimum_bulk_ess"] and tail >= config["minimum_tail_ess"] and ebfmi >= config["minimum_ebfmi"] and divergences <= config["maximum_divergences"] and depth_hits <= config["maximum_tree_depth_hits"]
    patient_draws = draws["patient_mean"].reshape(-1, len(config["patient_ids"]))
    global_posterior_mean = float(draws["global_mean"].mean())
    partial_pooling = []
    for index, (patient_id, values) in enumerate(zip(config["patient_ids"], config["patient_values"], strict=True)):
        raw_mean = float(values.mean())
        denominator = global_posterior_mean - raw_mean
        shrinkage = 0.0 if denominator == 0.0 else float((patient_draws[:, index].mean() - raw_mean) / denominator)
        item = summary(patient_draws[:, index])
        partial_pooling.append({"patient_id": patient_id, "observation_count": len(values), "raw_mean": raw_mean, "posterior_mean": item["mean"], "posterior_sd": item["sd"], "interval_lower": item["interval_lower"], "interval_upper": item["interval_upper"], "shrinkage": shrinkage, "warning": "shrinkage_is_model_dependent_not_a_quality_score"})
    observed_patient_means = np.asarray([values.mean() for values in config["patient_values"]])
    replicated_patient_means = np.stack([replicated[:, patient_index == index].mean(axis=1) for index in range(len(config["patient_ids"]))], axis=1)
    observed_residual = max(float(np.max(np.abs(values - values.mean()))) for values in config["patient_values"])
    replicated_residual = np.stack([np.max(np.abs(replicated[:, patient_index == index] - replicated_patient_means[:, index, None]), axis=1) for index in range(len(config["patient_ids"]))], axis=1).max(axis=1)
    return {"format": "marklab.pymc_student_t_hierarchy_worker_result", "version": 1, "backend": {"name": "pymc", "version": pm.__version__, "python_version": f"{sys.version_info.major}.{sys.version_info.minor}", "environment_lock_sha256": lock_sha, "worker_sha256": worker_sha}, "request_sha256": request_sha, "fit_state": "complete" if complete else "nonconverged", "sampling": {"chains": config["chains"], "tune_per_chain": config["tune"], "draws_per_chain": config["draws"], "completed_draws": config["chains"] * config["draws"]}, "posterior": {"global_mean": summary(draws["global_mean"]), "between_patient_sd": summary(draws["between_patient_sd"]), "observation_sd": summary(draws["observation_sd"]), "degrees_of_freedom": summary(draws["degrees_of_freedom"])}, "partial_pooling": partial_pooling, "diagnostics": {"prior_predictive_finite": prior_finite, "posterior_finite": posterior_finite, "r_hat": r_hat, "ess_bulk": bulk, "ess_tail": tail, "mcse_mean": mcse_mean, "mcse_sd": mcse_sd, "minimum_ebfmi": ebfmi, "divergences": divergences, "max_tree_depth_hits": depth_hits, "constraints_valid": constraints, "identifiability_checks_passed": True}, "posterior_predictive": {"observed_global_mean": float(observations.mean()), "replicated_global_mean_mean": float(replicated.mean(axis=1).mean()), "replicated_global_mean_sd": float(replicated.mean(axis=1).std(ddof=1)), "observed_patient_mean_sd": float(observed_patient_means.std(ddof=1)), "replicated_patient_mean_sd_mean": float(replicated_patient_means.std(axis=1, ddof=1).mean()), "observed_maximum_absolute_residual": observed_residual, "replicated_maximum_absolute_residual_mean": float(replicated_residual.mean()), "probability_replicated_maximum_absolute_residual_at_least_observed": float(np.mean(replicated_residual >= observed_residual))}}


def main() -> None:
    if pm.__version__ != PYMC_VERSION or sys.version_info[:2] != (3, 12):
        raise ContractError("PyMC or Python version drift")
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    raw = sys.stdin.buffer.read(2 * 1024 * 1024 + 1)
    if not raw or len(raw) > 2 * 1024 * 1024:
        raise ContractError("request size is invalid")
    result = fit(validate(json.loads(raw), lock_sha, worker_sha), hashlib.sha256(raw).hexdigest(), lock_sha, worker_sha)
    sys.stdout.write(json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")))
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"Marklab PyMC Student-t hierarchy worker failed: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from error
