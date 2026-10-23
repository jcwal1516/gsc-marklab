"""Backend-neutral request contract for Dirichlet-multinomial group regression."""

from __future__ import annotations

from typing import Any

import numpy as np

from . import pymc_request
from .protocol import ContractError, exact, integer, number, obj

SOURCE_WORKER = "marklab_pymc_dirichlet_multinomial_group_worker.py"

MODEL_KEYS = {
    "format",
    "version",
    "family",
    "reference_group",
    "comparison_group",
    "class_ids",
    "reference_class",
    "baseline_logit_prior",
    "logit_prior_sd",
    "group_effect_prior",
    "group_effect_prior_sd",
    "concentration_prior",
    "concentration_prior_sd",
    "likelihood",
    "observation_unit",
    "biological_unit",
    "backend_capability",
    "maturity",
}


def validate_request(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request, model = pymc_request.validate_envelope(
        request, lock_digest, worker_digest, ("patients",), MODEL_KEYS
    )
    exact(model["family"], "patient_dirichlet_multinomial_group_regression", "model.family")
    exact(model["baseline_logit_prior"], "normal_with_reference_class_zero", "model.baseline")
    exact(
        model["group_effect_prior"],
        "normal_log_ratio_difference_with_reference_class_zero",
        "model.group_effect",
    )
    exact(model["concentration_prior"], "half_normal", "model.concentration")
    exact(
        model["likelihood"],
        "dirichlet_multinomial_collapsed_patient_count_vectors",
        "model.likelihood",
    )
    exact(model["observation_unit"], "patient_complete_class_count_vector", "model.unit")
    exact(model["biological_unit"], "patient", "model.biological_unit")
    class_ids = model["class_ids"]
    if (
        not isinstance(class_ids, list)
        or not 3 <= len(class_ids) <= 16
        or any(not isinstance(value, str) or not value for value in class_ids)
        or len(set(class_ids)) != len(class_ids)
        or model["reference_class"] != class_ids[-1]
    ):
        raise ContractError("class identities are invalid")
    reference_group = model["reference_group"]
    comparison_group = model["comparison_group"]
    if (
        not isinstance(reference_group, str)
        or not reference_group
        or not isinstance(comparison_group, str)
        or not comparison_group
        or reference_group == comparison_group
    ):
        raise ContractError("group identities are invalid")
    logit_sd = number(model["logit_prior_sd"], "model.logit_sd")
    group_sd = number(model["group_effect_prior_sd"], "model.group_sd")
    concentration_sd = number(model["concentration_prior_sd"], "model.concentration_sd")
    if min(logit_sd, group_sd, concentration_sd) <= 0.0:
        raise ContractError("prior scales must be positive")
    patients = request["patients"]
    if not isinstance(patients, list) or not 8 <= len(patients) <= 512:
        raise ContractError("patient count is invalid")
    patient_ids = []
    groups = []
    counts = []
    for index, raw in enumerate(patients):
        row = obj(raw, {"patient_id", "group", "counts"}, f"patients[{index}]")
        patient_id = row["patient_id"]
        group = row["group"]
        raw_counts = row["counts"]
        if (
            not isinstance(patient_id, str)
            or not patient_id
            or len(patient_id) > 128
            or group not in {reference_group, comparison_group}
            or not isinstance(raw_counts, list)
            or len(raw_counts) != len(class_ids)
        ):
            raise ContractError("patient row identity or dimensions are invalid")
        vector = [integer(value, "patient.count", 0, 10_000_000) for value in raw_counts]
        if sum(vector) <= 0:
            raise ContractError("patient count vector must be positive")
        patient_ids.append(patient_id)
        groups.append(group)
        counts.append(vector)
    if patient_ids != sorted(patient_ids) or len(set(patient_ids)) != len(patient_ids):
        raise ContractError("patient identities must be unique and sorted")
    if min(groups.count(reference_group), groups.count(comparison_group)) < 4:
        raise ContractError("each group requires four patients")
    counts_array = np.asarray(counts, dtype=np.int64)
    totals = counts_array.sum(axis=1)
    sampling = pymc_request.validate_sampling(request["sampling"])
    resources = obj(
        request["resources"],
        {
            "maximum_patients",
            "maximum_classes",
            "maximum_total_cells",
            "maximum_total_iterations",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    if len(patients) > integer(resources["maximum_patients"], "resources.patients", 8, 512):
        raise ContractError("patient limit exceeded")
    if len(class_ids) > integer(resources["maximum_classes"], "resources.classes", 3, 16):
        raise ContractError("class limit exceeded")
    total_cells = int(totals.sum())
    if total_cells > integer(resources["maximum_total_cells"], "resources.cells", 1, 10_000_000):
        raise ContractError("cell limit exceeded")
    iterations = sampling["chains"] * (sampling["tune"] + sampling["draws"])
    if iterations > integer(
        resources["maximum_total_iterations"], "resources.iterations", 1, 400_000
    ):
        raise ContractError("iteration limit exceeded")
    policy = pymc_request.validate_policy(request["diagnostic_policy"])
    return {
        "class_ids": class_ids,
        "reference_group": reference_group,
        "comparison_group": comparison_group,
        "groups": groups,
        "indicator": np.asarray([group == comparison_group for group in groups], dtype=np.int64),
        "counts": counts_array,
        "totals": totals,
        "logit_sd": logit_sd,
        "group_sd": group_sd,
        "concentration_sd": concentration_sd,
        **sampling,
        **policy,
    }
