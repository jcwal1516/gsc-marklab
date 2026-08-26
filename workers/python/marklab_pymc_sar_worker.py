#!/usr/bin/env python3
"""Static PyMC worker for Marklab's Gaussian spatial autoregressive fit."""

from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path
import sys
from typing import Any

import numpy as np
import pymc as pm
import pytensor.tensor as pt

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


def numeric_vector(value: Any, length: int, path: str) -> np.ndarray:
    if not isinstance(value, list) or len(value) != length:
        raise ContractError(f"{path} must contain exactly {length} values")
    result = np.asarray([number(item, f"{path}[]") for item in value], dtype=np.float64)
    return result


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "model",
            "region_ids",
            "weights",
            "weights_digest_sha256",
            "response",
            "design",
            "predictor_names",
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
    exact(backend["name"], "pymc", "backend.name")
    exact(backend["version"], PYMC_VERSION, "backend.version")
    exact(backend["python_version"], "3.12", "backend.python_version")
    exact(backend["environment_lock_sha256"], lock_digest, "backend.lock")
    exact(backend["worker_sha256"], worker_digest, "backend.worker")

    model = obj(
        request["model"],
        {
            "format",
            "version",
            "family",
            "model_type",
            "interpretation",
            "intercept_prior_mean",
            "intercept_prior_sd",
            "coefficient_prior_sd",
            "rho_bound",
            "sigma_prior_sd",
            "weight_normalization",
            "likelihood_jacobian",
            "backend_capability",
            "maturity",
        },
        "request.model",
    )
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    integer(model["version"], "model.version", 1, 1)
    exact(model["family"], "gaussian_spatial_autoregressive", "model.family")
    model_type = model["model_type"]
    if model_type not in {"lag", "error"}:
        raise ContractError("model type must be lag or error")
    exact(model["interpretation"], "descriptive", "model.interpretation")
    exact(
        model["weight_normalization"],
        "row_standardized_zero_diagonal_island_free",
        "model.weight_normalization",
    )
    exact(
        model["likelihood_jacobian"],
        "log_abs_determinant_i_minus_rho_w",
        "model.likelihood_jacobian",
    )
    exact(model["backend_capability"], "nuts", "model.backend_capability")
    exact(model["maturity"], "experimental", "model.maturity")
    intercept_prior_mean = number(model["intercept_prior_mean"], "model.intercept prior mean")
    intercept_prior_sd = number(model["intercept_prior_sd"], "model.intercept prior sd")
    coefficient_prior_sd = number(model["coefficient_prior_sd"], "model.coefficient prior sd")
    rho_bound = number(model["rho_bound"], "model.rho bound")
    sigma_prior_sd = number(model["sigma_prior_sd"], "model.sigma prior sd")
    if min(intercept_prior_sd, coefficient_prior_sd, sigma_prior_sd) <= 0.0:
        raise ContractError("prior scales must be positive")
    if not 0.0 < rho_bound < 1.0:
        raise ContractError("rho bound must be in (0,1)")

    region_ids = request["region_ids"]
    if not isinstance(region_ids, list) or not 6 <= len(region_ids) <= 64:
        raise ContractError("region IDs must contain 6-64 entries")
    if any(
        not isinstance(value, str)
        or not value
        or value.strip() != value
        or (index > 0 and region_ids[index - 1] >= value)
        for index, value in enumerate(region_ids)
    ):
        raise ContractError("region IDs must be exact and strictly increasing")
    dimension = len(region_ids)
    predictor_names = request["predictor_names"]
    if not isinstance(predictor_names, list) or not 1 <= len(predictor_names) <= 16:
        raise ContractError("predictor names must contain 1-16 entries")
    if any(
        not isinstance(value, str)
        or not value
        or value.strip() != value
        or value == "intercept"
        or value in predictor_names[:index]
        for index, value in enumerate(predictor_names)
    ):
        raise ContractError("predictor names are invalid")
    predictors = len(predictor_names)
    weights = numeric_vector(request["weights"], dimension * dimension, "weights").reshape(
        dimension, dimension
    )
    if np.any(weights < 0.0) or np.any(np.diag(weights) != 0.0):
        raise ContractError("weights must be nonnegative with zero diagonal")
    if not np.allclose(weights.sum(axis=1), 1.0, rtol=0.0, atol=1e-12):
        raise ContractError("weights must be row standardized without islands")
    digest = request["weights_digest_sha256"]
    if not isinstance(digest, str) or len(digest) != 64 or any(
        value not in "0123456789abcdef" for value in digest.lower()
    ):
        raise ContractError("weights digest is invalid")
    response = numeric_vector(request["response"], dimension, "response")
    design = numeric_vector(request["design"], dimension * predictors, "design").reshape(
        dimension, predictors
    )
    if np.linalg.matrix_rank(np.column_stack([np.ones(dimension), design])) != predictors + 1:
        raise ContractError("design including intercept must have full column rank")

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
        raise ContractError("target acceptance must be in [0.5,1)")
    seed = integer(sampling["seed"], "sampling.seed", 0, 2**64 - 1)

    resources = obj(
        request["resources"],
        {"maximum_observations", "maximum_total_iterations", "maximum_output_bytes", "timeout_seconds"},
        "request.resources",
    )
    if dimension > integer(resources["maximum_observations"], "resources.observations", 1, 64):
        raise ContractError("region count exceeds resource limit")
    max_iterations = integer(resources["maximum_total_iterations"], "resources.iterations", 1, 400_000)
    integer(resources["maximum_output_bytes"], "resources.output", 1, 1_048_576)
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3_600)
    if chains * (tune + draws) > max_iterations:
        raise ContractError("iterations exceed resource limit")

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
        "maximum_r_hat": number(policy["maximum_r_hat"], "policy.rhat"),
        "minimum_bulk_ess": number(policy["minimum_bulk_ess"], "policy.bulk"),
        "minimum_tail_ess": number(policy["minimum_tail_ess"], "policy.tail"),
        "minimum_ebfmi": number(policy["minimum_ebfmi"], "policy.ebfmi"),
        "maximum_divergences": integer(policy["maximum_divergences"], "policy.divergences", 0, chains * draws),
        "maximum_tree_depth_hits": integer(policy["maximum_tree_depth_hits"], "policy.depth hits", 0, chains * draws),
        "maximum_tree_depth": integer(policy["maximum_tree_depth"], "policy.depth", 1, 32),
        "model_type": model_type,
        "intercept_prior_mean": intercept_prior_mean,
        "intercept_prior_sd": intercept_prior_sd,
        "coefficient_prior_sd": coefficient_prior_sd,
        "rho_bound": rho_bound,
        "sigma_prior_sd": sigma_prior_sd,
        "region_ids": region_ids,
        "predictor_names": predictor_names,
        "weights": weights,
        "response": response,
        "design": design,
        "chains": chains,
        "tune": tune,
        "draws": draws,
        "target_accept": target_accept,
        "seed": seed,
    }
    return config


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    digest = hashlib.sha256(f"marklab-pymc-sar-v1\0{seed}\0{purpose}\0{index}".encode()).digest()
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
    dimension = len(config["region_ids"])
    predictors = len(config["predictor_names"])
    identity = np.eye(dimension)
    with pm.Model():
        intercept = pm.Normal(
            "intercept", config["intercept_prior_mean"], config["intercept_prior_sd"]
        )
        coefficient = pm.Normal(
            "coefficient", 0.0, config["coefficient_prior_sd"], shape=predictors
        )
        rho = pm.Uniform("rho", -config["rho_bound"], config["rho_bound"])
        sigma = pm.HalfNormal("sigma", config["sigma_prior_sd"])
        mean = intercept + pt.dot(config["design"], coefficient)
        system = identity - rho * config["weights"]
        if config["model_type"] == "lag":
            residual = pt.dot(system, config["response"]) - mean
        else:
            residual = pt.dot(system, config["response"] - mean)
        log_jacobian = pt.log(pt.abs(pt.linalg.det(system)))
        pm.Potential(
            "sar_likelihood",
            log_jacobian + pm.logp(pm.Normal.dist(0.0, sigma), residual).sum(),
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

    arrays = {
        name: np.asarray(posterior["posterior"][name].values, dtype=np.float64)
        for name in ["intercept", "coefficient", "rho", "sigma"]
    }
    flat_intercept = arrays["intercept"].reshape(-1)
    flat_coefficient = arrays["coefficient"].reshape(-1, predictors)
    flat_rho = arrays["rho"].reshape(-1)
    flat_sigma = arrays["sigma"].reshape(-1)
    rng = np.random.default_rng(seed_for(config["seed"], "predictive"))
    replicated = np.empty((flat_rho.size, dimension), dtype=np.float64)
    direct = np.empty((flat_rho.size, predictors), dtype=np.float64)
    total = np.empty((flat_rho.size, predictors), dtype=np.float64)
    determinants = np.empty(flat_rho.size, dtype=np.float64)
    for draw in range(flat_rho.size):
        system_draw = identity - flat_rho[draw] * config["weights"]
        sign, log_determinant = np.linalg.slogdet(system_draw)
        determinants[draw] = log_determinant if sign != 0 else np.nan
        mean_draw = flat_intercept[draw] + config["design"] @ flat_coefficient[draw]
        innovation = rng.normal(0.0, flat_sigma[draw], size=dimension)
        if config["model_type"] == "lag":
            inverse = np.linalg.inv(system_draw)
            replicated[draw] = inverse @ (mean_draw + innovation)
            direct_multiplier = np.trace(inverse) / dimension
            total_multiplier = inverse.sum() / dimension
            direct[draw] = flat_coefficient[draw] * direct_multiplier
            total[draw] = flat_coefficient[draw] * total_multiplier
        else:
            replicated[draw] = mean_draw + np.linalg.solve(system_draw, innovation)

    monitored = ["intercept", "coefficient", "rho", "sigma"]
    prior_rho = np.asarray(prior["prior"]["rho"].values, dtype=np.float64).reshape(-1)
    prior_determinants = np.asarray(
        [np.linalg.slogdet(identity - value * config["weights"])[1] for value in prior_rho]
    )
    prior_finite = bool(
        all(np.isfinite(prior["prior"][name].values).all() for name in monitored)
        and np.isfinite(prior_determinants).all()
    )
    posterior_finite = bool(
        all(np.isfinite(value).all() for value in arrays.values())
        and np.isfinite(replicated).all()
        and np.isfinite(determinants).all()
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
    constraints_valid = bool(
        np.all(np.abs(flat_rho) < config["rho_bound"])
        and np.all(flat_sigma > 0.0)
        and np.isfinite(determinants).all()
    )
    complete = (
        prior_finite
        and posterior_finite
        and constraints_valid
        and r_hat <= config["maximum_r_hat"]
        and bulk >= config["minimum_bulk_ess"]
        and tail >= config["minimum_tail_ess"]
        and ebfmi >= config["minimum_ebfmi"]
        and divergences <= config["maximum_divergences"]
        and depth_hits <= config["maximum_tree_depth_hits"]
    )
    coefficient_summaries = [
        {"predictor": name, **summary(flat_coefficient[:, index])}
        for index, name in enumerate(config["predictor_names"])
    ]
    impacts = []
    if config["model_type"] == "lag":
        impacts = [
            {
                "predictor": name,
                "direct": summary(direct[:, index]),
                "indirect": summary(total[:, index] - direct[:, index]),
                "total": summary(total[:, index]),
            }
            for index, name in enumerate(config["predictor_names"])
        ]
    replicated_means = replicated.mean(axis=1)
    replicated_sds = replicated.std(axis=1, ddof=1)
    return {
        "format": "marklab.pymc_sar_worker_result",
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
            "intercept": summary(flat_intercept),
            "coefficients": coefficient_summaries,
            "rho": summary(flat_rho),
            "sigma": summary(flat_sigma),
        },
        "impacts": impacts,
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
            "constraints_valid": constraints_valid,
            "identifiability_checks_passed": True,
        },
        "posterior_predictive": {
            "observed_mean": float(config["response"].mean()),
            "observed_sd": float(config["response"].std(ddof=1)),
            "replicated_mean_mean": float(replicated_means.mean()),
            "replicated_mean_sd": float(replicated_means.std(ddof=1)),
            "replicated_sd_mean": float(replicated_sds.mean()),
        },
    }


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
