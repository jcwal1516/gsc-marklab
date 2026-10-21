#!/usr/bin/env python3
"""PyMC worker for Marklab's axis-aligned anisotropic three-dimensional Matérn GP."""

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


def coordinates(value: Any, path: str) -> list[float]:
    if not isinstance(value, list) or len(value) != 3:
        raise ContractError(f"{path} must contain exactly three coordinates")
    return [number(item, f"{path}[{index}]") for index, item in enumerate(value)]


def validate(request: Any, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "model",
            "observations",
            "predictions",
            "sampling",
            "resources",
            "diagnostic_policy",
        },
        "request",
    )
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

    model = obj(
        request["model"],
        {
            "format",
            "version",
            "family",
            "coordinate_dimension",
            "coordinate_unit",
            "kernel",
            "anisotropy",
            "mean_prior_mean",
            "mean_prior_sd",
            "amplitude_prior_sd",
            "length_scale_prior_sd_um",
            "noise_prior_sd",
            "jitter",
            "observation_unit",
            "backend_capability",
            "maturity",
        },
        "model",
    )
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    integer(model["version"], "model.version", 1, 1)
    exact(model["family"], "exact_axis_aligned_anisotropic_matern32_gp3d", "model.family")
    integer(model["coordinate_dimension"], "model.coordinate_dimension", 3, 3)
    exact(model["coordinate_unit"], "micrometre", "model.coordinate_unit")
    exact(model["kernel"], "matern_3_2", "model.kernel")
    exact(model["anisotropy"], "axis_aligned_positive_length_scales", "model.anisotropy")
    exact(model["observation_unit"], "scalar_field_value", "model.observation_unit")
    exact(model["backend_capability"], "nuts_exact_dense_anisotropic_gp3d", "model.capability")
    exact(model["maturity"], "experimental", "model.maturity")
    mean_prior_mean = number(model["mean_prior_mean"], "model.mean_prior_mean")
    positive_scales = [
        number(model["mean_prior_sd"], "model.mean_prior_sd"),
        number(model["amplitude_prior_sd"], "model.amplitude_prior_sd"),
        *coordinates(model["length_scale_prior_sd_um"], "model.length_scale_prior_sd_um"),
        number(model["noise_prior_sd"], "model.noise_prior_sd"),
        number(model["jitter"], "model.jitter"),
    ]
    if min(positive_scales) <= 0.0:
        raise ContractError("3-D GP prior scales and jitter must be positive")

    rows = request["observations"]
    if not isinstance(rows, list) or not 8 <= len(rows) <= 64:
        raise ContractError("observations must contain 8-64 rows")
    observation_ids: list[str] = []
    x: list[list[float]] = []
    y: list[float] = []
    coordinate_keys: set[tuple[int, int, int]] = set()
    for index, value in enumerate(rows):
        row = obj(value, {"observation_id", "coordinates_um", "value"}, f"observations[{index}]")
        point = coordinates(row["coordinates_um"], f"observations[{index}].coordinates_um")
        gp.append_id(observation_ids, row["observation_id"], "observation")
        key = tuple(gp.coordinate_key(item) for item in point)
        if key in coordinate_keys:
            raise ContractError("observation coordinates must be unique")
        coordinate_keys.add(key)
        x.append(point)
        y.append(number(row["value"], f"observations[{index}].value"))
    x = np.asarray(x, dtype=np.float64)
    with np.errstate(over="ignore"):
        spans = np.ptp(x, axis=0)
    if not np.isfinite(spans).all() or np.any(spans <= 0.0):
        raise ContractError("observations must vary on every coordinate axis")
    # The squared distance is linear in inverse squared axis lengths. Match the
    # Rust Gram-matrix rank check, including its scaling and pivot tolerance.
    left, right = np.triu_indices(len(x), 1)
    design = ((x[left] - x[right]) / spans) ** 2
    gram = design.T @ design
    tolerance = 1e-12 * max(1.0, float(np.max(np.abs(gram))))
    for pivot in range(3):
        selected = pivot + int(np.argmax(np.abs(gram[pivot:, pivot])))
        if abs(gram[selected, pivot]) <= tolerance:
            raise ContractError(
                "pairwise displacements cannot distinguish all three axis length scales"
            )
        gram[[pivot, selected]] = gram[[selected, pivot]]
        gram[pivot, pivot:] /= gram[pivot, pivot]
        for row in range(3):
            if row != pivot:
                gram[row, pivot:] -= gram[row, pivot] * gram[pivot, pivot:]

    prediction_rows = request["predictions"]
    if not isinstance(prediction_rows, list) or not 1 <= len(prediction_rows) <= 512:
        raise ContractError("predictions must contain 1-512 rows")
    prediction_ids: list[str] = []
    x_new: list[list[float]] = []
    for index, value in enumerate(prediction_rows):
        row = obj(value, {"prediction_id", "coordinates_um"}, f"predictions[{index}]")
        gp.append_id(prediction_ids, row["prediction_id"], "prediction")
        x_new.append(coordinates(row["coordinates_um"], f"predictions[{index}].coordinates_um"))

    sampling = nuts_sampling(request["sampling"], "sampling")
    chains, tune, draws = sampling["chains"], sampling["tune"], sampling["draws"]

    resources = obj(
        request["resources"],
        {
            "maximum_observations",
            "maximum_predictions",
            "maximum_total_iterations",
            "maximum_conditioning_work",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    max_observations = integer(resources["maximum_observations"], "resources.observations", 8, 64)
    max_predictions = integer(resources["maximum_predictions"], "resources.predictions", 1, 512)
    max_iterations = integer(
        resources["maximum_total_iterations"], "resources.iterations", 1, 800_000
    )
    max_work = integer(resources["maximum_conditioning_work"], "resources.work", 1, 2_000_000_000)
    output_limit = integer(resources["maximum_output_bytes"], "resources.output", 1, MIB)
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3_600)
    n, m = len(x), len(x_new)
    conditional_work = n**2 + n * m + m**2 + n**3 + n**2 * m + n * m**2 + m**3
    work = chains * (tune + draws) * n**3 + chains * draws * conditional_work
    if (
        len(x) > max_observations
        or len(x_new) > max_predictions
        or chains * (tune + draws) > max_iterations
        or work > max_work
    ):
        raise ContractError("anisotropic 3-D GP request exceeds resource limits")

    policy = nuts_policy(
        request["diagnostic_policy"],
        "diagnostic_policy",
        prior_draw_range=(100, 10_000),
        count_ceiling=chains * draws,
        check_thresholds=False,
    )
    return {
        "mean_prior_mean": mean_prior_mean,
        "mean_prior_sd": positive_scales[0],
        "amplitude_prior_sd": positive_scales[1],
        "length_prior_sd": np.asarray(positive_scales[2:5], dtype=np.float64),
        "noise_prior_sd": positive_scales[5],
        "jitter": positive_scales[6],
        "x": np.asarray(x, dtype=np.float64),
        "y": np.asarray(y, dtype=np.float64),
        "x_new": np.asarray(x_new, dtype=np.float64),
        "prediction_ids": prediction_ids,
        **sampling,
        **policy,
        "output_limit": output_limit,
    }


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    return derive_seed("marklab-pymc-anisotropic-gp3d-v1", seed, purpose, index)


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    """Fit the axis-aligned anisotropic 3-D Matern-3/2 GP by NUTS and predict at new points.

    Likelihood (marginalized over the latent field):
    ``y ~ MvNormal(mu 1, K + (sigma^2 + jitter) I)`` with
    ``K_ij = a^2 (1 + sqrt(3) r_ij) exp(-sqrt(3) r_ij)`` and the scaled distance
    ``r_ij = ||(p_i - p_j) / (l_x, l_y, l_z)||`` (floored at 1e-15 inside the model for a finite
    gradient). Priors: ``mu ~ Normal(mean_prior_mean, mean_prior_sd)``,
    ``a ~ HalfNormal(amplitude_prior_sd)``, ``l_k ~ HalfNormal(length_scale_prior_sd_um[k])``,
    ``sigma ~ HalfNormal(noise_prior_sd)``. Predictions draw the latent field at the new points
    from its Gaussian conditional for every posterior draw.
    """
    differences = config["x"][:, None, :] - config["x"][None, :, :]
    identity = np.eye(len(config["x"]))
    with pm.Model():
        mean = pm.Normal("mean", config["mean_prior_mean"], config["mean_prior_sd"])
        amplitude = pm.HalfNormal("amplitude", config["amplitude_prior_sd"])
        length_scale_x_um = pm.HalfNormal("length_scale_x_um", config["length_prior_sd"][0])
        length_scale_y_um = pm.HalfNormal("length_scale_y_um", config["length_prior_sd"][1])
        length_scale_z_um = pm.HalfNormal("length_scale_z_um", config["length_prior_sd"][2])
        noise_sd = pm.HalfNormal("noise_sd", config["noise_prior_sd"])
        length_scales = pt.stack([length_scale_x_um, length_scale_y_um, length_scale_z_um])
        radius = pt.sqrt(pt.maximum(pt.sum((differences / length_scales) ** 2, axis=2), 1e-30))
        scaled = math.sqrt(3.0) * radius
        covariance = amplitude**2 * (1.0 + scaled) * pt.exp(-scaled)
        covariance += (noise_sd**2 + config["jitter"]) * identity
        pm.MvNormal(
            "observed", mu=pt.ones(len(config["x"])) * mean, cov=covariance, observed=config["y"]
        )
        prior = pymc_nuts.prior_predictive(config, seed_for)
        posterior = pymc_nuts.nuts(config, seed_for)
        predictive = pymc_nuts.posterior_predictive(posterior, "observed", config, seed_for)

    names = [
        "mean",
        "amplitude",
        "length_scale_x_um",
        "length_scale_y_um",
        "length_scale_z_um",
        "noise_sd",
    ]
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
    for index, values in enumerate(
        zip(
            flat["mean"],
            flat["amplitude"],
            flat["length_scale_x_um"],
            flat["length_scale_y_um"],
            flat["length_scale_z_um"],
            flat["noise_sd"],
            strict=True,
        )
    ):
        mean_value, amplitude_value, lx, ly, lz, noise_value = values
        lengths = np.asarray([lx, ly, lz])
        observed_covariance = gp.anisotropic_matern32(
            config["x"], config["x"], amplitude_value, lengths
        )
        observed_covariance += (noise_value**2 + config["jitter"]) * identity
        shift, conditional_covariance = gp.condition(
            observed_covariance,
            gp.anisotropic_matern32(config["x_new"], config["x"], amplitude_value, lengths),
            gp.anisotropic_matern32(config["x_new"], config["x_new"], amplitude_value, lengths),
            config["y"] - mean_value,
        )
        conditional_covariance = (conditional_covariance + conditional_covariance.T) / 2.0
        conditional_covariance += config["jitter"] * np.eye(len(config["x_new"]))
        prediction_draws[index] = rng.multivariate_normal(
            mean_value + shift, conditional_covariance, check_valid="raise"
        )
    posterior_finite = posterior_finite and bool(np.isfinite(prediction_draws).all())
    complete = prior_finite and posterior_finite and convergence_passes(config, diagnostics)
    predictions = []
    for index, prediction_id in enumerate(config["prediction_ids"]):
        predictions.append(
            {
                "prediction_id": prediction_id,
                "coordinates_um": config["x_new"][index].tolist(),
                **summary(prediction_draws[:, index]),
            }
        )
    return {
        "format": "marklab.pymc_anisotropic_gp3d_worker_result",
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
                and np.all(arrays["length_scale_x_um"] >= 0.0)
                and np.all(arrays["length_scale_y_um"] >= 0.0)
                and np.all(arrays["length_scale_z_um"] >= 0.0)
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
