#!/usr/bin/env python3
"""PyMC worker for an exact one-dimensional spatially varying coefficient model."""

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

from marklab_worker_support import areal, pymc_nuts, versions
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


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "model",
            "observations",
            "centering_basis",
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
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )
    model = obj(
        request["model"],
        {
            "format",
            "version",
            "family",
            "kernel",
            "coordinate_unit",
            "field_constraint",
            "global_predictor_name",
            "spatial_predictor_name",
            "intercept_prior_mean",
            "intercept_prior_sd",
            "coefficient_prior_sd",
            "amplitude_prior_sd",
            "length_scale_prior_sd_um",
            "known_noise_sd",
            "jitter",
            "backend_capability",
            "maturity",
        },
        "model",
    )
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    integer(model["version"], "model.version", 1, 1)
    exact(model["family"], "gaussian_spatially_varying_coefficient", "model.family")
    exact(model["kernel"], "matern_3_2", "model.kernel")
    exact(model["coordinate_unit"], "micrometre", "model.unit")
    exact(
        model["field_constraint"],
        "exact_sum_to_zero_orthonormal_basis",
        "model.constraint",
    )
    exact(model["backend_capability"], "nuts", "model.capability")
    exact(model["maturity"], "experimental", "model.maturity")
    global_name = model["global_predictor_name"]
    spatial_name = model["spatial_predictor_name"]
    if (
        not isinstance(global_name, str)
        or not global_name
        or global_name.strip() != global_name
        or not isinstance(spatial_name, str)
        or not spatial_name
        or spatial_name.strip() != spatial_name
        or global_name == spatial_name
    ):
        raise ContractError("predictor names are invalid")
    intercept_prior_mean = number(model["intercept_prior_mean"], "model.intercept mean")
    intercept_prior_sd = number(model["intercept_prior_sd"], "model.intercept sd")
    coefficient_prior_sd = number(model["coefficient_prior_sd"], "model.coefficient sd")
    amplitude_prior_sd = number(model["amplitude_prior_sd"], "model.amplitude sd")
    length_prior_sd = number(model["length_scale_prior_sd_um"], "model.length sd")
    known_noise_sd = number(model["known_noise_sd"], "model.noise")
    jitter = number(model["jitter"], "model.jitter")
    if (
        min(
            intercept_prior_sd,
            coefficient_prior_sd,
            amplitude_prior_sd,
            length_prior_sd,
            known_noise_sd,
            jitter,
        )
        <= 0.0
    ):
        raise ContractError("all spatial coefficient scales must be positive")

    observations = request["observations"]
    if not isinstance(observations, list) or not 8 <= len(observations) <= 64:
        raise ContractError("observations must contain 8-64 entries")
    ids: list[str] = []
    x_values: list[float] = []
    outcome: list[float] = []
    global_predictor: list[float] = []
    spatial_predictor: list[float] = []
    for index, raw in enumerate(observations):
        observation = obj(
            raw,
            {"coordinate_id", "x_um", "outcome", "global_predictor", "spatial_predictor"},
            f"observations[{index}]",
        )
        coordinate_id = observation["coordinate_id"]
        if (
            not isinstance(coordinate_id, str)
            or not coordinate_id
            or coordinate_id.strip() != coordinate_id
            or coordinate_id in ids
        ):
            raise ContractError("coordinate ID is invalid or repeated")
        x_um = number(observation["x_um"], "observation.x_um")
        if x_values and x_um <= x_values[-1]:
            raise ContractError("coordinates must be strictly increasing")
        ids.append(coordinate_id)
        x_values.append(x_um)
        outcome.append(number(observation["outcome"], "observation.outcome"))
        global_predictor.append(
            number(observation["global_predictor"], "observation.global_predictor")
        )
        spatial_predictor.append(
            number(observation["spatial_predictor"], "observation.spatial_predictor")
        )
    dimension = len(ids)
    fixed_design = np.column_stack([np.ones(dimension), global_predictor, spatial_predictor])
    if np.linalg.matrix_rank(fixed_design) != 3:
        raise ContractError("fixed design including intercept must be full rank")
    basis_raw = request["centering_basis"]
    if not isinstance(basis_raw, list) or len(basis_raw) != dimension * (dimension - 1):
        raise ContractError("centering basis dimensions mismatch")
    basis = np.asarray(
        [number(value, "centering_basis[]") for value in basis_raw], dtype=np.float64
    ).reshape(dimension, dimension - 1)
    if (
        np.abs(basis.sum(axis=0)).max() > 1e-12
        or np.abs(basis.T @ basis - np.eye(dimension - 1)).max() > 1e-12
    ):
        raise ContractError("centering basis is not orthonormal and sum-zero")

    sampling = nuts_sampling(request["sampling"], "sampling")
    chains, draws = sampling["chains"], sampling["draws"]
    output_limit = areal.dense_resources(
        request["resources"], "resources", dimension, chains * (sampling["tune"] + draws)
    )
    policy = nuts_policy(
        request["diagnostic_policy"],
        "policy",
        prior_draw_range=(100, 10_000),
        count_ceiling=chains * draws,
        check_thresholds=False,
    )
    return {
        **policy,
        **sampling,
        "output_limit": output_limit,
        "intercept_prior_mean": intercept_prior_mean,
        "intercept_prior_sd": intercept_prior_sd,
        "coefficient_prior_sd": coefficient_prior_sd,
        "amplitude_prior_sd": amplitude_prior_sd,
        "length_prior_sd": length_prior_sd,
        "known_noise_sd": known_noise_sd,
        "jitter": jitter,
        "ids": ids,
        "x": np.asarray(x_values),
        "outcome": np.asarray(outcome),
        "global": np.asarray(global_predictor),
        "spatial": np.asarray(spatial_predictor),
        "basis": basis,
    }


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    return derive_seed("marklab-pymc-svc-v1", seed, purpose, index)


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    """Fit the Gaussian spatially varying coefficient model by NUTS.

    Likelihood: ``y_i ~ Normal(alpha + gamma g_i + s_i beta(x_i), known_noise_sd)`` for the
    global predictor ``g`` and the spatial predictor ``s``. The coefficient field is
    ``beta(x) = beta_0 + d(x)``, where the deviation ``d`` is a Matern-3/2 Gaussian process
    (amplitude ``a``, length scale ``l``, plus ``jitter`` on the diagonal) restricted to the
    sum-to-zero subspace: ``d = B L z`` with ``B`` the request's orthonormal sum-zero basis,
    ``L`` the Cholesky factor of ``B' K B``, and ``z ~ Normal(0, 1)`` (non-centred). Priors:
    ``alpha ~ Normal(intercept_prior_mean, intercept_prior_sd)``, ``gamma, beta_0 ~
    Normal(0, coefficient_prior_sd)``, ``a ~ HalfNormal(amplitude_prior_sd)``,
    ``l ~ HalfNormal(length_scale_prior_sd_um)``.
    """
    dimension = len(config["ids"])
    distances = np.abs(config["x"][:, None] - config["x"][None, :])
    with pm.Model(coords={"coordinate": config["ids"]}):
        intercept = pm.Normal(
            "intercept", config["intercept_prior_mean"], config["intercept_prior_sd"]
        )
        global_coefficient = pm.Normal("global_coefficient", 0.0, config["coefficient_prior_sd"])
        spatial_mean_coefficient = pm.Normal(
            "spatial_mean_coefficient", 0.0, config["coefficient_prior_sd"]
        )
        amplitude = pm.HalfNormal("amplitude", config["amplitude_prior_sd"])
        length_scale_um = pm.HalfNormal("length_scale_um", config["length_prior_sd"])
        field_raw = pm.Normal("field_raw", 0.0, 1.0, shape=dimension - 1)
        scaled_distance = math.sqrt(3.0) * distances / length_scale_um
        covariance = amplitude**2 * (1.0 + scaled_distance) * pm.math.exp(-scaled_distance)
        covariance = covariance + config["jitter"] * np.eye(dimension)
        projected_covariance = config["basis"].T @ covariance @ config["basis"]
        projected_lower = pt.linalg.cholesky(projected_covariance)
        deviation = pm.Deterministic(
            "deviation",
            pm.math.dot(config["basis"], pm.math.dot(projected_lower, field_raw)),
            dims="coordinate",
        )
        varying_coefficient = pm.Deterministic(
            "varying_coefficient", spatial_mean_coefficient + deviation, dims="coordinate"
        )
        mean = (
            intercept
            + global_coefficient * config["global"]
            + config["spatial"] * varying_coefficient
        )
        pm.Normal(
            "observed_outcome",
            mean,
            config["known_noise_sd"],
            observed=config["outcome"],
            dims="coordinate",
        )
        prior = pymc_nuts.prior_predictive(config, seed_for)
        posterior = pymc_nuts.nuts(config, seed_for)
        predictive = pymc_nuts.posterior_predictive(posterior, "observed_outcome", config, seed_for)

    names = [
        "intercept",
        "global_coefficient",
        "spatial_mean_coefficient",
        "amplitude",
        "length_scale_um",
        "field_raw",
    ]
    arrays = {
        name: np.asarray(posterior["posterior"][name].values, dtype=np.float64)
        for name in names + ["deviation", "varying_coefficient"]
    }
    predictive_draws = np.asarray(
        predictive["posterior_predictive"]["observed_outcome"].values, dtype=np.float64
    )
    prior_finite = bool(
        all(np.isfinite(prior["prior"][name].values).all() for name in names)
        and np.isfinite(prior["prior_predictive"]["observed_outcome"].values).all()
    )
    posterior_finite = bool(
        all(np.isfinite(value).all() for value in arrays.values())
        and np.isfinite(predictive_draws).all()
    )
    diagnostics = pymc_nuts_diagnostics(posterior, names)
    flat_deviation = arrays["deviation"].reshape(-1, dimension)
    constraints_valid = bool(
        np.all(arrays["amplitude"] > 0.0)
        and np.all(arrays["length_scale_um"] > 0.0)
        and np.abs(flat_deviation.sum(axis=1)).max() <= 1e-8
    )
    complete = (
        prior_finite
        and posterior_finite
        and constraints_valid
        and convergence_passes(config, diagnostics)
    )
    flat_varying = arrays["varying_coefficient"].reshape(-1, dimension)
    field = [
        {
            "coordinate_id": coordinate_id,
            "x_um": float(config["x"][index]),
            "deviation": summary(flat_deviation[:, index]),
            "varying_coefficient": summary(flat_varying[:, index]),
        }
        for index, coordinate_id in enumerate(config["ids"])
    ]
    replicated_means = predictive_draws.mean(axis=-1)
    replicated_sds = predictive_draws.std(axis=-1, ddof=1)
    return {
        "format": "marklab.pymc_spatial_varying_coefficient_worker_result",
        "version": 1,
        "backend": backend_identity("pymc", pm.__version__, lock_sha, worker_sha),
        "request_sha256": request_sha,
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": sampling_record(config),
        "posterior": {
            name: summary(arrays[name])
            for name in [
                "intercept",
                "global_coefficient",
                "spatial_mean_coefficient",
                "amplitude",
                "length_scale_um",
            ]
        },
        "coefficient_field": field,
        "diagnostics": {
            "prior_predictive_finite": prior_finite,
            "posterior_finite": posterior_finite,
            **diagnostics,
            "constraints_valid": constraints_valid,
            "identifiability_checks_passed": True,
        },
        "posterior_predictive": {
            "observed_mean": float(config["outcome"].mean()),
            "observed_sd": float(config["outcome"].std(ddof=1)),
            "replicated_mean_mean": float(replicated_means.mean()),
            "replicated_mean_sd": float(replicated_means.std(ddof=1)),
            "replicated_sd_mean": float(replicated_sds.mean()),
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
