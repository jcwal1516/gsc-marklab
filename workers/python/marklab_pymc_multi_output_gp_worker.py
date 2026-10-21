#!/usr/bin/env python3
"""PyMC worker for a one-factor, two-output exact Matérn GP."""

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
    "output_a_name",
    "output_b_name",
    "latent_processes",
    "kernel",
    "loading_a",
    "loading_b_constraint",
    "mean_prior_sd",
    "amplitude_prior_sd",
    "length_scale_prior_sd_um",
    "loading_b_prior_sd",
    "noise_a_sd",
    "noise_b_sd",
    "jitter",
    "backend_capability",
    "maturity",
}
PRIOR_NAMES = [
    "mean_prior_sd",
    "amplitude_prior_sd",
    "length_scale_prior_sd_um",
    "loading_b_prior_sd",
    "noise_a_sd",
    "noise_b_sd",
    "jitter",
]
RESOURCE_KEYS = {
    "maximum_observations",
    "maximum_predictions",
    "maximum_total_iterations",
    "maximum_conditioning_work",
    "maximum_output_bytes",
    "timeout_seconds",
}


def valid_label(value: Any) -> bool:
    return isinstance(value, str) and bool(value) and value.strip() == value


def validate(request: Any, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    request = obj(request, REQUEST_KEYS, "request")
    exact(request["format"], "marklab.pymc_worker_request", "request.format")
    integer(request["version"], "request.version", 1, 1)
    validate_backend(
        request["backend"],
        "backend",
        name="pymc",
        version=versions.PYMC,
        lock_sha256=lock_sha,
        worker_sha256=worker_sha,
    )
    model = obj(request["model"], MODEL_KEYS, "model")
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    integer(model["version"], "model.version", 1, 1)
    exact(model["family"], "one_factor_two_output_matern32_gp", "model.family")
    integer(model["coordinate_dimension"], "model.dimension", 1, 1)
    exact(model["coordinate_unit"], "micrometre", "model.unit")
    output_a_name = model["output_a_name"]
    output_b_name = model["output_b_name"]
    if (
        not (valid_label(output_a_name) and valid_label(output_b_name))
        or output_a_name == output_b_name
    ):
        raise ContractError("output names are invalid")
    integer(model["latent_processes"], "model.latent_processes", 1, 1)
    exact(model["kernel"], "matern_3_2", "model.kernel")
    exact(model["loading_a"], 1.0, "model.loading_a")
    exact(model["loading_b_constraint"], "positive", "model.loading_b_constraint")
    priors = {name: number(model[name], f"model.{name}") for name in PRIOR_NAMES}
    if min(priors.values()) <= 0.0:
        raise ContractError("prior scales and jitter must be positive")
    exact(model["backend_capability"], "nuts_exact_dense_multi_output_gp", "model.capability")
    exact(model["maturity"], "experimental", "model.maturity")

    observations = request["observations"]
    if not isinstance(observations, list) or not 5 <= len(observations) <= 64:
        raise ContractError("observations must contain 5-64 rows")
    ids: list[str] = []
    x: list[float] = []
    a: list[float] = []
    b: list[float] = []
    coordinate_keys: set[int] = set()
    for index, value in enumerate(observations):
        row = obj(
            value, {"coordinate_id", "x_um", "output_a", "output_b"}, f"observations[{index}]"
        )
        coordinate = number(row["x_um"], "x_um")
        gp.append_id(ids, row["coordinate_id"], "coordinate")
        key = gp.coordinate_key(coordinate)
        if key in coordinate_keys:
            raise ContractError("coordinates must be unique")
        coordinate_keys.add(key)
        x.append(coordinate)
        a.append(number(row["output_a"], "output_a"))
        b.append(number(row["output_b"], "output_b"))
    if len(set(a)) < 2 or len(set(b)) < 2:
        raise ContractError("each output field must vary")
    predictions = request["predictions"]
    if not isinstance(predictions, list) or not 1 <= len(predictions) <= 1_024:
        raise ContractError("predictions must contain 1-1024 rows")
    prediction_ids: list[str] = []
    x_new: list[float] = []
    for index, value in enumerate(predictions):
        row = obj(value, {"prediction_id", "x_um"}, f"predictions[{index}]")
        gp.append_id(prediction_ids, row["prediction_id"], "prediction")
        x_new.append(number(row["x_um"], "prediction x"))

    sampling = nuts_sampling(request["sampling"], "sampling")
    chains, tune, draws = sampling["chains"], sampling["tune"], sampling["draws"]
    resources = obj(request["resources"], RESOURCE_KEYS, "resources")
    max_n = integer(resources["maximum_observations"], "resources.observations", 5, 64)
    max_m = integer(resources["maximum_predictions"], "resources.predictions", 1, 1_024)
    max_iterations = integer(
        resources["maximum_total_iterations"], "resources.iterations", 1, 800_000
    )
    max_work = integer(resources["maximum_conditioning_work"], "resources.work", 1, 2_000_000_000)
    output_limit = integer(resources["maximum_output_bytes"], "resources.output", 1, MIB)
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3_600)
    # Cholesky work on the stacked 2n-vector per gradient plus prediction work per kept draw.
    stacked, stacked_new = 2 * len(x), 2 * len(x_new)
    work = chains * (tune + draws) * stacked**3 + chains * draws * stacked**2 * stacked_new
    if (
        len(x) > max_n
        or len(x_new) > max_m
        or chains * (tune + draws) > max_iterations
        or work > max_work
    ):
        raise ContractError("request exceeds resource limits")
    policy = nuts_policy(
        request["diagnostic_policy"],
        "policy",
        prior_draw_range=(100, 10_000),
        count_ceiling=chains * draws,
        check_thresholds=False,
    )
    return {
        **policy,
        **priors,
        **sampling,
        "output_a_name": output_a_name,
        "output_b_name": output_b_name,
        "x": np.asarray(x),
        "a": np.asarray(a),
        "b": np.asarray(b),
        "prediction_ids": prediction_ids,
        "x_new": np.asarray(x_new),
        "output_limit": output_limit,
    }


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    return derive_seed("marklab-pymc-mogp-v1", seed, purpose, index)


def block_covariance(base: np.ndarray, loading: float) -> np.ndarray:
    """Covariance of ``(f, loading * f)`` for a shared latent process with covariance ``base``."""
    return np.block([[base, loading * base], [loading * base, loading**2 * base]])


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    """Fit the one-factor two-output Matern-3/2 GP by NUTS and predict both outputs.

    Model: one latent Matern-3/2 process ``f`` (amplitude ``s``, length scale ``l``) loads on
    output A with fixed loading 1 and on output B with loading ``lambda > 0``. Marginalizing
    ``f``, the stacked outputs ``(a, b) ~ MvNormal((mu_a 1, mu_b 1), [[K, lambda K],
    [lambda K, lambda^2 K]] + diag(noise_a_sd^2 1, noise_b_sd^2 1) + jitter I)`` with known noise
    SDs. Priors: ``mu_a, mu_b ~ Normal(0, mean_prior_sd)``, ``s ~ HalfNormal(amplitude_prior_sd)``,
    ``l ~ HalfNormal(length_scale_prior_sd_um)``, ``lambda ~ HalfNormal(loading_b_prior_sd)``.
    Predictions draw both latent outputs at the new coordinates from their joint Gaussian
    conditional for every posterior draw.
    """
    n = len(config["x"])
    distance = np.abs(config["x"][:, None] - config["x"][None, :])
    observed = np.concatenate([config["a"], config["b"]])
    with pm.Model():
        mean_a = pm.Normal("mean_a", 0.0, config["mean_prior_sd"])
        mean_b = pm.Normal("mean_b", 0.0, config["mean_prior_sd"])
        amplitude = pm.HalfNormal("amplitude", config["amplitude_prior_sd"])
        length_scale_um = pm.HalfNormal("length_scale_um", config["length_scale_prior_sd_um"])
        loading_b = pm.HalfNormal("loading_b", config["loading_b_prior_sd"])
        scaled = math.sqrt(3.0) * distance / length_scale_um
        base = amplitude**2 * (1.0 + scaled) * pt.exp(-scaled)
        covariance = pt.concatenate(
            [
                pt.concatenate([base, loading_b * base], axis=1),
                pt.concatenate([loading_b * base, loading_b**2 * base], axis=1),
            ],
            axis=0,
        )
        noise_variances = pt.concatenate(
            [pt.ones(n) * config["noise_a_sd"] ** 2, pt.ones(n) * config["noise_b_sd"] ** 2]
        )
        covariance += pt.diag(noise_variances + config["jitter"])
        mean_vector = pt.concatenate([pt.ones(n) * mean_a, pt.ones(n) * mean_b])
        pm.MvNormal("observed", mean_vector, cov=covariance, observed=observed)
        prior = pymc_nuts.prior_predictive(config, seed_for)
        posterior = pymc_nuts.nuts(config, seed_for)
        predictive = pymc_nuts.posterior_predictive(posterior, "observed", config, seed_for)

    names = ["mean_a", "mean_b", "amplitude", "length_scale_um", "loading_b"]
    arrays = {name: np.asarray(posterior["posterior"][name].values) for name in names}
    replicated = np.asarray(predictive["posterior_predictive"]["observed"].values)
    prior_finite = bool(
        all(np.isfinite(prior["prior"][name].values).all() for name in names)
        and np.isfinite(prior["prior_predictive"]["observed"].values).all()
    )
    posterior_finite = bool(
        all(np.isfinite(array).all() for array in arrays.values()) and np.isfinite(replicated).all()
    )
    diagnostics = pymc_nuts_diagnostics(posterior, names)

    flat = {name: array.reshape(-1) for name, array in arrays.items()}
    rng = np.random.default_rng(seed_for(config["seed"], "conditional_predictions"))
    m = len(config["x_new"])
    prediction_draws = np.empty((len(flat["mean_a"]), 2 * m))
    identity = np.eye(2 * n)
    noise_diagonal = np.diag(
        np.concatenate(
            [np.full(n, config["noise_a_sd"] ** 2), np.full(n, config["noise_b_sd"] ** 2)]
        )
    )
    for index, parameters in enumerate(zip(*(flat[name] for name in names), strict=True)):
        mean_a, mean_b, amplitude, length, loading = parameters
        base_observed = gp.matern32(config["x"], config["x"], amplitude, length)
        observed_covariance = block_covariance(base_observed, loading)
        observed_covariance += noise_diagonal
        observed_covariance += config["jitter"] * identity
        base_cross = gp.matern32(config["x_new"], config["x"], amplitude, length)
        base_prediction = gp.matern32(config["x_new"], config["x_new"], amplitude, length)
        observed_mean = np.concatenate([np.full(n, mean_a), np.full(n, mean_b)])
        new_mean = np.concatenate([np.full(m, mean_a), np.full(m, mean_b)])
        shift, conditional_covariance = gp.condition(
            observed_covariance,
            block_covariance(base_cross, loading),
            block_covariance(base_prediction, loading),
            observed - observed_mean,
        )
        conditional_covariance += config["jitter"] * np.eye(2 * m)
        prediction_draws[index] = rng.multivariate_normal(
            new_mean + shift, conditional_covariance, check_valid="raise"
        )
    posterior_finite = posterior_finite and bool(np.isfinite(prediction_draws).all())
    complete = prior_finite and posterior_finite and convergence_passes(config, diagnostics)

    prediction_summaries = []
    for index, prediction_id in enumerate(config["prediction_ids"]):
        output_summaries = {
            "output_a": summary(prediction_draws[:, index]),
            "output_b": summary(prediction_draws[:, m + index]),
        }
        prediction_summaries.append(
            {
                "prediction_id": prediction_id,
                "x_um": float(config["x_new"][index]),
                **{
                    f"{output}_{statistic}": value
                    for output, values in output_summaries.items()
                    for statistic, value in values.items()
                },
            }
        )
    replicated_a = replicated[..., :n]
    replicated_b = replicated[..., n:]
    correlations = np.asarray(
        [
            np.corrcoef(a_values, b_values)[0, 1]
            for a_values, b_values in zip(
                replicated_a.reshape(-1, n), replicated_b.reshape(-1, n), strict=True
            )
        ]
    )
    positive = ["amplitude", "length_scale_um", "loading_b"]
    return {
        "format": "marklab.pymc_multi_output_gp_worker_result",
        "version": 1,
        "backend": backend_identity("pymc", pm.__version__, lock_sha, worker_sha),
        "request_sha256": request_sha,
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": sampling_record(config),
        "posterior": {name: summary(arrays[name]) for name in names},
        "predictions": prediction_summaries,
        "diagnostics": {
            "prior_predictive_finite": prior_finite,
            "posterior_finite": posterior_finite,
            **diagnostics,
            "constraints_valid": bool(all(np.all(arrays[name] >= 0.0) for name in positive)),
            "identifiability_checks_passed": True,
        },
        "posterior_predictive": {
            "observed_correlation": float(np.corrcoef(config["a"], config["b"])[0, 1]),
            "replicated_correlation_mean": float(correlations.mean()),
            "observed_a_mean": float(config["a"].mean()),
            "replicated_a_mean": float(replicated_a.mean()),
            "observed_b_mean": float(config["b"].mean()),
            "replicated_b_mean": float(replicated_b.mean()),
        },
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
