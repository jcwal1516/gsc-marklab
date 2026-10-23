"""Request contract and summaries for the robust Student-t patient hierarchy.

The PyMC worker's request is the source contract; the NumPyro agreement and SBC workers
validate their embedded source request against it.
"""

from __future__ import annotations

from typing import Any

import numpy as np

from . import versions
from .nuts_contract import UNBOUNDED_COUNT, nuts_policy, nuts_sampling
from .posterior import summary
from .protocol import ContractError, exact, integer, number, obj, validate_backend

SOURCE_WORKER = "marklab_pymc_student_t_hierarchy_worker.py"

MODEL_KEYS = {
    "format",
    "version",
    "family",
    "global_mean_prior",
    "between_patient_sd_prior",
    "patient_effect_parameterization",
    "likelihood",
    "observation_unit",
    "biological_unit",
    "hierarchy",
    "spatial_component",
    "posterior_predictive_statistics",
    "backend_capability",
    "maturity",
}
LIKELIHOOD_KEYS = {
    "family",
    "observation_sd_prior",
    "degrees_of_freedom",
    "degrees_of_freedom_excess_prior",
    "degrees_of_freedom_excess_rate",
    "finite_variance_floor",
    "link",
}


def student_t_scale_from_sd(observation_sd: Any, degrees_of_freedom: Any) -> Any:
    """Student-t scale whose finite variance ``scale**2 * nu / (nu - 2)`` equals ``sd**2``."""
    return observation_sd * ((degrees_of_freedom - 2.0) / degrees_of_freedom) ** 0.5


def normal_prior(value: Any, path: str) -> tuple[float, float]:
    prior = obj(value, {"family", "mean", "sd", "rationale"}, path)
    exact(prior["family"], "normal", f"{path}.family")
    exact(prior["rationale"], "user_supplied", f"{path}.rationale")
    mean = number(prior["mean"], f"{path}.mean")
    sd = number(prior["sd"], f"{path}.sd")
    if sd <= 0.0:
        raise ContractError(f"{path}.sd must be positive")
    return mean, sd


def half_normal_prior(value: Any, path: str) -> float:
    prior = obj(value, {"family", "sd", "support", "rationale"}, path)
    exact(prior["family"], "half_normal", f"{path}.family")
    exact(prior["support"], "positive", f"{path}.support")
    exact(prior["rationale"], "user_supplied", f"{path}.rationale")
    sd = number(prior["sd"], f"{path}.sd")
    if sd <= 0.0:
        raise ContractError(f"{path}.sd must be positive")
    return sd


def model_priors(value: Any, path: str) -> dict[str, float]:
    """Validate the model block; return the prior scales and degrees-of-freedom rate."""
    model = obj(value, MODEL_KEYS, path)
    exact(model["format"], "marklab.bayesian_model_ir", f"{path}.format")
    exact(model["version"], 1, f"{path}.version")
    exact(model["family"], "student_t_patient_varying_intercept", f"{path}.family")
    global_mean, global_sd = normal_prior(model["global_mean_prior"], f"{path}.global_mean_prior")
    between_sd = half_normal_prior(
        model["between_patient_sd_prior"], f"{path}.between_patient_sd_prior"
    )
    exact(
        model["patient_effect_parameterization"],
        "noncentered",
        f"{path}.patient_effect_parameterization",
    )
    likelihood = obj(model["likelihood"], LIKELIHOOD_KEYS, f"{path}.likelihood")
    exact(
        likelihood["family"],
        "student_t_inferred_scale_and_degrees_of_freedom",
        f"{path}.likelihood.family",
    )
    observation_sd = half_normal_prior(
        likelihood["observation_sd_prior"], f"{path}.likelihood.observation_sd_prior"
    )
    exact(
        likelihood["degrees_of_freedom"],
        "two_plus_positive_excess",
        f"{path}.likelihood.degrees_of_freedom",
    )
    exact(
        likelihood["degrees_of_freedom_excess_prior"],
        "exponential_rate",
        f"{path}.likelihood.degrees_of_freedom_excess_prior",
    )
    df_rate = number(
        likelihood["degrees_of_freedom_excess_rate"],
        f"{path}.likelihood.degrees_of_freedom_excess_rate",
    )
    floor = number(likelihood["finite_variance_floor"], f"{path}.likelihood.finite_variance_floor")
    exact(floor, 2.0, f"{path}.likelihood.finite_variance_floor")
    if df_rate <= 0.0:
        raise ContractError("degrees-of-freedom excess rate must be positive")
    exact(
        model["observation_unit"],
        "patient_nested_scalar_observation",
        f"{path}.observation_unit",
    )
    exact(model["biological_unit"], "patient", f"{path}.biological_unit")
    exact(model["hierarchy"], ["patient"], f"{path}.hierarchy")
    exact(model["spatial_component"], "none", f"{path}.spatial_component")
    exact(
        model["posterior_predictive_statistics"],
        ["global_mean", "patient_mean_sd", "maximum_absolute_residual"],
        f"{path}.posterior_predictive_statistics",
    )
    exact(model["backend_capability"], "nuts", f"{path}.backend_capability")
    exact(model["maturity"], "experimental", f"{path}.maturity")
    return {
        "global_mean": global_mean,
        "global_sd": global_sd,
        "between_sd": between_sd,
        "observation_sd": observation_sd,
        "df_rate": df_rate,
    }


def validate_request(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    """Validate a PyMC Student-t hierarchy request and return the fit configuration."""
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "model",
            "patients",
            "sampling",
            "resources",
            "diagnostic_policy",
        },
        "request",
    )
    exact(request["format"], "marklab.pymc_worker_request", "request.format")
    exact(request["version"], 1, "request.version")
    validate_backend(
        request["backend"],
        "backend",
        name="pymc",
        version=versions.PYMC,
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )
    config = model_priors(request["model"], "model")
    patients = request["patients"]
    if not isinstance(patients, list) or len(patients) < 3:
        raise ContractError("at least three patients are required")
    patient_ids: list[str] = []
    patient_values: list[np.ndarray] = []
    for index, raw in enumerate(patients):
        row = obj(raw, {"patient_id", "observations"}, f"patients[{index}]")
        patient_id = row["patient_id"]
        if (
            not isinstance(patient_id, str)
            or not patient_id
            or patient_id.strip() != patient_id
            or (patient_ids and patient_id <= patient_ids[-1])
        ):
            raise ContractError("patient identities must be nonempty and strictly increasing")
        values = row["observations"]
        if not isinstance(values, list) or len(values) < 2:
            raise ContractError("each patient requires at least two observations")
        patient_ids.append(patient_id)
        observations = [number(value, "patient.observation") for value in values]
        patient_values.append(np.asarray(observations, dtype=np.float64))
    sampling = nuts_sampling(request["sampling"], "sampling")
    # Only the observation and iteration ceilings of this resource block are enforced.
    resources = obj(
        request["resources"],
        {
            "maximum_observations",
            "maximum_total_iterations",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    observation_count = sum(map(len, patient_values))
    iterations = sampling["chains"] * (sampling["tune"] + sampling["draws"])
    maximum_observations = integer(
        resources["maximum_observations"], "resources.maximum_observations", 1, 100_000
    )
    if observation_count > maximum_observations or iterations > integer(
        resources["maximum_total_iterations"], "resources.maximum_total_iterations", 1, 800_000
    ):
        raise ContractError("Student-t resources are exceeded")
    policy = nuts_policy(
        request["diagnostic_policy"],
        "policy",
        prior_draw_range=(1, 100_000),
        count_ceiling=UNBOUNDED_COUNT,
        check_thresholds=False,
    )
    return {
        **config,
        "patient_ids": patient_ids,
        "patient_values": patient_values,
        **sampling,
        **policy,
    }


def hierarchy_summaries(
    config: dict[str, Any],
    observations: np.ndarray,
    observation_patient: np.ndarray,
    global_draws: np.ndarray,
    flat_patient_draws: np.ndarray,
    replicated: np.ndarray,
) -> dict[str, Any]:
    """Partial-pooling and posterior-predictive summaries of a Student-t hierarchy fit.

    ``flat_patient_draws`` is ``(draw, patient)`` with draws in chain-major order and
    ``replicated`` is ``(draw, observation)``. Shrinkage is the fraction of the distance from
    the raw patient mean to the population posterior mean covered by the patient's posterior
    mean. The residual statistic is the largest absolute deviation of an observation from its
    patient mean.
    """
    patient_count = len(config["patient_ids"])
    global_posterior_mean = float(global_draws.mean())
    partial_pooling = []
    for index, (patient_id, values) in enumerate(
        zip(config["patient_ids"], config["patient_values"], strict=True)
    ):
        raw_mean = float(values.mean())
        denominator = global_posterior_mean - raw_mean
        if denominator == 0.0:
            shrinkage = 0.0
        else:
            shrinkage = float((flat_patient_draws[:, index].mean() - raw_mean) / denominator)
        item = summary(flat_patient_draws[:, index], chains=config["chains"])
        partial_pooling.append(
            {
                "patient_id": patient_id,
                "observation_count": len(values),
                "raw_mean": raw_mean,
                "posterior_mean": item["mean"],
                "posterior_sd": item["sd"],
                "interval_lower": item["interval_lower"],
                "interval_upper": item["interval_upper"],
                "mcse_mean": item["mcse_mean"],
                "shrinkage": shrinkage,
                "warning": "shrinkage_is_model_dependent_not_a_quality_score",
            }
        )
    observed_patient_means = np.asarray([values.mean() for values in config["patient_values"]])
    replicated_patient_means = np.stack(
        [
            replicated[:, observation_patient == index].mean(axis=1)
            for index in range(patient_count)
        ],
        axis=1,
    )
    observed_residual = max(
        float(np.max(np.abs(values - values.mean()))) for values in config["patient_values"]
    )
    replicated_residual_by_patient = np.stack(
        [
            np.max(
                np.abs(
                    replicated[:, observation_patient == index]
                    - replicated_patient_means[:, index, None]
                ),
                axis=1,
            )
            for index in range(patient_count)
        ],
        axis=1,
    )
    replicated_residual = replicated_residual_by_patient.max(axis=1)
    replicated_global_means = replicated.mean(axis=1)
    return {
        "partial_pooling": partial_pooling,
        "posterior_predictive": {
            "observed_global_mean": float(observations.mean()),
            "replicated_global_mean_mean": float(replicated_global_means.mean()),
            "replicated_global_mean_sd": float(replicated_global_means.std(ddof=1)),
            "observed_patient_mean_sd": float(observed_patient_means.std(ddof=1)),
            "replicated_patient_mean_sd_mean": float(
                replicated_patient_means.std(axis=1, ddof=1).mean()
            ),
            "observed_maximum_absolute_residual": observed_residual,
            "replicated_maximum_absolute_residual_mean": float(replicated_residual.mean()),
            "probability_replicated_maximum_absolute_residual_at_least_observed": float(
                np.mean(replicated_residual >= observed_residual)
            ),
        },
    }
