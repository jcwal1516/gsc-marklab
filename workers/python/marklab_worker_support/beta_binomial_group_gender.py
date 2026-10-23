"""Backend-neutral request contract for beta-binomial group/gender regression."""

from __future__ import annotations

from typing import Any

import numpy as np

from . import pymc_request
from .protocol import ContractError, exact, integer, number, obj

SOURCE_WORKER = "marklab_pymc_beta_binomial_group_gender_regression_worker.py"

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
    "concentration_prior",
    "concentration_prior_sd",
    "patient_probability",
    "likelihood",
    "interaction",
    "standardization",
    "biological_unit",
    "backend_capability",
    "maturity",
}


def validate_request(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request, model = pymc_request.validate_envelope(
        request, lock_digest, worker_digest, ("patients",), MODEL_KEYS
    )
    exact(model["family"], "patient_beta_binomial_group_gender_regression", "model.family")
    exact(model["intercept_prior"], "normal_log_odds", "model.intercept_prior")
    exact(model["group_effect_prior"], "normal_log_odds_difference", "model.group_prior")
    exact(model["gender_effect_prior"], "normal_log_odds_difference", "model.gender_prior")
    exact(model["concentration_prior"], "half_normal", "model.concentration_prior")
    exact(
        model["patient_probability"],
        "beta_additive_group_gender_mean_concentration",
        "model.patient",
    )
    exact(model["likelihood"], "beta_binomial_collapsed_patient_counts", "model.likelihood")
    exact(model["interaction"], "none", "model.interaction")
    exact(model["standardization"], "observed_gender_distribution", "model.standardization")
    reference_group = model["reference_group"]
    comparison_group = model["comparison_group"]
    reference_gender = model["reference_gender"]
    comparison_gender = model["comparison_gender"]
    identities = [reference_group, comparison_group, reference_gender, comparison_gender]
    if any(not isinstance(value, str) or not value for value in identities):
        raise ContractError("group or gender identities are invalid")
    if reference_group == comparison_group or reference_gender == comparison_gender:
        raise ContractError("group or gender contrasts are degenerate")
    intercept_mean = number(model["intercept_prior_mean"], "model.intercept_mean")
    intercept_sd = number(model["intercept_prior_sd"], "model.intercept_sd")
    group_sd = number(model["group_effect_prior_sd"], "model.group_sd")
    gender_sd = number(model["gender_effect_prior_sd"], "model.gender_sd")
    concentration_sd = number(model["concentration_prior_sd"], "model.concentration_sd")
    if min(intercept_sd, group_sd, gender_sd, concentration_sd) <= 0.0:
        raise ContractError("model scales must be positive")
    patients = request["patients"]
    if not isinstance(patients, list) or not 16 <= len(patients) <= 512:
        raise ContractError("patient count is invalid")
    patient_ids: list[str] = []
    groups: list[str] = []
    genders: list[str] = []
    successes: list[int] = []
    trials: list[int] = []
    for index, raw in enumerate(patients):
        row = obj(
            raw, {"patient_id", "group", "gender", "successes", "trials"}, f"patients[{index}]"
        )
        patient_id = row["patient_id"]
        group = row["group"]
        gender = row["gender"]
        if (
            not isinstance(patient_id, str)
            or not patient_id
            or len(patient_id) > 128
            or group not in {reference_group, comparison_group}
            or gender not in {reference_gender, comparison_gender}
        ):
            raise ContractError("patient identity, group, or gender is invalid")
        patient_ids.append(patient_id)
        groups.append(group)
        genders.append(gender)
        trial = integer(row["trials"], "patient.trials", 1, 10_000_000)
        successes.append(integer(row["successes"], "patient.successes", 0, trial))
        trials.append(trial)
    if patient_ids != sorted(patient_ids) or len(set(patient_ids)) != len(patient_ids):
        raise ContractError("patient identities must be unique and sorted")
    design = list(zip(groups, genders, strict=True))
    for expected_group in (reference_group, comparison_group):
        for expected_gender in (reference_gender, comparison_gender):
            if design.count((expected_group, expected_gender)) < 4:
                raise ContractError("each group/gender design cell requires four patients")
    sampling = pymc_request.validate_sampling(request["sampling"])
    resources = obj(
        request["resources"],
        {
            "maximum_patients",
            "maximum_total_trials",
            "maximum_total_iterations",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    if len(patients) > integer(resources["maximum_patients"], "resources.patients", 16, 512):
        raise ContractError("patient limit exceeded")
    if sum(trials) > integer(resources["maximum_total_trials"], "resources.trials", 1, 10_000_000):
        raise ContractError("trial limit exceeded")
    iterations = sampling["chains"] * (sampling["tune"] + sampling["draws"])
    if iterations > integer(
        resources["maximum_total_iterations"], "resources.iterations", 1, 400_000
    ):
        raise ContractError("iteration limit exceeded")
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
        "concentration_sd": concentration_sd,
        "patient_ids": patient_ids,
        "groups": groups,
        "genders": genders,
        "group_indicator": np.asarray(
            [group == comparison_group for group in groups], dtype=np.float64
        ),
        "gender_indicator": np.asarray(
            [gender == comparison_gender for gender in genders], dtype=np.float64
        ),
        "successes": np.asarray(successes, dtype=np.int64),
        "trials": np.asarray(trials, dtype=np.int64),
        **sampling,
        **policy,
    }
