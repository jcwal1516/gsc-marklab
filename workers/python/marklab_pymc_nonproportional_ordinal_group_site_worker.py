#!/usr/bin/env python3
"""PyMC worker for a patient ordinal group model with group-specific cutpoints and site effects."""

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

from marklab_worker_support import ordinal, pymc_nuts, pymc_request
from marklab_worker_support.nuts_contract import fit_summary, pymc_nuts_diagnostics
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

RESULT_FORMAT = "marklab.pymc_nonproportional_ordinal_group_site_worker_result"
MODEL_KEYS = {
    "format",
    "version",
    "family",
    "reference_group",
    "comparison_group",
    "ordered_levels",
    "group_cutpoint_prior",
    "cutpoint_prior_sd",
    "site_intercept_prior",
    "site_intercept_sd_prior_sd",
    "threshold_effect_definition",
    "probability_constraint",
    "group_probability_standardization",
    "likelihood",
    "observation_unit",
    "biological_unit",
    "backend_capability",
    "maturity",
}
MODEL_CONSTANTS = {
    "family": "patient_nonproportional_ordinal_group_site",
    "group_cutpoint_prior": "two_independent_ordered_normal_vectors",
    "site_intercept_prior": "sum_zero_noncentered_normal",
    "threshold_effect_definition": "reference_cutpoint_minus_comparison_cutpoint",
    "probability_constraint": "each_group_cutpoint_vector_strictly_ordered",
    "group_probability_standardization": "equal_weight_sites",
    "likelihood": "ordered_logistic_patient_outcome",
    "observation_unit": "one_complete_ordered_outcome_per_patient",
    "biological_unit": "patient",
    "backend_capability": "nuts",
    "maturity": "experimental",
}
MONITORED = [
    "reference_cutpoints",
    "comparison_cutpoints",
    "site_intercept_sd",
    "site_intercept_raw",
]
REPORTED = ["reference_cutpoints", "comparison_cutpoints", "site_intercept_sd", "site_intercept"]


def validate(request: Any, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    request, model = pymc_request.validate_envelope(
        request, lock_sha, worker_sha, ("site_ids", "patients"), MODEL_KEYS
    )
    for key, expected in MODEL_CONSTANTS.items():
        exact(model[key], expected, f"model.{key}")
    reference_group = model["reference_group"]
    comparison_group = model["comparison_group"]
    levels = model["ordered_levels"]
    site_ids = request["site_ids"]
    valid_name = pymc_request.valid_name
    if (
        not valid_name(reference_group)
        or not valid_name(comparison_group)
        or reference_group == comparison_group
        or not isinstance(levels, list)
        or not 2 <= len(levels) <= 8
        or not all(valid_name(level) for level in levels)
        or len(set(levels)) != len(levels)
        or not isinstance(site_ids, list)
        or site_ids != sorted(site_ids)
        or not 8 <= len(site_ids) <= 64
        or len(set(site_ids)) != len(site_ids)
    ):
        raise ContractError("model groups, levels, or sites are invalid")
    cutpoint_sd = number(model["cutpoint_prior_sd"], "model.cutpoint_prior_sd")
    site_sd = number(model["site_intercept_sd_prior_sd"], "model.site_intercept_sd_prior_sd")
    if min(cutpoint_sd, site_sd) <= 0:
        raise ContractError("prior scales must be positive")
    site_lookup = {site: index for index, site in enumerate(site_ids)}
    patient_ids: list[str] = []
    groups: list[str] = []
    outcomes: list[int] = []
    site_index: list[int] = []
    group_counts = np.zeros((len(site_ids), 2), dtype=int)
    for index, raw in enumerate(request["patients"]):
        row = obj(raw, {"patient_id", "site_id", "group", "outcome_code"}, f"patients[{index}]")
        if (
            not valid_name(row["patient_id"])
            or row["site_id"] not in site_lookup
            or row["group"] not in {reference_group, comparison_group}
        ):
            raise ContractError("patient identity is invalid")
        patient_ids.append(row["patient_id"])
        groups.append(row["group"])
        outcomes.append(integer(row["outcome_code"], "patient.outcome", 0, len(levels) - 1))
        site_index.append(site_lookup[row["site_id"]])
        group_counts[site_index[-1], int(row["group"] == comparison_group)] += 1
    if (
        not 32 <= len(patient_ids) <= 1024
        or patient_ids != sorted(patient_ids)
        or len(set(patient_ids)) != len(patient_ids)
        or np.any(group_counts < 2)
        or set(outcomes) != set(range(len(levels)))
    ):
        raise ContractError("patient count, order, site support, or level support is invalid")
    sampling = pymc_request.validate_sampling(request["sampling"])
    resources = obj(
        request["resources"],
        {
            "maximum_patients",
            "maximum_sites",
            "maximum_levels",
            "maximum_total_iterations",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    iterations = sampling["chains"] * (sampling["tune"] + sampling["draws"])
    if (
        len(patient_ids) > integer(resources["maximum_patients"], "resources.patients", 32, 1024)
        or len(site_ids) > integer(resources["maximum_sites"], "resources.sites", 8, 64)
        or len(levels) > integer(resources["maximum_levels"], "resources.levels", 2, 8)
        or iterations
        > integer(resources["maximum_total_iterations"], "resources.iterations", 1, 400_000)
    ):
        raise ContractError("resource limit exceeded")
    output_limit = integer(resources["maximum_output_bytes"], "resources.output", 1, 2 * MIB)
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3600)
    return {
        "levels": levels,
        "site_ids": site_ids,
        "comparison": np.asarray([group == comparison_group for group in groups]),
        "outcomes": np.asarray(outcomes, dtype=int),
        "site_index": np.asarray(site_index, dtype=int),
        "cutpoint_sd": cutpoint_sd,
        "site_sd_prior": site_sd,
        **sampling,
        "output_limit": output_limit,
        **pymc_request.validate_policy(request["diagnostic_policy"]),
    }


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    return derive_seed("marklab-nonprop-ordinal-v1", seed, purpose, index)


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    """Fit the non-proportional ordinal group model with site intercepts by NUTS.

    Likelihood: ``y_i ~ Categorical(p_i)`` with ordered-logistic level probabilities from the
    cutpoints ``c^g`` of patient ``i``'s group: ``P(y_i <= k) = sigmoid(c^g_k - a_s)`` for site
    ``s``. Each group has its own ordered cutpoints, ``c^g_k ~ Normal(m_k, cutpoint_sd)`` around
    evenly spaced means on ``[-1.5, 1.5]``, so the group effect may differ by threshold. Site
    intercepts are non-centered and sum to zero, ``a = sigma * (z - mean(z))`` with
    ``z ~ Normal(0, 1)`` and ``sigma ~ HalfNormal(site_sd_prior)``. Group probabilities average
    the sites equally.
    Normal scales are SDs; half-normal scales are the SDs of their underlying normal distributions.
    """
    initial = ordinal.initial_cutpoints(config["levels"])
    with pm.Model():
        reference_cutpoints = ordinal.ordered_cutpoints(
            "reference_cutpoints", initial, config["cutpoint_sd"]
        )
        comparison_cutpoints = ordinal.ordered_cutpoints(
            "comparison_cutpoints", initial, config["cutpoint_sd"]
        )
        site_sd = pm.HalfNormal("site_intercept_sd", config["site_sd_prior"])
        raw = pm.Normal("site_intercept_raw", 0, 1, shape=len(config["site_ids"]))
        site_intercept = pm.Deterministic("site_intercept", (raw - raw.mean()) * site_sd)
        patient_cutpoints = pt.where(
            config["comparison"][:, None],
            comparison_cutpoints[None, :],
            reference_cutpoints[None, :],
        )
        location = site_intercept[config["site_index"]][:, None]
        cumulative = 1 / (1 + pt.exp(-(patient_cutpoints - location)))
        probability = pt.concatenate(
            [cumulative[:, :1], cumulative[:, 1:] - cumulative[:, :-1], 1 - cumulative[:, -1:]],
            axis=1,
        )
        pm.Categorical("outcome", p=probability, observed=config["outcomes"])
        posterior = pymc_nuts.nuts(config, seed_for)
        predictive = pymc_nuts.posterior_predictive(posterior, "outcome", config, seed_for)
    draws = {
        name: np.asarray(posterior["posterior"][name].values, dtype=float) for name in REPORTED
    }
    reference_probabilities = ordinal.category_probabilities(
        draws["reference_cutpoints"][:, :, None, :], draws["site_intercept"]
    )
    comparison_probabilities = ordinal.category_probabilities(
        draws["comparison_cutpoints"][:, :, None, :], draws["site_intercept"]
    )
    reference_marginal = reference_probabilities.mean(axis=2)
    comparison_marginal = comparison_probabilities.mean(axis=2)
    replicated = np.asarray(predictive["posterior_predictive"]["outcome"].values).reshape(
        -1, len(config["outcomes"])
    )
    # The prior check is a fixed-size finite draw; the model prior itself is not simulated.
    prior_rng = np.random.default_rng(seed_for(config["seed"], "prior"))
    prior_finite = bool(np.isfinite(prior_rng.normal(size=(config["prior_draws"], 4))).all())
    posterior_finite = bool(
        all(np.isfinite(value).all() for value in draws.values()) and np.isfinite(replicated).all()
    )
    convergence = pymc_nuts_diagnostics(posterior, MONITORED)
    constraints = bool(
        np.all(np.diff(draws["reference_cutpoints"], axis=-1) > 0)
        and np.all(np.diff(draws["comparison_cutpoints"], axis=-1) > 0)
        and np.allclose(draws["site_intercept"].sum(axis=-1), 0, atol=1e-10)
    )
    levels = config["levels"]
    level_posteriors, predictive_levels = ordinal.level_reports(
        levels,
        config["outcomes"],
        config["comparison"],
        reference_marginal.reshape(-1, len(levels)),
        comparison_marginal.reshape(-1, len(levels)),
        replicated,
    )
    threshold_effects = [
        {
            "threshold_after_level": levels[index],
            "summary": summary(
                draws["reference_cutpoints"][..., index] - draws["comparison_cutpoints"][..., index]
            ),
        }
        for index in range(len(levels) - 1)
    ]
    codes = np.arange(len(levels))
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": backend_identity("pymc", pm.__version__, lock_sha, worker_sha),
        "request_sha256": request_sha,
        **fit_summary(convergence, config, prior_finite, posterior_finite, constraints),
        "posterior": {
            "reference_cutpoints": ordinal.cutpoint_summaries(levels, draws["reference_cutpoints"]),
            "comparison_cutpoints": ordinal.cutpoint_summaries(
                levels, draws["comparison_cutpoints"]
            ),
            "threshold_group_log_odds_effects": threshold_effects,
            "site_intercept_sd": summary(draws["site_intercept_sd"]),
            "sites": [
                {"site_id": site, "intercept": summary(draws["site_intercept"][..., index])}
                for index, site in enumerate(config["site_ids"])
            ],
            "levels": level_posteriors,
            "reference_expected_code": summary((reference_marginal * codes).sum(axis=-1)),
            "comparison_expected_code": summary((comparison_marginal * codes).sum(axis=-1)),
        },
        "posterior_predictive": {"levels": predictive_levels},
    }


def main() -> None:
    require_versions(pymc=pm.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(2 * MIB)
    config = validate(json.loads(raw), lock_sha, worker_sha)
    result = fit(config, sha256_hex(raw), lock_sha, worker_sha)
    write_result(result, max_bytes=config["output_limit"])


if __name__ == "__main__":
    run_worker(main, "Marklab PyMC nonproportional ordinal worker failed")
