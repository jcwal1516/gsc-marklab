#!/usr/bin/env python3
"""PyMC worker for an exact dense gridded log-Gaussian Cox process."""

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
from marklab_worker_support.gridded_lgcp import validate_request as validate
from marklab_worker_support.posterior import summary
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
    backend_identity,
    derive_seed,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    worker_digests,
    write_result,
)

PRIOR_NAMES = [
    "intercept",
    "coefficient",
    "field_raw",
    "latent_effect",
    "intensity",
    "expected_count",
]
MONITORED = ["intercept", "coefficient", "field_raw"]


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    return derive_seed("marklab-pymc-lgcp-v1", seed, purpose, index)


def posterior_array(posterior: Any, name: str, shape: tuple[int, ...]) -> np.ndarray:
    return np.asarray(posterior["posterior"][name].values, dtype=np.float64).reshape(shape)


def predictive_patterns(
    config: dict[str, Any],
    intercept_draws: np.ndarray,
    coefficient_draws: np.ndarray,
    latent_draws: np.ndarray,
    intensity_draws: np.ndarray,
    expected_draws: np.ndarray,
) -> list[dict[str, Any]]:
    """Simulate posterior-predictive point patterns from evenly spaced posterior draws.

    Each replicate draws Poisson cell counts from one posterior draw of the expected counts and
    places the points uniformly within their cells.
    """
    patterns = []
    total_pattern_points = 0
    total_posterior_draws = config["chains"] * config["draws"]
    for replicate in range(config["predictive_replicates"]):
        flattened = replicate * total_posterior_draws // config["predictive_replicates"]
        chain = flattened // config["draws"]
        draw = flattened % config["draws"]
        rng = np.random.default_rng(
            seed_for(config["prediction_seed"], "posterior-predictive-pattern", replicate)
        )
        cell_counts = rng.poisson(expected_draws[flattened]).astype(np.int64)
        pattern_total = int(cell_counts.sum())
        total_pattern_points += pattern_total
        if total_pattern_points > config["maximum_total_points"]:
            raise ContractError("LGCP posterior predictive realization exceeds point cap")
        points = []
        for cell_index, count in enumerate(cell_counts):
            ix = cell_index % config["grid_x"]
            iy = cell_index // config["grid_x"]
            x = rng.uniform(
                config["xmin"] + ix * config["cell_width"],
                config["xmin"] + (ix + 1) * config["cell_width"],
                size=int(count),
            )
            y = rng.uniform(
                config["ymin"] + iy * config["cell_height"],
                config["ymin"] + (iy + 1) * config["cell_height"],
                size=int(count),
            )
            points.extend(
                {
                    "point_id": f"rep:{replicate}:cell:{cell_index}:point:{local_index}",
                    "ix": ix,
                    "iy": iy,
                    "x_um": float(x[local_index]),
                    "y_um": float(y[local_index]),
                }
                for local_index in range(int(count))
            )
        patterns.append(
            {
                "replicate": replicate,
                "posterior_draw": {
                    "chain": chain,
                    "draw": draw,
                    "intercept": float(intercept_draws[flattened]),
                    "coefficient": float(coefficient_draws[flattened]),
                    "latent_effect": latent_draws[flattened].tolist(),
                    "intensity": intensity_draws[flattened].tolist(),
                    "expected_count": expected_draws[flattened].tolist(),
                },
                "cell_counts": cell_counts.tolist(),
                "total_count": pattern_total,
                "points": points,
            }
        )
    return patterns


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    """Fit the gridded LGCP by NUTS and summarize cells and posterior-predictive patterns.

    Likelihood: ``y_k ~ Poisson(A * exp(eta_k))`` for each grid cell ``k`` of area ``A``, with
    ``eta_k = alpha + beta * x_k + offset_k + u_k``. Priors: ``alpha ~ Normal(intercept_mean,
    intercept_sd)`` and ``beta ~ Normal(coefficient_mean, coefficient_sd)``. The latent field is
    non-centred, ``u = L z`` with ``z ~ Normal(0, I)`` and ``L`` the fixed Cholesky factor of the
    Matern-3/2 cell-midpoint covariance supplied by the request.
    """
    dimension = config["covariate"].size
    chain_seeds = [seed_for(config["seed"], "chain", index) for index in range(config["chains"])]
    with pm.Model():
        intercept = pm.Normal("intercept", config["intercept_mean"], config["intercept_sd"])
        coefficient = pm.Normal("coefficient", config["coefficient_mean"], config["coefficient_sd"])
        field_raw = pm.Normal("field_raw", 0.0, 1.0, shape=dimension)
        latent_effect = pm.Deterministic("latent_effect", pt.dot(config["cholesky"], field_raw))
        eta = intercept + coefficient * config["covariate"] + config["offset"] + latent_effect
        intensity = pm.Deterministic("intensity", pt.exp(eta))
        expected_count = pm.Deterministic("expected_count", config["cell_area"] * intensity)
        pm.Poisson("observed_count", mu=expected_count, observed=config["counts"])
        prior = pm.sample_prior_predictive(
            draws=config["prior_draws"],
            var_names=PRIOR_NAMES,
            random_seed=seed_for(config["seed"], "prior"),
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
        predictive = pm.sample_posterior_predictive(
            posterior,
            var_names=["observed_count"],
            random_seed=seed_for(config["seed"], "predictive"),
            progressbar=False,
        )

    intercept_draws = posterior_array(posterior, "intercept", (-1,))
    coefficient_draws = posterior_array(posterior, "coefficient", (-1,))
    raw_draws = posterior_array(posterior, "field_raw", (-1, dimension))
    latent_draws = posterior_array(posterior, "latent_effect", (-1, dimension))
    intensity_draws = posterior_array(posterior, "intensity", (-1, dimension))
    expected_draws = posterior_array(posterior, "expected_count", (-1, dimension))
    pearson_draws = (config["counts"][None, :] - expected_draws) / np.sqrt(expected_draws)
    replicated = np.asarray(
        predictive["posterior_predictive"]["observed_count"].values, dtype=np.int64
    ).reshape(-1, dimension)
    prior_values = np.concatenate(
        [
            np.asarray(prior["prior"][name].values, dtype=np.float64).reshape(-1)
            for name in PRIOR_NAMES
        ]
    )
    prior_finite = bool(np.isfinite(prior_values).all())
    posterior_finite = bool(
        np.isfinite(intercept_draws).all()
        and np.isfinite(coefficient_draws).all()
        and np.isfinite(raw_draws).all()
        and np.isfinite(latent_draws).all()
        and np.isfinite(intensity_draws).all()
        and np.isfinite(expected_draws).all()
        and np.isfinite(pearson_draws).all()
        and np.isfinite(replicated).all()
    )
    r_hat, bulk, tail = rank_convergence(posterior, MONITORED)
    mcse_mean, mcse_sd = mcse_extremes(posterior, MONITORED)
    ebfmi, divergences, depth_hits = pymc_sampler_diagnostics(posterior["sample_stats"])
    constraints_valid = bool(np.all(intensity_draws > 0.0) and np.all(expected_draws > 0.0))
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
    patterns = []
    if complete:
        patterns = predictive_patterns(
            config,
            intercept_draws,
            coefficient_draws,
            latent_draws,
            intensity_draws,
            expected_draws,
        )
    cells = [
        {
            "ix": index % config["grid_x"],
            "iy": index // config["grid_x"],
            "count": int(config["counts"][index]),
            "latent_effect": summary(latent_draws[:, index], chains=config["chains"]),
            "intensity": summary(intensity_draws[:, index], chains=config["chains"]),
            "expected_count": summary(expected_draws[:, index], chains=config["chains"]),
            "pearson_residual": summary(pearson_draws[:, index], chains=config["chains"]),
        }
        for index in range(dimension)
    ]
    replicated_totals = replicated.sum(axis=1)
    replicated_zeros = (replicated == 0).sum(axis=1)
    return {
        "format": "marklab.pymc_gridded_lgcp_worker_result",
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
            "intercept": summary(intercept_draws, chains=config["chains"]),
            "coefficient": summary(coefficient_draws, chains=config["chains"]),
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
            "observed_total_count": int(config["counts"].sum()),
            "observed_zero_cells": int((config["counts"] == 0).sum()),
            "replicated_total_mean": float(replicated_totals.mean()),
            "replicated_total_sd": float(replicated_totals.std(ddof=1)),
            "replicated_zero_cells_mean": float(replicated_zeros.mean()),
        },
        "patterns": patterns,
    }


def main() -> None:
    require_versions(pymc=pm.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(16 * MIB)
    config = validate(json.loads(raw), lock_sha, worker_sha)
    result = fit(config, sha256_hex(raw), lock_sha, worker_sha)
    write_result(result, max_bytes=config["maximum_output_bytes"])


if __name__ == "__main__":
    run_worker(main, "marklab PyMC gridded LGCP worker failed")
