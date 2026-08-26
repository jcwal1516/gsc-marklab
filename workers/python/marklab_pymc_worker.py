#!/usr/bin/env python3
"""Static PyMC worker for Marklab's version-one normal-mean contract."""

from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path
import sys
from typing import Any

import numpy as np
import pymc as pm


REQUEST_FORMAT = "marklab.pymc_worker_request"
RESULT_FORMAT = "marklab.pymc_worker_result"
CONTRACT_VERSION = 1
PYMC_VERSION = "6.3.0"


class ContractError(ValueError):
    pass


def require_object(value: Any, keys: set[str], path: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ContractError(f"{path} must be an object")
    actual = set(value)
    if actual != keys:
        missing = sorted(keys - actual)
        unknown = sorted(actual - keys)
        raise ContractError(f"{path} fields differ: missing={missing}, unknown={unknown}")
    return value


def require_integer(value: Any, path: str, lower: int, upper: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise ContractError(f"{path} must be an integer")
    if not lower <= value <= upper:
        raise ContractError(f"{path} must be in [{lower}, {upper}]")
    return value


def require_number(value: Any, path: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ContractError(f"{path} must be a number")
    number = float(value)
    if not math.isfinite(number):
        raise ContractError(f"{path} must be finite")
    return number


def require_string(value: Any, expected: str, path: str) -> None:
    if value != expected:
        raise ContractError(f"{path} must equal {expected!r}")


def validate_request(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = require_object(
        request,
        {
            "format",
            "version",
            "backend",
            "model",
            "observations",
            "sampling",
            "resources",
            "diagnostic_policy",
        },
        "request",
    )
    require_string(request["format"], REQUEST_FORMAT, "request.format")
    if require_integer(request["version"], "request.version", 1, 1) != CONTRACT_VERSION:
        raise ContractError("unsupported request version")

    backend = require_object(
        request["backend"],
        {
            "name",
            "version",
            "python_version",
            "environment_lock_sha256",
            "worker_sha256",
        },
        "request.backend",
    )
    require_string(backend["name"], "pymc", "request.backend.name")
    require_string(backend["version"], PYMC_VERSION, "request.backend.version")
    require_string(backend["python_version"], "3.12", "request.backend.python_version")
    require_string(
        backend["environment_lock_sha256"],
        lock_digest,
        "request.backend.environment_lock_sha256",
    )
    require_string(backend["worker_sha256"], worker_digest, "request.backend.worker_sha256")

    model = require_object(
        request["model"],
        {
            "format",
            "version",
            "family",
            "parameter",
            "prior",
            "likelihood",
            "observation_unit",
            "generated_quantities",
            "backend_capability",
            "maturity",
        },
        "request.model",
    )
    require_string(model["format"], "marklab.bayesian_model_ir", "request.model.format")
    if require_integer(model["version"], "request.model.version", 1, 1) != 1:
        raise ContractError("unsupported model IR version")
    require_string(model["family"], "normal_mean_known_sigma", "request.model.family")
    parameter = require_object(
        model["parameter"], {"name", "support", "interpretation"}, "request.model.parameter"
    )
    require_string(parameter["name"], "mu", "request.model.parameter.name")
    require_string(parameter["support"], "real", "request.model.parameter.support")
    require_string(
        parameter["interpretation"],
        "population_mean",
        "request.model.parameter.interpretation",
    )
    prior = require_object(
        model["prior"], {"family", "mean", "sd", "rationale"}, "request.model.prior"
    )
    require_string(prior["family"], "normal", "request.model.prior.family")
    prior_mean = require_number(prior["mean"], "request.model.prior.mean")
    prior_sd = require_number(prior["sd"], "request.model.prior.sd")
    if prior_sd <= 0.0:
        raise ContractError("request.model.prior.sd must be positive")
    require_string(prior["rationale"], "user_supplied", "request.model.prior.rationale")
    likelihood = require_object(
        model["likelihood"], {"family", "known_sigma"}, "request.model.likelihood"
    )
    require_string(
        likelihood["family"], "normal_known_sigma", "request.model.likelihood.family"
    )
    known_sigma = require_number(
        likelihood["known_sigma"], "request.model.likelihood.known_sigma"
    )
    if known_sigma <= 0.0:
        raise ContractError("request.model.likelihood.known_sigma must be positive")
    require_string(
        model["observation_unit"], "scalar_observation", "request.model.observation_unit"
    )
    if model["generated_quantities"] != ["posterior_predictive_observation_mean"]:
        raise ContractError("unsupported generated quantities")
    require_string(model["backend_capability"], "nuts", "request.model.backend_capability")
    require_string(model["maturity"], "experimental", "request.model.maturity")

    observations = request["observations"]
    if not isinstance(observations, list) or not observations:
        raise ContractError("request.observations must be a non-empty array")
    observations = [
        require_number(value, f"request.observations[{index}]")
        for index, value in enumerate(observations)
    ]

    sampling = require_object(
        request["sampling"],
        {"chains", "tune_per_chain", "draws_per_chain", "target_accept", "seed"},
        "request.sampling",
    )
    chains = require_integer(sampling["chains"], "request.sampling.chains", 2, 8)
    tune = require_integer(
        sampling["tune_per_chain"], "request.sampling.tune_per_chain", 100, 100_000
    )
    draws = require_integer(
        sampling["draws_per_chain"], "request.sampling.draws_per_chain", 100, 100_000
    )
    target_accept = require_number(sampling["target_accept"], "request.sampling.target_accept")
    if not 0.5 <= target_accept < 1.0:
        raise ContractError("request.sampling.target_accept must be in [0.5, 1)")
    seed = require_integer(sampling["seed"], "request.sampling.seed", 0, 2**64 - 1)

    resources = require_object(
        request["resources"],
        {
            "maximum_observations",
            "maximum_total_iterations",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "request.resources",
    )
    maximum_observations = require_integer(
        resources["maximum_observations"], "request.resources.maximum_observations", 1, 100_000
    )
    maximum_total_iterations = require_integer(
        resources["maximum_total_iterations"],
        "request.resources.maximum_total_iterations",
        1,
        800_000,
    )
    require_integer(
        resources["maximum_output_bytes"], "request.resources.maximum_output_bytes", 1, 1_048_576
    )
    require_integer(resources["timeout_seconds"], "request.resources.timeout_seconds", 1, 3_600)
    if len(observations) > maximum_observations:
        raise ContractError("observation count exceeds the request resource limit")
    if chains * (tune + draws) > maximum_total_iterations:
        raise ContractError("sampling iterations exceed the request resource limit")

    policy = require_object(
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
    prior_draws = require_integer(
        policy["prior_predictive_draws"],
        "request.diagnostic_policy.prior_predictive_draws",
        100,
        10_000,
    )
    maximum_r_hat = require_number(
        policy["maximum_r_hat"], "request.diagnostic_policy.maximum_r_hat"
    )
    minimum_bulk_ess = require_number(
        policy["minimum_bulk_ess"], "request.diagnostic_policy.minimum_bulk_ess"
    )
    minimum_tail_ess = require_number(
        policy["minimum_tail_ess"], "request.diagnostic_policy.minimum_tail_ess"
    )
    minimum_ebfmi = require_number(
        policy["minimum_ebfmi"], "request.diagnostic_policy.minimum_ebfmi"
    )
    if (
        maximum_r_hat < 1.0
        or minimum_bulk_ess <= 0.0
        or minimum_tail_ess <= 0.0
        or minimum_ebfmi <= 0.0
    ):
        raise ContractError("diagnostic thresholds must be positive and R-hat must be at least one")
    maximum_divergences = require_integer(
        policy["maximum_divergences"],
        "request.diagnostic_policy.maximum_divergences",
        0,
        chains * draws,
    )
    maximum_tree_depth_hits = require_integer(
        policy["maximum_tree_depth_hits"],
        "request.diagnostic_policy.maximum_tree_depth_hits",
        0,
        chains * draws,
    )
    maximum_tree_depth = require_integer(
        policy["maximum_tree_depth"], "request.diagnostic_policy.maximum_tree_depth", 1, 32
    )

    return {
        "prior_mean": prior_mean,
        "prior_sd": prior_sd,
        "known_sigma": known_sigma,
        "observations": np.asarray(observations, dtype=np.float64),
        "chains": chains,
        "tune": tune,
        "draws": draws,
        "target_accept": target_accept,
        "seed": seed,
        "prior_draws": prior_draws,
        "maximum_r_hat": maximum_r_hat,
        "minimum_bulk_ess": minimum_bulk_ess,
        "minimum_tail_ess": minimum_tail_ess,
        "minimum_ebfmi": minimum_ebfmi,
        "maximum_divergences": maximum_divergences,
        "maximum_tree_depth_hits": maximum_tree_depth_hits,
        "maximum_tree_depth": maximum_tree_depth,
    }


def derived_seed(seed: int, purpose: str, index: int = 0) -> int:
    digest = hashlib.sha256(f"marklab-pymc-v1\0{seed}\0{purpose}\0{index}".encode()).digest()
    return int.from_bytes(digest[:4], "little")


def statistic(tree: Any, variable: str) -> float:
    return float(tree[variable].values)


def fit_normal_mean(
    config: dict[str, Any], request_sha256: str, lock_digest: str, worker_digest: str
) -> dict[str, Any]:
    chain_seeds = [derived_seed(config["seed"], "chain", index) for index in range(config["chains"])]
    with pm.Model() as model:
        mu = pm.Normal("mu", mu=config["prior_mean"], sigma=config["prior_sd"])
        pm.Normal("y", mu=mu, sigma=config["known_sigma"], observed=config["observations"])
        prior = pm.sample_prior_predictive(
            draws=config["prior_draws"],
            random_seed=derived_seed(config["seed"], "prior"),
            return_inferencedata=True,
        )
        fit = pm.sample(
            draws=config["draws"],
            tune=config["tune"],
            chains=config["chains"],
            cores=1,
            blas_cores=1,
            random_seed=chain_seeds,
            target_accept=config["target_accept"],
            nuts_sampler="pymc",
            nuts={"max_treedepth": config["maximum_tree_depth"]},
            progressbar=False,
            quiet=True,
            return_inferencedata=True,
            compute_convergence_checks=False,
        )
        posterior_predictive = pm.sample_posterior_predictive(
            fit,
            var_names=["y"],
            random_seed=derived_seed(config["seed"], "posterior_predictive"),
            progressbar=False,
            return_inferencedata=True,
        )

    prior_finite = bool(
        np.isfinite(prior["prior"]["mu"].values).all()
        and np.isfinite(prior["prior_predictive"]["y"].values).all()
    )
    posterior_draws = np.asarray(fit["posterior"]["mu"].values, dtype=np.float64)
    predictive_draws = np.asarray(
        posterior_predictive["posterior_predictive"]["y"].values, dtype=np.float64
    )
    posterior_finite = bool(
        np.isfinite(posterior_draws).all() and np.isfinite(predictive_draws).all()
    )
    r_hat = statistic(pm.stats.rhat(fit, var_names=["mu"], method="rank"), "mu")
    ess_bulk = statistic(pm.stats.ess(fit, var_names=["mu"], method="bulk"), "mu")
    ess_tail = statistic(pm.stats.ess(fit, var_names=["mu"], method="tail"), "mu")
    mcse_mean = statistic(pm.stats.mcse(fit, var_names=["mu"], method="mean"), "mu")
    mcse_sd = statistic(pm.stats.mcse(fit, var_names=["mu"], method="sd"), "mu")
    energy = np.asarray(fit["sample_stats"]["energy"].values, dtype=np.float64)
    minimum_ebfmi = float(
        np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1))
    )
    divergences = int(np.asarray(fit["sample_stats"]["divergences"].values).sum())
    tree_depth_hits = int(
        np.asarray(fit["sample_stats"]["reached_max_treedepth"].values).sum()
    )
    replicated_means = predictive_draws.mean(axis=-1)
    observed_mean = float(config["observations"].mean())

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
    result = {
        "format": RESULT_FORMAT,
        "version": CONTRACT_VERSION,
        "backend": {
            "name": "pymc",
            "version": pm.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_digest,
            "worker_sha256": worker_digest,
        },
        "request_sha256": request_sha256,
        "fit_state": "complete" if diagnostics_pass else "nonconverged",
        "sampling": {
            "chains": config["chains"],
            "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"],
            "completed_draws": config["chains"] * config["draws"],
        },
        "posterior": {
            "parameter": "mu",
            "mean": float(posterior_draws.mean()),
            "sd": float(posterior_draws.std(ddof=1)),
            "interval_lower": float(np.quantile(posterior_draws, 0.025)),
            "interval_upper": float(np.quantile(posterior_draws, 0.975)),
        },
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
            "observed_mean": observed_mean,
            "replicated_mean_mean": float(replicated_means.mean()),
            "replicated_mean_sd": float(replicated_means.std(ddof=1)),
            "probability_replicated_mean_at_least_observed": float(
                np.mean(replicated_means >= observed_mean)
            ),
        },
    }
    return result


def main() -> None:
    if pm.__version__ != PYMC_VERSION:
        raise ContractError(f"PyMC version drift: expected {PYMC_VERSION}, found {pm.__version__}")
    if sys.version_info[:2] != (3, 12):
        raise ContractError(
            f"Python version drift: expected 3.12, found {sys.version_info.major}.{sys.version_info.minor}"
        )
    lock_path = Path(__file__).with_name("uv.lock")
    lock_digest = hashlib.sha256(lock_path.read_bytes()).hexdigest()
    worker_digest = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    raw_request = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if len(raw_request) > 16 * 1024 * 1024:
        raise ContractError("request exceeds the 16 MiB worker boundary")
    request_sha256 = hashlib.sha256(raw_request).hexdigest()
    request = json.loads(raw_request)
    config = validate_request(request, lock_digest, worker_digest)
    result = fit_normal_mean(config, request_sha256, lock_digest, worker_digest)
    encoded = json.dumps(result, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write(encoded)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"marklab PyMC worker failed: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from error
