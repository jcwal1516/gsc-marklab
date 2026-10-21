#!/usr/bin/env python3
"""PyMC worker for patient-unit Dirichlet-multinomial group regression."""

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

from marklab_worker_support import pymc_nuts, pymc_request
from marklab_worker_support.count_summaries import (
    class_posterior_report,
    class_probabilities,
    predictive_class_report,
)
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

RESULT_FORMAT = "marklab.pymc_dirichlet_multinomial_group_worker_result"
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


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
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


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    return derive_seed("marklab-pymc-dirichlet-multinomial-group-v1", seed, purpose, index)


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    """Fit the collapsed patient Dirichlet-multinomial two-group model by NUTS.

    Likelihood: class-count vectors ``y_i ~ DirichletMultinomial(N_i, phi * pi_g(i))``. Class
    probabilities are ``pi_ref = softmax([b, 0])`` and ``pi_cmp = softmax([b + d, 0])`` with
    the last class as the zero-logit reference. Priors: baseline logits
    ``b_k ~ Normal(0, logit_sd)``, group log-ratio effects ``d_k ~ Normal(0, group_sd)``, and
    concentration ``phi ~ HalfNormal(concentration_sd)``.
    Normal scales are SDs; half-normal scales are the SDs of their underlying normal distributions.
    """
    classes = len(config["class_ids"])
    with pm.Model():
        baseline = pm.Normal("baseline_logits", 0.0, config["logit_sd"], shape=classes - 1)
        effect = pm.Normal("group_log_ratio_effects", 0.0, config["group_sd"], shape=classes - 1)
        concentration = pm.HalfNormal("concentration", config["concentration_sd"])
        zero = pt.zeros((1,))
        reference_probability = pm.math.softmax(pt.concatenate([baseline, zero]))
        comparison_probability = pm.math.softmax(pt.concatenate([baseline + effect, zero]))
        group_probability = pt.stack([reference_probability, comparison_probability])
        patient_probability = group_probability[config["indicator"]]
        pm.DirichletMultinomial(
            "counts",
            n=config["totals"],
            a=patient_probability * concentration,
            observed=config["counts"],
        )
        prior, posterior, predictive = pymc_nuts.sample_prior_posterior_predictive(
            config, seed_for, "counts"
        )
    draws = posterior["posterior"]
    baseline_draws = np.asarray(draws["baseline_logits"].values, dtype=np.float64)
    effect_draws = np.asarray(draws["group_log_ratio_effects"].values, dtype=np.float64)
    concentration_draws = np.asarray(draws["concentration"].values, dtype=np.float64)
    reference_draws, comparison_draws = class_probabilities(baseline_draws, effect_draws)
    replicated = np.asarray(predictive["posterior_predictive"]["counts"].values, dtype=np.int64)
    monitored = ["baseline_logits", "group_log_ratio_effects", "concentration"]
    prior_finite = pymc_nuts.prior_is_finite(prior)
    posterior_finite = bool(
        np.isfinite(reference_draws).all()
        and np.isfinite(comparison_draws).all()
        and np.isfinite(concentration_draws).all()
        and np.isfinite(replicated).all()
    )
    convergence = pymc_nuts_diagnostics(posterior, monitored)
    posterior_report, constraints = class_posterior_report(
        config["class_ids"],
        reference_draws,
        comparison_draws,
        concentration_draws,
        config["chains"],
    )
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": backend_identity("pymc", pm.__version__, lock_sha, worker_sha),
        "request_sha256": request_sha,
        **fit_summary(convergence, config, prior_finite, posterior_finite, constraints),
        "posterior": posterior_report,
        "posterior_predictive": {"classes": predictive_class_report(config, replicated)},
    }


def main() -> None:
    require_versions(pymc=pm.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(2 * MIB)
    config = validate(json.loads(raw), lock_sha, worker_sha)
    result = fit(config, sha256_hex(raw), lock_sha, worker_sha)
    write_result(result, max_bytes=2 * MIB)


if __name__ == "__main__":
    run_worker(main, "Marklab PyMC Dirichlet-multinomial group worker failed")
