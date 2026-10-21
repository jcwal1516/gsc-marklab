#!/usr/bin/env python3
"""PyMC worker for Marklab's version-one normal-mean contract."""

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

from marklab_worker_support import versions
from marklab_worker_support.normal_models import PYMC_REQUEST_FORMAT, normal_mean_model
from marklab_worker_support.nuts_contract import (
    convergence_passes,
    nuts_policy,
    nuts_resources,
    nuts_sampling,
    pymc_nuts_diagnostics,
)
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
from marklab_worker_support.pymc_nuts import sample_nuts

RESULT_FORMAT = "marklab.pymc_worker_result"
CONTRACT_VERSION = 1


def validate_request(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
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
    exact(request["format"], PYMC_REQUEST_FORMAT, "request.format")
    integer(request["version"], "request.version", CONTRACT_VERSION, CONTRACT_VERSION)
    validate_backend(
        request["backend"],
        "request.backend",
        name="pymc",
        version=versions.PYMC,
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )
    prior_mean, prior_sd, known_sigma = normal_mean_model(request["model"], "request.model", "nuts")
    observations = request["observations"]
    if not isinstance(observations, list) or not observations:
        raise ContractError("request.observations must be a non-empty array")
    observations = [
        number(value, f"request.observations[{index}]") for index, value in enumerate(observations)
    ]
    config = nuts_sampling(request["sampling"], "request.sampling")
    config["maximum_output_bytes"] = nuts_resources(
        request["resources"],
        "request.resources",
        observations=len(observations),
        iterations=config["chains"] * (config["tune"] + config["draws"]),
    )
    config.update(
        nuts_policy(
            request["diagnostic_policy"],
            "request.diagnostic_policy",
            count_ceiling=config["chains"] * config["draws"],
        )
    )
    config.update(
        prior_mean=prior_mean,
        prior_sd=prior_sd,
        known_sigma=known_sigma,
        observations=np.asarray(observations, dtype=np.float64),
    )
    return config


def derived_seed(seed: int, purpose: str, index: int = 0) -> int:
    return derive_seed("marklab-pymc-v1", seed, purpose, index)


def fit_normal_mean(
    config: dict[str, Any], request_sha256: str, lock_digest: str, worker_digest: str
) -> dict[str, Any]:
    """Fit a normal mean with known observation SD by NUTS.

    Model: ``mu ~ Normal(prior_mean, prior_sd)`` and ``y_i ~ Normal(mu, known_sigma)``
    independently. The posterior predictive check compares replicated sample means with the
    observed mean.
    Normal scale arguments are standard deviations.
    """
    chain_seeds = [
        derived_seed(config["seed"], "chain", index) for index in range(config["chains"])
    ]
    with pm.Model():
        mu = pm.Normal("mu", mu=config["prior_mean"], sigma=config["prior_sd"])
        pm.Normal("y", mu=mu, sigma=config["known_sigma"], observed=config["observations"])
        prior = pm.sample_prior_predictive(
            draws=config["prior_draws"],
            random_seed=derived_seed(config["seed"], "prior"),
            return_inferencedata=True,
        )
        fit = sample_nuts(config, chain_seeds)
        posterior_predictive = pm.sample_posterior_predictive(
            fit,
            var_names=["y"],
            random_seed=derived_seed(config["seed"], "posterior_predictive"),
            progressbar=False,
            return_inferencedata=True,
        )

    prior_finite = bool(
        np.isfinite(prior["prior"]["mu"].values).all()
        and np.isfinite(prior["prior_predictive"]["y"].values).all()
    )
    posterior_draws = np.asarray(fit["posterior"]["mu"].values, dtype=np.float64)
    predictive_draws = np.asarray(
        posterior_predictive["posterior_predictive"]["y"].values, dtype=np.float64
    )
    posterior_finite = bool(
        np.isfinite(posterior_draws).all() and np.isfinite(predictive_draws).all()
    )
    diagnostics = pymc_nuts_diagnostics(fit, ["mu"])
    replicated_means = predictive_draws.mean(axis=-1)
    observed_mean = float(config["observations"].mean())
    complete = prior_finite and posterior_finite and convergence_passes(config, diagnostics)
    return {
        "format": RESULT_FORMAT,
        "version": CONTRACT_VERSION,
        "backend": backend_identity("pymc", pm.__version__, lock_digest, worker_digest),
        "request_sha256": request_sha256,
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"],
            "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"],
            "completed_draws": config["chains"] * config["draws"],
        },
        "posterior": {
            "parameter": "mu",
            "mean": float(posterior_draws.mean()),
            "sd": float(posterior_draws.std(ddof=1)),
            "interval_lower": float(np.quantile(posterior_draws, 0.025)),
            "interval_upper": float(np.quantile(posterior_draws, 0.975)),
        },
        "diagnostics": {
            "prior_predictive_finite": prior_finite,
            "posterior_finite": posterior_finite,
            **diagnostics,
            "constraints_valid": True,
            "identifiability_checks_passed": True,
        },
        "posterior_predictive": {
            "observed_mean": observed_mean,
            "replicated_mean_mean": float(replicated_means.mean()),
            "replicated_mean_sd": float(replicated_means.std(ddof=1)),
            "probability_replicated_mean_at_least_observed": float(
                np.mean(replicated_means >= observed_mean)
            ),
        },
    }


def main() -> None:
    require_versions(pymc=pm.__version__)
    lock_digest, worker_digest = worker_digests(__file__)
    raw_request = read_request(16 * MIB)
    config = validate_request(json.loads(raw_request), lock_digest, worker_digest)
    result = fit_normal_mean(config, sha256_hex(raw_request), lock_digest, worker_digest)
    write_result(result, max_bytes=config["maximum_output_bytes"])


if __name__ == "__main__":
    run_worker(main, "marklab PyMC worker failed")
