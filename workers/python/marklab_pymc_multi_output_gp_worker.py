#!/usr/bin/env python3
"""Static PyMC worker for a one-factor, two-output exact Matérn GP."""

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
            f"{path} fields differ: missing={sorted(keys-actual)}, unknown={sorted(actual-keys)}"
        )
    return value


def num(value: Any, path: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ContractError(f"{path} must be numeric")
    value = float(value)
    if not math.isfinite(value):
        raise ContractError(f"{path} must be finite")
    return value


def integer(value: Any, path: str, low: int, high: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not low <= value <= high:
        raise ContractError(f"{path} must be an integer in [{low},{high}]")
    return value


def exact(value: Any, expected: Any, path: str) -> None:
    if value != expected:
        raise ContractError(f"{path} must equal {expected!r}")


def validate(request: Any, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    request = obj(
        request,
        {"format", "version", "backend", "model", "observations", "predictions", "sampling", "resources", "diagnostic_policy"},
        "request",
    )
    exact(request["format"], "marklab.pymc_worker_request", "request.format")
    integer(request["version"], "request.version", 1, 1)
    backend = obj(request["backend"], {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"}, "backend")
    exact(backend["name"], "pymc", "backend.name")
    exact(backend["version"], PYMC_VERSION, "backend.version")
    exact(backend["python_version"], "3.12", "backend.python")
    exact(backend["environment_lock_sha256"], lock_sha, "backend.lock")
    exact(backend["worker_sha256"], worker_sha, "backend.worker")
    model = obj(
        request["model"],
        {
            "format", "version", "family", "coordinate_dimension", "coordinate_unit",
            "output_a_name", "output_b_name", "latent_processes", "kernel", "loading_a",
            "loading_b_constraint", "mean_prior_sd", "amplitude_prior_sd",
            "length_scale_prior_sd_um", "loading_b_prior_sd", "noise_a_sd", "noise_b_sd", "jitter",
            "backend_capability", "maturity",
        },
        "model",
    )
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    integer(model["version"], "model.version", 1, 1)
    exact(model["family"], "one_factor_two_output_matern32_gp", "model.family")
    integer(model["coordinate_dimension"], "model.dimension", 1, 1)
    exact(model["coordinate_unit"], "micrometre", "model.unit")
    output_a_name, output_b_name = model["output_a_name"], model["output_b_name"]
    if not all(isinstance(name, str) and name and name.strip() == name for name in [output_a_name, output_b_name]) or output_a_name == output_b_name:
        raise ContractError("output names are invalid")
    integer(model["latent_processes"], "model.latent_processes", 1, 1)
    exact(model["kernel"], "matern_3_2", "model.kernel")
    exact(model["loading_a"], 1.0, "model.loading_a")
    exact(model["loading_b_constraint"], "positive", "model.loading_b_constraint")
    prior_names = ["mean_prior_sd", "amplitude_prior_sd", "length_scale_prior_sd_um", "loading_b_prior_sd", "noise_a_sd", "noise_b_sd", "jitter"]
    priors = {name: num(model[name], f"model.{name}") for name in prior_names}
    if min(priors.values()) <= 0.0:
        raise ContractError("prior scales and jitter must be positive")
    exact(model["backend_capability"], "nuts_exact_dense_multi_output_gp", "model.capability")
    exact(model["maturity"], "experimental", "model.maturity")

    observations = request["observations"]
    if not isinstance(observations, list) or not 5 <= len(observations) <= 64:
        raise ContractError("observations must contain 5-64 rows")
    ids, x, a, b = [], [], [], []
    coordinate_bits: set[int] = set()
    for index, value in enumerate(observations):
        row = obj(value, {"coordinate_id", "x_um", "output_a", "output_b"}, f"observations[{index}]")
        row_id = row["coordinate_id"]
        coordinate = num(row["x_um"], "x_um")
        if not isinstance(row_id, str) or not row_id or row_id.strip() != row_id or (ids and row_id <= ids[-1]):
            raise ContractError("coordinate IDs must be exact and strictly increasing")
        bits = np.float64(coordinate).view(np.uint64).item()
        if bits in coordinate_bits:
            raise ContractError("coordinates must be unique")
        coordinate_bits.add(bits)
        ids.append(row_id)
        x.append(coordinate)
        a.append(num(row["output_a"], "output_a"))
        b.append(num(row["output_b"], "output_b"))
    if len(set(a)) < 2 or len(set(b)) < 2:
        raise ContractError("each output field must vary")
    predictions = request["predictions"]
    if not isinstance(predictions, list) or not 1 <= len(predictions) <= 1_024:
        raise ContractError("predictions must contain 1-1024 rows")
    prediction_ids, x_new = [], []
    for index, value in enumerate(predictions):
        row = obj(value, {"prediction_id", "x_um"}, f"predictions[{index}]")
        row_id = row["prediction_id"]
        if not isinstance(row_id, str) or not row_id or row_id.strip() != row_id or (prediction_ids and row_id <= prediction_ids[-1]):
            raise ContractError("prediction IDs must be strictly increasing")
        prediction_ids.append(row_id)
        x_new.append(num(row["x_um"], "prediction x"))

    sampling = obj(request["sampling"], {"chains", "tune_per_chain", "draws_per_chain", "target_accept", "seed"}, "sampling")
    chains = integer(sampling["chains"], "sampling.chains", 2, 8)
    tune = integer(sampling["tune_per_chain"], "sampling.tune", 100, 100_000)
    draws = integer(sampling["draws_per_chain"], "sampling.draws", 100, 100_000)
    target_accept = num(sampling["target_accept"], "sampling.target_accept")
    if not 0.5 <= target_accept < 1.0:
        raise ContractError("target acceptance must be in [0.5,1)")
    seed = integer(sampling["seed"], "sampling.seed", 0, 2**64 - 1)
    resources = obj(request["resources"], {"maximum_observations", "maximum_predictions", "maximum_total_iterations", "maximum_conditioning_work", "maximum_output_bytes", "timeout_seconds"}, "resources")
    max_n = integer(resources["maximum_observations"], "resources.observations", 5, 64)
    max_m = integer(resources["maximum_predictions"], "resources.predictions", 1, 1_024)
    max_iterations = integer(resources["maximum_total_iterations"], "resources.iterations", 1, 800_000)
    max_work = integer(resources["maximum_conditioning_work"], "resources.work", 1, 2_000_000_000)
    integer(resources["maximum_output_bytes"], "resources.output", 1, 1_048_576)
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3_600)
    work = chains * (tune + draws) * (2 * len(x)) ** 3 + chains * draws * (2 * len(x)) ** 2 * (2 * len(x_new))
    if len(x) > max_n or len(x_new) > max_m or chains * (tune + draws) > max_iterations or work > max_work:
        raise ContractError("request exceeds resource limits")
    policy = obj(request["diagnostic_policy"], {"prior_predictive_draws", "maximum_r_hat", "minimum_bulk_ess", "minimum_tail_ess", "minimum_ebfmi", "maximum_divergences", "maximum_tree_depth_hits", "maximum_tree_depth"}, "policy")
    config = {
        "prior_draws": integer(policy["prior_predictive_draws"], "policy.prior", 100, 10_000),
        "maximum_r_hat": num(policy["maximum_r_hat"], "policy.rhat"),
        "minimum_bulk_ess": num(policy["minimum_bulk_ess"], "policy.bulk"),
        "minimum_tail_ess": num(policy["minimum_tail_ess"], "policy.tail"),
        "minimum_ebfmi": num(policy["minimum_ebfmi"], "policy.ebfmi"),
        "maximum_divergences": integer(policy["maximum_divergences"], "policy.divergences", 0, chains * draws),
        "maximum_tree_depth_hits": integer(policy["maximum_tree_depth_hits"], "policy.depth hits", 0, chains * draws),
        "maximum_tree_depth": integer(policy["maximum_tree_depth"], "policy.depth", 1, 32),
    }
    config.update(priors)
    config.update(
        output_a_name=output_a_name,
        output_b_name=output_b_name,
        x=np.asarray(x),
        a=np.asarray(a),
        b=np.asarray(b),
        prediction_ids=prediction_ids,
        x_new=np.asarray(x_new),
        chains=chains,
        tune=tune,
        draws=draws,
        target_accept=target_accept,
        seed=seed,
    )
    return config


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    digest = hashlib.sha256(f"marklab-pymc-mogp-v1\0{seed}\0{purpose}\0{index}".encode()).digest()
    return int.from_bytes(digest[:4], "little")


def kernel(left: np.ndarray, right: np.ndarray, amplitude: float, length: float) -> np.ndarray:
    scaled = math.sqrt(3.0) * np.abs(left[:, None] - right[None, :]) / length
    return amplitude**2 * (1.0 + scaled) * np.exp(-scaled)


def block_covariance(base: np.ndarray, loading: float) -> np.ndarray:
    return np.block([[base, loading * base], [loading * base, loading**2 * base]])


def summary(draws: np.ndarray) -> dict[str, float]:
    return {
        "mean": float(draws.mean()),
        "sd": float(draws.std(ddof=1)),
        "interval_lower": float(np.quantile(draws, 0.025)),
        "interval_upper": float(np.quantile(draws, 0.975)),
    }


def values(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    n = len(config["x"])
    distance = np.abs(config["x"][:, None] - config["x"][None, :])
    observed = np.concatenate([config["a"], config["b"]])
    with pm.Model():
        mean_a = pm.Normal("mean_a", 0.0, config["mean_prior_sd"])
        mean_b = pm.Normal("mean_b", 0.0, config["mean_prior_sd"])
        amplitude = pm.HalfNormal("amplitude", config["amplitude_prior_sd"])
        length_scale_um = pm.HalfNormal("length_scale_um", config["length_scale_prior_sd_um"])
        loading_b = pm.HalfNormal("loading_b", config["loading_b_prior_sd"])
        scaled = math.sqrt(3.0) * distance / length_scale_um
        base = amplitude**2 * (1.0 + scaled) * pt.exp(-scaled)
        covariance = pt.concatenate(
            [
                pt.concatenate([base, loading_b * base], axis=1),
                pt.concatenate([loading_b * base, loading_b**2 * base], axis=1),
            ],
            axis=0,
        )
        diagonal = pt.concatenate(
            [pt.ones(n) * config["noise_a_sd"] ** 2, pt.ones(n) * config["noise_b_sd"] ** 2]
        ) + config["jitter"]
        covariance += pt.diag(diagonal)
        mean_vector = pt.concatenate([pt.ones(n) * mean_a, pt.ones(n) * mean_b])
        pm.MvNormal("observed", mean_vector, cov=covariance, observed=observed)
        prior = pm.sample_prior_predictive(
            draws=config["prior_draws"], random_seed=seed_for(config["seed"], "prior")
        )
        posterior = pm.sample(
            draws=config["draws"], tune=config["tune"], chains=config["chains"], cores=1,
            blas_cores=1,
            random_seed=[seed_for(config["seed"], "chain", i) for i in range(config["chains"])],
            target_accept=config["target_accept"], nuts_sampler="pymc",
            nuts={"max_treedepth": config["maximum_tree_depth"]}, progressbar=False, quiet=True,
            compute_convergence_checks=False,
        )
        predictive = pm.sample_posterior_predictive(
            posterior, var_names=["observed"], random_seed=seed_for(config["seed"], "predictive"),
            progressbar=False,
        )
    names = ["mean_a", "mean_b", "amplitude", "length_scale_um", "loading_b"]
    arrays = {name: np.asarray(posterior["posterior"][name].values) for name in names}
    replicated = np.asarray(predictive["posterior_predictive"]["observed"].values)
    prior_finite = bool(all(np.isfinite(prior["prior"][name].values).all() for name in names) and np.isfinite(prior["prior_predictive"]["observed"].values).all())
    posterior_finite = bool(all(np.isfinite(array).all() for array in arrays.values()) and np.isfinite(replicated).all())
    rhat = float(values(pm.stats.rhat(posterior, var_names=names, method="rank"), names).max())
    bulk = float(values(pm.stats.ess(posterior, var_names=names, method="bulk"), names).min())
    tail = float(values(pm.stats.ess(posterior, var_names=names, method="tail"), names).min())
    mcse_mean = float(values(pm.stats.mcse(posterior, var_names=names, method="mean"), names).max())
    mcse_sd = float(values(pm.stats.mcse(posterior, var_names=names, method="sd"), names).max())
    energy = np.asarray(posterior["sample_stats"]["energy"].values)
    ebfmi = float(np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1)))
    divergences = int(np.asarray(posterior["sample_stats"]["diverging"].values).sum())
    depth_hits = int(np.asarray(posterior["sample_stats"]["reached_max_treedepth"].values).sum())
    flat = {name: array.reshape(-1) for name, array in arrays.items()}
    rng = np.random.default_rng(seed_for(config["seed"], "conditional_predictions"))
    prediction_draws = np.empty((len(flat["mean_a"]), 2 * len(config["x_new"])))
    identity = np.eye(2 * n)
    for index, parameters in enumerate(zip(*(flat[name] for name in names), strict=True)):
        mean_a, mean_b, amplitude, length, loading = parameters
        base_observed = kernel(config["x"], config["x"], amplitude, length)
        observed_cov = block_covariance(base_observed, loading)
        observed_cov += np.diag(np.concatenate([
            np.full(n, config["noise_a_sd"] ** 2),
            np.full(n, config["noise_b_sd"] ** 2),
        ]))
        observed_cov += config["jitter"] * identity
        base_cross = kernel(config["x_new"], config["x"], amplitude, length)
        cross_cov = block_covariance(base_cross, loading)
        base_prediction = kernel(config["x_new"], config["x_new"], amplitude, length)
        prediction_cov = block_covariance(base_prediction, loading)
        cholesky = np.linalg.cholesky(observed_cov)
        observed_mean = np.concatenate([np.full(n, mean_a), np.full(n, mean_b)])
        new_mean = np.concatenate([
            np.full(len(config["x_new"]), mean_a), np.full(len(config["x_new"]), mean_b)
        ])
        alpha = np.linalg.solve(cholesky.T, np.linalg.solve(cholesky, observed - observed_mean))
        conditional_mean = new_mean + cross_cov @ alpha
        solved = np.linalg.solve(cholesky, cross_cov.T)
        conditional_cov = prediction_cov - solved.T @ solved
        conditional_cov += config["jitter"] * np.eye(2 * len(config["x_new"]))
        prediction_draws[index] = rng.multivariate_normal(conditional_mean, conditional_cov, check_valid="raise")
    posterior_finite = posterior_finite and bool(np.isfinite(prediction_draws).all())
    complete = prior_finite and posterior_finite and rhat <= config["maximum_r_hat"] and bulk >= config["minimum_bulk_ess"] and tail >= config["minimum_tail_ess"] and ebfmi >= config["minimum_ebfmi"] and divergences <= config["maximum_divergences"] and depth_hits <= config["maximum_tree_depth_hits"]
    m = len(config["x_new"])
    prediction_summaries = []
    for index, prediction_id in enumerate(config["prediction_ids"]):
        a_draws, b_draws = prediction_draws[:, index], prediction_draws[:, m + index]
        a_summary, b_summary = summary(a_draws), summary(b_draws)
        prediction_summaries.append({
            "prediction_id": prediction_id, "x_um": float(config["x_new"][index]),
            "output_a_mean": a_summary["mean"], "output_a_sd": a_summary["sd"],
            "output_a_interval_lower": a_summary["interval_lower"], "output_a_interval_upper": a_summary["interval_upper"],
            "output_b_mean": b_summary["mean"], "output_b_sd": b_summary["sd"],
            "output_b_interval_lower": b_summary["interval_lower"], "output_b_interval_upper": b_summary["interval_upper"],
        })
    replicated_a, replicated_b = replicated[..., :n], replicated[..., n:]
    correlations = np.asarray([
        np.corrcoef(a_values, b_values)[0, 1]
        for a_values, b_values in zip(replicated_a.reshape(-1, n), replicated_b.reshape(-1, n), strict=True)
    ])
    return {
        "format": "marklab.pymc_multi_output_gp_worker_result", "version": 1,
        "backend": {"name": "pymc", "version": pm.__version__, "python_version": f"{sys.version_info.major}.{sys.version_info.minor}", "environment_lock_sha256": lock_sha, "worker_sha256": worker_sha},
        "request_sha256": request_sha, "fit_state": "complete" if complete else "nonconverged",
        "sampling": {"chains": config["chains"], "tune_per_chain": config["tune"], "draws_per_chain": config["draws"], "completed_draws": config["chains"] * config["draws"]},
        "posterior": {name: summary(arrays[name]) for name in names},
        "predictions": prediction_summaries,
        "diagnostics": {"prior_predictive_finite": prior_finite, "posterior_finite": posterior_finite, "r_hat": rhat, "ess_bulk": bulk, "ess_tail": tail, "mcse_mean": mcse_mean, "mcse_sd": mcse_sd, "minimum_ebfmi": ebfmi, "divergences": divergences, "max_tree_depth_hits": depth_hits, "constraints_valid": bool(all(np.all(arrays[name] >= 0.0) for name in ["amplitude", "length_scale_um", "loading_b"])), "identifiability_checks_passed": True},
        "posterior_predictive": {"observed_correlation": float(np.corrcoef(config["a"], config["b"])[0, 1]), "replicated_correlation_mean": float(correlations.mean()), "observed_a_mean": float(config["a"].mean()), "replicated_a_mean": float(replicated_a.mean()), "observed_b_mean": float(config["b"].mean()), "replicated_b_mean": float(replicated_b.mean())},
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
