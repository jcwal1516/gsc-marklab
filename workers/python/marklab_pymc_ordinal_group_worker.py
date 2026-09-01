#!/usr/bin/env python3
"""Static PyMC worker for patient-unit proportional-odds group regression."""

from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path
import sys
from typing import Any
import unicodedata

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


def valid_name(value: Any) -> bool:
    return (
        isinstance(value, str)
        and bool(value)
        and len(value) <= 128
        and value == value.strip()
        and not any(unicodedata.category(character) == "Cc" for character in value)
    )


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format", "version", "backend", "model", "patients", "sampling",
            "resources", "diagnostic_policy",
        },
        "request",
    )
    exact(request["format"], "marklab.pymc_worker_request", "request.format")
    exact(request["version"], 1, "request.version")
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
            "format", "version", "family", "reference_group", "comparison_group",
            "ordered_levels", "cutpoint_prior", "cutpoint_prior_sd", "group_effect_prior",
            "group_effect_prior_sd", "intercept", "likelihood",
            "proportional_odds_assumption", "observation_unit", "biological_unit",
            "backend_capability", "maturity",
        },
        "model",
    )
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    exact(model["version"], 1, "model.version")
    exact(model["family"], "patient_proportional_odds_group_regression", "model.family")
    exact(model["cutpoint_prior"], "ordered_normal", "model.cutpoint_prior")
    exact(model["group_effect_prior"], "normal_log_odds_difference", "model.group_prior")
    exact(model["intercept"], "none_cutpoints_own_baseline_location", "model.intercept")
    exact(model["likelihood"], "ordered_logistic_patient_outcome", "model.likelihood")
    exact(model["proportional_odds_assumption"], True, "model.proportional_odds")
    exact(model["observation_unit"], "one_complete_ordered_outcome_per_patient", "model.unit")
    exact(model["biological_unit"], "patient", "model.biological_unit")
    exact(model["backend_capability"], "nuts", "model.backend_capability")
    exact(model["maturity"], "experimental", "model.maturity")
    reference_group = model["reference_group"]
    comparison_group = model["comparison_group"]
    levels = model["ordered_levels"]
    if (
        not valid_name(reference_group)
        or not valid_name(comparison_group)
        or reference_group == comparison_group
        or not isinstance(levels, list)
        or not 2 <= len(levels) <= 8
        or not all(valid_name(level) for level in levels)
        or len(set(levels)) != len(levels)
    ):
        raise ContractError("model group or ordered-level identities are invalid")
    cutpoint_sd = number(model["cutpoint_prior_sd"], "model.cutpoint_prior_sd")
    group_sd = number(model["group_effect_prior_sd"], "model.group_effect_prior_sd")
    if cutpoint_sd <= 0.0 or group_sd <= 0.0:
        raise ContractError("model prior scales must be positive")
    patients = request["patients"]
    if not isinstance(patients, list) or not 8 <= len(patients) <= 512:
        raise ContractError("patient count is invalid")
    patient_ids: list[str] = []
    groups: list[str] = []
    outcomes: list[int] = []
    for index, raw in enumerate(patients):
        row = obj(raw, {"patient_id", "group", "outcome_code"}, f"patients[{index}]")
        patient_id = row["patient_id"]
        group = row["group"]
        if not valid_name(patient_id) or group not in {reference_group, comparison_group}:
            raise ContractError("patient identity or group is invalid")
        patient_ids.append(patient_id)
        groups.append(group)
        outcomes.append(integer(row["outcome_code"], "patient.outcome_code", 0, len(levels) - 1))
    if patient_ids != sorted(patient_ids) or len(set(patient_ids)) != len(patient_ids):
        raise ContractError("patient identities must be unique and sorted")
    if min(groups.count(reference_group), groups.count(comparison_group)) < 4:
        raise ContractError("each group requires four patients")
    if set(outcomes) != set(range(len(levels))):
        raise ContractError("every ordered level must be observed")
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
        raise ContractError("target acceptance is invalid")
    seed = integer(sampling["seed"], "sampling.seed", 0, 2**64 - 1)
    resources = obj(
        request["resources"],
        {
            "maximum_patients", "maximum_levels", "maximum_total_iterations",
            "maximum_output_bytes", "timeout_seconds",
        },
        "resources",
    )
    if len(patients) > integer(resources["maximum_patients"], "resources.patients", 8, 512):
        raise ContractError("patient limit exceeded")
    if len(levels) > integer(resources["maximum_levels"], "resources.levels", 2, 8):
        raise ContractError("level limit exceeded")
    if chains * (tune + draws) > integer(
        resources["maximum_total_iterations"], "resources.iterations", 1, 400_000
    ):
        raise ContractError("iteration limit exceeded")
    output_limit = integer(resources["maximum_output_bytes"], "resources.output", 1, 2 * 1_048_576)
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3_600)
    policy = obj(
        request["diagnostic_policy"],
        {
            "prior_predictive_draws", "maximum_r_hat", "minimum_bulk_ess", "minimum_tail_ess",
            "minimum_ebfmi", "maximum_divergences", "maximum_tree_depth_hits", "maximum_tree_depth",
        },
        "policy",
    )
    return {
        "reference_group": reference_group,
        "comparison_group": comparison_group,
        "levels": levels,
        "cutpoint_sd": cutpoint_sd,
        "group_sd": group_sd,
        "patient_ids": patient_ids,
        "groups": groups,
        "indicator": np.asarray([group == comparison_group for group in groups], dtype=np.float64),
        "outcomes": np.asarray(outcomes, dtype=np.int64),
        "chains": chains,
        "tune": tune,
        "draws": draws,
        "target_accept": target_accept,
        "seed": seed,
        "output_limit": output_limit,
        "prior_draws": integer(policy["prior_predictive_draws"], "policy.prior_draws", 1, 100_000),
        "maximum_r_hat": number(policy["maximum_r_hat"], "policy.rhat"),
        "minimum_bulk_ess": number(policy["minimum_bulk_ess"], "policy.bulk"),
        "minimum_tail_ess": number(policy["minimum_tail_ess"], "policy.tail"),
        "minimum_ebfmi": number(policy["minimum_ebfmi"], "policy.ebfmi"),
        "maximum_divergences": integer(policy["maximum_divergences"], "policy.divergences", 0, 2**63 - 1),
        "maximum_tree_depth_hits": integer(
            policy["maximum_tree_depth_hits"], "policy.depth_hits", 0, 2**63 - 1
        ),
        "maximum_tree_depth": integer(policy["maximum_tree_depth"], "policy.depth", 1, 32),
    }


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    digest = hashlib.sha256(
        f"marklab-pymc-ordinal-group-v1\0{seed}\0{purpose}\0{index}".encode()
    ).digest()
    return int.from_bytes(digest[:4], "little")


def summary(values: np.ndarray) -> dict[str, float]:
    flat = values.reshape(-1)
    return {
        "mean": float(flat.mean()),
        "sd": float(flat.std(ddof=1)),
        "interval_lower": float(np.quantile(flat, 0.025)),
        "interval_upper": float(np.quantile(flat, 0.975)),
    }


def tree_values(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def sigmoid(values: np.ndarray) -> np.ndarray:
    clipped = np.clip(values, -700.0, 700.0)
    return 1.0 / (1.0 + np.exp(-clipped))


def probabilities(cutpoints: np.ndarray, eta: np.ndarray) -> np.ndarray:
    cumulative = sigmoid(cutpoints - eta[..., None])
    zeros = np.zeros((*cumulative.shape[:-1], 1), dtype=np.float64)
    ones = np.ones((*cumulative.shape[:-1], 1), dtype=np.float64)
    return np.diff(np.concatenate([zeros, cumulative, ones], axis=-1), axis=-1)


def prior_predictive_is_finite(config: dict[str, Any], initial: np.ndarray) -> bool:
    rng = np.random.default_rng(seed_for(config["seed"], "prior"))
    raw = rng.normal(initial, config["cutpoint_sd"], size=(config["prior_draws"], len(initial)))
    cutpoints = np.sort(raw, axis=1)
    effects = rng.normal(0.0, config["group_sd"], size=config["prior_draws"])
    reference = probabilities(cutpoints, np.zeros(config["prior_draws"]))
    comparison = probabilities(cutpoints, effects)
    return bool(
        np.isfinite(reference).all()
        and np.isfinite(comparison).all()
        and np.all(reference >= 0.0)
        and np.all(comparison >= 0.0)
        and np.allclose(reference.sum(axis=1), 1.0)
        and np.allclose(comparison.sum(axis=1), 1.0)
    )


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    initial = np.linspace(-1.5, 1.5, len(config["levels"]) - 1, dtype=np.float64)
    with pm.Model():
        cutpoints = pm.Normal(
            "cutpoints",
            mu=initial,
            sigma=config["cutpoint_sd"],
            shape=len(initial),
            transform=pm.distributions.transforms.ordered,
            initval=initial,
        )
        effect = pm.Normal("group_log_odds_effect", 0.0, config["group_sd"])
        eta = config["indicator"] * effect
        pm.OrderedLogistic("outcome", eta=eta, cutpoints=cutpoints, observed=config["outcomes"])
        posterior = pm.sample(
            draws=config["draws"],
            tune=config["tune"],
            chains=config["chains"],
            cores=1,
            blas_cores=1,
            random_seed=[seed_for(config["seed"], "chain", index) for index in range(config["chains"])],
            target_accept=config["target_accept"],
            nuts_sampler="pymc",
            nuts={"max_treedepth": config["maximum_tree_depth"]},
            progressbar=False,
            quiet=True,
            compute_convergence_checks=False,
        )
        predictive = pm.sample_posterior_predictive(
            posterior,
            var_names=["outcome"],
            random_seed=seed_for(config["seed"], "predictive"),
            progressbar=False,
        )
    cutpoint_draws = np.asarray(posterior["posterior"]["cutpoints"].values, dtype=np.float64)
    effect_draws = np.asarray(
        posterior["posterior"]["group_log_odds_effect"].values, dtype=np.float64
    )
    reference_probabilities = probabilities(cutpoint_draws, np.zeros_like(effect_draws))
    comparison_probabilities = probabilities(cutpoint_draws, effect_draws)
    difference_probabilities = comparison_probabilities - reference_probabilities
    codes = np.arange(len(config["levels"]), dtype=np.float64)
    reference_expected = (reference_probabilities * codes).sum(axis=-1)
    comparison_expected = (comparison_probabilities * codes).sum(axis=-1)
    replicated = np.asarray(predictive["posterior_predictive"]["outcome"].values, dtype=np.int64)
    monitored = ["cutpoints", "group_log_odds_effect"]
    prior_finite = prior_predictive_is_finite(config, initial)
    posterior_finite = bool(
        np.isfinite(cutpoint_draws).all()
        and np.isfinite(effect_draws).all()
        and np.isfinite(reference_probabilities).all()
        and np.isfinite(comparison_probabilities).all()
        and np.isfinite(replicated).all()
    )
    r_hat = float(tree_values(pm.stats.rhat(posterior, var_names=monitored, method="rank"), monitored).max())
    bulk = float(tree_values(pm.stats.ess(posterior, var_names=monitored, method="bulk"), monitored).min())
    tail = float(tree_values(pm.stats.ess(posterior, var_names=monitored, method="tail"), monitored).min())
    mcse_mean = float(tree_values(pm.stats.mcse(posterior, var_names=monitored, method="mean"), monitored).max())
    mcse_sd = float(tree_values(pm.stats.mcse(posterior, var_names=monitored, method="sd"), monitored).max())
    energy = np.asarray(posterior["sample_stats"]["energy"].values, dtype=np.float64)
    ebfmi = float(np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1)))
    divergences = int(np.asarray(posterior["sample_stats"]["divergences"].values).sum())
    depth_hits = int(np.asarray(posterior["sample_stats"]["reached_max_treedepth"].values).sum())
    constraints = bool(
        np.all(np.diff(cutpoint_draws, axis=-1) > 0.0)
        and np.all(reference_probabilities >= 0.0)
        and np.all(comparison_probabilities >= 0.0)
        and np.allclose(reference_probabilities.sum(axis=-1), 1.0)
        and np.allclose(comparison_probabilities.sum(axis=-1), 1.0)
    )
    complete = bool(
        prior_finite
        and posterior_finite
        and constraints
        and r_hat <= config["maximum_r_hat"]
        and bulk >= config["minimum_bulk_ess"]
        and tail >= config["minimum_tail_ess"]
        and ebfmi >= config["minimum_ebfmi"]
        and divergences <= config["maximum_divergences"]
        and depth_hits <= config["maximum_tree_depth_hits"]
    )
    flat_reference = reference_probabilities.reshape(-1, len(config["levels"]))
    flat_comparison = comparison_probabilities.reshape(-1, len(config["levels"]))
    replicated_flat = replicated.reshape(-1, replicated.shape[-1])
    reference_mask = config["indicator"] == 0.0
    comparison_mask = ~reference_mask
    level_posteriors = []
    predictive_levels = []
    for index, level in enumerate(config["levels"]):
        reference_draws = flat_reference[:, index]
        comparison_draws = flat_comparison[:, index]
        observed_reference = float(np.mean(config["outcomes"][reference_mask] == index))
        observed_comparison = float(np.mean(config["outcomes"][comparison_mask] == index))
        replicated_reference = np.mean(replicated_flat[:, reference_mask] == index, axis=1)
        replicated_comparison = np.mean(replicated_flat[:, comparison_mask] == index, axis=1)
        observed_error = abs(observed_reference - float(reference_draws.mean())) + abs(
            observed_comparison - float(comparison_draws.mean())
        )
        replicated_error = np.abs(replicated_reference - reference_draws) + np.abs(
            replicated_comparison - comparison_draws
        )
        level_posteriors.append(
            {
                "level": level,
                "reference_probability": summary(reference_draws),
                "comparison_probability": summary(comparison_draws),
                "difference_comparison_minus_reference": summary(
                    comparison_draws - reference_draws
                ),
            }
        )
        predictive_levels.append(
            {
                "level": level,
                "observed_reference_proportion": observed_reference,
                "observed_comparison_proportion": observed_comparison,
                "replicated_reference_proportion_mean": float(replicated_reference.mean()),
                "replicated_comparison_proportion_mean": float(replicated_comparison.mean()),
                "probability_absolute_replication_error_at_least_observed": float(
                    np.mean(replicated_error >= observed_error)
                ),
            }
        )
    cutpoint_posteriors = [
        {
            "lower_level": config["levels"][index],
            "upper_level": config["levels"][index + 1],
            **summary(cutpoint_draws[..., index]),
        }
        for index in range(len(config["levels"]) - 1)
    ]
    return {
        "format": "marklab.pymc_ordinal_group_worker_result",
        "version": 1,
        "backend": {
            "name": "pymc", "version": pm.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha, "worker_sha256": worker_sha,
        },
        "request_sha256": request_sha,
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"], "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"],
            "completed_draws": config["chains"] * config["draws"],
        },
        "posterior": {
            "group_log_odds_effect": summary(effect_draws),
            "cutpoints": cutpoint_posteriors,
            "levels": level_posteriors,
            "reference_expected_code": summary(reference_expected),
            "comparison_expected_code": summary(comparison_expected),
        },
        "diagnostics": {
            "prior_predictive_finite": prior_finite, "posterior_finite": posterior_finite,
            "r_hat": r_hat, "ess_bulk": bulk, "ess_tail": tail,
            "mcse_mean": mcse_mean, "mcse_sd": mcse_sd, "minimum_ebfmi": ebfmi,
            "divergences": divergences, "max_tree_depth_hits": depth_hits,
            "constraints_valid": constraints, "identifiability_checks_passed": True,
        },
        "posterior_predictive": {"levels": predictive_levels},
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
    config = validate(json.loads(raw), lock_sha, worker_sha)
    result = fit(config, hashlib.sha256(raw).hexdigest(), lock_sha, worker_sha)
    encoded = json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":"))
    if len(encoded.encode()) > config["output_limit"]:
        raise ContractError("result exceeds output limit")
    sys.stdout.write(encoded)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(
            f"Marklab PyMC ordinal-group worker failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
