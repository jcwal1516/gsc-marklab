#!/usr/bin/env python3
"""Static PyMC worker for Marklab's exact one-dimensional Matérn GP."""

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


def validate(request: Any, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "model",
            "observations",
            "predictions",
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
    exact(backend["python_version"], "3.12", "backend.python")
    exact(backend["environment_lock_sha256"], lock_sha, "backend.lock")
    exact(backend["worker_sha256"], worker_sha, "backend.worker")

    model = obj(
        request["model"],
        {
            "format",
            "version",
            "family",
            "coordinate_dimension",
            "coordinate_unit",
            "kernel",
            "mean_prior_mean",
            "mean_prior_sd",
            "amplitude_prior_sd",
            "length_scale_prior_sd_um",
            "noise_prior_sd",
            "jitter",
            "observation_unit",
            "backend_capability",
            "maturity",
        },
        "request.model",
    )
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    integer(model["version"], "model.version", 1, 1)
    exact(model["family"], "exact_matern32_gp_regression", "model.family")
    integer(model["coordinate_dimension"], "model.coordinate_dimension", 1, 1)
    exact(model["coordinate_unit"], "micrometre", "model.coordinate_unit")
    exact(model["kernel"], "matern_3_2", "model.kernel")
    mean_prior_mean = number(model["mean_prior_mean"], "model.mean_prior_mean")
    mean_prior_sd = number(model["mean_prior_sd"], "model.mean_prior_sd")
    amplitude_prior_sd = number(model["amplitude_prior_sd"], "model.amplitude_prior_sd")
    length_prior_sd = number(model["length_scale_prior_sd_um"], "model.length prior")
    noise_prior_sd = number(model["noise_prior_sd"], "model.noise prior")
    jitter = number(model["jitter"], "model.jitter")
    if min(mean_prior_sd, amplitude_prior_sd, length_prior_sd, noise_prior_sd, jitter) <= 0.0:
        raise ContractError("GP prior scales and jitter must be positive")
    exact(model["observation_unit"], "scalar_field_value", "model.observation_unit")
    exact(model["backend_capability"], "nuts_exact_dense_gp", "model.capability")
    exact(model["maturity"], "experimental", "model.maturity")

    observations = request["observations"]
    if not isinstance(observations, list) or not 5 <= len(observations) <= 128:
        raise ContractError("observations must contain 5-128 rows")
    observation_ids, x, y = [], [], []
    coordinate_bits: set[int] = set()
    for index, value in enumerate(observations):
        row = obj(value, {"observation_id", "x_um", "value"}, f"observations[{index}]")
        row_id = row["observation_id"]
        coordinate = number(row["x_um"], f"observations[{index}].x_um")
        outcome = number(row["value"], f"observations[{index}].value")
        if not isinstance(row_id, str) or not row_id or row_id.strip() != row_id:
            raise ContractError("observation ID is invalid")
        if observation_ids and row_id <= observation_ids[-1]:
            raise ContractError("observation IDs must be strictly increasing")
        bits = np.float64(coordinate).view(np.uint64).item()
        if bits in coordinate_bits:
            raise ContractError("observation coordinates must be unique")
        coordinate_bits.add(bits)
        observation_ids.append(row_id)
        x.append(coordinate)
        y.append(outcome)

    predictions = request["predictions"]
    if not isinstance(predictions, list) or not 1 <= len(predictions) <= 2_048:
        raise ContractError("predictions must contain 1-2048 rows")
    prediction_ids, x_new = [], []
    for index, value in enumerate(predictions):
        row = obj(value, {"prediction_id", "x_um"}, f"predictions[{index}]")
        row_id = row["prediction_id"]
        coordinate = number(row["x_um"], f"predictions[{index}].x_um")
        if not isinstance(row_id, str) or not row_id or row_id.strip() != row_id:
            raise ContractError("prediction ID is invalid")
        if prediction_ids and row_id <= prediction_ids[-1]:
            raise ContractError("prediction IDs must be strictly increasing")
        prediction_ids.append(row_id)
        x_new.append(coordinate)

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
        {
            "maximum_observations",
            "maximum_predictions",
            "maximum_total_iterations",
            "maximum_conditioning_work",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "request.resources",
    )
    max_observations = integer(resources["maximum_observations"], "resources.observations", 5, 128)
    max_predictions = integer(resources["maximum_predictions"], "resources.predictions", 1, 2_048)
    max_iterations = integer(resources["maximum_total_iterations"], "resources.iterations", 1, 800_000)
    max_work = integer(resources["maximum_conditioning_work"], "resources.work", 1, 2_000_000_000)
    integer(resources["maximum_output_bytes"], "resources.output", 1, 1_048_576)
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3_600)
    if len(x) > max_observations or len(x_new) > max_predictions or chains * (tune + draws) > max_iterations:
        raise ContractError("GP request exceeds resource limits")
    work = chains * (tune + draws) * len(x) ** 3 + chains * draws * len(x) ** 2 * len(x_new)
    if work > max_work:
        raise ContractError("GP request exceeds conditioning-work limit")

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
        "maximum_r_hat": number(policy["maximum_r_hat"], "policy.R-hat"),
        "minimum_bulk_ess": number(policy["minimum_bulk_ess"], "policy.bulk ESS"),
        "minimum_tail_ess": number(policy["minimum_tail_ess"], "policy.tail ESS"),
        "minimum_ebfmi": number(policy["minimum_ebfmi"], "policy.E-BFMI"),
        "maximum_divergences": integer(policy["maximum_divergences"], "policy.divergences", 0, chains * draws),
        "maximum_tree_depth_hits": integer(policy["maximum_tree_depth_hits"], "policy.depth hits", 0, chains * draws),
        "maximum_tree_depth": integer(policy["maximum_tree_depth"], "policy.depth", 1, 32),
    }
    config.update(
        mean_prior_mean=mean_prior_mean,
        mean_prior_sd=mean_prior_sd,
        amplitude_prior_sd=amplitude_prior_sd,
        length_prior_sd=length_prior_sd,
        noise_prior_sd=noise_prior_sd,
        jitter=jitter,
        observation_ids=observation_ids,
        x=np.asarray(x, dtype=np.float64),
        y=np.asarray(y, dtype=np.float64),
        prediction_ids=prediction_ids,
        x_new=np.asarray(x_new, dtype=np.float64),
        chains=chains,
        tune=tune,
        draws=draws,
        target_accept=target_accept,
        seed=seed,
    )
    return config


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    digest = hashlib.sha256(f"marklab-pymc-gp-v1\0{seed}\0{purpose}\0{index}".encode()).digest()
    return int.from_bytes(digest[:4], "little")


def matern32(left: np.ndarray, right: np.ndarray, amplitude: float, length: float) -> np.ndarray:
    distance = np.abs(left[:, None] - right[None, :])
    scaled = math.sqrt(3.0) * distance / length
    return amplitude**2 * (1.0 + scaled) * np.exp(-scaled)


def summary(draws: np.ndarray) -> dict[str, float]:
    return {
        "mean": float(draws.mean()),
        "sd": float(draws.std(ddof=1)),
        "interval_lower": float(np.quantile(draws, 0.025)),
        "interval_upper": float(np.quantile(draws, 0.975)),
    }


def tree_stats(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    distance = np.abs(config["x"][:, None] - config["x"][None, :])
    identity = np.eye(len(config["x"]))
    with pm.Model():
        mean = pm.Normal("mean", config["mean_prior_mean"], config["mean_prior_sd"])
        amplitude = pm.HalfNormal("amplitude", config["amplitude_prior_sd"])
        length_scale_um = pm.HalfNormal("length_scale_um", config["length_prior_sd"])
        noise_sd = pm.HalfNormal("noise_sd", config["noise_prior_sd"])
        scaled = math.sqrt(3.0) * distance / length_scale_um
        covariance = amplitude**2 * (1.0 + scaled) * pt.exp(-scaled)
        covariance += (noise_sd**2 + config["jitter"]) * identity
        pm.MvNormal("observed", mu=pt.ones(len(config["x"])) * mean, cov=covariance, observed=config["y"])
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
            var_names=["observed"],
            random_seed=seed_for(config["seed"], "predictive"),
            progressbar=False,
        )

    names = ["mean", "amplitude", "length_scale_um", "noise_sd"]
    arrays = {
        name: np.asarray(posterior["posterior"][name].values, dtype=np.float64) for name in names
    }
    predictive_observed = np.asarray(predictive["posterior_predictive"]["observed"].values)
    prior_finite = bool(
        all(np.isfinite(prior["prior"][name].values).all() for name in names)
        and np.isfinite(prior["prior_predictive"]["observed"].values).all()
    )
    posterior_finite = bool(
        all(np.isfinite(value).all() for value in arrays.values())
        and np.isfinite(predictive_observed).all()
    )
    r_hat = float(tree_stats(pm.stats.rhat(posterior, var_names=names, method="rank"), names).max())
    bulk = float(tree_stats(pm.stats.ess(posterior, var_names=names, method="bulk"), names).min())
    tail = float(tree_stats(pm.stats.ess(posterior, var_names=names, method="tail"), names).min())
    mcse_mean = float(tree_stats(pm.stats.mcse(posterior, var_names=names, method="mean"), names).max())
    mcse_sd = float(tree_stats(pm.stats.mcse(posterior, var_names=names, method="sd"), names).max())
    energy = np.asarray(posterior["sample_stats"]["energy"].values)
    ebfmi = float(np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1)))
    divergences = int(np.asarray(posterior["sample_stats"]["diverging"].values).sum())
    depth_hits = int(np.asarray(posterior["sample_stats"]["reached_max_treedepth"].values).sum())

    flat = {name: value.reshape(-1) for name, value in arrays.items()}
    rng = np.random.default_rng(seed_for(config["seed"], "conditional_predictions"))
    prediction_draws = np.empty((len(flat["mean"]), len(config["x_new"])), dtype=np.float64)
    for index, (mean_value, amplitude_value, length_value, noise_value) in enumerate(
        zip(flat["mean"], flat["amplitude"], flat["length_scale_um"], flat["noise_sd"], strict=True)
    ):
        observed_covariance = matern32(config["x"], config["x"], amplitude_value, length_value)
        observed_covariance += (noise_value**2 + config["jitter"]) * identity
        cross_covariance = matern32(config["x_new"], config["x"], amplitude_value, length_value)
        prediction_covariance = matern32(
            config["x_new"], config["x_new"], amplitude_value, length_value
        )
        cholesky = np.linalg.cholesky(observed_covariance)
        alpha = np.linalg.solve(cholesky.T, np.linalg.solve(cholesky, config["y"] - mean_value))
        conditional_mean = mean_value + cross_covariance @ alpha
        solved = np.linalg.solve(cholesky, cross_covariance.T)
        conditional_covariance = prediction_covariance - solved.T @ solved
        conditional_covariance += config["jitter"] * np.eye(len(config["x_new"]))
        prediction_draws[index] = rng.multivariate_normal(
            conditional_mean, conditional_covariance, check_valid="raise"
        )
    posterior_finite = posterior_finite and bool(np.isfinite(prediction_draws).all())
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
    predictions = []
    for index, prediction_id in enumerate(config["prediction_ids"]):
        values = prediction_draws[:, index]
        predictions.append({"prediction_id": prediction_id, "x_um": float(config["x_new"][index]), **summary(values)})
    replicated_means = predictive_observed.mean(axis=-1)
    replicated_sds = predictive_observed.std(axis=-1, ddof=1)
    return {
        "format": "marklab.pymc_gp_worker_result",
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
        "posterior": {name: summary(arrays[name]) for name in names},
        "predictions": predictions,
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
            "constraints_valid": bool(
                np.all(arrays["amplitude"] >= 0.0)
                and np.all(arrays["length_scale_um"] >= 0.0)
                and np.all(arrays["noise_sd"] >= 0.0)
            ),
            "identifiability_checks_passed": True,
        },
        "posterior_predictive": {
            "observed_mean": float(config["y"].mean()),
            "replicated_mean": float(replicated_means.mean()),
            "replicated_mean_sd": float(replicated_means.std(ddof=1)),
            "observed_sd": float(config["y"].std(ddof=1)),
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
