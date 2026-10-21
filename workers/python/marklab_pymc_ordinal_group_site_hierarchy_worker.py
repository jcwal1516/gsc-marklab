#!/usr/bin/env python3
"""PyMC worker for a patient ordinal group model with site-varying effects."""

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

RESULT_FORMAT = "marklab.pymc_ordinal_group_site_hierarchy_worker_result"
MODEL_KEYS = {
    "format",
    "version",
    "family",
    "reference_group",
    "comparison_group",
    "ordered_levels",
    "cutpoint_prior",
    "cutpoint_prior_sd",
    "group_effect_prior",
    "group_effect_prior_sd",
    "site_intercept_prior",
    "site_intercept_sd_prior_sd",
    "site_group_slope_prior",
    "site_group_slope_sd_prior_sd",
    "constraints",
    "likelihood",
    "group_probability_standardization",
    "observation_unit",
    "biological_unit",
    "backend_capability",
    "maturity",
}
MODEL_CONSTANTS = {
    "family": "patient_proportional_odds_site_varying_group_regression",
    "cutpoint_prior": "ordered_normal",
    "group_effect_prior": "normal_global_log_odds_difference",
    "site_intercept_prior": "sum_zero_noncentered_normal",
    "site_group_slope_prior": "sum_zero_noncentered_normal",
    "constraints": "site_intercepts_and_site_group_slope_deviations_sum_zero",
    "likelihood": "ordered_logistic_patient_outcome",
    "group_probability_standardization": "equal_weight_sites",
    "observation_unit": "one_complete_ordered_outcome_per_patient",
    "biological_unit": "patient",
    "backend_capability": "nuts",
    "maturity": "experimental",
}
MONITORED = [
    "cutpoints",
    "global_group_log_odds_effect",
    "site_intercept_sd",
    "site_group_slope_sd",
    "site_intercept_raw",
    "site_group_slope_raw",
]
REPORTED = [
    "cutpoints",
    "global_group_log_odds_effect",
    "site_intercept_sd",
    "site_group_slope_sd",
    "site_intercept",
    "site_group_slope_deviation",
]


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
        or not 8 <= len(site_ids) <= 64
        or site_ids != sorted(site_ids)
        or not all(valid_name(site) for site in site_ids)
        or len(set(site_ids)) != len(site_ids)
    ):
        raise ContractError("model groups, levels, or sites are invalid")
    scales = [
        number(model["cutpoint_prior_sd"], "model.cutpoint_sd"),
        number(model["group_effect_prior_sd"], "model.group_sd"),
        number(model["site_intercept_sd_prior_sd"], "model.site_intercept_sd"),
        number(model["site_group_slope_sd_prior_sd"], "model.site_slope_sd"),
    ]
    if min(scales) <= 0.0:
        raise ContractError("prior scales must be positive")
    patients = request["patients"]
    if not isinstance(patients, list) or not 32 <= len(patients) <= 1024:
        raise ContractError("patient count is invalid")
    site_lookup = {site: index for index, site in enumerate(site_ids)}
    patient_ids: list[str] = []
    groups: list[str] = []
    outcomes: list[int] = []
    site_index: list[int] = []
    group_counts = np.zeros((len(site_ids), 2), dtype=np.int64)
    for index, raw in enumerate(patients):
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
        patient_ids != sorted(patient_ids)
        or len(set(patient_ids)) != len(patient_ids)
        or np.any(group_counts < 2)
        or set(outcomes) != set(range(len(levels)))
    ):
        raise ContractError("patient order, site support, or level support is invalid")
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
        len(patients) > integer(resources["maximum_patients"], "resources.patients", 32, 1024)
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
        "indicator": np.asarray([group == comparison_group for group in groups], dtype=np.float64),
        "outcomes": np.asarray(outcomes, dtype=np.int64),
        "site_index": np.asarray(site_index, dtype=np.int64),
        "cutpoint_sd": scales[0],
        "group_sd": scales[1],
        "site_intercept_sd_prior": scales[2],
        "site_slope_sd_prior": scales[3],
        **sampling,
        "output_limit": output_limit,
        **pymc_request.validate_policy(request["diagnostic_policy"]),
    }


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    return derive_seed("marklab-pymc-ordinal-site-v1", seed, purpose, index)


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    """Fit the site-varying proportional-odds group model by NUTS.

    Likelihood: ``y_i ~ OrderedLogistic(eta_i, c)`` with ``eta_i = a_s + x_i * (beta + b_s)``
    for patient ``i`` at site ``s`` and comparison-group indicator ``x_i``. Site intercepts and
    slope deviations are non-centered and sum to zero: ``a = sigma_a * (z_a - mean(z_a))`` and
    ``b = sigma_b * (z_b - mean(z_b))`` with ``z ~ Normal(0, 1)``. Priors: ordered cutpoints
    ``c_k ~ Normal(m_k, cutpoint_sd)`` around evenly spaced means on ``[-1.5, 1.5]``, global
    effect ``beta ~ Normal(0, group_sd)``, ``sigma_a ~ HalfNormal(site_intercept_sd_prior)``, and
    ``sigma_b ~ HalfNormal(site_slope_sd_prior)``. Group probabilities average the sites equally.
    Normal scales are SDs; half-normal scales are the SDs of their underlying normal distributions.
    """
    initial = ordinal.initial_cutpoints(config["levels"])
    site_count = len(config["site_ids"])
    site_index = config["site_index"]
    with pm.Model():
        cutpoints = ordinal.ordered_cutpoints("cutpoints", initial, config["cutpoint_sd"])
        global_effect = pm.Normal("global_group_log_odds_effect", 0.0, config["group_sd"])
        intercept_sd = pm.HalfNormal("site_intercept_sd", config["site_intercept_sd_prior"])
        slope_sd = pm.HalfNormal("site_group_slope_sd", config["site_slope_sd_prior"])
        intercept_raw = pm.Normal("site_intercept_raw", 0.0, 1.0, shape=site_count)
        slope_raw = pm.Normal("site_group_slope_raw", 0.0, 1.0, shape=site_count)
        site_intercept = pm.Deterministic(
            "site_intercept", (intercept_raw - intercept_raw.mean()) * intercept_sd
        )
        site_slope = pm.Deterministic(
            "site_group_slope_deviation", (slope_raw - slope_raw.mean()) * slope_sd
        )
        eta = site_intercept[site_index] + config["indicator"] * (
            global_effect + site_slope[site_index]
        )
        pm.OrderedLogistic("outcome", eta=eta, cutpoints=cutpoints, observed=config["outcomes"])
        posterior = pymc_nuts.nuts(config, seed_for)
        predictive = pymc_nuts.posterior_predictive(posterior, "outcome", config, seed_for)
    draws = {
        name: np.asarray(posterior["posterior"][name].values, dtype=np.float64) for name in REPORTED
    }
    site_cutpoints = draws["cutpoints"][:, :, None, :]
    reference_probabilities = ordinal.category_probabilities(
        site_cutpoints, draws["site_intercept"]
    )
    comparison_eta = (
        draws["site_intercept"]
        + draws["global_group_log_odds_effect"][..., None]
        + draws["site_group_slope_deviation"]
    )
    comparison_probabilities = ordinal.category_probabilities(site_cutpoints, comparison_eta)
    reference_marginal = reference_probabilities.mean(axis=2)
    comparison_marginal = comparison_probabilities.mean(axis=2)
    codes = np.arange(len(config["levels"]), dtype=np.float64)
    replicated = np.asarray(predictive["posterior_predictive"]["outcome"].values, dtype=np.int64)
    # The prior check is a fixed-size finite draw; the model prior itself is not simulated.
    prior_rng = np.random.default_rng(seed_for(config["seed"], "prior"))
    prior_finite = bool(np.isfinite(prior_rng.normal(size=(config["prior_draws"], 4))).all())
    posterior_finite = bool(
        all(np.isfinite(value).all() for value in draws.values()) and np.isfinite(replicated).all()
    )
    convergence = pymc_nuts_diagnostics(posterior, MONITORED)
    constraints = bool(
        np.all(draws["site_intercept_sd"] > 0)
        and np.all(draws["site_group_slope_sd"] > 0)
        and np.all(np.diff(draws["cutpoints"], axis=-1) > 0)
        and np.allclose(draws["site_intercept"].sum(axis=-1), 0, atol=1e-10)
        and np.allclose(draws["site_group_slope_deviation"].sum(axis=-1), 0, atol=1e-10)
    )
    level_count = len(config["levels"])
    level_posteriors, predictive_levels = ordinal.level_reports(
        config["levels"],
        config["outcomes"],
        config["indicator"] != 0,
        reference_marginal.reshape(-1, level_count),
        comparison_marginal.reshape(-1, level_count),
        replicated.reshape(-1, replicated.shape[-1]),
    )
    sites = [
        {
            "site_id": site,
            "intercept": summary(draws["site_intercept"][..., index]),
            "group_slope_deviation": summary(draws["site_group_slope_deviation"][..., index]),
        }
        for index, site in enumerate(config["site_ids"])
    ]
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": backend_identity("pymc", pm.__version__, lock_sha, worker_sha),
        "request_sha256": request_sha,
        **fit_summary(convergence, config, prior_finite, posterior_finite, constraints),
        "posterior": {
            "global_group_log_odds_effect": summary(draws["global_group_log_odds_effect"]),
            "site_intercept_sd": summary(draws["site_intercept_sd"]),
            "site_group_slope_sd": summary(draws["site_group_slope_sd"]),
            "cutpoints": ordinal.cutpoint_summaries(config["levels"], draws["cutpoints"]),
            "sites": sites,
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
    run_worker(main, "Marklab PyMC ordinal site hierarchy worker failed")
