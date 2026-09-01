#!/usr/bin/env python3
"""Pinned PyMC worker for one identified nested/crossed Gaussian hierarchy."""

from __future__ import annotations

from collections import defaultdict
import hashlib
import json
import math
from pathlib import Path
import sys
from typing import Any

import numpy as np
import pymc as pm


PYMC_VERSION = "6.3.0"


class ContractError(ValueError):
    pass


def obj(value: Any, keys: set[str], path: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        actual = set(value) if isinstance(value, dict) else set()
        raise ContractError(
            f"{path} fields differ: missing={sorted(keys - actual)}, unknown={sorted(actual - keys)}"
        )
    return value


def exact(value: Any, expected: Any, path: str) -> None:
    if value != expected:
        raise ContractError(f"{path} must equal {expected!r}")


def integer(value: Any, path: str, low: int, high: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not low <= value <= high:
        raise ContractError(f"{path} must be an integer in [{low},{high}]")
    return value


def number(value: Any, path: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ContractError(f"{path} must be numeric")
    result = float(value)
    if not math.isfinite(result):
        raise ContractError(f"{path} must be finite")
    return result


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
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "model",
            "observations",
            "sampling",
            "resources",
            "diagnostic_policy",
        },
        "request",
    )
    exact(request["format"], "marklab.pymc_worker_request", "request.format")
    exact(request["version"], 1, "request.version")
    backend = obj(
        request["backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "backend",
    )
    exact(backend["name"], "pymc", "backend.name")
    exact(backend["version"], PYMC_VERSION, "backend.version")
    exact(backend["python_version"], "3.12", "backend.python")
    exact(backend["environment_lock_sha256"], lock_sha, "backend.lock")
    exact(backend["worker_sha256"], worker_sha, "backend.worker")
    model = obj(
        request["model"],
        {
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
        },
        "model",
    )
    expected_model = {
        "format": "marklab.bayesian_model_ir",
        "version": 1,
        "family": "gaussian_crossed_nested_random_slope_hierarchy",
        "likelihood": "normal_unknown_residual_scale",
        "intercept_prior": "normal_zero",
        "exposure_slope_prior": "normal_zero",
        "variance_component_prior": "half_normal_shared_scale",
        "nesting": "roi_within_slide_within_patient",
        "crossed_effect": "batch_crossed_with_patient",
        "random_slopes": ["patient_exposure", "cohort_exposure"],
        "random_effect_parameterization": "hybrid_centered_replicated_levels_noncentered_cohort_sum_zero",
        "exposure_standardization": "global_center_and_population_sd",
        "observation_unit": "replicated_scalar_measurement",
        "statistical_unit": "patient",
        "null": "zero_fixed_exposure_slope_and_zero_variance_components",
        "backend_capability": "nuts",
        "maturity": "experimental",
    }
    for key, expected in expected_model.items():
        exact(model[key], expected, f"model.{key}")
    intercept_prior_sd = number(model["intercept_prior_sd"], "model.intercept_prior_sd")
    slope_prior_sd = number(model["exposure_slope_prior_sd"], "model.slope_prior_sd")
    component_prior_sd = number(
        model["variance_component_prior_sd"], "model.component_prior_sd"
    )
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
            if not isinstance(value, str) or not value or len(value) > 128 or value.strip() != value:
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
        if row["patient_id"] in patient_cohort and patient_cohort[row["patient_id"]] != row["cohort_id"]:
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

    sampling = obj(
        request["sampling"],
        {"chains", "tune_per_chain", "draws_per_chain", "target_accept", "seed"},
        "sampling",
    )
    chains = integer(sampling["chains"], "sampling.chains", 2, 8)
    tune = integer(sampling["tune_per_chain"], "sampling.tune", 100, 100000)
    draws = integer(sampling["draws_per_chain"], "sampling.draws", 100, 100000)
    target_accept = number(sampling["target_accept"], "sampling.target_accept")
    seed = integer(sampling["seed"], "sampling.seed", 0, 2**64 - 1)
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
    actual = [len(patient_ids), len(slide_ids), len(roi_ids), len(batch_ids), len(cohort_ids), len(raw_rows)]
    maxima = [
        integer(resources["maximum_patients"], "resources.patients", 12, 512),
        integer(resources["maximum_slides"], "resources.slides", 24, 1024),
        integer(resources["maximum_rois"], "resources.rois", 48, 2048),
        integer(resources["maximum_batches"], "resources.batches", 2, 64),
        integer(resources["maximum_cohorts"], "resources.cohorts", 3, 32),
        integer(resources["maximum_observations"], "resources.observations", 48, 4096),
    ]
    if any(value > maximum for value, maximum in zip(actual, maxima)):
        raise ContractError("crossed/nested resource limit exceeded")
    if chains * (tune + draws) > integer(
        resources["maximum_total_iterations"], "resources.iterations", 1, 400000
    ):
        raise ContractError("sampling work exceeds resource limit")
    maximum_output_bytes = integer(
        resources["maximum_output_bytes"], "resources.output", 1, 2 * 1048576
    )
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3600)

    policy = obj(
        request["diagnostic_policy"],
        {
            "prior_predictive_draws",
            "maximum_r_hat",
            "minimum_bulk_ess",
            "minimum_tail_ess",
            "minimum_ebfmi",
            "maximum_divergences",
            "maximum_tree_depth_hits",
            "maximum_tree_depth",
        },
        "policy",
    )
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
        "chains": chains,
        "tune": tune,
        "draws": draws,
        "target_accept": target_accept,
        "seed": seed,
        "prior_draws": integer(policy["prior_predictive_draws"], "policy.prior", 1, 100000),
        "maximum_r_hat": number(policy["maximum_r_hat"], "policy.r_hat"),
        "minimum_bulk_ess": number(policy["minimum_bulk_ess"], "policy.bulk"),
        "minimum_tail_ess": number(policy["minimum_tail_ess"], "policy.tail"),
        "minimum_ebfmi": number(policy["minimum_ebfmi"], "policy.ebfmi"),
        "maximum_divergences": integer(policy["maximum_divergences"], "policy.divergences", 0, 2**63 - 1),
        "maximum_tree_depth_hits": integer(policy["maximum_tree_depth_hits"], "policy.depth_hits", 0, 2**63 - 1),
        "maximum_tree_depth": integer(policy["maximum_tree_depth"], "policy.depth", 1, 32),
        "maximum_output_bytes": maximum_output_bytes,
    }


def seed_for(seed: int, purpose: str, position: int = 0) -> int:
    digest = hashlib.sha256(
        f"marklab-pymc-gaussian-crossed-nested-v1\0{seed}\0{purpose}\0{position}".encode()
    ).digest()
    return int.from_bytes(digest[:4], "little")


def summary(values: np.ndarray) -> dict[str, float]:
    flat = values.reshape(-1)
    return {
        "mean": float(flat.mean()),
        "sd": float(flat.std(ddof=1)),
        "interval_lower": float(np.quantile(flat, 0.025)),
        "interval_upper": float(np.quantile(flat, 0.975)),
    }


def tree_values(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    component_names = [
        "cohort_intercept_sd",
        "cohort_exposure_slope_sd",
        "patient_intercept_sd",
        "patient_exposure_slope_sd",
        "slide_intercept_sd",
        "roi_intercept_sd",
        "batch_intercept_sd",
        "residual_sd",
    ]
    with pm.Model():
        intercept = pm.Normal("intercept", 0.0, config["intercept_prior_sd"])
        slope = pm.Normal("exposure_slope", 0.0, config["slope_prior_sd"])
        components = {
            name: pm.HalfNormal(name, config["component_prior_sd"])
            for name in component_names
        }
        cohort_intercept = pm.Deterministic(
            "cohort_intercept",
            components["cohort_intercept_sd"]
            * pm.math.dot(
                config["cohort_center"],
                pm.Normal("cohort_intercept_z", 0.0, 1.0, shape=config["cohorts"]),
            ),
        )
        cohort_slope = pm.Deterministic(
            "cohort_exposure_slope",
            components["cohort_exposure_slope_sd"]
            * pm.math.dot(
                config["cohort_center"],
                pm.Normal("cohort_exposure_slope_z", 0.0, 1.0, shape=config["cohorts"]),
            ),
        )
        patient_intercept = pm.Deterministic(
            "patient_intercept",
            pm.math.dot(
                config["patient_center"],
                pm.Normal(
                    "patient_intercept_raw",
                    0.0,
                    components["patient_intercept_sd"],
                    shape=config["patients"],
                ),
            ),
        )
        patient_slope = pm.Deterministic(
            "patient_exposure_slope",
            pm.math.dot(
                config["patient_center"],
                pm.Normal(
                    "patient_exposure_slope_raw",
                    0.0,
                    components["patient_exposure_slope_sd"],
                    shape=config["patients"],
                ),
            ),
        )
        slide_intercept = pm.Deterministic(
            "slide_intercept",
            pm.math.dot(
                config["slide_center"],
                pm.Normal(
                    "slide_intercept_raw",
                    0.0,
                    components["slide_intercept_sd"],
                    shape=config["slides"],
                ),
            ),
        )
        roi_intercept = pm.Deterministic(
            "roi_intercept",
            pm.math.dot(
                config["roi_center"],
                pm.Normal(
                    "roi_intercept_raw",
                    0.0,
                    components["roi_intercept_sd"],
                    shape=config["rois"],
                ),
            ),
        )
        batch_intercept = pm.Deterministic(
            "batch_intercept",
            pm.math.dot(
                config["batch_center"],
                pm.Normal(
                    "batch_intercept_raw",
                    0.0,
                    components["batch_intercept_sd"],
                    shape=config["batches"],
                ),
            ),
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
        pm.Normal("outcome", mu=mu, sigma=components["residual_sd"], observed=config["outcome"])
        prior = pm.sample_prior_predictive(
            draws=config["prior_draws"], random_seed=seed_for(config["seed"], "prior")
        )
        posterior = pm.sample(
            draws=config["draws"],
            tune=config["tune"],
            chains=config["chains"],
            cores=1,
            blas_cores=1,
            random_seed=[seed_for(config["seed"], "chain", chain) for chain in range(config["chains"])],
            target_accept=config["target_accept"],
            nuts_sampler="pymc",
            nuts={"max_treedepth": config["maximum_tree_depth"]},
            progressbar=False,
            quiet=True,
            compute_convergence_checks=False,
        )
        predictive = pm.sample_posterior_predictive(
            posterior,
            var_names=["outcome"],
            random_seed=seed_for(config["seed"], "predictive"),
            progressbar=False,
        )
    draws = posterior["posterior"]
    fixed_names = ["intercept", "exposure_slope"]
    monitored = fixed_names + component_names
    arrays = {name: np.asarray(draws[name].values, dtype=np.float64) for name in monitored}
    replicated = np.asarray(predictive["posterior_predictive"]["outcome"].values, dtype=np.float64)
    prior_finite = bool(
        all(
            np.isfinite(np.asarray(value.values)).all()
            for group_name in ["prior", "prior_predictive"]
            for value in prior[group_name].values()
        )
    )
    posterior_finite = bool(
        all(np.isfinite(value).all() for value in [*arrays.values(), replicated])
    )
    r_hat = float(tree_values(pm.stats.rhat(posterior, var_names=monitored, method="rank"), monitored).max())
    bulk = float(tree_values(pm.stats.ess(posterior, var_names=monitored, method="bulk"), monitored).min())
    tail = float(tree_values(pm.stats.ess(posterior, var_names=monitored, method="tail"), monitored).min())
    mcse_mean = float(tree_values(pm.stats.mcse(posterior, var_names=monitored, method="mean"), monitored).max())
    mcse_sd = float(tree_values(pm.stats.mcse(posterior, var_names=monitored, method="sd"), monitored).max())
    energy = np.asarray(posterior["sample_stats"]["energy"].values, dtype=np.float64)
    ebfmi = float(np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1)))
    divergences = int(np.asarray(posterior["sample_stats"]["diverging"].values).sum())
    depth_hits = int(np.asarray(posterior["sample_stats"]["reached_max_treedepth"].values).sum())
    constraints = bool(all(np.all(arrays[name] > 0.0) for name in component_names))
    complete = bool(
        prior_finite
        and posterior_finite
        and constraints
        and r_hat <= config["maximum_r_hat"]
        and bulk >= config["minimum_bulk_ess"]
        and tail >= config["minimum_tail_ess"]
        and ebfmi >= config["minimum_ebfmi"]
        and divergences <= config["maximum_divergences"]
        and depth_hits <= config["maximum_tree_depth_hits"]
    )
    component_variances = np.stack([arrays[name].reshape(-1) ** 2 for name in component_names], axis=1)
    fractions = component_variances / component_variances.sum(axis=1, keepdims=True)
    fraction_means = fractions.mean(axis=0)
    fraction_means /= fraction_means.sum()
    observed_mean = float(config["outcome"].mean())
    observed_sd = float(config["outcome"].std(ddof=1))
    replicated_flat = replicated.reshape(-1, replicated.shape[-1])
    replicated_means = replicated_flat.mean(axis=1)
    replicated_sds = replicated_flat.std(axis=1, ddof=1)
    result = {
        "format": "marklab.pymc_gaussian_crossed_nested_hierarchy_worker_result",
        "version": 1,
        "backend": {
            "name": "pymc",
            "version": pm.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha,
            "worker_sha256": worker_sha,
        },
        "request_sha256": request_sha,
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"],
            "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"],
            "completed_draws": config["chains"] * config["draws"],
        },
        "posterior": {name: summary(arrays[name]) for name in monitored},
        "variance_partition": {
            "cohort_intercept": float(fraction_means[0]),
            "cohort_exposure_slope": float(fraction_means[1]),
            "patient_intercept": float(fraction_means[2]),
            "patient_exposure_slope": float(fraction_means[3]),
            "slide_intercept": float(fraction_means[4]),
            "roi_intercept": float(fraction_means[5]),
            "batch_intercept": float(fraction_means[6]),
            "residual": float(fraction_means[7]),
        },
        "diagnostics": {
            "prior_predictive_finite": prior_finite,
            "posterior_finite": posterior_finite,
            "r_hat": r_hat,
            "ess_bulk": bulk,
            "ess_tail": tail,
            "mcse_mean": mcse_mean,
            "mcse_sd": mcse_sd,
            "minimum_ebfmi": ebfmi,
            "divergences": divergences,
            "max_tree_depth_hits": depth_hits,
            "constraints_valid": constraints,
            "identifiability_checks_passed": True,
        },
        "posterior_predictive": {
            "observed_outcome_mean": observed_mean,
            "replicated_outcome_mean_mean": float(replicated_means.mean()),
            "probability_replicated_mean_at_least_observed": float(np.mean(replicated_means >= observed_mean)),
            "observed_outcome_sd": observed_sd,
            "replicated_outcome_sd_mean": float(replicated_sds.mean()),
            "probability_replicated_sd_at_least_observed": float(np.mean(replicated_sds >= observed_sd)),
        },
    }
    encoded = json.dumps(result, allow_nan=False, separators=(",", ":"), sort_keys=True).encode()
    if len(encoded) > config["maximum_output_bytes"]:
        raise ContractError("result exceeds output limit")
    return result


def main() -> None:
    if pm.__version__ != PYMC_VERSION:
        raise ContractError(f"PyMC version drift: expected {PYMC_VERSION}, found {pm.__version__}")
    if sys.version_info[:2] != (3, 12):
        raise ContractError(
            f"Python version drift: expected 3.12, found {sys.version_info.major}.{sys.version_info.minor}"
        )
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    raw = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if len(raw) > 16 * 1024 * 1024:
        raise ContractError("request exceeds 16 MiB")
    request_sha = hashlib.sha256(raw).hexdigest()
    config = validate(json.loads(raw), lock_sha, worker_sha)
    result = fit(config, request_sha, lock_sha, worker_sha)
    sys.stdout.write(json.dumps(result, allow_nan=False, separators=(",", ":"), sort_keys=True))
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(
            f"Marklab PyMC crossed/nested worker failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
