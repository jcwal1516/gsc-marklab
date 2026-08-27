#!/usr/bin/env python3
"""Static PyMC worker for a patient beta-binomial hierarchy."""

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


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {"format", "version", "backend", "model", "patients", "sampling", "resources", "diagnostic_policy"},
        "request",
    )
    exact(request["format"], "marklab.pymc_worker_request", "request.format")
    exact(request["version"], 1, "request.version")
    backend = obj(request["backend"], {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"}, "backend")
    exact(backend["name"], "pymc", "backend.name")
    exact(backend["version"], PYMC_VERSION, "backend.version")
    exact(backend["python_version"], "3.12", "backend.python")
    exact(backend["environment_lock_sha256"], lock_digest, "backend.lock")
    exact(backend["worker_sha256"], worker_digest, "backend.worker")
    model = obj(
        request["model"],
        {
            "format", "version", "family", "population_probability_prior", "population_alpha",
            "population_beta", "concentration_prior", "concentration_prior_sd",
            "patient_probability", "likelihood", "overdispersion", "biological_unit",
            "backend_capability", "maturity",
        },
        "model",
    )
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    exact(model["version"], 1, "model.version")
    exact(model["family"], "patient_beta_binomial_hierarchy", "model.family")
    exact(model["population_probability_prior"], "beta", "model.population prior")
    exact(model["concentration_prior"], "half_normal", "model.concentration prior")
    exact(model["patient_probability"], "beta_population_mean_concentration", "model.patient probability")
    exact(model["likelihood"], "binomial_successes_given_patient_trials", "model.likelihood")
    population_alpha = number(model["population_alpha"], "model.population_alpha")
    population_beta = number(model["population_beta"], "model.population_beta")
    concentration_sd = number(model["concentration_prior_sd"], "model.concentration_sd")
    if min(population_alpha, population_beta, concentration_sd) <= 0.0:
        raise ContractError("model priors must be positive")
    patients = request["patients"]
    if not isinstance(patients, list) or not 3 <= len(patients) <= 512:
        raise ContractError("patient count is invalid")
    patient_ids: list[str] = []
    successes: list[int] = []
    trials: list[int] = []
    for index, raw in enumerate(patients):
        row = obj(raw, {"patient_id", "successes", "trials"}, f"patients[{index}]")
        patient_id = row["patient_id"]
        if not isinstance(patient_id, str) or not patient_id or len(patient_id) > 128:
            raise ContractError("patient identity is invalid")
        patient_ids.append(patient_id)
        trial = integer(row["trials"], "patient.trials", 1, 10_000_000)
        successes.append(integer(row["successes"], "patient.successes", 0, trial))
        trials.append(trial)
    if patient_ids != sorted(patient_ids) or len(set(patient_ids)) != len(patient_ids):
        raise ContractError("patient identities must be unique and sorted")
    sampling = obj(request["sampling"], {"chains", "tune_per_chain", "draws_per_chain", "target_accept", "seed"}, "sampling")
    chains = integer(sampling["chains"], "sampling.chains", 2, 8)
    tune = integer(sampling["tune_per_chain"], "sampling.tune", 100, 100_000)
    draws = integer(sampling["draws_per_chain"], "sampling.draws", 100, 100_000)
    target_accept = number(sampling["target_accept"], "sampling.target_accept")
    seed = integer(sampling["seed"], "sampling.seed", 0, 2**64 - 1)
    resources = obj(request["resources"], {"maximum_patients", "maximum_total_trials", "maximum_total_iterations", "maximum_output_bytes", "timeout_seconds"}, "resources")
    if len(patients) > integer(resources["maximum_patients"], "resources.patients", 3, 512):
        raise ContractError("patient limit exceeded")
    if sum(trials) > integer(resources["maximum_total_trials"], "resources.trials", 1, 10_000_000):
        raise ContractError("trial limit exceeded")
    if chains * (tune + draws) > integer(resources["maximum_total_iterations"], "resources.iterations", 1, 400_000):
        raise ContractError("iteration limit exceeded")
    policy = obj(request["diagnostic_policy"], {"prior_predictive_draws", "maximum_r_hat", "minimum_bulk_ess", "minimum_tail_ess", "minimum_ebfmi", "maximum_divergences", "maximum_tree_depth_hits", "maximum_tree_depth"}, "policy")
    return {
        "population_alpha": population_alpha, "population_beta": population_beta,
        "concentration_sd": concentration_sd, "patient_ids": patient_ids,
        "successes": np.asarray(successes, dtype=np.int64), "trials": np.asarray(trials, dtype=np.int64),
        "chains": chains, "tune": tune, "draws": draws, "target_accept": target_accept, "seed": seed,
        "prior_draws": integer(policy["prior_predictive_draws"], "policy.prior_draws", 1, 100_000),
        "maximum_r_hat": number(policy["maximum_r_hat"], "policy.rhat"),
        "minimum_bulk_ess": number(policy["minimum_bulk_ess"], "policy.bulk"),
        "minimum_tail_ess": number(policy["minimum_tail_ess"], "policy.tail"),
        "minimum_ebfmi": number(policy["minimum_ebfmi"], "policy.ebfmi"),
        "maximum_divergences": integer(policy["maximum_divergences"], "policy.divergences", 0, 2**63 - 1),
        "maximum_tree_depth_hits": integer(policy["maximum_tree_depth_hits"], "policy.depth hits", 0, 2**63 - 1),
        "maximum_tree_depth": integer(policy["maximum_tree_depth"], "policy.depth", 1, 32),
    }


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    digest = hashlib.sha256(f"marklab-pymc-beta-binomial-v1\0{seed}\0{purpose}\0{index}".encode()).digest()
    return int.from_bytes(digest[:4], "little")


def summary(values: np.ndarray) -> dict[str, float]:
    values = values.reshape(-1)
    return {"mean": float(values.mean()), "sd": float(values.std(ddof=1)), "interval_lower": float(np.quantile(values, 0.025)), "interval_upper": float(np.quantile(values, 0.975))}


def tree_values(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    with pm.Model():
        population = pm.Beta("population_probability", config["population_alpha"], config["population_beta"])
        concentration = pm.HalfNormal("concentration", config["concentration_sd"])
        patient_probability = pm.Beta("patient_probability", population * concentration, (1.0 - population) * concentration, shape=len(config["patient_ids"]))
        pm.Binomial("successes", n=config["trials"], p=patient_probability, observed=config["successes"])
        prior = pm.sample_prior_predictive(draws=config["prior_draws"], random_seed=seed_for(config["seed"], "prior"))
        posterior = pm.sample(
            draws=config["draws"], tune=config["tune"], chains=config["chains"], cores=1,
            blas_cores=1, random_seed=[seed_for(config["seed"], "chain", index) for index in range(config["chains"])],
            target_accept=config["target_accept"], nuts_sampler="pymc",
            nuts={"max_treedepth": config["maximum_tree_depth"]}, progressbar=False, quiet=True,
            compute_convergence_checks=False,
        )
        predictive = pm.sample_posterior_predictive(posterior, var_names=["successes"], random_seed=seed_for(config["seed"], "predictive"), progressbar=False)
    population_draws = np.asarray(posterior["posterior"]["population_probability"].values, dtype=np.float64)
    concentration_draws = np.asarray(posterior["posterior"]["concentration"].values, dtype=np.float64)
    patient_draws = np.asarray(posterior["posterior"]["patient_probability"].values, dtype=np.float64)
    replicated = np.asarray(predictive["posterior_predictive"]["successes"].values, dtype=np.int64)
    prior_finite = bool(all(np.isfinite(np.asarray(value.values)).all() for value in prior["prior"].values()))
    posterior_finite = bool(np.isfinite(population_draws).all() and np.isfinite(concentration_draws).all() and np.isfinite(patient_draws).all() and np.isfinite(replicated).all())
    monitored = ["population_probability", "concentration", "patient_probability"]
    r_hat = float(tree_values(pm.stats.rhat(posterior, var_names=monitored, method="rank"), monitored).max())
    bulk = float(tree_values(pm.stats.ess(posterior, var_names=monitored, method="bulk"), monitored).min())
    tail = float(tree_values(pm.stats.ess(posterior, var_names=monitored, method="tail"), monitored).min())
    mcse_mean = float(tree_values(pm.stats.mcse(posterior, var_names=monitored, method="mean"), monitored).max())
    mcse_sd = float(tree_values(pm.stats.mcse(posterior, var_names=monitored, method="sd"), monitored).max())
    energy = np.asarray(posterior["sample_stats"]["energy"].values, dtype=np.float64)
    ebfmi = float(np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1)))
    divergences = int(np.asarray(posterior["sample_stats"]["divergences"].values).sum())
    depth_hits = int(np.asarray(posterior["sample_stats"]["reached_max_treedepth"].values).sum())
    constraints = bool(np.all((0.0 < population_draws) & (population_draws < 1.0)) and np.all(concentration_draws > 0.0) and np.all((0.0 < patient_draws) & (patient_draws < 1.0)))
    complete = prior_finite and posterior_finite and constraints and r_hat <= config["maximum_r_hat"] and bulk >= config["minimum_bulk_ess"] and tail >= config["minimum_tail_ess"] and ebfmi >= config["minimum_ebfmi"] and divergences <= config["maximum_divergences"] and depth_hits <= config["maximum_tree_depth_hits"]
    flat_patient = patient_draws.reshape(-1, patient_draws.shape[-1])
    population_mean = float(population_draws.mean())
    patients = []
    for index, patient_id in enumerate(config["patient_ids"]):
        observed = float(config["successes"][index] / config["trials"][index])
        denominator = population_mean - observed
        shrinkage = 0.0 if denominator == 0.0 else float((flat_patient[:, index].mean() - observed) / denominator)
        patients.append({"patient_id": patient_id, "successes": int(config["successes"][index]), "trials": int(config["trials"][index]), "observed_proportion": observed, "posterior_probability": summary(flat_patient[:, index]), "shrinkage_toward_population": shrinkage})
    replicated_flat = replicated.reshape(-1, replicated.shape[-1])
    replicated_totals = replicated_flat.sum(axis=1)
    replicated_proportion_sd = (replicated_flat / config["trials"]).std(axis=1, ddof=1)
    observed_proportions = config["successes"] / config["trials"]
    return {
        "format": "marklab.pymc_beta_binomial_hierarchy_worker_result", "version": 1,
        "backend": {"name": "pymc", "version": pm.__version__, "python_version": f"{sys.version_info.major}.{sys.version_info.minor}", "environment_lock_sha256": lock_sha, "worker_sha256": worker_sha},
        "request_sha256": request_sha, "fit_state": "complete" if complete else "nonconverged",
        "sampling": {"chains": config["chains"], "tune_per_chain": config["tune"], "draws_per_chain": config["draws"], "completed_draws": config["chains"] * config["draws"]},
        "posterior": {"population_probability": summary(population_draws), "concentration": summary(concentration_draws), "overdispersion_mean": float((1.0 / (concentration_draws + 1.0)).mean())},
        "patients": patients,
        "diagnostics": {"prior_predictive_finite": prior_finite, "posterior_finite": posterior_finite, "r_hat": r_hat, "ess_bulk": bulk, "ess_tail": tail, "mcse_mean": mcse_mean, "mcse_sd": mcse_sd, "minimum_ebfmi": ebfmi, "divergences": divergences, "max_tree_depth_hits": depth_hits, "constraints_valid": constraints, "identifiability_checks_passed": True},
        "posterior_predictive": {"observed_total_successes": int(config["successes"].sum()), "replicated_total_successes_mean": float(replicated_totals.mean()), "replicated_total_successes_sd": float(replicated_totals.std(ddof=1)), "observed_patient_proportion_sd": float(observed_proportions.std(ddof=1)), "replicated_patient_proportion_sd_mean": float(replicated_proportion_sd.mean())},
    }


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
        print(f"Marklab PyMC beta-binomial hierarchy worker failed: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from error
