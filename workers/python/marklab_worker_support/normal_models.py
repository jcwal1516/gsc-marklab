"""Request contracts and summaries for the normal-mean and Gaussian patient-hierarchy models.

The normal-mean model block is shared by the PyMC NUTS and annealed-SMC workers. The Gaussian
patient-hierarchy request is the PyMC worker's contract; the NumPyro agreement and SBC workers
validate their embedded source request against it.
"""

from __future__ import annotations

from typing import Any

import numpy as np

from . import versions
from .diagnostics import mean_mcse
from .nuts_contract import nuts_policy, nuts_resources, nuts_sampling
from .posterior import summary
from .protocol import ContractError, exact, integer, number, obj, validate_backend

PYMC_REQUEST_FORMAT = "marklab.pymc_worker_request"

NORMAL_MEAN_MODEL_KEYS = {
    "format",
    "version",
    "family",
    "parameter",
    "prior",
    "likelihood",
    "observation_unit",
    "generated_quantities",
    "backend_capability",
    "maturity",
}
HIERARCHY_MODEL_KEYS = {
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
    "generated_quantities",
    "posterior_predictive_statistics",
    "backend_capability",
    "maturity",
}


def normal_mean_model(value: Any, path: str, capability: str) -> tuple[float, float, float]:
    """Validate the normal-mean model block; return the prior mean, prior SD, and known sigma."""
    model = obj(value, NORMAL_MEAN_MODEL_KEYS, path)
    exact(model["format"], "marklab.bayesian_model_ir", f"{path}.format")
    integer(model["version"], f"{path}.version", 1, 1)
    exact(model["family"], "normal_mean_known_sigma", f"{path}.family")
    parameter = obj(model["parameter"], {"name", "support", "interpretation"}, f"{path}.parameter")
    exact(parameter["name"], "mu", f"{path}.parameter.name")
    exact(parameter["support"], "real", f"{path}.parameter.support")
    exact(parameter["interpretation"], "population_mean", f"{path}.parameter.interpretation")
    prior = obj(model["prior"], {"family", "mean", "sd", "rationale"}, f"{path}.prior")
    exact(prior["family"], "normal", f"{path}.prior.family")
    prior_mean = number(prior["mean"], f"{path}.prior.mean")
    prior_sd = number(prior["sd"], f"{path}.prior.sd")
    exact(prior["rationale"], "user_supplied", f"{path}.prior.rationale")
    likelihood = obj(model["likelihood"], {"family", "known_sigma"}, f"{path}.likelihood")
    exact(likelihood["family"], "normal_known_sigma", f"{path}.likelihood.family")
    known_sigma = number(likelihood["known_sigma"], f"{path}.likelihood.known_sigma")
    if prior_sd <= 0.0 or known_sigma <= 0.0:
        raise ContractError("normal scales must be positive")
    exact(model["observation_unit"], "scalar_observation", f"{path}.observation_unit")
    exact(
        model["generated_quantities"],
        ["posterior_predictive_observation_mean"],
        f"{path}.generated_quantities",
    )
    exact(model["backend_capability"], capability, f"{path}.backend_capability")
    exact(model["maturity"], "experimental", f"{path}.maturity")
    return prior_mean, prior_sd, known_sigma


def hierarchy_model(value: Any, path: str) -> dict[str, float]:
    """Validate the Gaussian patient-hierarchy model block; return its prior and noise scales."""
    model = obj(value, HIERARCHY_MODEL_KEYS, path)
    exact(model["format"], "marklab.bayesian_model_ir", f"{path}.format")
    integer(model["version"], f"{path}.version", 1, 1)
    exact(model["family"], "gaussian_patient_varying_intercept", f"{path}.family")
    global_prior = obj(
        model["global_mean_prior"],
        {"family", "mean", "sd", "rationale"},
        f"{path}.global_mean_prior",
    )
    exact(global_prior["family"], "normal", f"{path}.global_mean_prior.family")
    global_prior_mean = number(global_prior["mean"], f"{path}.global_mean_prior.mean")
    global_prior_sd = number(global_prior["sd"], f"{path}.global_mean_prior.sd")
    if global_prior_sd <= 0.0:
        raise ContractError("global prior SD must be positive")
    exact(global_prior["rationale"], "user_supplied", f"{path}.global_mean_prior.rationale")
    between_prior = obj(
        model["between_patient_sd_prior"],
        {"family", "sd", "support", "rationale"},
        f"{path}.between_patient_sd_prior",
    )
    exact(between_prior["family"], "half_normal", f"{path}.between_patient_sd_prior.family")
    between_prior_sd = number(between_prior["sd"], f"{path}.between_patient_sd_prior.sd")
    if between_prior_sd <= 0.0:
        raise ContractError("between-patient prior SD must be positive")
    exact(between_prior["support"], "positive", f"{path}.between_patient_sd_prior.support")
    exact(
        between_prior["rationale"],
        "user_supplied",
        f"{path}.between_patient_sd_prior.rationale",
    )
    exact(
        model["patient_effect_parameterization"],
        "noncentered",
        f"{path}.patient_effect_parameterization",
    )
    likelihood = obj(model["likelihood"], {"family", "known_sigma", "link"}, f"{path}.likelihood")
    exact(likelihood["family"], "normal_known_sigma", f"{path}.likelihood.family")
    known_sigma = number(likelihood["known_sigma"], f"{path}.likelihood.known_sigma")
    if known_sigma <= 0.0:
        raise ContractError("known sigma must be positive")
    exact(likelihood["link"], "identity", f"{path}.likelihood.link")
    exact(
        model["observation_unit"],
        "patient_nested_scalar_observation",
        f"{path}.observation_unit",
    )
    exact(model["biological_unit"], "patient", f"{path}.biological_unit")
    exact(model["hierarchy"], ["patient"], f"{path}.hierarchy")
    exact(model["spatial_component"], "none", f"{path}.spatial_component")
    exact(
        model["generated_quantities"],
        ["patient_mean", "variance_partition", "posterior_predictive_observation"],
        f"{path}.generated_quantities",
    )
    exact(
        model["posterior_predictive_statistics"],
        ["global_mean", "patient_mean_sd"],
        f"{path}.posterior_predictive_statistics",
    )
    exact(model["backend_capability"], "nuts", f"{path}.backend_capability")
    exact(model["maturity"], "experimental", f"{path}.maturity")
    return {
        "global_prior_mean": global_prior_mean,
        "global_prior_sd": global_prior_sd,
        "between_prior_sd": between_prior_sd,
        "known_sigma": known_sigma,
    }


def patient_observations(value: Any, path: str) -> tuple[list[str], list[np.ndarray]]:
    """Validate at least three patients, strictly increasing IDs, two or more values each."""
    if not isinstance(value, list) or len(value) < 3:
        raise ContractError(f"{path} must contain at least three patients")
    patient_ids: list[str] = []
    patient_values: list[np.ndarray] = []
    for index, raw in enumerate(value):
        patient = obj(raw, {"patient_id", "observations"}, f"{path}[{index}]")
        patient_id = patient["patient_id"]
        if not isinstance(patient_id, str) or not patient_id or patient_id.strip() != patient_id:
            raise ContractError(f"{path}[{index}].patient_id is invalid")
        if patient_ids and patient_id <= patient_ids[-1]:
            raise ContractError("patient IDs must be strictly increasing")
        values = patient["observations"]
        if not isinstance(values, list) or len(values) < 2:
            raise ContractError(f"patient {patient_id} requires at least two observations")
        observations = [number(item, f"{path}[{index}].observations") for item in values]
        patient_ids.append(patient_id)
        patient_values.append(np.asarray(observations, dtype=np.float64))
    return patient_ids, patient_values


def validate_hierarchy_request(
    request: Any, lock_digest: str, worker_digest: str
) -> dict[str, Any]:
    """Validate a PyMC Gaussian patient-hierarchy request and return the fit configuration."""
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
    exact(request["format"], PYMC_REQUEST_FORMAT, "request.format")
    integer(request["version"], "request.version", 1, 1)
    validate_backend(
        request["backend"],
        "request.backend",
        name="pymc",
        version=versions.PYMC,
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )
    config = hierarchy_model(request["model"], "request.model")
    patient_ids, patient_values = patient_observations(request["patients"], "request.patients")
    sampling = nuts_sampling(request["sampling"], "request.sampling")
    config["maximum_output_bytes"] = nuts_resources(
        request["resources"],
        "request.resources",
        observations=sum(len(values) for values in patient_values),
        iterations=sampling["chains"] * (sampling["tune"] + sampling["draws"]),
    )
    config.update(
        nuts_policy(
            request["diagnostic_policy"],
            "request.diagnostic_policy",
            count_ceiling=sampling["chains"] * sampling["draws"],
        )
    )
    config.update(sampling)
    config["patient_ids"] = patient_ids
    config["patient_values"] = patient_values
    return config


def patient_index(patient_values: list[np.ndarray], dtype: Any) -> np.ndarray:
    """Patient position of every observation, in concatenation order."""
    return np.concatenate(
        [np.full(len(values), index, dtype=dtype) for index, values in enumerate(patient_values)]
    )


def hierarchy_summaries(
    config: dict[str, Any],
    observations: np.ndarray,
    observation_patient: np.ndarray,
    global_draws: np.ndarray,
    between_draws: np.ndarray,
    patient_draws: np.ndarray,
    predictive_draws: np.ndarray,
) -> dict[str, Any]:
    """Posterior, partial-pooling, and posterior-predictive summaries of a hierarchy fit.

    ``patient_draws`` has a trailing patient axis and ``predictive_draws`` a trailing
    observation axis; shrinkage compares each patient's posterior variance with the
    known-sigma variance of its raw mean.
    """
    known_sigma = config["known_sigma"]
    partial_pooling = []
    for index, (patient_id, values) in enumerate(
        zip(config["patient_ids"], config["patient_values"], strict=True)
    ):
        draws = patient_draws[..., index]
        posterior_variance = float(draws.var(ddof=1))
        unpooled_variance = known_sigma**2 / len(values)
        partial_pooling.append(
            {
                "patient_id": patient_id,
                "observation_count": len(values),
                "raw_mean": float(values.mean()),
                "posterior_mean": float(draws.mean()),
                "posterior_sd": float(draws.std(ddof=1)),
                "interval_lower": float(np.quantile(draws, 0.025)),
                "interval_upper": float(np.quantile(draws, 0.975)),
                "mcse_mean": mean_mcse(draws, config["chains"]),
                "shrinkage": 1.0 - posterior_variance / unpooled_variance,
                "warning": "shrinkage_is_model_dependent_not_a_quality_score",
            }
        )

    replicated_global_means = predictive_draws.mean(axis=-1)
    replicated_patient_means = np.stack(
        [
            predictive_draws[..., observation_patient == index].mean(axis=-1)
            for index in range(len(config["patient_ids"]))
        ],
        axis=-1,
    )
    replicated_patient_mean_sd = replicated_patient_means.std(axis=-1, ddof=1)
    observed_global_mean = float(observations.mean())
    raw_means = np.asarray([values.mean() for values in config["patient_values"]])
    observed_patient_mean_sd = float(raw_means.std(ddof=1))
    variance_partition = between_draws**2 / (between_draws**2 + known_sigma**2)
    return {
        "posterior": {
            "global_mean": summary(global_draws, chains=config["chains"]),
            "between_patient_sd": summary(between_draws, chains=config["chains"]),
            "variance_partition_mean": float(variance_partition.mean()),
        },
        "partial_pooling": partial_pooling,
        "posterior_predictive": {
            "observed_global_mean": observed_global_mean,
            "replicated_global_mean_mean": float(replicated_global_means.mean()),
            "replicated_global_mean_sd": float(replicated_global_means.std(ddof=1)),
            "probability_replicated_global_mean_at_least_observed": float(
                np.mean(replicated_global_means >= observed_global_mean)
            ),
            "observed_patient_mean_sd": observed_patient_mean_sd,
            "replicated_patient_mean_sd_mean": float(replicated_patient_mean_sd.mean()),
        },
    }
