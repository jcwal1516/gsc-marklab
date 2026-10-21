"""Posterior summaries of the patient-level count models, shared by PyMC and NumPyro workers.

Both backends produce posterior draws of the same parameters with the same ``(chain, draw)``
leading shape, so the derived probabilities, per-patient conditional summaries, and posterior
predictive checks are computed here once. Every array argument holds posterior draws.
"""

from __future__ import annotations

from collections.abc import Callable
from typing import Any

import numpy as np

from .posterior import sigmoid, softmax, summary


def group_gender_probabilities(
    intercept: np.ndarray, group_effect: np.ndarray, gender_effect: np.ndarray, gender_weight: float
) -> dict[str, np.ndarray]:
    """Cell probabilities of the additive logit model and the gender-standardized contrast.

    ``pGS`` is the probability for group ``G`` and gender ``S`` (0 = reference, 1 =
    comparison). The marginal group probabilities average over genders with the observed
    comparison-gender share ``gender_weight``.
    """
    p00 = sigmoid(intercept)
    p10 = sigmoid(intercept + group_effect)
    p01 = sigmoid(intercept + gender_effect)
    p11 = sigmoid(intercept + group_effect + gender_effect)
    marginal_reference = (1.0 - gender_weight) * p00 + gender_weight * p01
    marginal_comparison = (1.0 - gender_weight) * p10 + gender_weight * p11
    return {
        "p00": p00,
        "p10": p10,
        "p01": p01,
        "p11": p11,
        "marginal_reference": marginal_reference,
        "marginal_comparison": marginal_comparison,
        "marginal_difference": marginal_comparison - marginal_reference,
    }


def all_strictly_inside_unit_interval(*arrays: np.ndarray) -> bool:
    return all(np.all((0.0 < values) & (values < 1.0)) for values in arrays)


def group_mean_difference(proportions: np.ndarray, comparison: np.ndarray) -> Any:
    """Comparison-minus-reference mean proportion along the last axis."""
    return proportions[..., comparison].mean(axis=-1) - proportions[..., ~comparison].mean(axis=-1)


def conditional_patient_summaries(
    config: dict[str, Any],
    linear_probability: Callable[[int], np.ndarray],
    concentration: np.ndarray,
    rng: np.random.Generator,
    shrinkage_key: str,
    extra_fields: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    """Summarize each patient's conditional beta posterior given the collapsed model draws.

    Patient ``i``'s probability is drawn from ``Beta(y_i + mu_i * phi, n_i - y_i + (1 - mu_i)
    * phi)`` per posterior draw of the mean ``mu_i = linear_probability(i)`` and concentration
    ``phi``; patients are visited in order so the generator stream matches the patient order.
    Shrinkage is the fraction of the gap between the observed proportion and the mean of
    ``mu_i`` that the posterior mean closes.
    """
    chains = config["chains"]
    patients = []
    for index, patient_id in enumerate(config["patient_ids"]):
        mean_probability = linear_probability(index)
        successes = config["successes"][index]
        trials = config["trials"][index]
        conditional = rng.beta(
            successes + mean_probability * concentration,
            trials - successes + (1.0 - mean_probability) * concentration,
        )
        observed = float(successes / trials)
        denominator = float(mean_probability.mean()) - observed
        if denominator == 0.0:
            shrinkage = 0.0
        else:
            shrinkage = float((conditional.mean() - observed) / denominator)
        patients.append(
            {
                "patient_id": patient_id,
                **extra_fields[index],
                "successes": int(successes),
                "trials": int(trials),
                "observed_proportion": observed,
                "posterior_probability": summary(conditional, chains=chains),
                shrinkage_key: shrinkage,
            }
        )
    return patients


def group_gender_report(
    config: dict[str, Any],
    intercept: np.ndarray,
    group_effect: np.ndarray,
    gender_effect: np.ndarray,
    concentration: np.ndarray,
    replicated: np.ndarray,
    rng: np.random.Generator,
) -> tuple[dict[str, Any], bool]:
    """Posterior, patient, and posterior predictive blocks of the group/gender regression.

    Returns the result blocks and whether the draws satisfy the parameter constraints.
    """
    chains = config["chains"]
    gender_weight = float(config["gender_indicator"].mean())
    cells = group_gender_probabilities(intercept, group_effect, gender_effect, gender_weight)
    group_odds_ratio = np.exp(group_effect)
    gender_odds_ratio = np.exp(gender_effect)
    constraints = bool(
        np.all(concentration > 0.0)
        and all_strictly_inside_unit_interval(
            cells["p00"], cells["p10"], cells["p01"], cells["p11"]
        )
        and np.isfinite(group_odds_ratio).all()
        and np.isfinite(gender_odds_ratio).all()
    )
    flat_intercept = intercept.reshape(-1)
    flat_group = group_effect.reshape(-1)
    flat_gender = gender_effect.reshape(-1)

    def linear_probability(index: int) -> np.ndarray:
        return sigmoid(
            flat_intercept
            + config["group_indicator"][index] * flat_group
            + config["gender_indicator"][index] * flat_gender
        )

    patients = conditional_patient_summaries(
        config,
        linear_probability,
        concentration.reshape(-1),
        rng,
        "shrinkage_toward_linear_predictor",
        [
            {"group": group, "gender": gender}
            for group, gender in zip(config["groups"], config["genders"], strict=True)
        ],
    )
    replicated_flat = replicated.reshape(-1, replicated.shape[-1])
    observed_proportions = config["successes"] / config["trials"]
    replicated_proportions = replicated_flat / config["trials"]
    group_comparison = config["group_indicator"] != 0.0
    gender_comparison = config["gender_indicator"] != 0.0
    observed_group_difference = float(group_mean_difference(observed_proportions, group_comparison))
    replicated_group_differences = group_mean_difference(replicated_proportions, group_comparison)
    observed_gender_difference = float(
        group_mean_difference(observed_proportions, gender_comparison)
    )
    replicated_gender_differences = group_mean_difference(replicated_proportions, gender_comparison)
    replicated_totals = replicated_flat.sum(axis=1)
    observed_total = int(config["successes"].sum())
    report = {
        "posterior": {
            "intercept_log_odds": summary(intercept, chains=chains),
            "group_log_odds_effect": summary(group_effect, chains=chains),
            "gender_log_odds_effect": summary(gender_effect, chains=chains),
            "reference_group_reference_gender_probability": summary(cells["p00"], chains=chains),
            "comparison_group_reference_gender_probability": summary(cells["p10"], chains=chains),
            "reference_group_comparison_gender_probability": summary(cells["p01"], chains=chains),
            "comparison_group_comparison_gender_probability": summary(cells["p11"], chains=chains),
            "reference_gender_probability_difference": summary(
                cells["p10"] - cells["p00"], chains=chains
            ),
            "comparison_gender_probability_difference": summary(
                cells["p11"] - cells["p01"], chains=chains
            ),
            "marginal_reference_group_probability": summary(
                cells["marginal_reference"], chains=chains
            ),
            "marginal_comparison_group_probability": summary(
                cells["marginal_comparison"], chains=chains
            ),
            "marginal_probability_difference_comparison_minus_reference": summary(
                cells["marginal_difference"], chains=chains
            ),
            "group_odds_ratio": summary(group_odds_ratio, chains=chains),
            "gender_odds_ratio": summary(gender_odds_ratio, chains=chains),
            "concentration": summary(concentration, chains=chains),
            "overdispersion_mean": float((1.0 / (concentration + 1.0)).mean()),
        },
        "patients": patients,
        "posterior_predictive": {
            "observed_total_successes": observed_total,
            "replicated_total_successes_mean": float(replicated_totals.mean()),
            "probability_replicated_total_successes_at_least_observed": float(
                np.mean(replicated_totals >= observed_total)
            ),
            "observed_group_mean_proportion_difference": observed_group_difference,
            "replicated_group_mean_proportion_difference_mean": float(
                replicated_group_differences.mean()
            ),
            "probability_replicated_group_difference_at_least_observed": float(
                np.mean(replicated_group_differences >= observed_group_difference)
            ),
            "observed_gender_mean_proportion_difference": observed_gender_difference,
            "replicated_gender_mean_proportion_difference_mean": float(
                replicated_gender_differences.mean()
            ),
            "probability_replicated_gender_difference_at_least_observed": float(
                np.mean(replicated_gender_differences >= observed_gender_difference)
            ),
        },
    }
    return report, constraints


def group_report(
    config: dict[str, Any],
    intercept: np.ndarray,
    effect: np.ndarray,
    concentration: np.ndarray,
    replicated: np.ndarray,
    rng: np.random.Generator,
) -> tuple[dict[str, Any], bool]:
    """Posterior, patient, and posterior predictive blocks of the two-group regression.

    Returns the result blocks and whether the draws satisfy the parameter constraints.
    """
    chains = config["chains"]
    reference = sigmoid(intercept)
    comparison = sigmoid(intercept + effect)
    difference = comparison - reference
    odds_ratio = np.exp(effect)
    constraints = bool(
        np.all(concentration > 0.0)
        and all_strictly_inside_unit_interval(reference, comparison)
        and np.isfinite(odds_ratio).all()
    )
    flat_intercept = intercept.reshape(-1)
    flat_effect = effect.reshape(-1)

    def group_probability(index: int) -> np.ndarray:
        return sigmoid(flat_intercept + config["indicator"][index] * flat_effect)

    patients = conditional_patient_summaries(
        config,
        group_probability,
        concentration.reshape(-1),
        rng,
        "shrinkage_toward_group",
        [{"group": group} for group in config["groups"]],
    )
    replicated_flat = replicated.reshape(-1, replicated.shape[-1])
    comparison_mask = config["indicator"] != 0.0
    reference_mask = ~comparison_mask
    observed_proportions = config["successes"] / config["trials"]
    observed_difference = float(group_mean_difference(observed_proportions, comparison_mask))
    replicated_proportions = replicated_flat / config["trials"]
    replicated_differences = group_mean_difference(replicated_proportions, comparison_mask)
    replicated_reference_totals = replicated_flat[:, reference_mask].sum(axis=1)
    replicated_comparison_totals = replicated_flat[:, comparison_mask].sum(axis=1)
    observed_reference_total = int(config["successes"][reference_mask].sum())
    observed_comparison_total = int(config["successes"][comparison_mask].sum())
    report = {
        "posterior": {
            "intercept_log_odds": summary(intercept, chains=chains),
            "group_log_odds_effect": summary(effect, chains=chains),
            "reference_probability": summary(reference, chains=chains),
            "comparison_probability": summary(comparison, chains=chains),
            "probability_difference_comparison_minus_reference": summary(difference, chains=chains),
            "odds_ratio": summary(odds_ratio, chains=chains),
            "concentration": summary(concentration, chains=chains),
            "overdispersion_mean": float((1.0 / (concentration + 1.0)).mean()),
        },
        "patients": patients,
        "posterior_predictive": {
            "observed_reference_successes": observed_reference_total,
            "observed_comparison_successes": observed_comparison_total,
            "replicated_reference_successes_mean": float(replicated_reference_totals.mean()),
            "replicated_comparison_successes_mean": float(replicated_comparison_totals.mean()),
            "probability_replicated_reference_successes_at_least_observed": float(
                np.mean(replicated_reference_totals >= observed_reference_total)
            ),
            "probability_replicated_comparison_successes_at_least_observed": float(
                np.mean(replicated_comparison_totals >= observed_comparison_total)
            ),
            "observed_group_mean_proportion_difference": observed_difference,
            "replicated_group_mean_proportion_difference_mean": float(
                replicated_differences.mean()
            ),
            "probability_replicated_difference_at_least_observed": float(
                np.mean(replicated_differences >= observed_difference)
            ),
        },
    }
    return report, constraints


def hierarchy_report(
    config: dict[str, Any],
    population: np.ndarray,
    concentration: np.ndarray,
    patient_probability: np.ndarray,
    replicated: np.ndarray,
) -> tuple[dict[str, Any], bool]:
    """Posterior, patient, and posterior predictive blocks of the beta-binomial hierarchy.

    Returns the result blocks and whether the draws satisfy the parameter constraints.
    """
    chains = config["chains"]
    constraints = bool(
        all_strictly_inside_unit_interval(population)
        and np.all(concentration > 0.0)
        and all_strictly_inside_unit_interval(patient_probability)
    )
    flat_patient = patient_probability.reshape(-1, patient_probability.shape[-1])
    population_mean = float(population.mean())
    patients = []
    for index, patient_id in enumerate(config["patient_ids"]):
        observed = float(config["successes"][index] / config["trials"][index])
        denominator = population_mean - observed
        if denominator == 0.0:
            shrinkage = 0.0
        else:
            shrinkage = float((flat_patient[:, index].mean() - observed) / denominator)
        patients.append(
            {
                "patient_id": patient_id,
                "successes": int(config["successes"][index]),
                "trials": int(config["trials"][index]),
                "observed_proportion": observed,
                "posterior_probability": summary(flat_patient[:, index], chains=chains),
                "shrinkage_toward_population": shrinkage,
            }
        )
    replicated_flat = replicated.reshape(-1, replicated.shape[-1])
    replicated_totals = replicated_flat.sum(axis=1)
    replicated_proportion_sd = (replicated_flat / config["trials"]).std(axis=1, ddof=1)
    observed_proportions = config["successes"] / config["trials"]
    report = {
        "posterior": {
            "population_probability": summary(population, chains=chains),
            "concentration": summary(concentration, chains=chains),
            "overdispersion_mean": float((1.0 / (concentration + 1.0)).mean()),
        },
        "patients": patients,
        "posterior_predictive": {
            "observed_total_successes": int(config["successes"].sum()),
            "replicated_total_successes_mean": float(replicated_totals.mean()),
            "replicated_total_successes_sd": float(replicated_totals.std(ddof=1)),
            "observed_patient_proportion_sd": float(observed_proportions.std(ddof=1)),
            "replicated_patient_proportion_sd_mean": float(replicated_proportion_sd.mean()),
        },
    }
    return report, constraints


def slide_hierarchy_report(
    config: dict[str, Any],
    draws: dict[str, np.ndarray],
    replicated: np.ndarray,
) -> tuple[dict[str, Any], bool]:
    """Posterior, patient, slide, and posterior predictive blocks of the slide hierarchy.

    ``draws`` holds the posterior draws of ``intercept_log_odds``, ``group_log_odds_effect``,
    ``gender_log_odds_effect``, ``patient_log_odds_sd``, ``slide_concentration``,
    ``patient_log_odds_effect``, and ``patient_probability``. Returns the result blocks and
    whether the draws satisfy the parameter constraints.
    """
    chains = config["chains"]
    intercept = draws["intercept_log_odds"]
    group_effect = draws["group_log_odds_effect"]
    gender_effect = draws["gender_log_odds_effect"]
    patient_sd = draws["patient_log_odds_sd"]
    concentration = draws["slide_concentration"]
    patient_effect = draws["patient_log_odds_effect"]
    patient_probability = draws["patient_probability"]
    patient_count = len(config["patient_ids"])
    gender_weight = float(config["patient_gender_indicator"].mean())
    cells = group_gender_probabilities(intercept, group_effect, gender_effect, gender_weight)
    constraints = bool(
        np.all(patient_sd > 0.0)
        and np.all(concentration > 0.0)
        and all_strictly_inside_unit_interval(patient_probability)
        and np.isfinite(np.exp(group_effect)).all()
        and np.isfinite(np.exp(gender_effect)).all()
    )
    patients = []
    patient_summaries = []
    for index, patient_id in enumerate(config["patient_ids"]):
        probability_summary = summary(patient_probability[..., index], chains=chains)
        patient_summaries.append(probability_summary)
        patient_successes = config["patient_successes"][index]
        patient_trials = config["patient_trials"][index]
        patients.append(
            {
                "patient_id": patient_id,
                "group": config["patient_groups"][index],
                "gender": config["patient_genders"][index],
                "slide_count": int(config["slide_counts"][patient_id]),
                "successes": int(patient_successes),
                "trials": int(patient_trials),
                "observed_proportion": float(patient_successes / patient_trials),
                "random_log_odds_effect": summary(patient_effect[..., index], chains=chains),
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
    group_comparison = config["patient_group_indicator"] != 0.0
    gender_comparison = config["patient_gender_indicator"] != 0.0
    observed_group_difference = float(
        group_mean_difference(observed_patient_proportions, group_comparison)
    )
    replicated_group_differences = group_mean_difference(
        replicated_patient_proportions, group_comparison
    )
    observed_gender_difference = float(
        group_mean_difference(observed_patient_proportions, gender_comparison)
    )
    replicated_gender_differences = group_mean_difference(
        replicated_patient_proportions, gender_comparison
    )
    observed_slide_proportions = config["successes"] / config["trials"]
    replicated_slide_proportions = replicated_flat / config["trials"]
    observed_slide_deviation = float(
        np.abs(
            observed_slide_proportions - observed_patient_proportions[config["patient_index"]]
        ).mean()
    )
    replicated_slide_deviations = np.abs(
        replicated_slide_proportions - replicated_patient_proportions[:, config["patient_index"]]
    ).mean(axis=1)
    replicated_totals = replicated_flat.sum(axis=1)
    observed_total = int(config["successes"].sum())
    report = {
        "posterior": {
            "intercept_log_odds": summary(intercept, chains=chains),
            "group_log_odds_effect": summary(group_effect, chains=chains),
            "gender_log_odds_effect": summary(gender_effect, chains=chains),
            "patient_log_odds_sd": summary(patient_sd, chains=chains),
            "slide_concentration": summary(concentration, chains=chains),
            "reference_group_reference_gender_probability": summary(cells["p00"], chains=chains),
            "comparison_group_reference_gender_probability": summary(cells["p10"], chains=chains),
            "reference_group_comparison_gender_probability": summary(cells["p01"], chains=chains),
            "comparison_group_comparison_gender_probability": summary(cells["p11"], chains=chains),
            "marginal_reference_group_probability": summary(
                cells["marginal_reference"], chains=chains
            ),
            "marginal_comparison_group_probability": summary(
                cells["marginal_comparison"], chains=chains
            ),
            "marginal_probability_difference_comparison_minus_reference": summary(
                cells["marginal_difference"], chains=chains
            ),
            "group_odds_ratio": summary(np.exp(group_effect), chains=chains),
            "gender_odds_ratio": summary(np.exp(gender_effect), chains=chains),
            "slide_overdispersion_mean": float((1.0 / (concentration + 1.0)).mean()),
        },
        "patients": patients,
        "slides": slides,
        "posterior_predictive": {
            "observed_total_successes": observed_total,
            "replicated_total_successes_mean": float(replicated_totals.mean()),
            "probability_replicated_total_successes_at_least_observed": float(
                np.mean(replicated_totals >= observed_total)
            ),
            "observed_patient_group_mean_proportion_difference": observed_group_difference,
            "replicated_patient_group_mean_proportion_difference_mean": float(
                replicated_group_differences.mean()
            ),
            "probability_replicated_patient_group_difference_at_least_observed": float(
                np.mean(replicated_group_differences >= observed_group_difference)
            ),
            "observed_patient_gender_mean_proportion_difference": observed_gender_difference,
            "replicated_patient_gender_mean_proportion_difference_mean": float(
                replicated_gender_differences.mean()
            ),
            "probability_replicated_patient_gender_difference_at_least_observed": float(
                np.mean(replicated_gender_differences >= observed_gender_difference)
            ),
            "observed_mean_absolute_slide_patient_deviation": observed_slide_deviation,
            "replicated_mean_absolute_slide_patient_deviation_mean": float(
                replicated_slide_deviations.mean()
            ),
            "probability_replicated_slide_deviation_at_least_observed": float(
                np.mean(replicated_slide_deviations >= observed_slide_deviation)
            ),
        },
    }
    return report, constraints


def class_probabilities(
    baseline_logits: np.ndarray, effects: np.ndarray
) -> tuple[np.ndarray, np.ndarray]:
    """Reference- and comparison-group class probabilities of the Dirichlet-multinomial model.

    The last class is the reference class with logit fixed at zero; the comparison group adds
    the log-ratio effects to the baseline logits.
    """
    zeros = np.zeros((*baseline_logits.shape[:-1], 1), dtype=np.float64)
    reference = softmax(np.concatenate([baseline_logits, zeros], axis=-1))
    comparison = softmax(np.concatenate([baseline_logits + effects, zeros], axis=-1))
    return reference, comparison


def class_posterior_report(
    class_ids: list[str],
    reference: np.ndarray,
    comparison: np.ndarray,
    concentration: np.ndarray,
    chains: int,
) -> tuple[dict[str, Any], bool]:
    """Posterior block of the Dirichlet-multinomial group model and its constraint check."""
    difference = comparison - reference
    constraints = bool(
        np.all(concentration > 0.0)
        and np.all(reference > 0.0)
        and np.all(comparison > 0.0)
        and np.allclose(reference.sum(axis=-1), 1.0)
        and np.allclose(comparison.sum(axis=-1), 1.0)
    )
    classes = [
        {
            "class_id": class_id,
            "reference_probability": summary(reference[..., index], chains=chains),
            "comparison_probability": summary(comparison[..., index], chains=chains),
            "difference_comparison_minus_reference": summary(difference[..., index], chains=chains),
        }
        for index, class_id in enumerate(class_ids)
    ]
    posterior = {
        "classes": classes,
        "concentration": summary(concentration, chains=chains),
        "overdispersion_mean": float((1.0 / (concentration + 1.0)).mean()),
    }
    return posterior, constraints


def predictive_class_report(config: dict[str, Any], replicated: np.ndarray) -> list[dict[str, Any]]:
    """Observed versus replicated group-mean class proportions for each class.

    ``replicated`` holds replicated count vectors with patients and classes on the last two axes.
    """
    observed_proportions = config["counts"] / config["totals"][:, None]
    replicated_proportions = replicated / config["totals"][:, None]
    reference_mask = config["indicator"] == 0
    comparison_mask = ~reference_mask
    observed_reference = observed_proportions[reference_mask].mean(axis=0)
    observed_comparison = observed_proportions[comparison_mask].mean(axis=0)
    replicated_reference = replicated_proportions[..., reference_mask, :].mean(axis=-2)
    replicated_comparison = replicated_proportions[..., comparison_mask, :].mean(axis=-2)
    observed_difference = observed_comparison - observed_reference
    replicated_difference = replicated_comparison - replicated_reference
    return [
        {
            "class_id": class_id,
            "observed_reference_mean_proportion": float(observed_reference[index]),
            "observed_comparison_mean_proportion": float(observed_comparison[index]),
            "replicated_reference_mean_proportion": float(replicated_reference[..., index].mean()),
            "replicated_comparison_mean_proportion": float(
                replicated_comparison[..., index].mean()
            ),
            "probability_replicated_difference_at_least_observed": float(
                np.mean(replicated_difference[..., index] >= observed_difference[index])
            ),
        }
        for index, class_id in enumerate(config["class_ids"])
    ]
