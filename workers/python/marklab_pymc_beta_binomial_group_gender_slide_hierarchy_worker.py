#!/usr/bin/env python3
"""PyMC worker for the slide-within-patient beta-binomial count hierarchy."""

from __future__ import annotations

import json
import sys
from collections import Counter
from pathlib import Path
from typing import Any

sys.path.append(str(Path(__file__).resolve().parent))

# Configure PyTensor before PyMC is imported.
from marklab_worker_support import pytensor_config  # noqa: F401

# isort: split
import numpy as np
import pymc as pm

from marklab_worker_support import pymc_nuts, pymc_request
from marklab_worker_support.count_summaries import slide_hierarchy_report
from marklab_worker_support.nuts_contract import fit_summary, pymc_nuts_diagnostics
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

RESULT_FORMAT = "marklab.pymc_beta_binomial_group_gender_slide_hierarchy_worker_result"
MODEL_KEYS = {
    "format",
    "version",
    "family",
    "reference_group",
    "comparison_group",
    "reference_gender",
    "comparison_gender",
    "intercept_prior",
    "intercept_prior_mean",
    "intercept_prior_sd",
    "group_effect_prior",
    "group_effect_prior_sd",
    "gender_effect_prior",
    "gender_effect_prior_sd",
    "patient_effect",
    "patient_log_odds_sd_prior",
    "patient_log_odds_sd_prior_sd",
    "slide_concentration_prior",
    "slide_concentration_prior_sd",
    "likelihood",
    "interaction",
    "standardization",
    "nesting",
    "biological_unit",
    "backend_capability",
    "maturity",
}
MONITORED = [
    "intercept_log_odds",
    "group_log_odds_effect",
    "gender_log_odds_effect",
    "patient_log_odds_sd",
    "slide_concentration",
    "patient_log_odds_effect",
]


def validate(request: Any, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    request, model = pymc_request.validate_envelope(
        request, lock_sha, worker_sha, ("slides",), MODEL_KEYS
    )
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
    identities = [reference_group, comparison_group, reference_gender, comparison_gender]
    if (
        any(not isinstance(value, str) or not value for value in identities)
        or reference_group == comparison_group
        or reference_gender == comparison_gender
    ):
        raise ContractError("group or gender identities are invalid")
    intercept_mean = number(model["intercept_prior_mean"], "model.intercept_mean")
    intercept_sd = number(model["intercept_prior_sd"], "model.intercept_sd")
    group_sd = number(model["group_effect_prior_sd"], "model.group_sd")
    gender_sd = number(model["gender_effect_prior_sd"], "model.gender_sd")
    patient_sd_prior = number(model["patient_log_odds_sd_prior_sd"], "model.patient_sd_prior")
    concentration_sd = number(model["slide_concentration_prior_sd"], "model.slide_concentration_sd")
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
    ordering = list(zip(slide_patient_ids, slide_ids, strict=True))
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
    design = list(zip(patient_groups, patient_genders, strict=True))
    for expected_group in (reference_group, comparison_group):
        for expected_gender in (reference_gender, comparison_gender):
            if design.count((expected_group, expected_gender)) < 4:
                raise ContractError("each group/gender cell requires four patients")
    patient_successes = np.zeros(len(patient_ids), dtype=np.int64)
    patient_trials = np.zeros(len(patient_ids), dtype=np.int64)
    for slide, patient in enumerate(patient_index):
        patient_successes[patient] += successes[slide]
        patient_trials[patient] += trials[slide]
    sampling = pymc_request.validate_sampling(request["sampling"])
    resources = obj(
        request["resources"],
        {
            "maximum_patients",
            "maximum_slides",
            "maximum_total_trials",
            "maximum_total_iterations",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    iterations = sampling["chains"] * (sampling["tune"] + sampling["draws"])
    if (
        len(patient_ids) > integer(resources["maximum_patients"], "resources.patients", 16, 512)
        or len(raw_slides) > integer(resources["maximum_slides"], "resources.slides", 32, 2048)
        or sum(trials)
        > integer(resources["maximum_total_trials"], "resources.trials", 1, 10_000_000)
        or iterations
        > integer(resources["maximum_total_iterations"], "resources.iterations", 1, 400_000)
    ):
        raise ContractError("slide hierarchy resources exceeded")
    policy = pymc_request.validate_policy(request["diagnostic_policy"])
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
        **sampling,
        **policy,
        "maximum_output_bytes": integer(
            resources["maximum_output_bytes"], "resources.output", 1, 2 * MIB
        ),
    }


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    return derive_seed(
        "marklab-pymc-beta-binomial-group-gender-slide-hierarchy-v1", seed, purpose, index
    )


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    """Fit the slide-within-patient beta-binomial hierarchy by NUTS.

    Likelihood: slide counts ``y_s ~ BetaBinomial(n_s, p_i * phi, (1 - p_i) * phi)`` for the
    slide's patient ``i``, with ``logit(p_i) = alpha + beta_group * g_i + beta_gender * s_i +
    u_i``. Patient effects are non-centred: ``u_i = tau * z_i`` with ``z_i ~ Normal(0, 1)`` and
    ``tau ~ HalfNormal(patient_sd_prior)``. Priors: ``alpha ~ Normal(intercept_mean,
    intercept_sd)``, ``beta_group ~ Normal(0, group_sd)``, ``beta_gender ~ Normal(0,
    gender_sd)``, and slide concentration ``phi ~ HalfNormal(concentration_sd)``.
    Normal scales are SDs; half-normal scales are the SDs of their underlying normal distributions.
    """
    patient_count = len(config["patient_ids"])
    with pm.Model():
        intercept = pm.Normal(
            "intercept_log_odds", config["intercept_mean"], config["intercept_sd"]
        )
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
        slide_concentration = pm.HalfNormal("slide_concentration", config["concentration_sd"])
        slide_probability = patient_probability[config["patient_index"]]
        pm.BetaBinomial(
            "successes",
            alpha=slide_probability * slide_concentration,
            beta=(1.0 - slide_probability) * slide_concentration,
            n=config["trials"],
            observed=config["successes"],
        )
        prior, posterior, predictive = pymc_nuts.sample_prior_posterior_predictive(
            config, seed_for, "successes"
        )
    draws = {
        name: np.asarray(posterior["posterior"][name].values, dtype=np.float64)
        for name in [*MONITORED, "patient_probability"]
    }
    replicated = np.asarray(predictive["posterior_predictive"]["successes"].values, dtype=np.int64)
    prior_finite = pymc_nuts.prior_is_finite(prior, ("prior", "prior_predictive"))
    posterior_finite = bool(
        all(np.isfinite(value).all() for value in [*draws.values(), replicated])
    )
    convergence = pymc_nuts_diagnostics(posterior, MONITORED)
    report, constraints = slide_hierarchy_report(config, draws, replicated)
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": backend_identity("pymc", pm.__version__, lock_sha, worker_sha),
        "request_sha256": request_sha,
        **fit_summary(convergence, config, prior_finite, posterior_finite, constraints),
        **report,
    }


def main() -> None:
    require_versions(pymc=pm.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(2 * MIB)
    config = validate(json.loads(raw), lock_sha, worker_sha)
    result = fit(config, sha256_hex(raw), lock_sha, worker_sha)
    write_result(result, max_bytes=config["maximum_output_bytes"], newline=False)


if __name__ == "__main__":
    run_worker(main, "Marklab PyMC slide hierarchy worker failed")
