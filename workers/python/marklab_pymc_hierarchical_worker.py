#!/usr/bin/env python3
"""Static PyMC worker for Marklab's Gaussian patient-hierarchy contract."""

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
RESULT_FORMAT = "marklab.pymc_hierarchical_worker_result"
CONTRACT_VERSION = 1
PYMC_VERSION = "6.3.0"


class ContractError(ValueError):
    pass


def require_object(value: Any, keys: set[str], path: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ContractError(f"{path} must be an object")
    actual = set(value)
    if actual != keys:
        raise ContractError(
            f"{path} fields differ: missing={sorted(keys - actual)}, unknown={sorted(actual - keys)}"
        )
    return value


def require_integer(value: Any, path: str, lower: int, upper: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not lower <= value <= upper:
        raise ContractError(f"{path} must be an integer in [{lower}, {upper}]")
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
            "patients",
            "sampling",
            "resources",
            "diagnostic_policy",
        },
        "request",
    )
    require_string(request["format"], REQUEST_FORMAT, "request.format")
    require_integer(request["version"], "request.version", 1, 1)
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
            "global_mean_prior",
            "between_patient_sd_prior",
            "patient_effect_parameterization",
            "likelihood",
            "observation_unit",
            "biological_unit",
            "hierarchy",
            "spatial_component",
            "generated_quantities",
            "posterior_predictive_statistics",
            "backend_capability",
            "maturity",
        },
        "request.model",
    )
    require_string(model["format"], "marklab.bayesian_model_ir", "request.model.format")
    require_integer(model["version"], "request.model.version", 1, 1)
    require_string(
        model["family"], "gaussian_patient_varying_intercept", "request.model.family"
    )
    global_prior = require_object(
        model["global_mean_prior"],
        {"family", "mean", "sd", "rationale"},
        "request.model.global_mean_prior",
    )
    require_string(
        global_prior["family"], "normal", "request.model.global_mean_prior.family"
    )
    global_prior_mean = require_number(
        global_prior["mean"], "request.model.global_mean_prior.mean"
    )
    global_prior_sd = require_number(global_prior["sd"], "request.model.global_mean_prior.sd")
    if global_prior_sd <= 0.0:
        raise ContractError("global prior SD must be positive")
    require_string(
        global_prior["rationale"], "user_supplied", "request.model.global_mean_prior.rationale"
    )
    between_prior = require_object(
        model["between_patient_sd_prior"],
        {"family", "sd", "support", "rationale"},
        "request.model.between_patient_sd_prior",
    )
    require_string(
        between_prior["family"],
        "half_normal",
        "request.model.between_patient_sd_prior.family",
    )
    between_prior_sd = require_number(
        between_prior["sd"], "request.model.between_patient_sd_prior.sd"
    )
    if between_prior_sd <= 0.0:
        raise ContractError("between-patient prior SD must be positive")
    require_string(
        between_prior["support"], "positive", "request.model.between_patient_sd_prior.support"
    )
    require_string(
        between_prior["rationale"],
        "user_supplied",
        "request.model.between_patient_sd_prior.rationale",
    )
    require_string(
        model["patient_effect_parameterization"],
        "noncentered",
        "request.model.patient_effect_parameterization",
    )
    likelihood = require_object(
        model["likelihood"], {"family", "known_sigma", "link"}, "request.model.likelihood"
    )
    require_string(
        likelihood["family"], "normal_known_sigma", "request.model.likelihood.family"
    )
    known_sigma = require_number(
        likelihood["known_sigma"], "request.model.likelihood.known_sigma"
    )
    if known_sigma <= 0.0:
        raise ContractError("known sigma must be positive")
    require_string(likelihood["link"], "identity", "request.model.likelihood.link")
    require_string(
        model["observation_unit"],
        "patient_nested_scalar_observation",
        "request.model.observation_unit",
    )
    require_string(model["biological_unit"], "patient", "request.model.biological_unit")
    if model["hierarchy"] != ["patient"]:
        raise ContractError("request.model.hierarchy must be ['patient']")
    require_string(model["spatial_component"], "none", "request.model.spatial_component")
    if model["generated_quantities"] != [
        "patient_mean",
        "variance_partition",
        "posterior_predictive_observation",
    ]:
        raise ContractError("unsupported generated quantities")
    if model["posterior_predictive_statistics"] != ["global_mean", "patient_mean_sd"]:
        raise ContractError("unsupported posterior predictive statistics")
    require_string(model["backend_capability"], "nuts", "request.model.backend_capability")
    require_string(model["maturity"], "experimental", "request.model.maturity")

    patients = request["patients"]
    if not isinstance(patients, list) or len(patients) < 3:
        raise ContractError("request.patients must contain at least three patients")
    patient_ids: list[str] = []
    patient_values: list[np.ndarray] = []
    total_observations = 0
    for index, value in enumerate(patients):
        patient = require_object(value, {"patient_id", "observations"}, f"request.patients[{index}]")
        patient_id = patient["patient_id"]
        if not isinstance(patient_id, str) or not patient_id or patient_id.strip() != patient_id:
            raise ContractError(f"request.patients[{index}].patient_id is invalid")
        if patient_ids and patient_id <= patient_ids[-1]:
            raise ContractError("patient IDs must be strictly increasing")
        values = patient["observations"]
        if not isinstance(values, list) or len(values) < 2:
            raise ContractError(f"patient {patient_id} requires at least two observations")
        array = np.asarray(
            [require_number(item, f"request.patients[{index}].observations") for item in values],
            dtype=np.float64,
        )
        patient_ids.append(patient_id)
        patient_values.append(array)
        total_observations += len(array)

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
        raise ContractError("target acceptance must be in [0.5, 1)")
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
    maximum_iterations = require_integer(
        resources["maximum_total_iterations"],
        "request.resources.maximum_total_iterations",
        1,
        800_000,
    )
    require_integer(
        resources["maximum_output_bytes"], "request.resources.maximum_output_bytes", 1, 1_048_576
    )
    require_integer(resources["timeout_seconds"], "request.resources.timeout_seconds", 1, 3_600)
    if total_observations > maximum_observations:
        raise ContractError("observation count exceeds the request limit")
    if chains * (tune + draws) > maximum_iterations:
        raise ContractError("sampling iterations exceed the request limit")

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
        raise ContractError("invalid diagnostic thresholds")
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
        "global_prior_mean": global_prior_mean,
        "global_prior_sd": global_prior_sd,
        "between_prior_sd": between_prior_sd,
        "known_sigma": known_sigma,
        "patient_ids": patient_ids,
        "patient_values": patient_values,
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
    digest = hashlib.sha256(
        f"marklab-pymc-hierarchical-v1\0{seed}\0{purpose}\0{index}".encode()
    ).digest()
    return int.from_bytes(digest[:4], "little")


def scalar_summary(draws: np.ndarray) -> dict[str, float]:
    return {
        "mean": float(draws.mean()),
        "sd": float(draws.std(ddof=1)),
        "interval_lower": float(np.quantile(draws, 0.025)),
        "interval_upper": float(np.quantile(draws, 0.975)),
    }


def flattened_statistic(tree: Any, variables: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in variables])


def fit_hierarchy(
    config: dict[str, Any], request_sha256: str, lock_digest: str, worker_digest: str
) -> dict[str, Any]:
    patient_index = np.concatenate(
        [np.full(len(values), index, dtype=np.int64) for index, values in enumerate(config["patient_values"])]
    )
    observations = np.concatenate(config["patient_values"])
    coordinates = {"patient": config["patient_ids"]}
    chain_seeds = [derived_seed(config["seed"], "chain", index) for index in range(config["chains"])]
    with pm.Model(coords=coordinates) as model:
        global_mean = pm.Normal(
            "global_mean", mu=config["global_prior_mean"], sigma=config["global_prior_sd"]
        )
        between_patient_sd = pm.HalfNormal(
            "between_patient_sd", sigma=config["between_prior_sd"]
        )
        patient_z = pm.Normal("patient_z", mu=0.0, sigma=1.0, dims="patient")
        patient_mean = pm.Deterministic(
            "patient_mean", global_mean + between_patient_sd * patient_z, dims="patient"
        )
        pm.Normal(
            "y",
            mu=patient_mean[patient_index],
            sigma=config["known_sigma"],
            observed=observations,
        )
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
        predictive = pm.sample_posterior_predictive(
            fit,
            var_names=["y"],
            random_seed=derived_seed(config["seed"], "posterior_predictive"),
            progressbar=False,
            return_inferencedata=True,
        )

    monitored = ["global_mean", "between_patient_sd", "patient_mean"]
    prior_finite = bool(
        all(np.isfinite(prior["prior"][name].values).all() for name in monitored)
        and np.isfinite(prior["prior_predictive"]["y"].values).all()
    )
    global_draws = np.asarray(fit["posterior"]["global_mean"].values, dtype=np.float64)
    between_draws = np.asarray(fit["posterior"]["between_patient_sd"].values, dtype=np.float64)
    patient_draws = np.asarray(fit["posterior"]["patient_mean"].values, dtype=np.float64)
    predictive_draws = np.asarray(predictive["posterior_predictive"]["y"].values, dtype=np.float64)
    posterior_finite = bool(
        np.isfinite(global_draws).all()
        and np.isfinite(between_draws).all()
        and np.isfinite(patient_draws).all()
        and np.isfinite(predictive_draws).all()
    )
    r_hat = float(
        flattened_statistic(pm.stats.rhat(fit, var_names=monitored, method="rank"), monitored).max()
    )
    ess_bulk = float(
        flattened_statistic(pm.stats.ess(fit, var_names=monitored, method="bulk"), monitored).min()
    )
    ess_tail = float(
        flattened_statistic(pm.stats.ess(fit, var_names=monitored, method="tail"), monitored).min()
    )
    mcse_mean = float(
        flattened_statistic(pm.stats.mcse(fit, var_names=monitored, method="mean"), monitored).max()
    )
    mcse_sd = float(
        flattened_statistic(pm.stats.mcse(fit, var_names=monitored, method="sd"), monitored).max()
    )
    energy = np.asarray(fit["sample_stats"]["energy"].values, dtype=np.float64)
    minimum_ebfmi = float(
        np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1))
    )
    divergences = int(np.asarray(fit["sample_stats"]["divergences"].values).sum())
    tree_depth_hits = int(
        np.asarray(fit["sample_stats"]["reached_max_treedepth"].values).sum()
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
        [predictive_draws[..., patient_index == index].mean(axis=-1) for index in range(len(config["patient_ids"]))],
        axis=-1,
    )
    replicated_patient_mean_sd = replicated_patient_means.std(axis=-1, ddof=1)
    observed_global_mean = float(observations.mean())
    observed_patient_mean_sd = float(
        np.asarray([values.mean() for values in config["patient_values"]]).std(ddof=1)
    )
    variance_partition = between_draws**2 / (between_draws**2 + config["known_sigma"] ** 2)
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
            "constraints_valid": bool(np.all(between_draws >= 0.0)),
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
            "replicated_patient_mean_sd_mean": float(replicated_patient_mean_sd.mean()),
        },
    }


def main() -> None:
    if pm.__version__ != PYMC_VERSION:
        raise ContractError(f"PyMC version drift: expected {PYMC_VERSION}, found {pm.__version__}")
    if sys.version_info[:2] != (3, 12):
        raise ContractError(
            f"Python version drift: expected 3.12, found {sys.version_info.major}.{sys.version_info.minor}"
        )
    script_path = Path(__file__)
    lock_digest = hashlib.sha256(script_path.with_name("uv.lock").read_bytes()).hexdigest()
    worker_digest = hashlib.sha256(script_path.read_bytes()).hexdigest()
    raw_request = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if len(raw_request) > 16 * 1024 * 1024:
        raise ContractError("request exceeds the 16 MiB worker boundary")
    request_sha256 = hashlib.sha256(raw_request).hexdigest()
    config = validate_request(json.loads(raw_request), lock_digest, worker_digest)
    result = fit_hierarchy(config, request_sha256, lock_digest, worker_digest)
    sys.stdout.write(json.dumps(result, allow_nan=False, separators=(",", ":"), sort_keys=True))
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"marklab PyMC worker failed: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from error
