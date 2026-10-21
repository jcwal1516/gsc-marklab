#!/usr/bin/env python3
"""PyMC worker for an identified Gaussian hierarchy with nested and crossed random effects."""

from __future__ import annotations

import json
import math
import sys
from collections import defaultdict
from pathlib import Path
from typing import Any

sys.path.append(str(Path(__file__).resolve().parent))

# Configure PyTensor before PyMC is imported.
from marklab_worker_support import pytensor_config  # noqa: F401

# isort: split
import numpy as np
import pymc as pm

from marklab_worker_support import pymc_nuts, pymc_request
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

RESULT_FORMAT = "marklab.pymc_gaussian_crossed_nested_hierarchy_worker_result"
MODEL_KEYS = {
    "format",
    "version",
    "family",
    "likelihood",
    "intercept_prior",
    "intercept_prior_sd",
    "exposure_slope_prior",
    "exposure_slope_prior_sd",
    "variance_component_prior",
    "variance_component_prior_sd",
    "nesting",
    "crossed_effect",
    "random_slopes",
    "random_effect_parameterization",
    "exposure_standardization",
    "observation_unit",
    "statistical_unit",
    "null",
    "backend_capability",
    "maturity",
}
MODEL_CONSTANTS = {
    "family": "gaussian_crossed_nested_random_slope_hierarchy",
    "likelihood": "normal_unknown_residual_scale",
    "intercept_prior": "normal_zero",
    "exposure_slope_prior": "normal_zero",
    "variance_component_prior": "half_normal_shared_scale",
    "nesting": "roi_within_slide_within_patient",
    "crossed_effect": "batch_crossed_with_patient",
    "random_slopes": ["patient_exposure", "cohort_exposure"],
    "random_effect_parameterization": (
        "hybrid_centered_replicated_levels_noncentered_cohort_sum_zero"
    ),
    "exposure_standardization": "global_center_and_population_sd",
    "observation_unit": "replicated_scalar_measurement",
    "statistical_unit": "patient",
    "null": "zero_fixed_exposure_slope_and_zero_variance_components",
    "backend_capability": "nuts",
    "maturity": "experimental",
}
COMPONENT_NAMES = [
    "cohort_intercept_sd",
    "cohort_exposure_slope_sd",
    "patient_intercept_sd",
    "patient_exposure_slope_sd",
    "slide_intercept_sd",
    "roi_intercept_sd",
    "batch_intercept_sd",
    "residual_sd",
]
MONITORED = ["intercept", "exposure_slope", *COMPONENT_NAMES]


def index(values: list[str]) -> tuple[list[str], np.ndarray]:
    levels = sorted(set(values))
    lookup = {value: position for position, value in enumerate(levels)}
    return levels, np.asarray([lookup[value] for value in values], dtype=np.int64)


def center_within(parents: np.ndarray) -> np.ndarray:
    dimension = len(parents)
    transform = np.eye(dimension, dtype=np.float64)
    for parent in sorted(set(int(value) for value in parents)):
        members = np.flatnonzero(parents == parent)
        transform[np.ix_(members, members)] -= 1.0 / len(members)
    return transform


def center_global(dimension: int) -> np.ndarray:
    return np.eye(dimension, dtype=np.float64) - np.full(
        (dimension, dimension), 1.0 / dimension, dtype=np.float64
    )


def validate(request: Any, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    request, model = pymc_request.validate_envelope(
        request, lock_sha, worker_sha, ("observations",), MODEL_KEYS
    )
    for key, expected in MODEL_CONSTANTS.items():
        exact(model[key], expected, f"model.{key}")
    intercept_prior_sd = number(model["intercept_prior_sd"], "model.intercept_prior_sd")
    slope_prior_sd = number(model["exposure_slope_prior_sd"], "model.slope_prior_sd")
    component_prior_sd = number(model["variance_component_prior_sd"], "model.component_prior_sd")
    if min(intercept_prior_sd, slope_prior_sd, component_prior_sd) <= 0.0:
        raise ContractError("prior scales must be positive")

    raw_rows = request["observations"]
    if not isinstance(raw_rows, list) or not 48 <= len(raw_rows) <= 4096:
        raise ContractError("observation count is invalid")
    columns: dict[str, list[Any]] = defaultdict(list)
    ordering: list[tuple[Any, ...]] = []
    slide_patient: dict[str, str] = {}
    roi_slide: dict[str, str] = {}
    patient_cohort: dict[str, str] = {}
    for position, raw in enumerate(raw_rows):
        row = obj(
            raw,
            {"patient_id", "slide_id", "roi_id", "batch_id", "cohort_id", "exposure", "outcome"},
            f"observations[{position}]",
        )
        for key in ["patient_id", "slide_id", "roi_id", "batch_id", "cohort_id"]:
            value = row[key]
            if (
                not isinstance(value, str)
                or not value
                or len(value) > 128
                or value.strip() != value
            ):
                raise ContractError(f"{key} is invalid")
            columns[key].append(value)
        exposure = number(row["exposure"], "observation.exposure")
        outcome = number(row["outcome"], "observation.outcome")
        columns["exposure"].append(exposure)
        columns["outcome"].append(outcome)
        ordering.append(
            (
                row["patient_id"],
                row["slide_id"],
                row["roi_id"],
                row["batch_id"],
                exposure,
                outcome,
            )
        )
        if row["slide_id"] in slide_patient and slide_patient[row["slide_id"]] != row["patient_id"]:
            raise ContractError("slide parent differs")
        if row["roi_id"] in roi_slide and roi_slide[row["roi_id"]] != row["slide_id"]:
            raise ContractError("ROI parent differs")
        if (
            row["patient_id"] in patient_cohort
            and patient_cohort[row["patient_id"]] != row["cohort_id"]
        ):
            raise ContractError("patient cohort differs")
        slide_patient[row["slide_id"]] = row["patient_id"]
        roi_slide[row["roi_id"]] = row["slide_id"]
        patient_cohort[row["patient_id"]] = row["cohort_id"]
    if ordering != sorted(ordering):
        raise ContractError("observations must be canonically sorted")

    patient_ids, patient_index = index(columns["patient_id"])
    slide_ids, slide_index = index(columns["slide_id"])
    roi_ids, roi_index = index(columns["roi_id"])
    batch_ids, batch_index = index(columns["batch_id"])
    cohort_ids, cohort_index = index(columns["cohort_id"])
    if not 12 <= len(patient_ids) <= 512 or not 3 <= len(cohort_ids) <= 32:
        raise ContractError("patient or cohort count is invalid")
    if not 24 <= len(slide_ids) <= 1024 or not 48 <= len(roi_ids) <= 2048:
        raise ContractError("slide or ROI count is invalid")
    if not 2 <= len(batch_ids) <= 64:
        raise ContractError("batch count is invalid")

    patient_by_id = {value: position for position, value in enumerate(patient_ids)}
    slide_by_id = {value: position for position, value in enumerate(slide_ids)}
    patient_parent_cohort = np.asarray(
        [cohort_ids.index(patient_cohort[value]) for value in patient_ids], dtype=np.int64
    )
    slide_parent_patient = np.asarray(
        [patient_by_id[slide_patient[value]] for value in slide_ids], dtype=np.int64
    )
    roi_parent_slide = np.asarray(
        [slide_by_id[roi_slide[value]] for value in roi_ids], dtype=np.int64
    )
    patient_slides = [set() for _ in patient_ids]
    slide_rois = [set() for _ in slide_ids]
    roi_replicates = np.zeros(len(roi_ids), dtype=np.int64)
    patient_batches = [set() for _ in patient_ids]
    batch_patients = [set() for _ in batch_ids]
    patient_exposure = [set() for _ in patient_ids]
    cohort_exposure = [set() for _ in cohort_ids]
    cohort_patients = [set() for _ in cohort_ids]
    for row in range(len(raw_rows)):
        p = int(patient_index[row])
        s = int(slide_index[row])
        r = int(roi_index[row])
        b = int(batch_index[row])
        c = int(cohort_index[row])
        patient_slides[p].add(s)
        slide_rois[s].add(r)
        roi_replicates[r] += 1
        patient_batches[p].add(b)
        batch_patients[b].add(p)
        patient_exposure[p].add(columns["exposure"][row])
        cohort_exposure[c].add(columns["exposure"][row])
        cohort_patients[c].add(p)
    if (
        any(len(values) < 2 for values in patient_slides)
        or any(len(values) < 2 for values in slide_rois)
        or np.any(roi_replicates < 2)
        or any(len(values) < 2 for values in patient_batches)
        or any(len(values) < 2 for values in batch_patients)
        or any(len(values) < 2 for values in patient_exposure)
        or any(len(values) < 2 for values in cohort_exposure)
        or any(len(values) < 4 for values in cohort_patients)
    ):
        raise ContractError("declared crossed/nested design is not identified")

    exposure = np.asarray(columns["exposure"], dtype=np.float64)
    exposure_mean = float(exposure.mean())
    exposure_sd = float(exposure.std(ddof=0))
    if not math.isfinite(exposure_sd) or exposure_sd <= np.finfo(np.float64).eps:
        raise ContractError("exposure variance is invalid")
    outcome = np.asarray(columns["outcome"], dtype=np.float64)

    sampling = pymc_request.validate_sampling(request["sampling"])
    resources = obj(
        request["resources"],
        {
            "maximum_patients",
            "maximum_slides",
            "maximum_rois",
            "maximum_batches",
            "maximum_cohorts",
            "maximum_observations",
            "maximum_total_iterations",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    actual = [
        len(patient_ids),
        len(slide_ids),
        len(roi_ids),
        len(batch_ids),
        len(cohort_ids),
        len(raw_rows),
    ]
    maxima = [
        integer(resources["maximum_patients"], "resources.patients", 12, 512),
        integer(resources["maximum_slides"], "resources.slides", 24, 1024),
        integer(resources["maximum_rois"], "resources.rois", 48, 2048),
        integer(resources["maximum_batches"], "resources.batches", 2, 64),
        integer(resources["maximum_cohorts"], "resources.cohorts", 3, 32),
        integer(resources["maximum_observations"], "resources.observations", 48, 4096),
    ]
    if any(value > maximum for value, maximum in zip(actual, maxima, strict=True)):
        raise ContractError("crossed/nested resource limit exceeded")
    iterations = sampling["chains"] * (sampling["tune"] + sampling["draws"])
    if iterations > integer(
        resources["maximum_total_iterations"], "resources.iterations", 1, 400000
    ):
        raise ContractError("sampling work exceeds resource limit")
    maximum_output_bytes = integer(
        resources["maximum_output_bytes"], "resources.output", 1, 2 * 1048576
    )
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3600)

    return {
        "intercept_prior_sd": intercept_prior_sd,
        "slope_prior_sd": slope_prior_sd,
        "component_prior_sd": component_prior_sd,
        "exposure": (exposure - exposure_mean) / exposure_sd,
        "outcome": outcome,
        "patient_index": patient_index,
        "slide_index": slide_index,
        "roi_index": roi_index,
        "batch_index": batch_index,
        "cohort_index": cohort_index,
        "patient_center": center_within(patient_parent_cohort),
        "slide_center": center_within(slide_parent_patient),
        "roi_center": center_within(roi_parent_slide),
        "cohort_center": center_global(len(cohort_ids)),
        "batch_center": center_global(len(batch_ids)),
        "patients": len(patient_ids),
        "slides": len(slide_ids),
        "rois": len(roi_ids),
        "batches": len(batch_ids),
        "cohorts": len(cohort_ids),
        **sampling,
        **pymc_request.validate_policy(request["diagnostic_policy"]),
        "maximum_output_bytes": maximum_output_bytes,
    }


def seed_for(seed: int, purpose: str, position: int = 0) -> int:
    return derive_seed("marklab-pymc-gaussian-crossed-nested-v1", seed, purpose, position)


def noncentered_effect(name: str, sd: Any, center: np.ndarray, size: int) -> Any:
    """``sd * C z`` with ``z ~ Normal(0, 1)`` named ``<name>_z`` and centring matrix ``C``."""
    raw = pm.Normal(f"{name}_z", 0.0, 1.0, shape=size)
    return pm.Deterministic(name, sd * pm.math.dot(center, raw))


def centered_effect(name: str, sd: Any, center: np.ndarray, size: int) -> Any:
    """``C u`` with ``u ~ Normal(0, sd)`` named ``<name>_raw`` and centring matrix ``C``."""
    raw = pm.Normal(f"{name}_raw", 0.0, sd, shape=size)
    return pm.Deterministic(name, pm.math.dot(center, raw))


def variance_partition(config: dict[str, Any], arrays: dict[str, np.ndarray]) -> dict[str, float]:
    """Posterior mean shares of design-averaged random-effect and residual variance.

    Conditional on a component SD, centered effects have covariance ``sd**2 * C C.T``.
    Average their marginal observation variances over the supplied rows, including ``x**2``
    for random slopes, then normalize within each posterior draw. Fixed effects are excluded;
    this is neither an R-squared nor the sample variance of fitted random-effect realizations.
    """
    weights = {"residual_sd": 1.0}
    for level in ("cohort", "patient", "slide", "roi", "batch"):
        center = config[f"{level}_center"]
        marginal = np.einsum("ij,ij->i", center, center)[config[f"{level}_index"]]
        weights[f"{level}_intercept_sd"] = float(marginal.mean())
        if level in ("cohort", "patient"):
            weights[f"{level}_exposure_slope_sd"] = float(
                np.mean(marginal * config["exposure"] ** 2)
            )
    component_variances = np.stack(
        [arrays[name].reshape(-1) ** 2 * weights[name] for name in COMPONENT_NAMES], axis=1
    )
    fractions = component_variances / component_variances.sum(axis=1, keepdims=True)
    fraction_means = fractions.mean(axis=0)
    fraction_means /= fraction_means.sum()
    return {
        name.removesuffix("_sd"): float(fraction)
        for name, fraction in zip(COMPONENT_NAMES, fraction_means, strict=True)
    }


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    """Fit the crossed/nested Gaussian hierarchy by NUTS.

    Likelihood: ``y ~ Normal(mu, sigma_residual)`` with ``mu = alpha + beta * x + c_k + g_k * x
    + p_i + q_i * x + s_j + r_m + b_l`` for the standardized exposure ``x``, cohort ``k``,
    patient ``i``, slide ``j``, ROI ``m``, and batch ``l``. Exposure is centered over observations
    and divided by its population SD. Cohort intercepts and slopes are
    non-centered and sum to zero (``sd * C z``); patient, slide, and ROI effects are centered
    within their parent level and batch effects sum to zero (``C u`` with ``u ~ Normal(0, sd)``).
    Priors: ``alpha ~ Normal(0, intercept_prior_sd)``, ``beta ~ Normal(0, slope_prior_sd)``, and
    every component SD, including ``sigma_residual``, ``~ HalfNormal(component_prior_sd)``.
    Normal scales are SDs; half-normal scales are the SDs of their underlying normal distributions.
    """
    with pm.Model():
        intercept = pm.Normal("intercept", 0.0, config["intercept_prior_sd"])
        slope = pm.Normal("exposure_slope", 0.0, config["slope_prior_sd"])
        sd = {name: pm.HalfNormal(name, config["component_prior_sd"]) for name in COMPONENT_NAMES}
        cohorts = config["cohorts"]
        cohort_intercept = noncentered_effect(
            "cohort_intercept", sd["cohort_intercept_sd"], config["cohort_center"], cohorts
        )
        cohort_slope = noncentered_effect(
            "cohort_exposure_slope",
            sd["cohort_exposure_slope_sd"],
            config["cohort_center"],
            cohorts,
        )
        patient_center, patients = config["patient_center"], config["patients"]
        patient_intercept = centered_effect(
            "patient_intercept", sd["patient_intercept_sd"], patient_center, patients
        )
        patient_slope = centered_effect(
            "patient_exposure_slope", sd["patient_exposure_slope_sd"], patient_center, patients
        )
        slide_intercept = centered_effect(
            "slide_intercept", sd["slide_intercept_sd"], config["slide_center"], config["slides"]
        )
        roi_intercept = centered_effect(
            "roi_intercept", sd["roi_intercept_sd"], config["roi_center"], config["rois"]
        )
        batch_intercept = centered_effect(
            "batch_intercept", sd["batch_intercept_sd"], config["batch_center"], config["batches"]
        )
        x = config["exposure"]
        mu = (
            intercept
            + slope * x
            + cohort_intercept[config["cohort_index"]]
            + cohort_slope[config["cohort_index"]] * x
            + patient_intercept[config["patient_index"]]
            + patient_slope[config["patient_index"]] * x
            + slide_intercept[config["slide_index"]]
            + roi_intercept[config["roi_index"]]
            + batch_intercept[config["batch_index"]]
        )
        pm.Normal("outcome", mu=mu, sigma=sd["residual_sd"], observed=config["outcome"])
        prior, posterior, predictive = pymc_nuts.sample_prior_posterior_predictive(
            config, seed_for, "outcome"
        )
    arrays = {
        name: np.asarray(posterior["posterior"][name].values, dtype=np.float64)
        for name in MONITORED
    }
    replicated = np.asarray(predictive["posterior_predictive"]["outcome"].values, dtype=np.float64)
    prior_finite = pymc_nuts.prior_is_finite(prior, ("prior", "prior_predictive"))
    posterior_finite = bool(
        all(np.isfinite(value).all() for value in [*arrays.values(), replicated])
    )
    convergence = pymc_nuts_diagnostics(posterior, MONITORED)
    constraints = bool(all(np.all(arrays[name] > 0.0) for name in COMPONENT_NAMES))
    observed_mean = float(config["outcome"].mean())
    observed_sd = float(config["outcome"].std(ddof=1))
    replicated_flat = replicated.reshape(-1, replicated.shape[-1])
    replicated_means = replicated_flat.mean(axis=1)
    replicated_sds = replicated_flat.std(axis=1, ddof=1)
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": backend_identity("pymc", pm.__version__, lock_sha, worker_sha),
        "request_sha256": request_sha,
        **fit_summary(convergence, config, prior_finite, posterior_finite, constraints),
        "posterior": {name: summary(arrays[name]) for name in MONITORED},
        "variance_partition": variance_partition(config, arrays),
        "posterior_predictive": {
            "observed_outcome_mean": observed_mean,
            "replicated_outcome_mean_mean": float(replicated_means.mean()),
            "probability_replicated_mean_at_least_observed": float(
                np.mean(replicated_means >= observed_mean)
            ),
            "observed_outcome_sd": observed_sd,
            "replicated_outcome_sd_mean": float(replicated_sds.mean()),
            "probability_replicated_sd_at_least_observed": float(
                np.mean(replicated_sds >= observed_sd)
            ),
        },
    }


def main() -> None:
    require_versions(pymc=pm.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(16 * MIB)
    config = validate(json.loads(raw), lock_sha, worker_sha)
    result = fit(config, sha256_hex(raw), lock_sha, worker_sha)
    write_result(result, max_bytes=config["maximum_output_bytes"])


if __name__ == "__main__":
    run_worker(main, "Marklab PyMC crossed/nested worker failed")
