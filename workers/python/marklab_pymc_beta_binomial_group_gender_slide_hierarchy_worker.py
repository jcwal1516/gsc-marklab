#!/usr/bin/env python3
"""Static PyMC worker for slide-within-patient beta-binomial count hierarchy."""

from __future__ import annotations

from collections import Counter
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
        raise ContractError(f"{path} must be integer in [{low},{high}]")
    return value


def exact(value: Any, expected: Any, path: str) -> None:
    if value != expected:
        raise ContractError(f"{path} must equal {expected!r}")


def validate(request: Any, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    request = obj(
        request,
        {"format", "version", "backend", "model", "slides", "sampling", "resources", "diagnostic_policy"},
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
    exact(backend["environment_lock_sha256"], lock_sha, "backend.lock")
    exact(backend["worker_sha256"], worker_sha, "backend.worker")
    model = obj(
        request["model"],
        {
            "format", "version", "family", "reference_group", "comparison_group",
            "reference_gender", "comparison_gender", "intercept_prior", "intercept_prior_mean",
            "intercept_prior_sd", "group_effect_prior", "group_effect_prior_sd",
            "gender_effect_prior", "gender_effect_prior_sd", "patient_effect",
            "patient_log_odds_sd_prior", "patient_log_odds_sd_prior_sd",
            "slide_concentration_prior", "slide_concentration_prior_sd", "likelihood",
            "interaction", "standardization", "nesting", "biological_unit",
            "backend_capability", "maturity",
        },
        "model",
    )
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    exact(model["version"], 1, "model.version")
    exact(
        model["family"],
        "beta_binomial_group_gender_slide_within_patient_hierarchy",
        "model.family",
    )
    exact(model["intercept_prior"], "normal_log_odds", "model.intercept_prior")
    exact(model["group_effect_prior"], "normal_log_odds_difference", "model.group_prior")
    exact(model["gender_effect_prior"], "normal_log_odds_difference", "model.gender_prior")
    exact(model["patient_effect"], "noncentered_normal_random_intercept", "model.patient")
    exact(model["patient_log_odds_sd_prior"], "half_normal", "model.patient_sd")
    exact(model["slide_concentration_prior"], "half_normal", "model.slide_concentration")
    exact(model["likelihood"], "beta_binomial_slide_counts", "model.likelihood")
    exact(model["interaction"], "none", "model.interaction")
    exact(
        model["standardization"],
        "observed_gender_distribution_typical_patient",
        "model.standardization",
    )
    exact(model["nesting"], "slide_within_patient", "model.nesting")
    exact(model["biological_unit"], "patient", "model.biological_unit")
    reference_group = model["reference_group"]
    comparison_group = model["comparison_group"]
    reference_gender = model["reference_gender"]
    comparison_gender = model["comparison_gender"]
    if any(
        not isinstance(value, str) or not value
        for value in [reference_group, comparison_group, reference_gender, comparison_gender]
    ) or reference_group == comparison_group or reference_gender == comparison_gender:
        raise ContractError("group or gender identities are invalid")
    intercept_mean = number(model["intercept_prior_mean"], "model.intercept_mean")
    intercept_sd = number(model["intercept_prior_sd"], "model.intercept_sd")
    group_sd = number(model["group_effect_prior_sd"], "model.group_sd")
    gender_sd = number(model["gender_effect_prior_sd"], "model.gender_sd")
    patient_sd_prior = number(
        model["patient_log_odds_sd_prior_sd"], "model.patient_sd_prior"
    )
    concentration_sd = number(
        model["slide_concentration_prior_sd"], "model.slide_concentration_sd"
    )
    if min(intercept_sd, group_sd, gender_sd, patient_sd_prior, concentration_sd) <= 0.0:
        raise ContractError("model scales must be positive")
    raw_slides = request["slides"]
    if not isinstance(raw_slides, list) or not 32 <= len(raw_slides) <= 2048:
        raise ContractError("slide count is invalid")
    slide_ids: list[str] = []
    slide_patient_ids: list[str] = []
    slide_groups: list[str] = []
    slide_genders: list[str] = []
    successes: list[int] = []
    trials: list[int] = []
    patient_design: dict[str, tuple[str, str]] = {}
    for index, raw in enumerate(raw_slides):
        row = obj(
            raw,
            {"slide_id", "patient_id", "group", "gender", "successes", "trials"},
            f"slides[{index}]",
        )
        slide_id = row["slide_id"]
        patient_id = row["patient_id"]
        group = row["group"]
        gender = row["gender"]
        if (
            not isinstance(slide_id, str)
            or not slide_id
            or len(slide_id) > 128
            or not isinstance(patient_id, str)
            or not patient_id
            or len(patient_id) > 128
            or group not in {reference_group, comparison_group}
            or gender not in {reference_gender, comparison_gender}
        ):
            raise ContractError("slide identity or design is invalid")
        if patient_id in patient_design and patient_design[patient_id] != (group, gender):
            raise ContractError("patient design differs across slides")
        patient_design[patient_id] = (group, gender)
        slide_ids.append(slide_id)
        slide_patient_ids.append(patient_id)
        slide_groups.append(group)
        slide_genders.append(gender)
        trial = integer(row["trials"], "slide.trials", 1, 10_000_000)
        successes.append(integer(row["successes"], "slide.successes", 0, trial))
        trials.append(trial)
    ordering = list(zip(slide_patient_ids, slide_ids))
    if ordering != sorted(ordering) or len(set(slide_ids)) != len(slide_ids):
        raise ContractError("slide rows must be uniquely patient/slide sorted")
    patient_ids = sorted(patient_design)
    if not 16 <= len(patient_ids) <= 512:
        raise ContractError("patient count is invalid")
    patient_index_by_id = {patient_id: index for index, patient_id in enumerate(patient_ids)}
    patient_index = np.asarray(
        [patient_index_by_id[patient_id] for patient_id in slide_patient_ids], dtype=np.int64
    )
    patient_groups = [patient_design[patient_id][0] for patient_id in patient_ids]
    patient_genders = [patient_design[patient_id][1] for patient_id in patient_ids]
    slide_counts = Counter(slide_patient_ids)
    if sum(count >= 2 for count in slide_counts.values()) < 8:
        raise ContractError("too few repeated patients")
    if any(
        sum(group == expected_group and gender == expected_gender for group, gender in zip(patient_groups, patient_genders)) < 4
        for expected_group in (reference_group, comparison_group)
        for expected_gender in (reference_gender, comparison_gender)
    ):
        raise ContractError("each group/gender cell requires four patients")
    patient_successes = np.zeros(len(patient_ids), dtype=np.int64)
    patient_trials = np.zeros(len(patient_ids), dtype=np.int64)
    for slide, patient in enumerate(patient_index):
        patient_successes[patient] += successes[slide]
        patient_trials[patient] += trials[slide]
    sampling = obj(
        request["sampling"],
        {"chains", "tune_per_chain", "draws_per_chain", "target_accept", "seed"},
        "sampling",
    )
    chains = integer(sampling["chains"], "sampling.chains", 2, 8)
    tune = integer(sampling["tune_per_chain"], "sampling.tune", 100, 100_000)
    draws = integer(sampling["draws_per_chain"], "sampling.draws", 100, 100_000)
    target_accept = number(sampling["target_accept"], "sampling.target_accept")
    seed = integer(sampling["seed"], "sampling.seed", 0, 2**64 - 1)
    resources = obj(
        request["resources"],
        {
            "maximum_patients", "maximum_slides", "maximum_total_trials",
            "maximum_total_iterations", "maximum_output_bytes", "timeout_seconds",
        },
        "resources",
    )
    if (
        len(patient_ids) > integer(resources["maximum_patients"], "resources.patients", 16, 512)
        or len(raw_slides) > integer(resources["maximum_slides"], "resources.slides", 32, 2048)
        or sum(trials) > integer(resources["maximum_total_trials"], "resources.trials", 1, 10_000_000)
        or chains * (tune + draws)
        > integer(resources["maximum_total_iterations"], "resources.iterations", 1, 400_000)
    ):
        raise ContractError("slide hierarchy resources exceeded")
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
        "reference_gender": reference_gender,
        "comparison_gender": comparison_gender,
        "intercept_mean": intercept_mean,
        "intercept_sd": intercept_sd,
        "group_sd": group_sd,
        "gender_sd": gender_sd,
        "patient_sd_prior": patient_sd_prior,
        "concentration_sd": concentration_sd,
        "slide_ids": slide_ids,
        "slide_patient_ids": slide_patient_ids,
        "slide_groups": slide_groups,
        "slide_genders": slide_genders,
        "successes": np.asarray(successes, dtype=np.int64),
        "trials": np.asarray(trials, dtype=np.int64),
        "patient_ids": patient_ids,
        "patient_groups": patient_groups,
        "patient_genders": patient_genders,
        "patient_index": patient_index,
        "patient_group_indicator": np.asarray(
            [group == comparison_group for group in patient_groups], dtype=np.float64
        ),
        "patient_gender_indicator": np.asarray(
            [gender == comparison_gender for gender in patient_genders], dtype=np.float64
        ),
        "patient_successes": patient_successes,
        "patient_trials": patient_trials,
        "slide_counts": slide_counts,
        "chains": chains,
        "tune": tune,
        "draws": draws,
        "target_accept": target_accept,
        "seed": seed,
        "prior_draws": integer(policy["prior_predictive_draws"], "policy.prior", 1, 100_000),
        "maximum_r_hat": number(policy["maximum_r_hat"], "policy.rhat"),
        "minimum_bulk_ess": number(policy["minimum_bulk_ess"], "policy.bulk"),
        "minimum_tail_ess": number(policy["minimum_tail_ess"], "policy.tail"),
        "minimum_ebfmi": number(policy["minimum_ebfmi"], "policy.ebfmi"),
        "maximum_divergences": integer(policy["maximum_divergences"], "policy.divergences", 0, 2**63 - 1),
        "maximum_tree_depth_hits": integer(policy["maximum_tree_depth_hits"], "policy.depth_hits", 0, 2**63 - 1),
        "maximum_tree_depth": integer(policy["maximum_tree_depth"], "policy.depth", 1, 32),
        "maximum_output_bytes": integer(resources["maximum_output_bytes"], "resources.output", 1, 2 * 1_048_576),
    }


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    digest = hashlib.sha256(
        f"marklab-pymc-beta-binomial-group-gender-slide-hierarchy-v1\0{seed}\0{purpose}\0{index}".encode()
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
    return 1.0 / (1.0 + np.exp(-values))


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    patient_count = len(config["patient_ids"])
    with pm.Model():
        intercept = pm.Normal("intercept_log_odds", config["intercept_mean"], config["intercept_sd"])
        group_effect = pm.Normal("group_log_odds_effect", 0.0, config["group_sd"])
        gender_effect = pm.Normal("gender_log_odds_effect", 0.0, config["gender_sd"])
        patient_sd = pm.HalfNormal("patient_log_odds_sd", config["patient_sd_prior"])
        patient_z = pm.Normal("patient_standardized_effect", 0.0, 1.0, shape=patient_count)
        patient_effect = pm.Deterministic("patient_log_odds_effect", patient_sd * patient_z)
        patient_probability = pm.Deterministic(
            "patient_probability",
            pm.math.sigmoid(
                intercept
                + config["patient_group_indicator"] * group_effect
                + config["patient_gender_indicator"] * gender_effect
                + patient_effect
            ),
        )
        slide_concentration = pm.HalfNormal(
            "slide_concentration", config["concentration_sd"]
        )
        slide_probability = patient_probability[config["patient_index"]]
        pm.BetaBinomial(
            "successes",
            alpha=slide_probability * slide_concentration,
            beta=(1.0 - slide_probability) * slide_concentration,
            n=config["trials"],
            observed=config["successes"],
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
            var_names=["successes"],
            random_seed=seed_for(config["seed"], "predictive"),
            progressbar=False,
        )
    draws = posterior["posterior"]
    intercept_draws = np.asarray(draws["intercept_log_odds"].values, dtype=np.float64)
    group_draws = np.asarray(draws["group_log_odds_effect"].values, dtype=np.float64)
    gender_draws = np.asarray(draws["gender_log_odds_effect"].values, dtype=np.float64)
    patient_sd_draws = np.asarray(draws["patient_log_odds_sd"].values, dtype=np.float64)
    concentration_draws = np.asarray(draws["slide_concentration"].values, dtype=np.float64)
    patient_effect_draws = np.asarray(draws["patient_log_odds_effect"].values, dtype=np.float64)
    patient_probability_draws = np.asarray(draws["patient_probability"].values, dtype=np.float64)
    p00 = sigmoid(intercept_draws)
    p10 = sigmoid(intercept_draws + group_draws)
    p01 = sigmoid(intercept_draws + gender_draws)
    p11 = sigmoid(intercept_draws + group_draws + gender_draws)
    gender_weight = float(config["patient_gender_indicator"].mean())
    marginal_reference = (1.0 - gender_weight) * p00 + gender_weight * p01
    marginal_comparison = (1.0 - gender_weight) * p10 + gender_weight * p11
    marginal_difference = marginal_comparison - marginal_reference
    replicated = np.asarray(predictive["posterior_predictive"]["successes"].values, dtype=np.int64)
    monitored = [
        "intercept_log_odds", "group_log_odds_effect", "gender_log_odds_effect",
        "patient_log_odds_sd", "slide_concentration", "patient_log_odds_effect",
    ]
    prior_finite = bool(
        all(
            np.isfinite(np.asarray(value.values)).all()
            for group in [prior["prior"], prior["prior_predictive"]]
            for value in group.values()
        )
    )
    posterior_finite = bool(
        all(
            np.isfinite(value).all()
            for value in [
                intercept_draws, group_draws, gender_draws, patient_sd_draws,
                concentration_draws, patient_effect_draws, patient_probability_draws, replicated,
            ]
        )
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
        np.all(patient_sd_draws > 0.0)
        and np.all(concentration_draws > 0.0)
        and np.all((0.0 < patient_probability_draws) & (patient_probability_draws < 1.0))
        and np.isfinite(np.exp(group_draws)).all()
        and np.isfinite(np.exp(gender_draws)).all()
    )
    complete = bool(
        prior_finite and posterior_finite and constraints
        and r_hat <= config["maximum_r_hat"]
        and bulk >= config["minimum_bulk_ess"]
        and tail >= config["minimum_tail_ess"]
        and ebfmi >= config["minimum_ebfmi"]
        and divergences <= config["maximum_divergences"]
        and depth_hits <= config["maximum_tree_depth_hits"]
    )
    patients = []
    patient_summaries = []
    for index, patient_id in enumerate(config["patient_ids"]):
        probability_summary = summary(patient_probability_draws[..., index])
        patient_summaries.append(probability_summary)
        patients.append(
            {
                "patient_id": patient_id,
                "group": config["patient_groups"][index],
                "gender": config["patient_genders"][index],
                "slide_count": int(config["slide_counts"][patient_id]),
                "successes": int(config["patient_successes"][index]),
                "trials": int(config["patient_trials"][index]),
                "observed_proportion": float(config["patient_successes"][index] / config["patient_trials"][index]),
                "random_log_odds_effect": summary(patient_effect_draws[..., index]),
                "posterior_probability": probability_summary,
            }
        )
    slides = []
    for index, slide_id in enumerate(config["slide_ids"]):
        patient = int(config["patient_index"][index])
        slides.append(
            {
                "slide_id": slide_id,
                "patient_id": config["slide_patient_ids"][index],
                "group": config["slide_groups"][index],
                "gender": config["slide_genders"][index],
                "successes": int(config["successes"][index]),
                "trials": int(config["trials"][index]),
                "observed_proportion": float(config["successes"][index] / config["trials"][index]),
                "posterior_patient_probability": patient_summaries[patient],
            }
        )
    replicated_flat = replicated.reshape(-1, replicated.shape[-1])
    replicated_patient_successes = np.zeros(
        (replicated_flat.shape[0], patient_count), dtype=np.int64
    )
    for slide, patient in enumerate(config["patient_index"]):
        replicated_patient_successes[:, patient] += replicated_flat[:, slide]
    observed_patient_proportions = config["patient_successes"] / config["patient_trials"]
    replicated_patient_proportions = replicated_patient_successes / config["patient_trials"]
    group_reference = config["patient_group_indicator"] == 0.0
    group_comparison = ~group_reference
    gender_reference = config["patient_gender_indicator"] == 0.0
    gender_comparison = ~gender_reference
    observed_group_difference = float(
        observed_patient_proportions[group_comparison].mean()
        - observed_patient_proportions[group_reference].mean()
    )
    replicated_group_differences = (
        replicated_patient_proportions[:, group_comparison].mean(axis=1)
        - replicated_patient_proportions[:, group_reference].mean(axis=1)
    )
    observed_gender_difference = float(
        observed_patient_proportions[gender_comparison].mean()
        - observed_patient_proportions[gender_reference].mean()
    )
    replicated_gender_differences = (
        replicated_patient_proportions[:, gender_comparison].mean(axis=1)
        - replicated_patient_proportions[:, gender_reference].mean(axis=1)
    )
    observed_slide_proportions = config["successes"] / config["trials"]
    replicated_slide_proportions = replicated_flat / config["trials"]
    observed_slide_deviation = float(
        np.abs(
            observed_slide_proportions
            - observed_patient_proportions[config["patient_index"]]
        ).mean()
    )
    replicated_slide_deviations = np.abs(
        replicated_slide_proportions
        - replicated_patient_proportions[:, config["patient_index"]]
    ).mean(axis=1)
    replicated_totals = replicated_flat.sum(axis=1)
    observed_total = int(config["successes"].sum())
    return {
        "format": "marklab.pymc_beta_binomial_group_gender_slide_hierarchy_worker_result",
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
            "draws_per_chain": config["draws"], "completed_draws": config["chains"] * config["draws"],
        },
        "posterior": {
            "intercept_log_odds": summary(intercept_draws),
            "group_log_odds_effect": summary(group_draws),
            "gender_log_odds_effect": summary(gender_draws),
            "patient_log_odds_sd": summary(patient_sd_draws),
            "slide_concentration": summary(concentration_draws),
            "reference_group_reference_gender_probability": summary(p00),
            "comparison_group_reference_gender_probability": summary(p10),
            "reference_group_comparison_gender_probability": summary(p01),
            "comparison_group_comparison_gender_probability": summary(p11),
            "marginal_reference_group_probability": summary(marginal_reference),
            "marginal_comparison_group_probability": summary(marginal_comparison),
            "marginal_probability_difference_comparison_minus_reference": summary(marginal_difference),
            "group_odds_ratio": summary(np.exp(group_draws)),
            "gender_odds_ratio": summary(np.exp(gender_draws)),
            "slide_overdispersion_mean": float((1.0 / (concentration_draws + 1.0)).mean()),
        },
        "patients": patients,
        "slides": slides,
        "diagnostics": {
            "prior_predictive_finite": prior_finite, "posterior_finite": posterior_finite,
            "r_hat": r_hat, "ess_bulk": bulk, "ess_tail": tail,
            "mcse_mean": mcse_mean, "mcse_sd": mcse_sd, "minimum_ebfmi": ebfmi,
            "divergences": divergences, "max_tree_depth_hits": depth_hits,
            "constraints_valid": constraints, "identifiability_checks_passed": True,
        },
        "posterior_predictive": {
            "observed_total_successes": observed_total,
            "replicated_total_successes_mean": float(replicated_totals.mean()),
            "probability_replicated_total_successes_at_least_observed": float(np.mean(replicated_totals >= observed_total)),
            "observed_patient_group_mean_proportion_difference": observed_group_difference,
            "replicated_patient_group_mean_proportion_difference_mean": float(replicated_group_differences.mean()),
            "probability_replicated_patient_group_difference_at_least_observed": float(np.mean(replicated_group_differences >= observed_group_difference)),
            "observed_patient_gender_mean_proportion_difference": observed_gender_difference,
            "replicated_patient_gender_mean_proportion_difference_mean": float(replicated_gender_differences.mean()),
            "probability_replicated_patient_gender_difference_at_least_observed": float(np.mean(replicated_gender_differences >= observed_gender_difference)),
            "observed_mean_absolute_slide_patient_deviation": observed_slide_deviation,
            "replicated_mean_absolute_slide_patient_deviation_mean": float(replicated_slide_deviations.mean()),
            "probability_replicated_slide_deviation_at_least_observed": float(np.mean(replicated_slide_deviations >= observed_slide_deviation)),
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
    result = fit(
        validate(json.loads(raw), lock_sha, worker_sha),
        hashlib.sha256(raw).hexdigest(),
        lock_sha,
        worker_sha,
    )
    encoded = json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")).encode()
    if len(encoded) > 2 * 1_048_576:
        raise ContractError("result exceeds output limit")
    sys.stdout.buffer.write(encoded)


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(
            f"Marklab PyMC slide hierarchy worker failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
