#!/usr/bin/env python3
"""Static PyMC worker for patient-unit hurdle beta-binomial group regression."""

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
import pytensor.tensor as pt
from scipy.special import betaln
from scipy.stats import betabinom


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
            "presence_intercept_prior", "presence_intercept_prior_mean",
            "presence_intercept_prior_sd", "presence_group_effect_prior",
            "presence_group_effect_prior_sd", "abundance_intercept_prior",
            "abundance_intercept_prior_mean", "abundance_intercept_prior_sd",
            "abundance_group_effect_prior", "abundance_group_effect_prior_sd",
            "concentration_prior", "concentration_prior_sd", "likelihood", "zero_process",
            "positive_process", "observation_unit", "biological_unit", "backend_capability",
            "maturity",
        },
        "model",
    )
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    exact(model["version"], 1, "model.version")
    exact(model["family"], "patient_hurdle_beta_binomial_group_regression", "model.family")
    exact(model["presence_intercept_prior"], "normal_log_odds", "model.presence_intercept")
    exact(model["presence_group_effect_prior"], "normal_log_odds_difference", "model.presence_group")
    exact(model["abundance_intercept_prior"], "normal_log_odds", "model.abundance_intercept")
    exact(model["abundance_group_effect_prior"], "normal_log_odds_difference", "model.abundance_group")
    exact(model["concentration_prior"], "half_normal", "model.concentration")
    exact(
        model["likelihood"],
        "bernoulli_presence_and_zero_truncated_beta_binomial_positive_count",
        "model.likelihood",
    )
    exact(model["zero_process"], "structural_hurdle_absence", "model.zero_process")
    exact(model["positive_process"], "exposure_adjusted_positive_abundance", "model.positive")
    exact(
        model["observation_unit"],
        "one_complete_class_count_and_total_exposure_per_patient",
        "model.observation_unit",
    )
    exact(model["biological_unit"], "patient", "model.biological_unit")
    exact(model["backend_capability"], "nuts", "model.backend_capability")
    exact(model["maturity"], "experimental", "model.maturity")
    reference_group = model["reference_group"]
    comparison_group = model["comparison_group"]
    if (
        not valid_name(reference_group)
        or not valid_name(comparison_group)
        or reference_group == comparison_group
    ):
        raise ContractError("group identities are invalid")
    presence_intercept_mean = number(
        model["presence_intercept_prior_mean"], "model.presence_intercept_mean"
    )
    presence_intercept_sd = number(
        model["presence_intercept_prior_sd"], "model.presence_intercept_sd"
    )
    presence_group_sd = number(
        model["presence_group_effect_prior_sd"], "model.presence_group_sd"
    )
    abundance_intercept_mean = number(
        model["abundance_intercept_prior_mean"], "model.abundance_intercept_mean"
    )
    abundance_intercept_sd = number(
        model["abundance_intercept_prior_sd"], "model.abundance_intercept_sd"
    )
    abundance_group_sd = number(
        model["abundance_group_effect_prior_sd"], "model.abundance_group_sd"
    )
    concentration_sd = number(model["concentration_prior_sd"], "model.concentration_sd")
    if min(
        presence_intercept_sd,
        presence_group_sd,
        abundance_intercept_sd,
        abundance_group_sd,
        concentration_sd,
    ) <= 0.0:
        raise ContractError("model prior scales must be positive")
    patients = request["patients"]
    if not isinstance(patients, list) or not 8 <= len(patients) <= 512:
        raise ContractError("patient count is invalid")
    patient_ids: list[str] = []
    groups: list[str] = []
    successes: list[int] = []
    trials: list[int] = []
    for index, raw in enumerate(patients):
        row = obj(raw, {"patient_id", "group", "successes", "trials"}, f"patients[{index}]")
        patient_id = row["patient_id"]
        group = row["group"]
        if not valid_name(patient_id) or group not in {reference_group, comparison_group}:
            raise ContractError("patient identity or group is invalid")
        trial = integer(row["trials"], "patient.trials", 1, 10_000_000)
        patient_ids.append(patient_id)
        groups.append(group)
        successes.append(integer(row["successes"], "patient.successes", 0, trial))
        trials.append(trial)
    if patient_ids != sorted(patient_ids) or len(set(patient_ids)) != len(patient_ids):
        raise ContractError("patient identities must be unique and sorted")
    for group in (reference_group, comparison_group):
        values = [success for success, observed_group in zip(successes, groups) if observed_group == group]
        if len(values) < 4 or not any(value == 0 for value in values) or not any(value > 0 for value in values):
            raise ContractError("each group requires four patients plus zeros and positives")
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
            "maximum_patients", "maximum_total_trials", "maximum_total_iterations",
            "maximum_predictive_quantile_evaluations", "maximum_output_bytes", "timeout_seconds",
        },
        "resources",
    )
    if len(patients) > integer(resources["maximum_patients"], "resources.patients", 8, 512):
        raise ContractError("patient limit exceeded")
    if sum(trials) > integer(resources["maximum_total_trials"], "resources.trials", 1, 10_000_000):
        raise ContractError("trial limit exceeded")
    if chains * (tune + draws) > integer(
        resources["maximum_total_iterations"], "resources.iterations", 1, 400_000
    ):
        raise ContractError("iteration limit exceeded")
    maximum_quantile_evaluations = integer(
        resources["maximum_predictive_quantile_evaluations"],
        "resources.predictive_quantiles",
        1,
        5_000_000,
    )
    if chains * draws * len(patients) > maximum_quantile_evaluations:
        raise ContractError("predictive quantile work limit exceeded")
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
        "presence_intercept_mean": presence_intercept_mean,
        "presence_intercept_sd": presence_intercept_sd,
        "presence_group_sd": presence_group_sd,
        "abundance_intercept_mean": abundance_intercept_mean,
        "abundance_intercept_sd": abundance_intercept_sd,
        "abundance_group_sd": abundance_group_sd,
        "concentration_sd": concentration_sd,
        "patient_ids": patient_ids,
        "groups": groups,
        "indicator": np.asarray([group == comparison_group for group in groups], dtype=np.float64),
        "successes": np.asarray(successes, dtype=np.int64),
        "trials": np.asarray(trials, dtype=np.int64),
        "chains": chains,
        "tune": tune,
        "draws": draws,
        "target_accept": target_accept,
        "seed": seed,
        "maximum_quantile_evaluations": maximum_quantile_evaluations,
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
        f"marklab-pymc-hurdle-beta-binomial-group-v1\0{seed}\0{purpose}\0{index}".encode()
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


def prior_predictive_is_finite(config: dict[str, Any]) -> bool:
    rng = np.random.default_rng(seed_for(config["seed"], "prior"))
    presence_intercept = rng.normal(
        config["presence_intercept_mean"], config["presence_intercept_sd"], config["prior_draws"]
    )
    presence_effect = rng.normal(0.0, config["presence_group_sd"], config["prior_draws"])
    abundance_intercept = rng.normal(
        config["abundance_intercept_mean"], config["abundance_intercept_sd"], config["prior_draws"]
    )
    abundance_effect = rng.normal(0.0, config["abundance_group_sd"], config["prior_draws"])
    concentration = np.abs(rng.normal(0.0, config["concentration_sd"], config["prior_draws"]))
    probabilities_to_check = [
        sigmoid(presence_intercept),
        sigmoid(presence_intercept + presence_effect),
        sigmoid(abundance_intercept),
        sigmoid(abundance_intercept + abundance_effect),
    ]
    return bool(
        np.isfinite(concentration).all()
        and all(np.isfinite(values).all() for values in probabilities_to_check)
        and all(np.all((0.0 <= values) & (values <= 1.0)) for values in probabilities_to_check)
    )


def posterior_predictive(
    config: dict[str, Any],
    presence_intercept: np.ndarray,
    presence_effect: np.ndarray,
    abundance_intercept: np.ndarray,
    abundance_effect: np.ndarray,
    concentration: np.ndarray,
) -> tuple[np.ndarray, int]:
    rng = np.random.default_rng(seed_for(config["seed"], "predictive"))
    flat_presence_intercept = presence_intercept.reshape(-1, 1)
    flat_presence_effect = presence_effect.reshape(-1, 1)
    flat_abundance_intercept = abundance_intercept.reshape(-1, 1)
    flat_abundance_effect = abundance_effect.reshape(-1, 1)
    flat_concentration = concentration.reshape(-1, 1)
    indicator = config["indicator"].reshape(1, -1)
    presence_probability = sigmoid(flat_presence_intercept + indicator * flat_presence_effect)
    abundance_probability = sigmoid(flat_abundance_intercept + indicator * flat_abundance_effect)
    present = rng.random(presence_probability.shape) < presence_probability
    replicated = np.zeros(present.shape, dtype=np.int64)
    alpha = abundance_probability * flat_concentration
    beta = (1.0 - abundance_probability) * flat_concentration
    trials = np.broadcast_to(config["trials"].reshape(1, -1), present.shape)
    evaluations = int(present.sum())
    if evaluations > config["maximum_quantile_evaluations"]:
        raise ContractError("positive-count posterior prediction exceeds quantile work limit")
    if evaluations:
        selected_alpha = alpha[present]
        selected_beta = beta[present]
        selected_trials = trials[present]
        log_zero_probability = (
            betaln(selected_alpha, selected_beta + selected_trials)
            - betaln(selected_alpha, selected_beta)
        )
        zero_probability = np.exp(log_zero_probability)
        lower = np.nextafter(zero_probability, np.ones_like(zero_probability))
        quantiles = zero_probability + (1.0 - zero_probability) * rng.random(evaluations)
        quantiles = np.maximum(quantiles, lower)
        quantiles = np.minimum(quantiles, np.nextafter(1.0, 0.0))
        candidate = betabinom.ppf(
            quantiles, selected_trials, selected_alpha, selected_beta
        )
        if (
            not np.isfinite(candidate).all()
            or np.any(candidate < 1)
            or np.any(candidate > selected_trials)
        ):
            raise ContractError("zero-truncated beta-binomial quantile inversion failed")
        replicated[present] = candidate.astype(np.int64)
    return replicated, evaluations


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    present = (config["successes"] > 0).astype(np.int8)
    with pm.Model():
        presence_intercept = pm.Normal(
            "presence_intercept_log_odds",
            config["presence_intercept_mean"],
            config["presence_intercept_sd"],
        )
        presence_effect = pm.Normal(
            "presence_group_log_odds_effect", 0.0, config["presence_group_sd"]
        )
        abundance_intercept = pm.Normal(
            "positive_abundance_intercept_log_odds",
            config["abundance_intercept_mean"],
            config["abundance_intercept_sd"],
        )
        abundance_effect = pm.Normal(
            "positive_abundance_group_log_odds_effect", 0.0, config["abundance_group_sd"]
        )
        concentration = pm.HalfNormal("concentration", config["concentration_sd"])
        presence_probability = pm.math.sigmoid(
            presence_intercept + config["indicator"] * presence_effect
        )
        abundance_probability = pm.math.sigmoid(
            abundance_intercept + config["indicator"] * abundance_effect
        )
        pm.Bernoulli("presence", p=presence_probability, observed=present)
        beta_binomial = pm.BetaBinomial.dist(
            alpha=abundance_probability * concentration,
            beta=(1.0 - abundance_probability) * concentration,
            n=config["trials"],
        )
        positive_logp = pm.logp(beta_binomial, config["successes"])
        zero_logp = pm.logp(beta_binomial, np.zeros_like(config["successes"]))
        log_positive_mass = pt.log(-pt.expm1(zero_logp))
        pm.Potential(
            "positive_count_log_likelihood",
            pt.sum(pt.switch(present, positive_logp - log_positive_mass, 0.0)),
        )
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
    presence_intercept_draws = np.asarray(
        posterior["posterior"]["presence_intercept_log_odds"].values, dtype=np.float64
    )
    presence_effect_draws = np.asarray(
        posterior["posterior"]["presence_group_log_odds_effect"].values, dtype=np.float64
    )
    abundance_intercept_draws = np.asarray(
        posterior["posterior"]["positive_abundance_intercept_log_odds"].values,
        dtype=np.float64,
    )
    abundance_effect_draws = np.asarray(
        posterior["posterior"]["positive_abundance_group_log_odds_effect"].values,
        dtype=np.float64,
    )
    concentration_draws = np.asarray(posterior["posterior"]["concentration"].values, dtype=np.float64)
    reference_presence = sigmoid(presence_intercept_draws)
    comparison_presence = sigmoid(presence_intercept_draws + presence_effect_draws)
    reference_abundance = sigmoid(abundance_intercept_draws)
    comparison_abundance = sigmoid(abundance_intercept_draws + abundance_effect_draws)
    reference_unconditional = reference_presence * reference_abundance
    comparison_unconditional = comparison_presence * comparison_abundance
    replicated, predictive_quantile_evaluations = posterior_predictive(
        config,
        presence_intercept_draws,
        presence_effect_draws,
        abundance_intercept_draws,
        abundance_effect_draws,
        concentration_draws,
    )
    monitored = [
        "presence_intercept_log_odds",
        "presence_group_log_odds_effect",
        "positive_abundance_intercept_log_odds",
        "positive_abundance_group_log_odds_effect",
        "concentration",
    ]
    prior_finite = prior_predictive_is_finite(config)
    posterior_finite = bool(
        np.isfinite(presence_intercept_draws).all()
        and np.isfinite(presence_effect_draws).all()
        and np.isfinite(abundance_intercept_draws).all()
        and np.isfinite(abundance_effect_draws).all()
        and np.isfinite(concentration_draws).all()
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
        np.all(concentration_draws > 0.0)
        and all(
            np.all((0.0 < values) & (values < 1.0))
            for values in [
                reference_presence,
                comparison_presence,
                reference_abundance,
                comparison_abundance,
            ]
        )
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
    reference_mask = config["indicator"] == 0.0
    comparison_mask = ~reference_mask
    observed_reference_positive = int(present[reference_mask].sum())
    observed_comparison_positive = int(present[comparison_mask].sum())
    observed_reference_successes = int(config["successes"][reference_mask].sum())
    observed_comparison_successes = int(config["successes"][comparison_mask].sum())
    replicated_reference_positive = (replicated[:, reference_mask] > 0).sum(axis=1)
    replicated_comparison_positive = (replicated[:, comparison_mask] > 0).sum(axis=1)
    replicated_reference_successes = replicated[:, reference_mask].sum(axis=1)
    replicated_comparison_successes = replicated[:, comparison_mask].sum(axis=1)
    return {
        "format": "marklab.pymc_hurdle_beta_binomial_group_worker_result",
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
            "presence_intercept_log_odds": summary(presence_intercept_draws),
            "presence_group_log_odds_effect": summary(presence_effect_draws),
            "reference_presence_probability": summary(reference_presence),
            "comparison_presence_probability": summary(comparison_presence),
            "presence_probability_difference_comparison_minus_reference": summary(
                comparison_presence - reference_presence
            ),
            "positive_abundance_intercept_log_odds": summary(abundance_intercept_draws),
            "positive_abundance_group_log_odds_effect": summary(abundance_effect_draws),
            "reference_positive_abundance_probability": summary(reference_abundance),
            "comparison_positive_abundance_probability": summary(comparison_abundance),
            "positive_abundance_difference_comparison_minus_reference": summary(
                comparison_abundance - reference_abundance
            ),
            "reference_unconditional_expected_proportion": summary(reference_unconditional),
            "comparison_unconditional_expected_proportion": summary(comparison_unconditional),
            "unconditional_expected_proportion_difference": summary(
                comparison_unconditional - reference_unconditional
            ),
            "concentration": summary(concentration_draws),
            "overdispersion_mean": float((1.0 / (concentration_draws + 1.0)).mean()),
        },
        "diagnostics": {
            "prior_predictive_finite": prior_finite, "posterior_finite": posterior_finite,
            "r_hat": r_hat, "ess_bulk": bulk, "ess_tail": tail,
            "mcse_mean": mcse_mean, "mcse_sd": mcse_sd, "minimum_ebfmi": ebfmi,
            "divergences": divergences, "max_tree_depth_hits": depth_hits,
            "constraints_valid": constraints, "identifiability_checks_passed": True,
        },
        "posterior_predictive": {
            "observed_reference_positive_patients": observed_reference_positive,
            "observed_comparison_positive_patients": observed_comparison_positive,
            "replicated_reference_positive_patients_mean": float(replicated_reference_positive.mean()),
            "replicated_comparison_positive_patients_mean": float(replicated_comparison_positive.mean()),
            "probability_replicated_reference_positive_patients_at_least_observed": float(
                np.mean(replicated_reference_positive >= observed_reference_positive)
            ),
            "probability_replicated_comparison_positive_patients_at_least_observed": float(
                np.mean(replicated_comparison_positive >= observed_comparison_positive)
            ),
            "observed_reference_total_successes": observed_reference_successes,
            "observed_comparison_total_successes": observed_comparison_successes,
            "replicated_reference_total_successes_mean": float(replicated_reference_successes.mean()),
            "replicated_comparison_total_successes_mean": float(replicated_comparison_successes.mean()),
            "probability_replicated_reference_total_successes_at_least_observed": float(
                np.mean(replicated_reference_successes >= observed_reference_successes)
            ),
            "probability_replicated_comparison_total_successes_at_least_observed": float(
                np.mean(replicated_comparison_successes >= observed_comparison_successes)
            ),
            "predictive_quantile_evaluations": predictive_quantile_evaluations,
        },
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
            f"Marklab PyMC hurdle beta-binomial group worker failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
