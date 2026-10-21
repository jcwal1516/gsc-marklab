#!/usr/bin/env python3
"""PyMC worker for Marklab's Gaussian spatial autoregressive (SAR lag or error) fit."""

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

REQUEST_KEYS = {
    "format",
    "version",
    "backend",
    "model",
    "region_ids",
    "weights",
    "weights_digest_sha256",
    "response",
    "design",
    "predictor_names",
    "sampling",
    "resources",
    "diagnostic_policy",
}
MODEL_KEYS = {
    "format",
    "version",
    "family",
    "model_type",
    "interpretation",
    "intercept_prior_mean",
    "intercept_prior_sd",
    "coefficient_prior_sd",
    "rho_bound",
    "sigma_prior_sd",
    "weight_normalization",
    "likelihood_jacobian",
    "backend_capability",
    "maturity",
}


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(request, REQUEST_KEYS, "request")
    exact(request["format"], "marklab.pymc_worker_request", "request.format")
    integer(request["version"], "request.version", 1, 1)
    validate_backend(
        request["backend"],
        "request.backend",
        name="pymc",
        version=versions.PYMC,
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )

    model = obj(request["model"], MODEL_KEYS, "request.model")
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    integer(model["version"], "model.version", 1, 1)
    exact(model["family"], "gaussian_spatial_autoregressive", "model.family")
    model_type = model["model_type"]
    if model_type not in {"lag", "error"}:
        raise ContractError("model type must be lag or error")
    exact(model["interpretation"], "descriptive", "model.interpretation")
    exact(
        model["weight_normalization"],
        "row_standardized_zero_diagonal_island_free",
        "model.weight_normalization",
    )
    exact(
        model["likelihood_jacobian"],
        "log_abs_determinant_i_minus_rho_w",
        "model.likelihood_jacobian",
    )
    exact(model["backend_capability"], "nuts", "model.backend_capability")
    exact(model["maturity"], "experimental", "model.maturity")
    intercept_prior_mean = number(model["intercept_prior_mean"], "model.intercept prior mean")
    intercept_prior_sd = number(model["intercept_prior_sd"], "model.intercept prior sd")
    coefficient_prior_sd = number(model["coefficient_prior_sd"], "model.coefficient prior sd")
    rho_bound = number(model["rho_bound"], "model.rho bound")
    sigma_prior_sd = number(model["sigma_prior_sd"], "model.sigma prior sd")
    if min(intercept_prior_sd, coefficient_prior_sd, sigma_prior_sd) <= 0.0:
        raise ContractError("prior scales must be positive")
    if not 0.0 < rho_bound < 1.0:
        raise ContractError("rho bound must be in (0,1)")

    region_ids = areal.region_ids(request["region_ids"])
    dimension = len(region_ids)
    predictor_names = areal.predictor_names(request["predictor_names"])
    weights = areal.finite_vector(request["weights"], dimension * dimension, "weights").reshape(
        dimension, dimension
    )
    if np.any(weights < 0.0) or np.any(np.diag(weights) != 0.0):
        raise ContractError("weights must be nonnegative with zero diagonal")
    if not np.allclose(weights.sum(axis=1), 1.0, rtol=0.0, atol=1e-12):
        raise ContractError("weights must be row standardized without islands")
    areal.weights_digest(request["weights_digest_sha256"])
    response = areal.finite_vector(request["response"], dimension, "response")
    design = areal.design_matrix(request["design"], dimension, len(predictor_names))

    sampling = nuts_sampling(request["sampling"], "request.sampling")
    chains, draws = sampling["chains"], sampling["draws"]
    output_limit = areal.dense_resources(
        request["resources"], "request.resources", dimension, chains * (sampling["tune"] + draws)
    )
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
        "model_type": model_type,
        "intercept_prior_mean": intercept_prior_mean,
        "intercept_prior_sd": intercept_prior_sd,
        "coefficient_prior_sd": coefficient_prior_sd,
        "rho_bound": rho_bound,
        "sigma_prior_sd": sigma_prior_sd,
        "region_ids": region_ids,
        "predictor_names": predictor_names,
        "weights": weights,
        "response": response,
        "design": design,
        "output_limit": output_limit,
    }


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    return derive_seed("marklab-pymc-sar-v1", seed, purpose, index)


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    """Fit the Gaussian SAR lag or error model by NUTS.

    With row-standardized weights ``W`` and ``A = I - rho W``, the lag model is
    ``A y = alpha + X beta + e`` and the error model is ``A (y - alpha - X beta) = e``, where
    ``e ~ Normal(0, sigma^2 I)``. The likelihood adds the Jacobian ``log|det A|`` to the Gaussian
    log density of ``e``. Priors: ``alpha ~ Normal(intercept_prior_mean, intercept_prior_sd)``,
    ``beta_j ~ Normal(0, coefficient_prior_sd)``, ``rho ~ Uniform(-rho_bound, rho_bound)``,
    ``sigma ~ HalfNormal(sigma_prior_sd)``. Replicates are simulated from the fitted model, and
    lag-model direct and total impacts use the average diagonal and row sum of ``A^-1``.
    """
    dimension = len(config["region_ids"])
    predictors = len(config["predictor_names"])
    identity = np.eye(dimension)
    with pm.Model():
        intercept = pm.Normal(
            "intercept", config["intercept_prior_mean"], config["intercept_prior_sd"]
        )
        coefficient = pm.Normal(
            "coefficient", 0.0, config["coefficient_prior_sd"], shape=predictors
        )
        rho = pm.Uniform("rho", -config["rho_bound"], config["rho_bound"])
        sigma = pm.HalfNormal("sigma", config["sigma_prior_sd"])
        mean = intercept + pt.dot(config["design"], coefficient)
        system = identity - rho * config["weights"]
        if config["model_type"] == "lag":
            residual = pt.dot(system, config["response"]) - mean
        else:
            residual = pt.dot(system, config["response"] - mean)
        log_jacobian = pt.log(pt.abs(pt.linalg.det(system)))
        pm.Potential(
            "sar_likelihood",
            log_jacobian + pm.logp(pm.Normal.dist(0.0, sigma), residual).sum(),
        )
        prior = pymc_nuts.prior_predictive(config, seed_for)
        posterior = pymc_nuts.nuts(config, seed_for)

    arrays = {
        name: np.asarray(posterior["posterior"][name].values, dtype=np.float64)
        for name in ["intercept", "coefficient", "rho", "sigma"]
    }
    flat_intercept = arrays["intercept"].reshape(-1)
    flat_coefficient = arrays["coefficient"].reshape(-1, predictors)
    flat_rho = arrays["rho"].reshape(-1)
    flat_sigma = arrays["sigma"].reshape(-1)
    rng = np.random.default_rng(seed_for(config["seed"], "predictive"))
    replicated = np.empty((flat_rho.size, dimension), dtype=np.float64)
    direct = np.empty((flat_rho.size, predictors), dtype=np.float64)
    total = np.empty((flat_rho.size, predictors), dtype=np.float64)
    determinants = np.empty(flat_rho.size, dtype=np.float64)
    for draw in range(flat_rho.size):
        system_draw = identity - flat_rho[draw] * config["weights"]
        sign, log_determinant = np.linalg.slogdet(system_draw)
        determinants[draw] = log_determinant if sign != 0 else np.nan
        mean_draw = flat_intercept[draw] + config["design"] @ flat_coefficient[draw]
        innovation = rng.normal(0.0, flat_sigma[draw], size=dimension)
        if config["model_type"] == "lag":
            inverse = np.linalg.inv(system_draw)
            replicated[draw] = inverse @ (mean_draw + innovation)
            direct_multiplier = np.trace(inverse) / dimension
            total_multiplier = inverse.sum() / dimension
            direct[draw] = flat_coefficient[draw] * direct_multiplier
            total[draw] = flat_coefficient[draw] * total_multiplier
        else:
            replicated[draw] = mean_draw + np.linalg.solve(system_draw, innovation)

    monitored = ["intercept", "coefficient", "rho", "sigma"]
    prior_rho = np.asarray(prior["prior"]["rho"].values, dtype=np.float64).reshape(-1)
    prior_determinants = np.asarray(
        [np.linalg.slogdet(identity - value * config["weights"])[1] for value in prior_rho]
    )
    prior_finite = bool(
        all(np.isfinite(prior["prior"][name].values).all() for name in monitored)
        and np.isfinite(prior_determinants).all()
    )
    posterior_finite = bool(
        all(np.isfinite(value).all() for value in arrays.values())
        and np.isfinite(replicated).all()
        and np.isfinite(determinants).all()
    )
    diagnostics = pymc_nuts_diagnostics(posterior, monitored)
    constraints_valid = bool(
        np.all(np.abs(flat_rho) < config["rho_bound"])
        and np.all(flat_sigma > 0.0)
        and np.isfinite(determinants).all()
    )
    complete = (
        prior_finite
        and posterior_finite
        and constraints_valid
        and convergence_passes(config, diagnostics)
    )
    coefficient_summaries = [
        {"predictor": name, **summary(flat_coefficient[:, index])}
        for index, name in enumerate(config["predictor_names"])
    ]
    impacts = []
    if config["model_type"] == "lag":
        impacts = [
            {
                "predictor": name,
                "direct": summary(direct[:, index]),
                "indirect": summary(total[:, index] - direct[:, index]),
                "total": summary(total[:, index]),
            }
            for index, name in enumerate(config["predictor_names"])
        ]
    replicated_means = replicated.mean(axis=1)
    replicated_sds = replicated.std(axis=1, ddof=1)
    return {
        "format": "marklab.pymc_sar_worker_result",
        "version": 1,
        "backend": backend_identity("pymc", pm.__version__, lock_sha, worker_sha),
        "request_sha256": request_sha,
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": sampling_record(config),
        "posterior": {
            "intercept": summary(flat_intercept),
            "coefficients": coefficient_summaries,
            "rho": summary(flat_rho),
            "sigma": summary(flat_sigma),
        },
        "impacts": impacts,
        "diagnostics": {
            "prior_predictive_finite": prior_finite,
            "posterior_finite": posterior_finite,
            **diagnostics,
            "constraints_valid": constraints_valid,
            "identifiability_checks_passed": True,
        },
        "posterior_predictive": {
            "observed_mean": float(config["response"].mean()),
            "observed_sd": float(config["response"].std(ddof=1)),
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
