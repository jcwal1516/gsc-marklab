#!/usr/bin/env python3
"""PyMC worker for patient-level hurdle beta-binomial group regression."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

sys.path.append(str(Path(__file__).resolve().parent))

# Configure PyTensor before PyMC is imported.
from marklab_worker_support import pytensor_config  # noqa: F401

# isort: split
import numpy as np
import pymc as pm
import pytensor.tensor as pt
from scipy.special import betaln
from scipy.stats import betabinom

from marklab_worker_support import pymc_nuts, pymc_request
from marklab_worker_support.nuts_contract import fit_summary, pymc_nuts_diagnostics
from marklab_worker_support.posterior import clipped_sigmoid as sigmoid
from marklab_worker_support.posterior import summary
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
    backend_identity,
    derive_seed,
    exact,
    integer,
    number,
    obj,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    worker_digests,
    write_result,
)

RESULT_FORMAT = "marklab.pymc_hurdle_beta_binomial_group_worker_result"
MODEL_KEYS = {
    "format",
    "version",
    "family",
    "reference_group",
    "comparison_group",
    "presence_intercept_prior",
    "presence_intercept_prior_mean",
    "presence_intercept_prior_sd",
    "presence_group_effect_prior",
    "presence_group_effect_prior_sd",
    "abundance_intercept_prior",
    "abundance_intercept_prior_mean",
    "abundance_intercept_prior_sd",
    "abundance_group_effect_prior",
    "abundance_group_effect_prior_sd",
    "concentration_prior",
    "concentration_prior_sd",
    "likelihood",
    "zero_process",
    "positive_process",
    "observation_unit",
    "biological_unit",
    "backend_capability",
    "maturity",
}
MODEL_CONSTANTS = {
    "family": "patient_hurdle_beta_binomial_group_regression",
    "presence_intercept_prior": "normal_log_odds",
    "presence_group_effect_prior": "normal_log_odds_difference",
    "abundance_intercept_prior": "normal_log_odds",
    "abundance_group_effect_prior": "normal_log_odds_difference",
    "concentration_prior": "half_normal",
    "likelihood": "bernoulli_presence_and_zero_truncated_beta_binomial_positive_count",
    "zero_process": "structural_hurdle_absence",
    "positive_process": "exposure_adjusted_positive_abundance",
    "observation_unit": "one_complete_class_count_and_total_exposure_per_patient",
    "biological_unit": "patient",
    "backend_capability": "nuts",
    "maturity": "experimental",
}
MONITORED = [
    "presence_intercept_log_odds",
    "presence_group_log_odds_effect",
    "positive_abundance_intercept_log_odds",
    "positive_abundance_group_log_odds_effect",
    "concentration",
]
PRIOR_SCALES = {
    "presence_intercept_mean": "presence_intercept_prior_mean",
    "presence_intercept_sd": "presence_intercept_prior_sd",
    "presence_group_sd": "presence_group_effect_prior_sd",
    "abundance_intercept_mean": "abundance_intercept_prior_mean",
    "abundance_intercept_sd": "abundance_intercept_prior_sd",
    "abundance_group_sd": "abundance_group_effect_prior_sd",
    "concentration_sd": "concentration_prior_sd",
}


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request, model = pymc_request.validate_envelope(
        request, lock_digest, worker_digest, ("patients",), MODEL_KEYS
    )
    for key, expected in MODEL_CONSTANTS.items():
        exact(model[key], expected, f"model.{key}")
    reference_group = model["reference_group"]
    comparison_group = model["comparison_group"]
    if (
        not pymc_request.valid_name(reference_group)
        or not pymc_request.valid_name(comparison_group)
        or reference_group == comparison_group
    ):
        raise ContractError("group identities are invalid")
    priors = {name: number(model[key], f"model.{key}") for name, key in PRIOR_SCALES.items()}
    if min(value for name, value in priors.items() if name.endswith("_sd")) <= 0.0:
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
        if not pymc_request.valid_name(patient_id) or group not in {
            reference_group,
            comparison_group,
        }:
            raise ContractError("patient identity or group is invalid")
        trial = integer(row["trials"], "patient.trials", 1, 10_000_000)
        patient_ids.append(patient_id)
        groups.append(group)
        successes.append(integer(row["successes"], "patient.successes", 0, trial))
        trials.append(trial)
    if patient_ids != sorted(patient_ids) or len(set(patient_ids)) != len(patient_ids):
        raise ContractError("patient identities must be unique and sorted")
    for group in (reference_group, comparison_group):
        values = [
            success
            for success, observed_group in zip(successes, groups, strict=True)
            if observed_group == group
        ]
        if (
            len(values) < 4
            or not any(value == 0 for value in values)
            or not any(value > 0 for value in values)
        ):
            raise ContractError("each group requires four patients plus zeros and positives")
    sampling = pymc_request.validate_sampling(request["sampling"], bounded_target_accept=True)
    resources = obj(
        request["resources"],
        {
            "maximum_patients",
            "maximum_total_trials",
            "maximum_total_iterations",
            "maximum_predictive_quantile_evaluations",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    if len(patients) > integer(resources["maximum_patients"], "resources.patients", 8, 512):
        raise ContractError("patient limit exceeded")
    if sum(trials) > integer(resources["maximum_total_trials"], "resources.trials", 1, 10_000_000):
        raise ContractError("trial limit exceeded")
    chains, draws = sampling["chains"], sampling["draws"]
    if chains * (sampling["tune"] + draws) > integer(
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
    output_limit = integer(resources["maximum_output_bytes"], "resources.output", 1, 2 * MIB)
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3_600)
    return {
        **priors,
        "indicator": np.asarray([group == comparison_group for group in groups], dtype=np.float64),
        "successes": np.asarray(successes, dtype=np.int64),
        "trials": np.asarray(trials, dtype=np.int64),
        **sampling,
        "maximum_quantile_evaluations": maximum_quantile_evaluations,
        "output_limit": output_limit,
        **pymc_request.validate_policy(request["diagnostic_policy"]),
    }


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    return derive_seed("marklab-pymc-hurdle-beta-binomial-group-v1", seed, purpose, index)


def zero_truncated_beta_binomial_mean_proportion(
    probability: np.ndarray,
    concentration: np.ndarray,
    trials: np.ndarray,
) -> np.ndarray:
    """Return E[Y / n | Y > 0] for each draw and trial count."""
    probability = np.asarray(probability, dtype=np.float64)
    concentration = np.asarray(concentration, dtype=np.float64)
    trials = np.asarray(trials, dtype=np.float64)
    alpha = probability[..., None] * concentration[..., None]
    beta = (1.0 - probability[..., None]) * concentration[..., None]
    log_zero_probability = betaln(alpha, beta + trials) - betaln(alpha, beta)
    positive_probability = -np.expm1(log_zero_probability)
    return probability[..., None] / positive_probability


def prior_predictive_is_finite(config: dict[str, Any]) -> bool:
    """Simulate the prior group probabilities with NumPy and check that they are valid."""
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
    """Replicate counts: presence ~ Bernoulli, then a zero-truncated beta-binomial count.

    Positive counts are drawn by inverting the beta-binomial CDF at a uniform quantile above the
    zero mass. Returns the ``(draw, patient)`` counts and the number of inversions.
    """
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
        log_zero_probability = betaln(selected_alpha, selected_beta + selected_trials) - betaln(
            selected_alpha, selected_beta
        )
        zero_probability = np.exp(log_zero_probability)
        lower = np.nextafter(zero_probability, np.ones_like(zero_probability))
        quantiles = zero_probability + (1.0 - zero_probability) * rng.random(evaluations)
        quantiles = np.maximum(quantiles, lower)
        quantiles = np.minimum(quantiles, np.nextafter(1.0, 0.0))
        candidate = betabinom.ppf(quantiles, selected_trials, selected_alpha, selected_beta)
        if (
            not np.isfinite(candidate).all()
            or np.any(candidate < 1)
            or np.any(candidate > selected_trials)
        ):
            raise ContractError("zero-truncated beta-binomial quantile inversion failed")
        replicated[present] = candidate.astype(np.int64)
    return replicated, evaluations


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    """Fit the hurdle beta-binomial group model by NUTS.

    Presence: ``1[y_i > 0] ~ Bernoulli(sigmoid(a_p + x_i * b_p))``. Positive counts:
    ``y_i | y_i > 0`` follows a zero-truncated ``BetaBinomial(n_i, q_i * phi, (1 - q_i) * phi)``
    with ``q_i = sigmoid(a_q + x_i * b_q)``, entered as a potential. Priors: intercepts
    ``a ~ Normal(mean, sd)`` and group log-odds effects ``b ~ Normal(0, sd)`` from the request,
    and concentration ``phi ~ HalfNormal(concentration_sd)``. The abundance probability
    ``q_i`` is the untruncated mean proportion, not the positive-count conditional mean.
    Normal scales are SDs; half-normal scales are the SDs of their underlying normal distributions.
    """
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
        posterior = pymc_nuts.nuts(config, seed_for)
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
    concentration_draws = np.asarray(
        posterior["posterior"]["concentration"].values, dtype=np.float64
    )
    reference_presence = sigmoid(presence_intercept_draws)
    comparison_presence = sigmoid(presence_intercept_draws + presence_effect_draws)
    reference_abundance = sigmoid(abundance_intercept_draws)
    comparison_abundance = sigmoid(abundance_intercept_draws + abundance_effect_draws)
    reference_trials = config["trials"][config["indicator"] == 0.0]
    comparison_trials = config["trials"][config["indicator"] != 0.0]
    reference_positive_mean = zero_truncated_beta_binomial_mean_proportion(
        reference_abundance, concentration_draws, reference_trials
    ).mean(axis=-1)
    comparison_positive_mean = zero_truncated_beta_binomial_mean_proportion(
        comparison_abundance, concentration_draws, comparison_trials
    ).mean(axis=-1)
    reference_unconditional = reference_presence * reference_positive_mean
    comparison_unconditional = comparison_presence * comparison_positive_mean
    replicated, predictive_quantile_evaluations = posterior_predictive(
        config,
        presence_intercept_draws,
        presence_effect_draws,
        abundance_intercept_draws,
        abundance_effect_draws,
        concentration_draws,
    )
    prior_finite = prior_predictive_is_finite(config)
    posterior_finite = bool(
        np.isfinite(presence_intercept_draws).all()
        and np.isfinite(presence_effect_draws).all()
        and np.isfinite(abundance_intercept_draws).all()
        and np.isfinite(abundance_effect_draws).all()
        and np.isfinite(concentration_draws).all()
        and np.isfinite(replicated).all()
    )
    convergence = pymc_nuts_diagnostics(posterior, MONITORED)
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
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": backend_identity("pymc", pm.__version__, lock_sha, worker_sha),
        "request_sha256": request_sha,
        **fit_summary(convergence, config, prior_finite, posterior_finite, constraints),
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
        "posterior_predictive": {
            "observed_reference_positive_patients": observed_reference_positive,
            "observed_comparison_positive_patients": observed_comparison_positive,
            "replicated_reference_positive_patients_mean": float(
                replicated_reference_positive.mean()
            ),
            "replicated_comparison_positive_patients_mean": float(
                replicated_comparison_positive.mean()
            ),
            "probability_replicated_reference_positive_patients_at_least_observed": float(
                np.mean(replicated_reference_positive >= observed_reference_positive)
            ),
            "probability_replicated_comparison_positive_patients_at_least_observed": float(
                np.mean(replicated_comparison_positive >= observed_comparison_positive)
            ),
            "observed_reference_total_successes": observed_reference_successes,
            "observed_comparison_total_successes": observed_comparison_successes,
            "replicated_reference_total_successes_mean": float(
                replicated_reference_successes.mean()
            ),
            "replicated_comparison_total_successes_mean": float(
                replicated_comparison_successes.mean()
            ),
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
    require_versions(pymc=pm.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(2 * MIB)
    config = validate(json.loads(raw), lock_sha, worker_sha)
    result = fit(config, sha256_hex(raw), lock_sha, worker_sha)
    write_result(result, max_bytes=config["output_limit"])


if __name__ == "__main__":
    run_worker(main, "Marklab PyMC hurdle beta-binomial group worker failed")
