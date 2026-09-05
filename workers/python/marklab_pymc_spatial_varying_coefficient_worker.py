#!/usr/bin/env python3
"""Static PyMC worker for an exact 1-D spatially varying coefficient."""

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


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "model",
            "observations",
            "centering_basis",
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
        "backend",
    )
    exact(backend["name"], "pymc", "backend.name")
    exact(backend["version"], PYMC_VERSION, "backend.version")
    exact(backend["python_version"], "3.12", "backend.python")
    exact(backend["environment_lock_sha256"], lock_digest, "backend.lock")
    exact(backend["worker_sha256"], worker_digest, "backend.worker")
    model = obj(
        request["model"],
        {
            "format",
            "version",
            "family",
            "kernel",
            "coordinate_unit",
            "field_constraint",
            "global_predictor_name",
            "spatial_predictor_name",
            "intercept_prior_mean",
            "intercept_prior_sd",
            "coefficient_prior_sd",
            "amplitude_prior_sd",
            "length_scale_prior_sd_um",
            "known_noise_sd",
            "jitter",
            "backend_capability",
            "maturity",
        },
        "model",
    )
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    integer(model["version"], "model.version", 1, 1)
    exact(model["family"], "gaussian_spatially_varying_coefficient", "model.family")
    exact(model["kernel"], "matern_3_2", "model.kernel")
    exact(model["coordinate_unit"], "micrometre", "model.unit")
    exact(
        model["field_constraint"],
        "exact_sum_to_zero_orthonormal_basis",
        "model.constraint",
    )
    exact(model["backend_capability"], "nuts", "model.capability")
    exact(model["maturity"], "experimental", "model.maturity")
    global_name = model["global_predictor_name"]
    spatial_name = model["spatial_predictor_name"]
    if (
        not isinstance(global_name, str)
        or not global_name
        or global_name.strip() != global_name
        or not isinstance(spatial_name, str)
        or not spatial_name
        or spatial_name.strip() != spatial_name
        or global_name == spatial_name
    ):
        raise ContractError("predictor names are invalid")
    intercept_prior_mean = number(model["intercept_prior_mean"], "model.intercept mean")
    intercept_prior_sd = number(model["intercept_prior_sd"], "model.intercept sd")
    coefficient_prior_sd = number(model["coefficient_prior_sd"], "model.coefficient sd")
    amplitude_prior_sd = number(model["amplitude_prior_sd"], "model.amplitude sd")
    length_prior_sd = number(model["length_scale_prior_sd_um"], "model.length sd")
    known_noise_sd = number(model["known_noise_sd"], "model.noise")
    jitter = number(model["jitter"], "model.jitter")
    if min(
        intercept_prior_sd,
        coefficient_prior_sd,
        amplitude_prior_sd,
        length_prior_sd,
        known_noise_sd,
        jitter,
    ) <= 0.0:
        raise ContractError("all spatial coefficient scales must be positive")

    observations = request["observations"]
    if not isinstance(observations, list) or not 8 <= len(observations) <= 64:
        raise ContractError("observations must contain 8-64 entries")
    ids: list[str] = []
    x_values: list[float] = []
    outcome: list[float] = []
    global_predictor: list[float] = []
    spatial_predictor: list[float] = []
    for index, raw in enumerate(observations):
        observation = obj(
            raw,
            {"coordinate_id", "x_um", "outcome", "global_predictor", "spatial_predictor"},
            f"observations[{index}]",
        )
        coordinate_id = observation["coordinate_id"]
        if (
            not isinstance(coordinate_id, str)
            or not coordinate_id
            or coordinate_id.strip() != coordinate_id
            or coordinate_id in ids
        ):
            raise ContractError("coordinate ID is invalid or repeated")
        x_um = number(observation["x_um"], "observation.x_um")
        if x_values and x_um <= x_values[-1]:
            raise ContractError("coordinates must be strictly increasing")
        ids.append(coordinate_id)
        x_values.append(x_um)
        outcome.append(number(observation["outcome"], "observation.outcome"))
        global_predictor.append(
            number(observation["global_predictor"], "observation.global_predictor")
        )
        spatial_predictor.append(
            number(observation["spatial_predictor"], "observation.spatial_predictor")
        )
    dimension = len(ids)
    fixed_design = np.column_stack(
        [np.ones(dimension), global_predictor, spatial_predictor]
    )
    if np.linalg.matrix_rank(fixed_design) != 3:
        raise ContractError("fixed design including intercept must be full rank")
    basis_raw = request["centering_basis"]
    if not isinstance(basis_raw, list) or len(basis_raw) != dimension * (dimension - 1):
        raise ContractError("centering basis dimensions mismatch")
    basis = np.asarray(
        [number(value, "centering_basis[]") for value in basis_raw], dtype=np.float64
    ).reshape(dimension, dimension - 1)
    if (
        np.abs(basis.sum(axis=0)).max() > 1e-12
        or np.abs(basis.T @ basis - np.eye(dimension - 1)).max() > 1e-12
    ):
        raise ContractError("centering basis is not orthonormal and sum-zero")

    sampling = obj(
        request["sampling"],
        {"chains", "tune_per_chain", "draws_per_chain", "target_accept", "seed"},
        "sampling",
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
        "resources",
    )
    if dimension > integer(resources["maximum_observations"], "resources.observations", 1, 64):
        raise ContractError("observation count exceeds resource limit")
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
        "policy",
    )
    return {
        "prior_draws": integer(policy["prior_predictive_draws"], "policy.prior", 100, 10_000),
        "maximum_r_hat": number(policy["maximum_r_hat"], "policy.rhat"),
        "minimum_bulk_ess": number(policy["minimum_bulk_ess"], "policy.bulk"),
        "minimum_tail_ess": number(policy["minimum_tail_ess"], "policy.tail"),
        "minimum_ebfmi": number(policy["minimum_ebfmi"], "policy.ebfmi"),
        "maximum_divergences": integer(policy["maximum_divergences"], "policy.divergences", 0, chains * draws),
        "maximum_tree_depth_hits": integer(policy["maximum_tree_depth_hits"], "policy.depth hits", 0, chains * draws),
        "maximum_tree_depth": integer(policy["maximum_tree_depth"], "policy.depth", 1, 32),
        "intercept_prior_mean": intercept_prior_mean,
        "intercept_prior_sd": intercept_prior_sd,
        "coefficient_prior_sd": coefficient_prior_sd,
        "amplitude_prior_sd": amplitude_prior_sd,
        "length_prior_sd": length_prior_sd,
        "known_noise_sd": known_noise_sd,
        "jitter": jitter,
        "ids": ids,
        "x": np.asarray(x_values),
        "outcome": np.asarray(outcome),
        "global": np.asarray(global_predictor),
        "spatial": np.asarray(spatial_predictor),
        "basis": basis,
        "chains": chains,
        "tune": tune,
        "draws": draws,
        "target_accept": target_accept,
        "seed": seed,
    }


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    digest = hashlib.sha256(f"marklab-pymc-svc-v1\0{seed}\0{purpose}\0{index}".encode()).digest()
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
    dimension = len(config["ids"])
    distances = np.abs(config["x"][:, None] - config["x"][None, :])
    with pm.Model(coords={"coordinate": config["ids"]}):
        intercept = pm.Normal(
            "intercept", config["intercept_prior_mean"], config["intercept_prior_sd"]
        )
        global_coefficient = pm.Normal(
            "global_coefficient", 0.0, config["coefficient_prior_sd"]
        )
        spatial_mean_coefficient = pm.Normal(
            "spatial_mean_coefficient", 0.0, config["coefficient_prior_sd"]
        )
        amplitude = pm.HalfNormal("amplitude", config["amplitude_prior_sd"])
        length_scale_um = pm.HalfNormal("length_scale_um", config["length_prior_sd"])
        field_raw = pm.Normal("field_raw", 0.0, 1.0, shape=dimension - 1)
        scaled_distance = math.sqrt(3.0) * distances / length_scale_um
        covariance = amplitude**2 * (1.0 + scaled_distance) * pm.math.exp(-scaled_distance)
        covariance = covariance + config["jitter"] * np.eye(dimension)
        projected_covariance = config["basis"].T @ covariance @ config["basis"]
        projected_lower = pt.linalg.cholesky(projected_covariance)
        deviation = pm.Deterministic(
            "deviation",
            pm.math.dot(config["basis"], pm.math.dot(projected_lower, field_raw)),
            dims="coordinate",
        )
        varying_coefficient = pm.Deterministic(
            "varying_coefficient", spatial_mean_coefficient + deviation, dims="coordinate"
        )
        mean = (
            intercept
            + global_coefficient * config["global"]
            + config["spatial"] * varying_coefficient
        )
        pm.Normal(
            "observed_outcome",
            mean,
            config["known_noise_sd"],
            observed=config["outcome"],
            dims="coordinate",
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
            var_names=["observed_outcome"],
            random_seed=seed_for(config["seed"], "predictive"),
            progressbar=False,
        )

    names = [
        "intercept",
        "global_coefficient",
        "spatial_mean_coefficient",
        "amplitude",
        "length_scale_um",
        "field_raw",
    ]
    arrays = {
        name: np.asarray(posterior["posterior"][name].values, dtype=np.float64)
        for name in names + ["deviation", "varying_coefficient"]
    }
    predictive_draws = np.asarray(
        predictive["posterior_predictive"]["observed_outcome"].values, dtype=np.float64
    )
    prior_finite = bool(
        all(np.isfinite(prior["prior"][name].values).all() for name in names)
        and np.isfinite(prior["prior_predictive"]["observed_outcome"].values).all()
    )
    posterior_finite = bool(
        all(np.isfinite(value).all() for value in arrays.values())
        and np.isfinite(predictive_draws).all()
    )
    r_hat = float(stats(pm.stats.rhat(posterior, var_names=names, method="rank"), names).max())
    bulk = float(stats(pm.stats.ess(posterior, var_names=names, method="bulk"), names).min())
    tail = float(stats(pm.stats.ess(posterior, var_names=names, method="tail"), names).min())
    mcse_mean = float(stats(pm.stats.mcse(posterior, var_names=names, method="mean"), names).max())
    mcse_sd = float(stats(pm.stats.mcse(posterior, var_names=names, method="sd"), names).max())
    energy = np.asarray(posterior["sample_stats"]["energy"].values)
    ebfmi = float(np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1)))
    divergences = int(np.asarray(posterior["sample_stats"]["diverging"].values).sum())
    depth_hits = int(np.asarray(posterior["sample_stats"]["reached_max_treedepth"].values).sum())
    flat_deviation = arrays["deviation"].reshape(-1, dimension)
    constraints_valid = bool(
        np.all(arrays["amplitude"] > 0.0)
        and np.all(arrays["length_scale_um"] > 0.0)
        and np.abs(flat_deviation.sum(axis=1)).max() <= 1e-8
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
    flat_varying = arrays["varying_coefficient"].reshape(-1, dimension)
    field = [
        {
            "coordinate_id": coordinate_id,
            "x_um": float(config["x"][index]),
            "deviation": summary(flat_deviation[:, index]),
            "varying_coefficient": summary(flat_varying[:, index]),
        }
        for index, coordinate_id in enumerate(config["ids"])
    ]
    replicated_means = predictive_draws.mean(axis=-1)
    replicated_sds = predictive_draws.std(axis=-1, ddof=1)
    return {
        "format": "marklab.pymc_spatial_varying_coefficient_worker_result",
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
            "intercept": summary(arrays["intercept"].reshape(-1)),
            "global_coefficient": summary(arrays["global_coefficient"].reshape(-1)),
            "spatial_mean_coefficient": summary(
                arrays["spatial_mean_coefficient"].reshape(-1)
            ),
            "amplitude": summary(arrays["amplitude"].reshape(-1)),
            "length_scale_um": summary(arrays["length_scale_um"].reshape(-1)),
        },
        "coefficient_field": field,
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
            "observed_mean": float(config["outcome"].mean()),
            "observed_sd": float(config["outcome"].std(ddof=1)),
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
