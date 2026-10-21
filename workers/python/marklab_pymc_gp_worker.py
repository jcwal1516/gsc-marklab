#!/usr/bin/env python3
"""PyMC worker for Marklab's exact one-dimensional Matérn GP regression."""

from __future__ import annotations

import json
import math
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

from marklab_worker_support import gp, pymc_nuts, versions
from marklab_worker_support.nuts_contract import (
    convergence_passes,
    nuts_policy,
    nuts_sampling,
    pymc_nuts_diagnostics,
    sampling_record,
)
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
    validate_backend,
    worker_digests,
    write_result,
)

REQUEST_KEYS = {
    "format",
    "version",
    "backend",
    "model",
    "observations",
    "predictions",
    "sampling",
    "resources",
    "diagnostic_policy",
}
MODEL_KEYS = {
    "format",
    "version",
    "family",
    "coordinate_dimension",
    "coordinate_unit",
    "kernel",
    "mean_prior_mean",
    "mean_prior_sd",
    "amplitude_prior_sd",
    "length_scale_prior_sd_um",
    "noise_prior_sd",
    "jitter",
    "observation_unit",
    "backend_capability",
    "maturity",
}
RESOURCE_KEYS = {
    "maximum_observations",
    "maximum_predictions",
    "maximum_total_iterations",
    "maximum_conditioning_work",
    "maximum_output_bytes",
    "timeout_seconds",
}


def validate(request: Any, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    request = obj(request, REQUEST_KEYS, "request")
    exact(request["format"], "marklab.pymc_worker_request", "request.format")
    integer(request["version"], "request.version", 1, 1)
    validate_backend(
        request["backend"],
        "request.backend",
        name="pymc",
        version=versions.PYMC,
        lock_sha256=lock_sha,
        worker_sha256=worker_sha,
    )

    model = obj(request["model"], MODEL_KEYS, "request.model")
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    integer(model["version"], "model.version", 1, 1)
    exact(model["family"], "exact_matern32_gp_regression", "model.family")
    integer(model["coordinate_dimension"], "model.coordinate_dimension", 1, 1)
    exact(model["coordinate_unit"], "micrometre", "model.coordinate_unit")
    exact(model["kernel"], "matern_3_2", "model.kernel")
    mean_prior_mean = number(model["mean_prior_mean"], "model.mean_prior_mean")
    mean_prior_sd = number(model["mean_prior_sd"], "model.mean_prior_sd")
    amplitude_prior_sd = number(model["amplitude_prior_sd"], "model.amplitude_prior_sd")
    length_prior_sd = number(model["length_scale_prior_sd_um"], "model.length prior")
    noise_prior_sd = number(model["noise_prior_sd"], "model.noise prior")
    jitter = number(model["jitter"], "model.jitter")
    if min(mean_prior_sd, amplitude_prior_sd, length_prior_sd, noise_prior_sd, jitter) <= 0.0:
        raise ContractError("GP prior scales and jitter must be positive")
    exact(model["observation_unit"], "scalar_field_value", "model.observation_unit")
    exact(model["backend_capability"], "nuts_exact_dense_gp", "model.capability")
    exact(model["maturity"], "experimental", "model.maturity")

    observations = request["observations"]
    if not isinstance(observations, list) or not 5 <= len(observations) <= 128:
        raise ContractError("observations must contain 5-128 rows")
    observation_ids: list[str] = []
    x: list[float] = []
    y: list[float] = []
    coordinate_keys: set[int] = set()
    for index, value in enumerate(observations):
        row = obj(value, {"observation_id", "x_um", "value"}, f"observations[{index}]")
        coordinate = number(row["x_um"], f"observations[{index}].x_um")
        outcome = number(row["value"], f"observations[{index}].value")
        gp.append_id(observation_ids, row["observation_id"], "observation")
        key = gp.coordinate_key(coordinate)
        if key in coordinate_keys:
            raise ContractError("observation coordinates must be unique")
        coordinate_keys.add(key)
        x.append(coordinate)
        y.append(outcome)

    predictions = request["predictions"]
    if not isinstance(predictions, list) or not 1 <= len(predictions) <= 2_048:
        raise ContractError("predictions must contain 1-2048 rows")
    prediction_ids: list[str] = []
    x_new: list[float] = []
    for index, value in enumerate(predictions):
        row = obj(value, {"prediction_id", "x_um"}, f"predictions[{index}]")
        coordinate = number(row["x_um"], f"predictions[{index}].x_um")
        gp.append_id(prediction_ids, row["prediction_id"], "prediction")
        x_new.append(coordinate)

    sampling = nuts_sampling(request["sampling"], "request.sampling")
    chains, tune, draws = sampling["chains"], sampling["tune"], sampling["draws"]

    resources = obj(request["resources"], RESOURCE_KEYS, "request.resources")
    max_observations = integer(resources["maximum_observations"], "resources.observations", 5, 128)
    max_predictions = integer(resources["maximum_predictions"], "resources.predictions", 1, 2_048)
    max_iterations = integer(
        resources["maximum_total_iterations"], "resources.iterations", 1, 800_000
    )
    max_work = integer(resources["maximum_conditioning_work"], "resources.work", 1, 2_000_000_000)
    output_limit = integer(resources["maximum_output_bytes"], "resources.output", 1, MIB)
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3_600)
    if (
        len(x) > max_observations
        or len(x_new) > max_predictions
        or chains * (tune + draws) > max_iterations
    ):
        raise ContractError("GP request exceeds resource limits")
    # Cholesky work per gradient evaluation plus conditional prediction work per kept draw.
    work = chains * (tune + draws) * len(x) ** 3 + chains * draws * len(x) ** 2 * len(x_new)
    if work > max_work:
        raise ContractError("GP request exceeds conditioning-work limit")

    policy = nuts_policy(
        request["diagnostic_policy"],
        "request.diagnostic_policy",
        prior_draw_range=(100, 10_000),
        count_ceiling=chains * draws,
        check_thresholds=False,
    )
    return {
        **policy,
        **sampling,
        "mean_prior_mean": mean_prior_mean,
        "mean_prior_sd": mean_prior_sd,
        "amplitude_prior_sd": amplitude_prior_sd,
        "length_prior_sd": length_prior_sd,
        "noise_prior_sd": noise_prior_sd,
        "jitter": jitter,
        "observation_ids": observation_ids,
        "x": np.asarray(x, dtype=np.float64),
        "y": np.asarray(y, dtype=np.float64),
        "prediction_ids": prediction_ids,
        "x_new": np.asarray(x_new, dtype=np.float64),
        "output_limit": output_limit,
    }


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    return derive_seed("marklab-pymc-gp-v1", seed, purpose, index)


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    """Fit the exact Matern-3/2 GP regression by NUTS and predict at new coordinates.

    Likelihood (marginalized over the latent field):
    ``y ~ MvNormal(mu 1, K + (sigma^2 + jitter) I)`` with
    ``K_ij = a^2 (1 + sqrt(3) |x_i - x_j| / l) exp(-sqrt(3) |x_i - x_j| / l)``. Priors:
    ``mu ~ Normal(mean_prior_mean, mean_prior_sd)``, ``a ~ HalfNormal(amplitude_prior_sd)``,
    ``l ~ HalfNormal(length_scale_prior_sd_um)``, ``sigma ~ HalfNormal(noise_prior_sd)``.
    Predictions draw the latent field at ``x_new`` from its Gaussian conditional for every
    posterior draw.
    """
    distance = np.abs(config["x"][:, None] - config["x"][None, :])
    identity = np.eye(len(config["x"]))
    with pm.Model():
        mean = pm.Normal("mean", config["mean_prior_mean"], config["mean_prior_sd"])
        amplitude = pm.HalfNormal("amplitude", config["amplitude_prior_sd"])
        length_scale_um = pm.HalfNormal("length_scale_um", config["length_prior_sd"])
        noise_sd = pm.HalfNormal("noise_sd", config["noise_prior_sd"])
        scaled = math.sqrt(3.0) * distance / length_scale_um
        covariance = amplitude**2 * (1.0 + scaled) * pt.exp(-scaled)
        covariance += (noise_sd**2 + config["jitter"]) * identity
        pm.MvNormal(
            "observed", mu=pt.ones(len(config["x"])) * mean, cov=covariance, observed=config["y"]
        )
        prior = pymc_nuts.prior_predictive(config, seed_for)
        posterior = pymc_nuts.nuts(config, seed_for)
        predictive = pymc_nuts.posterior_predictive(posterior, "observed", config, seed_for)

    names = ["mean", "amplitude", "length_scale_um", "noise_sd"]
    arrays = {
        name: np.asarray(posterior["posterior"][name].values, dtype=np.float64) for name in names
    }
    predictive_observed = np.asarray(predictive["posterior_predictive"]["observed"].values)
    prior_finite = bool(
        all(np.isfinite(prior["prior"][name].values).all() for name in names)
        and np.isfinite(prior["prior_predictive"]["observed"].values).all()
    )
    posterior_finite = bool(
        all(np.isfinite(value).all() for value in arrays.values())
        and np.isfinite(predictive_observed).all()
    )
    diagnostics = pymc_nuts_diagnostics(posterior, names)

    flat = {name: value.reshape(-1) for name, value in arrays.items()}
    rng = np.random.default_rng(seed_for(config["seed"], "conditional_predictions"))
    prediction_draws = np.empty((len(flat["mean"]), len(config["x_new"])), dtype=np.float64)
    draws = zip(
        flat["mean"], flat["amplitude"], flat["length_scale_um"], flat["noise_sd"], strict=True
    )
    for index, (mean_value, amplitude_value, length_value, noise_value) in enumerate(draws):
        observed_covariance = gp.matern32(config["x"], config["x"], amplitude_value, length_value)
        observed_covariance += (noise_value**2 + config["jitter"]) * identity
        shift, conditional_covariance = gp.condition(
            observed_covariance,
            gp.matern32(config["x_new"], config["x"], amplitude_value, length_value),
            gp.matern32(config["x_new"], config["x_new"], amplitude_value, length_value),
            config["y"] - mean_value,
        )
        conditional_covariance += config["jitter"] * np.eye(len(config["x_new"]))
        prediction_draws[index] = rng.multivariate_normal(
            mean_value + shift, conditional_covariance, check_valid="raise"
        )
    posterior_finite = posterior_finite and bool(np.isfinite(prediction_draws).all())
    complete = prior_finite and posterior_finite and convergence_passes(config, diagnostics)
    predictions = [
        {
            "prediction_id": prediction_id,
            "x_um": float(config["x_new"][index]),
            **summary(prediction_draws[:, index]),
        }
        for index, prediction_id in enumerate(config["prediction_ids"])
    ]
    return {
        "format": "marklab.pymc_gp_worker_result",
        "version": 1,
        "backend": backend_identity("pymc", pm.__version__, lock_sha, worker_sha),
        "request_sha256": request_sha,
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": sampling_record(config),
        "posterior": {name: summary(arrays[name]) for name in names},
        "predictions": predictions,
        "diagnostics": {
            "prior_predictive_finite": prior_finite,
            "posterior_finite": posterior_finite,
            **diagnostics,
            "constraints_valid": bool(
                np.all(arrays["amplitude"] >= 0.0)
                and np.all(arrays["length_scale_um"] >= 0.0)
                and np.all(arrays["noise_sd"] >= 0.0)
            ),
            "identifiability_checks_passed": True,
        },
        "posterior_predictive": gp.observed_value_check(config["y"], predictive_observed),
    }


def main() -> None:
    require_versions(pymc=pm.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(16 * MIB)
    config = validate(json.loads(raw), lock_sha, worker_sha)
    result = fit(config, sha256_hex(raw), lock_sha, worker_sha)
    write_result(result, max_bytes=config["output_limit"])


if __name__ == "__main__":
    run_worker(main, "marklab PyMC worker failed")
