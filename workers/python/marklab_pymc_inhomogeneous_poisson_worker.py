#!/usr/bin/env python3
"""PyMC worker for a rectangular log-linear Poisson point process."""

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

from marklab_worker_support.diagnostics import (
    mcse_extremes,
    pymc_sampler_diagnostics,
    rank_convergence,
)
from marklab_worker_support.inhomogeneous_poisson import validate_request as validate
from marklab_worker_support.posterior import summary
from marklab_worker_support.protocol import (
    MIB,
    backend_identity,
    derive_seed,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    worker_digests,
    write_result,
)

MONITORED = ["intercept", "coefficient"]


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    return derive_seed("marklab-pymc-ipp-v1", seed, purpose, index)


def sample_model(config: dict[str, Any]) -> dict[str, Any]:
    """Sample the log-linear Poisson process and derive per-cell posterior draws.

    The likelihood is the point-process log density ``sum_i eta(s_i) - A * sum_k
    exp(eta_k)``: the event linear predictors minus the intensity integral over the complete
    grid of equal cell area ``A``, with ``eta = alpha + beta * covariate + offset``. Priors:
    ``alpha ~ Normal(intercept_prior_mean, intercept_prior_sd)`` and ``beta ~
    Normal(coefficient_prior_mean, coefficient_prior_sd)``.
    """
    chain_seeds = [seed_for(config["seed"], "chain", i) for i in range(config["chains"])]
    with pm.Model():
        intercept = pm.Normal(
            "intercept", config["intercept_prior_mean"], config["intercept_prior_sd"]
        )
        coefficient = pm.Normal(
            "coefficient", config["coefficient_prior_mean"], config["coefficient_prior_sd"]
        )
        event_eta = intercept + coefficient * config["event_covariate"] + config["event_offset"]
        grid_eta = intercept + coefficient * config["grid_covariate"] + config["grid_offset"]
        pm.Potential(
            "point_process_likelihood",
            pt.sum(event_eta) - config["cell_area"] * pt.sum(pt.exp(grid_eta)),
        )
        prior = pm.sample_prior_predictive(
            draws=config["prior_draws"], random_seed=seed_for(config["seed"], "prior")
        )
        posterior = pm.sample(
            draws=config["draws"],
            tune=config["tune"],
            chains=config["chains"],
            cores=1,
            blas_cores=1,
            random_seed=chain_seeds,
            target_accept=config["target_accept"],
            nuts_sampler="pymc",
            nuts={"max_treedepth": config["maximum_tree_depth"]},
            progressbar=False,
            quiet=True,
            compute_convergence_checks=False,
        )

    intercept_draws = np.asarray(
        posterior["posterior"]["intercept"].values, dtype=np.float64
    ).reshape(-1)
    coefficient_draws = np.asarray(
        posterior["posterior"]["coefficient"].values, dtype=np.float64
    ).reshape(-1)
    eta = (
        intercept_draws[:, None]
        + coefficient_draws[:, None] * config["grid_covariate"][None, :]
        + config["grid_offset"][None, :]
    )
    intensity = np.exp(eta)
    expected = config["cell_area"] * intensity
    pearson = (config["observed_counts"][None, :] - expected) / np.sqrt(expected)
    return {
        "prior": prior,
        "posterior": posterior,
        "intercept": intercept_draws,
        "coefficient": coefficient_draws,
        "intensity": intensity,
        "expected": expected,
        "pearson": pearson,
    }


def result_from_sample(
    config: dict[str, Any],
    sampled: dict[str, Any],
    request_sha: str,
    lock_sha: str,
    worker_sha: str,
) -> dict[str, Any]:
    prior = sampled["prior"]
    posterior = sampled["posterior"]
    intercept_draws = sampled["intercept"]
    coefficient_draws = sampled["coefficient"]
    intensity = sampled["intensity"]
    expected = sampled["expected"]
    pearson = sampled["pearson"]
    rng = np.random.default_rng(seed_for(config["seed"], "predictive"))
    replicated = rng.poisson(expected)
    prior_intercept = np.asarray(prior["prior"]["intercept"].values, dtype=np.float64).reshape(-1)
    prior_coefficient = np.asarray(prior["prior"]["coefficient"].values, dtype=np.float64).reshape(
        -1
    )
    prior_eta = (
        prior_intercept[:, None]
        + prior_coefficient[:, None] * config["grid_covariate"][None, :]
        + config["grid_offset"][None, :]
    )
    prior_finite = bool(np.isfinite(prior_eta).all() and np.isfinite(np.exp(prior_eta)).all())
    posterior_finite = bool(
        np.isfinite(intercept_draws).all()
        and np.isfinite(coefficient_draws).all()
        and np.isfinite(intensity).all()
        and np.isfinite(expected).all()
        and np.isfinite(pearson).all()
        and np.isfinite(replicated).all()
    )
    r_hat, bulk, tail = rank_convergence(posterior, MONITORED)
    mcse_mean, mcse_sd = mcse_extremes(posterior, MONITORED)
    ebfmi, divergences, depth_hits = pymc_sampler_diagnostics(posterior["sample_stats"])
    constraints_valid = bool(np.all(intensity > 0.0) and np.all(expected > 0.0))
    complete = (
        prior_finite
        and posterior_finite
        and constraints_valid
        and r_hat <= config["maximum_r_hat"]
        and bulk >= config["minimum_bulk_ess"]
        and tail >= config["minimum_tail_ess"]
        and ebfmi >= config["minimum_ebfmi"]
        and divergences <= config["maximum_divergences"]
        and depth_hits <= config["maximum_tree_depth_hits"]
    )
    cells = []
    for index in range(config["grid_covariate"].size):
        ix = index % config["grid_x"]
        iy = index // config["grid_x"]
        cells.append(
            {
                "ix": ix,
                "iy": iy,
                "midpoint_x_um": config["xmin"] + (ix + 0.5) * config["cell_width"],
                "midpoint_y_um": config["ymin"] + (iy + 0.5) * config["cell_height"],
                "covariate": float(config["grid_covariate"][index]),
                "offset": float(config["grid_offset"][index]),
                "observed_count": int(config["observed_counts"][index]),
                "intensity": summary(intensity[:, index]),
                "expected_count": summary(expected[:, index]),
                "pearson_residual": summary(pearson[:, index]),
            }
        )
    replicated_totals = replicated.sum(axis=1)
    replicated_zeros = (replicated == 0).sum(axis=1)
    return {
        "format": "marklab.pymc_inhomogeneous_poisson_worker_result",
        "version": 1,
        "backend": backend_identity("pymc", pm.__version__, lock_sha, worker_sha),
        "request_sha256": request_sha,
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"],
            "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"],
            "completed_draws": config["chains"] * config["draws"],
        },
        "posterior": {
            "intercept": summary(intercept_draws),
            "coefficient": summary(coefficient_draws),
        },
        "cells": cells,
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
            "constraints_valid": constraints_valid,
            "identifiability_checks_passed": True,
        },
        "posterior_predictive": {
            "observed_total_count": int(config["observed_counts"].sum()),
            "observed_zero_cells": int((config["observed_counts"] == 0).sum()),
            "replicated_total_mean": float(replicated_totals.mean()),
            "replicated_total_sd": float(replicated_totals.std(ddof=1)),
            "replicated_zero_cells_mean": float(replicated_zeros.mean()),
        },
    }


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    return result_from_sample(config, sample_model(config), request_sha, lock_sha, worker_sha)


def main() -> None:
    require_versions(pymc=pm.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(16 * MIB)
    config = validate(json.loads(raw), lock_sha, worker_sha)
    result = fit(config, sha256_hex(raw), lock_sha, worker_sha)
    write_result(result, max_bytes=config["maximum_output_bytes"])


if __name__ == "__main__":
    run_worker(main, "marklab PyMC IPP worker failed")
