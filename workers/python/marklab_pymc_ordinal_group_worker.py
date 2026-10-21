#!/usr/bin/env python3
"""PyMC worker for patient-level proportional-odds group regression."""

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

RESULT_FORMAT = "marklab.pymc_ordinal_group_worker_result"
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
    "intercept",
    "likelihood",
    "proportional_odds_assumption",
    "observation_unit",
    "biological_unit",
    "backend_capability",
    "maturity",
}
MODEL_CONSTANTS = {
    "family": "patient_proportional_odds_group_regression",
    "cutpoint_prior": "ordered_normal",
    "group_effect_prior": "normal_log_odds_difference",
    "intercept": "none_cutpoints_own_baseline_location",
    "likelihood": "ordered_logistic_patient_outcome",
    "proportional_odds_assumption": True,
    "observation_unit": "one_complete_ordered_outcome_per_patient",
    "biological_unit": "patient",
    "backend_capability": "nuts",
    "maturity": "experimental",
}


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request, model = pymc_request.validate_envelope(
        request, lock_digest, worker_digest, ("patients",), MODEL_KEYS
    )
    for key, expected in MODEL_CONSTANTS.items():
        exact(model[key], expected, f"model.{key}")
    reference_group = model["reference_group"]
    comparison_group = model["comparison_group"]
    levels = model["ordered_levels"]
    if (
        not pymc_request.valid_name(reference_group)
        or not pymc_request.valid_name(comparison_group)
        or reference_group == comparison_group
        or not isinstance(levels, list)
        or not 2 <= len(levels) <= 8
        or not all(pymc_request.valid_name(level) for level in levels)
        or len(set(levels)) != len(levels)
    ):
        raise ContractError("model group or ordered-level identities are invalid")
    cutpoint_sd = number(model["cutpoint_prior_sd"], "model.cutpoint_prior_sd")
    group_sd = number(model["group_effect_prior_sd"], "model.group_effect_prior_sd")
    if cutpoint_sd <= 0.0 or group_sd <= 0.0:
        raise ContractError("model prior scales must be positive")
    patients = request["patients"]
    if not isinstance(patients, list) or not 8 <= len(patients) <= 512:
        raise ContractError("patient count is invalid")
    patient_ids: list[str] = []
    groups: list[str] = []
    outcomes: list[int] = []
    for index, raw in enumerate(patients):
        row = obj(raw, {"patient_id", "group", "outcome_code"}, f"patients[{index}]")
        patient_id = row["patient_id"]
        group = row["group"]
        if not pymc_request.valid_name(patient_id) or group not in {
            reference_group,
            comparison_group,
        }:
            raise ContractError("patient identity or group is invalid")
        patient_ids.append(patient_id)
        groups.append(group)
        outcomes.append(integer(row["outcome_code"], "patient.outcome_code", 0, len(levels) - 1))
    if patient_ids != sorted(patient_ids) or len(set(patient_ids)) != len(patient_ids):
        raise ContractError("patient identities must be unique and sorted")
    if min(groups.count(reference_group), groups.count(comparison_group)) < 4:
        raise ContractError("each group requires four patients")
    if set(outcomes) != set(range(len(levels))):
        raise ContractError("every ordered level must be observed")
    sampling = pymc_request.validate_sampling(request["sampling"], bounded_target_accept=True)
    resources = obj(
        request["resources"],
        {
            "maximum_patients",
            "maximum_levels",
            "maximum_total_iterations",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    if len(patients) > integer(resources["maximum_patients"], "resources.patients", 8, 512):
        raise ContractError("patient limit exceeded")
    if len(levels) > integer(resources["maximum_levels"], "resources.levels", 2, 8):
        raise ContractError("level limit exceeded")
    iterations = sampling["chains"] * (sampling["tune"] + sampling["draws"])
    if iterations > integer(
        resources["maximum_total_iterations"], "resources.iterations", 1, 400_000
    ):
        raise ContractError("iteration limit exceeded")
    output_limit = integer(resources["maximum_output_bytes"], "resources.output", 1, 2 * MIB)
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3_600)
    return {
        "levels": levels,
        "cutpoint_sd": cutpoint_sd,
        "group_sd": group_sd,
        "indicator": np.asarray([group == comparison_group for group in groups], dtype=np.float64),
        "outcomes": np.asarray(outcomes, dtype=np.int64),
        **sampling,
        "output_limit": output_limit,
        **pymc_request.validate_policy(request["diagnostic_policy"]),
    }


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    return derive_seed("marklab-pymc-ordinal-group-v1", seed, purpose, index)


def prior_predictive_is_finite(config: dict[str, Any], initial: np.ndarray) -> bool:
    """Simulate prior level probabilities with NumPy and check that they are valid."""
    rng = np.random.default_rng(seed_for(config["seed"], "prior"))
    raw = rng.normal(initial, config["cutpoint_sd"], size=(config["prior_draws"], len(initial)))
    cutpoints = np.sort(raw, axis=1)
    effects = rng.normal(0.0, config["group_sd"], size=config["prior_draws"])
    reference = ordinal.category_probabilities(cutpoints, np.zeros(config["prior_draws"]))
    comparison = ordinal.category_probabilities(cutpoints, effects)
    return bool(
        np.isfinite(reference).all()
        and np.isfinite(comparison).all()
        and np.all(reference >= 0.0)
        and np.all(comparison >= 0.0)
        and np.allclose(reference.sum(axis=1), 1.0)
        and np.allclose(comparison.sum(axis=1), 1.0)
    )


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    """Fit the proportional-odds group model by NUTS.

    Likelihood: ``y_i ~ OrderedLogistic(eta_i, c)`` with ``eta_i = x_i * beta`` for the
    comparison-group indicator ``x_i``. Priors: ordered cutpoints ``c_k ~ Normal(m_k,
    cutpoint_sd)`` around evenly spaced means ``m`` on ``[-1.5, 1.5]``, and group log-odds effect
    ``beta ~ Normal(0, group_sd)``; there is no separate intercept.
    Normal scale arguments are standard deviations.
    """
    initial = ordinal.initial_cutpoints(config["levels"])
    with pm.Model():
        cutpoints = ordinal.ordered_cutpoints("cutpoints", initial, config["cutpoint_sd"])
        effect = pm.Normal("group_log_odds_effect", 0.0, config["group_sd"])
        eta = config["indicator"] * effect
        pm.OrderedLogistic("outcome", eta=eta, cutpoints=cutpoints, observed=config["outcomes"])
        posterior = pymc_nuts.nuts(config, seed_for)
        predictive = pymc_nuts.posterior_predictive(posterior, "outcome", config, seed_for)
    cutpoint_draws = np.asarray(posterior["posterior"]["cutpoints"].values, dtype=np.float64)
    effect_draws = np.asarray(
        posterior["posterior"]["group_log_odds_effect"].values, dtype=np.float64
    )
    reference_probabilities = ordinal.category_probabilities(
        cutpoint_draws, np.zeros_like(effect_draws)
    )
    comparison_probabilities = ordinal.category_probabilities(cutpoint_draws, effect_draws)
    codes = np.arange(len(config["levels"]), dtype=np.float64)
    reference_expected = (reference_probabilities * codes).sum(axis=-1)
    comparison_expected = (comparison_probabilities * codes).sum(axis=-1)
    replicated = np.asarray(predictive["posterior_predictive"]["outcome"].values, dtype=np.int64)
    prior_finite = prior_predictive_is_finite(config, initial)
    posterior_finite = bool(
        np.isfinite(cutpoint_draws).all()
        and np.isfinite(effect_draws).all()
        and np.isfinite(reference_probabilities).all()
        and np.isfinite(comparison_probabilities).all()
        and np.isfinite(replicated).all()
    )
    convergence = pymc_nuts_diagnostics(posterior, ["cutpoints", "group_log_odds_effect"])
    constraints = bool(
        np.all(np.diff(cutpoint_draws, axis=-1) > 0.0)
        and np.all(reference_probabilities >= 0.0)
        and np.all(comparison_probabilities >= 0.0)
        and np.allclose(reference_probabilities.sum(axis=-1), 1.0)
        and np.allclose(comparison_probabilities.sum(axis=-1), 1.0)
    )
    level_count = len(config["levels"])
    level_posteriors, predictive_levels = ordinal.level_reports(
        config["levels"],
        config["outcomes"],
        config["indicator"] != 0.0,
        reference_probabilities.reshape(-1, level_count),
        comparison_probabilities.reshape(-1, level_count),
        replicated.reshape(-1, replicated.shape[-1]),
    )
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": backend_identity("pymc", pm.__version__, lock_sha, worker_sha),
        "request_sha256": request_sha,
        **fit_summary(convergence, config, prior_finite, posterior_finite, constraints),
        "posterior": {
            "group_log_odds_effect": summary(effect_draws),
            "cutpoints": ordinal.cutpoint_summaries(config["levels"], cutpoint_draws),
            "levels": level_posteriors,
            "reference_expected_code": summary(reference_expected),
            "comparison_expected_code": summary(comparison_expected),
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
    run_worker(main, "Marklab PyMC ordinal-group worker failed")
