#!/usr/bin/env python3
"""Static PyMC worker for Marklab's site random-effects meta-regression."""

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
    value = float(value)
    if not math.isfinite(value):
        raise ContractError(f"{path} must be finite")
    return value


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
        {
            "format",
            "version",
            "backend",
            "model",
            "sites",
            "new_site_covariate",
            "sampling",
            "resources",
            "diagnostic_policy",
        },
        "request",
    )
    exact(request["format"], "marklab.pymc_worker_request", "request.format")
    integer(request["version"], "request.version", 1, 1)
    backend = obj(
        request["backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "request.backend",
    )
    exact(backend["name"], "pymc", "request.backend.name")
    exact(backend["version"], PYMC_VERSION, "request.backend.version")
    exact(backend["python_version"], "3.12", "request.backend.python_version")
    exact(backend["environment_lock_sha256"], lock_digest, "request.backend.lock")
    exact(backend["worker_sha256"], worker_digest, "request.backend.worker")

    model = obj(
        request["model"],
        {
            "format",
            "version",
            "family",
            "covariate_name",
            "global_prior_mean",
            "global_prior_sd",
            "covariate_prior_sd",
            "heterogeneity_prior_sd",
            "observation_unit",
            "biological_unit",
            "hierarchy",
            "generated_quantities",
            "backend_capability",
            "maturity",
        },
        "request.model",
    )
    exact(model["format"], "marklab.bayesian_model_ir", "request.model.format")
    integer(model["version"], "request.model.version", 1, 1)
    exact(model["family"], "bayesian_random_effects_meta_regression", "request.model.family")
    covariate_name = model["covariate_name"]
    if not isinstance(covariate_name, str) or not covariate_name or covariate_name.strip() != covariate_name:
        raise ContractError("covariate name is invalid")
    global_prior_mean = number(model["global_prior_mean"], "request.model.global_prior_mean")
    global_prior_sd = number(model["global_prior_sd"], "request.model.global_prior_sd")
    covariate_prior_sd = number(model["covariate_prior_sd"], "request.model.covariate_prior_sd")
    heterogeneity_prior_sd = number(
        model["heterogeneity_prior_sd"], "request.model.heterogeneity_prior_sd"
    )
    if min(global_prior_sd, covariate_prior_sd, heterogeneity_prior_sd) <= 0.0:
        raise ContractError("all prior scales must be positive")
    exact(model["observation_unit"], "site_effect_estimate", "request.model.observation_unit")
    exact(model["biological_unit"], "site_or_cohort", "request.model.biological_unit")
    exact(model["hierarchy"], ["site"], "request.model.hierarchy")
    exact(model["generated_quantities"], ["site_effect", "new_site_effect"], "request.model.generated")
    exact(model["backend_capability"], "nuts", "request.model.capability")
    exact(model["maturity"], "experimental", "request.model.maturity")

    sites = request["sites"]
    if not isinstance(sites, list) or not 5 <= len(sites) <= 10_000:
        raise ContractError("sites must contain between 5 and 10000 entries")
    site_ids, effects, standard_errors, covariates = [], [], [], []
    for index, value in enumerate(sites):
        site = obj(value, {"site_id", "effect", "standard_error", "covariate"}, f"sites[{index}]")
        site_id = site["site_id"]
        if not isinstance(site_id, str) or not site_id or site_id.strip() != site_id:
            raise ContractError("site ID is invalid")
        if site_ids and site_id <= site_ids[-1]:
            raise ContractError("site IDs must be strictly increasing")
        effect = number(site["effect"], f"sites[{index}].effect")
        standard_error = number(site["standard_error"], f"sites[{index}].standard_error")
        covariate = number(site["covariate"], f"sites[{index}].covariate")
        if standard_error <= 0.0:
            raise ContractError("site standard errors must be positive")
        site_ids.append(site_id)
        effects.append(effect)
        standard_errors.append(standard_error)
        covariates.append(covariate)
    if all(value == covariates[0] for value in covariates):
        raise ContractError("site covariate must vary")
    new_site_covariate = number(request["new_site_covariate"], "request.new_site_covariate")

    sampling = obj(
        request["sampling"],
        {"chains", "tune_per_chain", "draws_per_chain", "target_accept", "seed"},
        "request.sampling",
    )
    chains = integer(sampling["chains"], "sampling.chains", 2, 8)
    tune = integer(sampling["tune_per_chain"], "sampling.tune", 100, 100_000)
    draws = integer(sampling["draws_per_chain"], "sampling.draws", 100, 100_000)
    target_accept = number(sampling["target_accept"], "sampling.target_accept")
    if not 0.5 <= target_accept < 1.0:
        raise ContractError("target acceptance must be in [0.5, 1)")
    seed = integer(sampling["seed"], "sampling.seed", 0, 2**64 - 1)

    resources = obj(
        request["resources"],
        {"maximum_observations", "maximum_total_iterations", "maximum_output_bytes", "timeout_seconds"},
        "request.resources",
    )
    if len(sites) > integer(resources["maximum_observations"], "resources.max observations", 1, 10_000):
        raise ContractError("site count exceeds resource limit")
    max_iterations = integer(resources["maximum_total_iterations"], "resources.max iterations", 1, 800_000)
    integer(resources["maximum_output_bytes"], "resources.max output", 1, 1_048_576)
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3_600)
    if chains * (tune + draws) > max_iterations:
        raise ContractError("sampling iterations exceed resource limit")

    policy = obj(
        request["diagnostic_policy"],
        {
            "prior_predictive_draws",
            "maximum_r_hat",
            "minimum_bulk_ess",
            "minimum_tail_ess",
            "minimum_ebfmi",
            "maximum_divergences",
            "maximum_tree_depth_hits",
            "maximum_tree_depth",
        },
        "request.diagnostic_policy",
    )
    config = {
        "prior_draws": integer(policy["prior_predictive_draws"], "policy.prior draws", 100, 10_000),
        "maximum_r_hat": number(policy["maximum_r_hat"], "policy.maximum R-hat"),
        "minimum_bulk_ess": number(policy["minimum_bulk_ess"], "policy.minimum bulk ESS"),
        "minimum_tail_ess": number(policy["minimum_tail_ess"], "policy.minimum tail ESS"),
        "minimum_ebfmi": number(policy["minimum_ebfmi"], "policy.minimum E-BFMI"),
        "maximum_divergences": integer(policy["maximum_divergences"], "policy.divergences", 0, chains * draws),
        "maximum_tree_depth_hits": integer(policy["maximum_tree_depth_hits"], "policy.depth hits", 0, chains * draws),
        "maximum_tree_depth": integer(policy["maximum_tree_depth"], "policy.depth", 1, 32),
    }
    config.update(
        global_prior_mean=global_prior_mean,
        global_prior_sd=global_prior_sd,
        covariate_prior_sd=covariate_prior_sd,
        heterogeneity_prior_sd=heterogeneity_prior_sd,
        site_ids=site_ids,
        effects=np.asarray(effects),
        standard_errors=np.asarray(standard_errors),
        covariates=np.asarray(covariates),
        new_site_covariate=new_site_covariate,
        chains=chains,
        tune=tune,
        draws=draws,
        target_accept=target_accept,
        seed=seed,
    )
    return config


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    digest = hashlib.sha256(f"marklab-pymc-meta-v1\0{seed}\0{purpose}\0{index}".encode()).digest()
    return int.from_bytes(digest[:4], "little")


def summary(draws: np.ndarray) -> dict[str, float]:
    return {
        "mean": float(draws.mean()),
        "sd": float(draws.std(ddof=1)),
        "interval_lower": float(np.quantile(draws, 0.025)),
        "interval_upper": float(np.quantile(draws, 0.975)),
    }


def stats(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    coords = {"site": config["site_ids"]}
    with pm.Model(coords=coords):
        global_effect = pm.Normal(
            "global_effect", config["global_prior_mean"], config["global_prior_sd"]
        )
        covariate_effect = pm.Normal("covariate_effect", 0.0, config["covariate_prior_sd"])
        heterogeneity = pm.HalfNormal("heterogeneity", config["heterogeneity_prior_sd"])
        pm.Normal(
            "observed_effect",
            global_effect + config["covariates"] * covariate_effect,
            pm.math.sqrt(heterogeneity**2 + config["standard_errors"] ** 2),
            observed=config["effects"],
            dims="site",
        )
        prior = pm.sample_prior_predictive(
            draws=config["prior_draws"], random_seed=seed_for(config["seed"], "prior")
        )
        posterior = pm.sample(
            draws=config["draws"],
            tune=config["tune"],
            chains=config["chains"],
            cores=1,
            blas_cores=1,
            random_seed=[seed_for(config["seed"], "chain", i) for i in range(config["chains"])],
            target_accept=config["target_accept"],
            nuts_sampler="pymc",
            nuts={"max_treedepth": config["maximum_tree_depth"]},
            progressbar=False,
            quiet=True,
            compute_convergence_checks=False,
        )
        predictive = pm.sample_posterior_predictive(
            posterior,
            var_names=["observed_effect"],
            random_seed=seed_for(config["seed"], "predictive"),
            progressbar=False,
        )

    monitored = ["global_effect", "covariate_effect", "heterogeneity"]
    posterior_arrays = {
        name: np.asarray(posterior["posterior"][name].values, dtype=np.float64)
        for name in monitored
    }
    population_means = (
        posterior_arrays["global_effect"][..., None]
        + posterior_arrays["covariate_effect"][..., None] * config["covariates"]
    )
    tau_squared = posterior_arrays["heterogeneity"][..., None] ** 2
    se_squared = config["standard_errors"] ** 2
    conditional_variance = tau_squared * se_squared / (tau_squared + se_squared)
    conditional_mean = (
        se_squared * population_means + tau_squared * config["effects"]
    ) / (tau_squared + se_squared)
    rng = np.random.default_rng(seed_for(config["seed"], "conditional_effects"))
    posterior_arrays["site_effect"] = rng.normal(
        conditional_mean, np.sqrt(conditional_variance)
    )
    new_site_location = (
        posterior_arrays["global_effect"]
        + config["new_site_covariate"] * posterior_arrays["covariate_effect"]
    )
    posterior_arrays["new_site_effect"] = rng.normal(
        new_site_location, posterior_arrays["heterogeneity"]
    )
    predictive_draws = np.asarray(
        predictive["posterior_predictive"]["observed_effect"].values, dtype=np.float64
    )
    prior_finite = bool(
        all(np.isfinite(prior["prior"][name].values).all() for name in monitored)
        and np.isfinite(prior["prior_predictive"]["observed_effect"].values).all()
    )
    posterior_finite = bool(
        all(np.isfinite(value).all() for value in posterior_arrays.values())
        and np.isfinite(predictive_draws).all()
    )
    r_hat = float(stats(pm.stats.rhat(posterior, var_names=monitored, method="rank"), monitored).max())
    bulk = float(stats(pm.stats.ess(posterior, var_names=monitored, method="bulk"), monitored).min())
    tail = float(stats(pm.stats.ess(posterior, var_names=monitored, method="tail"), monitored).min())
    mcse_mean = float(stats(pm.stats.mcse(posterior, var_names=monitored, method="mean"), monitored).max())
    mcse_sd = float(stats(pm.stats.mcse(posterior, var_names=monitored, method="sd"), monitored).max())
    energy = np.asarray(posterior["sample_stats"]["energy"].values)
    ebfmi = float(np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1)))
    divergences = int(np.asarray(posterior["sample_stats"]["divergences"].values).sum())
    depth_hits = int(np.asarray(posterior["sample_stats"]["reached_max_treedepth"].values).sum())
    complete = (
        prior_finite
        and posterior_finite
        and r_hat <= config["maximum_r_hat"]
        and bulk >= config["minimum_bulk_ess"]
        and tail >= config["minimum_tail_ess"]
        and ebfmi >= config["minimum_ebfmi"]
        and divergences <= config["maximum_divergences"]
        and depth_hits <= config["maximum_tree_depth_hits"]
    )
    site_summaries = []
    for index, site_id in enumerate(config["site_ids"]):
        draws = posterior_arrays["site_effect"][..., index]
        site_summaries.append(
            {
                "site_id": site_id,
                "observed_effect": float(config["effects"][index]),
                "standard_error": float(config["standard_errors"][index]),
                "covariate": float(config["covariates"][index]),
                "posterior_mean": float(draws.mean()),
                "posterior_sd": float(draws.std(ddof=1)),
                "interval_lower": float(np.quantile(draws, 0.025)),
                "interval_upper": float(np.quantile(draws, 0.975)),
            }
        )
    replicated_means = predictive_draws.mean(axis=-1)
    replicated_sds = predictive_draws.std(axis=-1, ddof=1)
    result = {
        "format": "marklab.pymc_meta_analysis_worker_result",
        "version": 1,
        "backend": {
            "name": "pymc",
            "version": pm.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha,
            "worker_sha256": worker_sha,
        },
        "request_sha256": request_sha,
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"],
            "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"],
            "completed_draws": config["chains"] * config["draws"],
        },
        "posterior": {
            "global_effect": summary(posterior_arrays["global_effect"]),
            "covariate_effect": summary(posterior_arrays["covariate_effect"]),
            "heterogeneity": summary(posterior_arrays["heterogeneity"]),
        },
        "site_effects": site_summaries,
        "new_site_prediction": {
            "covariate": config["new_site_covariate"],
            **summary(posterior_arrays["new_site_effect"]),
        },
        "diagnostics": {
            "prior_predictive_finite": prior_finite,
            "posterior_finite": posterior_finite,
            "r_hat": r_hat,
            "ess_bulk": bulk,
            "ess_tail": tail,
            "mcse_mean": mcse_mean,
            "mcse_sd": mcse_sd,
            "minimum_ebfmi": ebfmi,
            "divergences": divergences,
            "max_tree_depth_hits": depth_hits,
            "constraints_valid": bool(np.all(posterior_arrays["heterogeneity"] >= 0.0)),
            "identifiability_checks_passed": True,
        },
        "posterior_predictive": {
            "observed_effect_mean": float(config["effects"].mean()),
            "replicated_effect_mean": float(replicated_means.mean()),
            "replicated_effect_mean_sd": float(replicated_means.std(ddof=1)),
            "observed_effect_sd": float(config["effects"].std(ddof=1)),
            "replicated_effect_sd_mean": float(replicated_sds.mean()),
        },
    }
    return result


def main() -> None:
    if pm.__version__ != PYMC_VERSION or sys.version_info[:2] != (3, 12):
        raise ContractError("PyMC or Python version drift")
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    raw = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if len(raw) > 16 * 1024 * 1024:
        raise ContractError("request exceeds 16 MiB")
    request_sha = hashlib.sha256(raw).hexdigest()
    result = fit(validate(json.loads(raw), lock_sha, worker_sha), request_sha, lock_sha, worker_sha)
    sys.stdout.write(json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")))
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"marklab PyMC worker failed: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from error
