#!/usr/bin/env python3
"""PyMC worker for an exact dense gridded log-Gaussian Cox process."""

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

from marklab_worker_support import versions
from marklab_worker_support.diagnostics import (
    mcse_extremes,
    pymc_sampler_diagnostics,
    rank_convergence,
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
    sha256_digest,
    sha256_hex,
    validate_backend,
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


def close(left: float, right: float) -> bool:
    return math.isclose(left, right, rel_tol=1e-12, abs_tol=1e-12)


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "model",
            "window",
            "grid_x",
            "grid_y",
            "event_count",
            "cell_area_um2",
            "cells",
            "field_covariance",
            "field_cholesky",
            "covariance_sha256",
            "sampling",
            "resources",
            "prediction",
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
        {"construction", "field_parameterization", "backend_capability", "maturity"},
        "model",
    )
    construction = obj(
        model["construction"],
        {
            "family",
            "coordinate_unit",
            "window",
            "cell_rule",
            "intercept_prior",
            "intercept_prior_mean",
            "intercept_prior_sd",
            "coefficient_prior",
            "coefficient_prior_mean",
            "coefficient_prior_sd",
            "latent_field",
            "kernel",
            "field_amplitude",
            "field_length_scale_um",
            "jitter",
            "likelihood",
            "maturity",
        },
        "model.construction",
    )
    exact(construction["family"], "gridded_log_gaussian_cox_process", "model.family")
    exact(construction["coordinate_unit"], "micrometer", "model.unit")
    exact(construction["window"], "half_open_rectangle_complete_cells", "model.window")
    exact(construction["cell_rule"], "center_evaluated_covariate_and_offset", "model.cell_rule")
    exact(construction["intercept_prior"], "normal", "model.intercept_prior")
    exact(construction["coefficient_prior"], "normal", "model.coefficient_prior")
    exact(construction["latent_field"], "zero_mean_dense_gaussian", "model.field")
    exact(construction["kernel"], "matern_3_2_euclidean_2d_cell_midpoints", "model.kernel")
    exact(
        construction["likelihood"],
        "poisson_cell_area_exp_fixed_plus_latent_log_intensity",
        "model.likelihood",
    )
    exact(
        construction["maturity"],
        "experimental_model_construction",
        "model.construction.maturity",
    )
    exact(
        model["field_parameterization"],
        "noncentered_fixed_cholesky_times_standard_normal",
        "model.parameterization",
    )
    exact(model["backend_capability"], "nuts", "model.capability")
    exact(model["maturity"], "experimental", "model.maturity")
    intercept_mean = number(construction["intercept_prior_mean"], "model.intercept_mean")
    intercept_sd = number(construction["intercept_prior_sd"], "model.intercept_sd")
    coefficient_mean = number(construction["coefficient_prior_mean"], "model.coefficient_mean")
    coefficient_sd = number(construction["coefficient_prior_sd"], "model.coefficient_sd")
    amplitude = number(construction["field_amplitude"], "model.amplitude")
    length_scale = number(construction["field_length_scale_um"], "model.length_scale")
    jitter = number(construction["jitter"], "model.jitter")
    if min(intercept_sd, coefficient_sd, amplitude, length_scale, jitter) <= 0.0:
        raise ContractError("LGCP prior and kernel scales must be positive")

    window = obj(request["window"], {"xmin_um", "ymin_um", "xmax_um", "ymax_um"}, "window")
    xmin = number(window["xmin_um"], "window.xmin")
    ymin = number(window["ymin_um"], "window.ymin")
    xmax = number(window["xmax_um"], "window.xmax")
    ymax = number(window["ymax_um"], "window.ymax")
    grid_x = integer(request["grid_x"], "grid_x", 1, 36)
    grid_y = integer(request["grid_y"], "grid_y", 1, 36)
    dimension = grid_x * grid_y
    event_count = integer(request["event_count"], "event_count", 1, 100_000)
    cell_area = number(request["cell_area_um2"], "cell_area")
    if not 4 <= dimension <= 36 or xmin >= xmax or ymin >= ymax or cell_area <= 0.0:
        raise ContractError("LGCP window, grid, or cell area is invalid")
    expected_area = (xmax - xmin) * (ymax - ymin) / dimension
    if not close(cell_area, expected_area):
        raise ContractError("LGCP cell area does not match the window and grid")

    cells = request["cells"]
    if not isinstance(cells, list) or len(cells) != dimension:
        raise ContractError("LGCP cell dimensions mismatch")
    covariate: list[float] = []
    offset: list[float] = []
    counts: list[int] = []
    cell_width = (xmax - xmin) / grid_x
    cell_height = (ymax - ymin) / grid_y
    cell_keys = {
        "ix",
        "iy",
        "midpoint_x_um",
        "midpoint_y_um",
        "area_um2",
        "covariate",
        "offset",
        "count",
    }
    for index, raw in enumerate(cells):
        cell = obj(raw, cell_keys, f"cells[{index}]")
        ix = index % grid_x
        iy = index // grid_x
        exact(cell["ix"], ix, "cell.ix")
        exact(cell["iy"], iy, "cell.iy")
        x = number(cell["midpoint_x_um"], "cell.midpoint_x")
        y = number(cell["midpoint_y_um"], "cell.midpoint_y")
        if (
            not close(x, xmin + (ix + 0.5) * cell_width)
            or not close(y, ymin + (iy + 0.5) * cell_height)
            or not close(number(cell["area_um2"], "cell.area"), cell_area)
        ):
            raise ContractError("LGCP cell geometry mismatch")
        covariate.append(number(cell["covariate"], "cell.covariate"))
        offset.append(number(cell["offset"], "cell.offset"))
        counts.append(integer(cell["count"], "cell.count", 0, 2**63 - 1))
    if sum(counts) != event_count:
        raise ContractError("LGCP cell counts do not sum to the event count")
    covariate_array = np.asarray(covariate, dtype=np.float64)
    covariate_floor = math.sqrt(np.finfo(np.float64).eps) * max(
        1.0, float(np.abs(covariate_array).max())
    )
    if np.ptp(covariate_array) <= covariate_floor:
        raise ContractError("LGCP covariate does not vary materially")

    covariance_raw = request["field_covariance"]
    cholesky_raw = request["field_cholesky"]
    if (
        not isinstance(covariance_raw, list)
        or len(covariance_raw) != dimension * dimension
        or not isinstance(cholesky_raw, list)
        or len(cholesky_raw) != dimension * dimension
    ):
        raise ContractError("LGCP covariance dimensions mismatch")
    covariance = np.asarray(
        [number(value, "field_covariance[]") for value in covariance_raw], dtype=np.float64
    ).reshape(dimension, dimension)
    cholesky = np.asarray(
        [number(value, "field_cholesky[]") for value in cholesky_raw], dtype=np.float64
    ).reshape(dimension, dimension)
    sha256_digest(request["covariance_sha256"], "covariance_sha256")
    if (
        not np.allclose(covariance, covariance.T, rtol=1e-12, atol=1e-12)
        or np.max(np.abs(np.triu(cholesky, k=1))) > 1e-14
        or np.any(np.diag(cholesky) <= 0.0)
        or not np.allclose(cholesky @ cholesky.T, covariance, rtol=1e-10, atol=1e-12)
    ):
        raise ContractError("LGCP covariance or Cholesky factor is invalid")

    sampling = obj(
        request["sampling"],
        {"chains", "tune_per_chain", "draws_per_chain", "target_accept", "seed"},
        "sampling",
    )
    chains = integer(sampling["chains"], "sampling.chains", 2, 8)
    tune = integer(sampling["tune_per_chain"], "sampling.tune", 100, 100_000)
    draws = integer(sampling["draws_per_chain"], "sampling.draws", 100, 100_000)
    target_accept = number(sampling["target_accept"], "sampling.target_accept")
    seed = integer(sampling["seed"], "sampling.seed", 0, 2**64 - 1)
    if not 0.5 <= target_accept < 1.0:
        raise ContractError("target acceptance must be in [0.5,1)")
    resources = obj(
        request["resources"],
        {
            "maximum_cells",
            "maximum_total_iterations",
            "maximum_draw_cell_work",
            "maximum_output_bytes",
            "maximum_predictive_replicates",
            "maximum_predictive_points",
            "timeout_seconds",
        },
        "resources",
    )
    if dimension > integer(resources["maximum_cells"], "resources.cells", 4, 36):
        raise ContractError("LGCP cell limit exceeded")
    maximum_iterations = integer(
        resources["maximum_total_iterations"], "resources.iterations", 1, 400_000
    )
    maximum_work = integer(resources["maximum_draw_cell_work"], "resources.work", 1, 1_000_000)
    maximum_output_bytes = integer(
        resources["maximum_output_bytes"], "resources.output", 1, 16 * MIB
    )
    maximum_replicates = integer(
        resources["maximum_predictive_replicates"], "resources.predictive_replicates", 1, 32
    )
    maximum_points = integer(
        resources["maximum_predictive_points"], "resources.predictive_points", 1, 100_000
    )
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3_600)
    if chains * (tune + draws) > maximum_iterations or chains * draws * dimension > maximum_work:
        raise ContractError("LGCP sampling exceeds resource limits")
    prediction = obj(
        request["prediction"], {"replicates", "seed", "maximum_total_points"}, "prediction"
    )
    predictive_replicates = integer(
        prediction["replicates"], "prediction.replicates", 0, maximum_replicates
    )
    prediction_seed = integer(prediction["seed"], "prediction.seed", 0, 2**64 - 1)
    maximum_total_points = integer(
        prediction["maximum_total_points"], "prediction.maximum_total_points", 1, maximum_points
    )
    policy = obj(
        request["diagnostic_policy"],
        {
            "prior_predictive_draws",
            "maximum_r_hat",
            "minimum_bulk_ess",
            "minimum_tail_ess",
            "minimum_ebfmi",
            "maximum_divergences",
            "maximum_tree_depth_hits",
            "maximum_tree_depth",
        },
        "policy",
    )
    return {
        "intercept_mean": intercept_mean,
        "intercept_sd": intercept_sd,
        "coefficient_mean": coefficient_mean,
        "coefficient_sd": coefficient_sd,
        "covariate": covariate_array,
        "offset": np.asarray(offset, dtype=np.float64),
        "counts": np.asarray(counts, dtype=np.int64),
        "cholesky": cholesky,
        "grid_x": grid_x,
        "grid_y": grid_y,
        "xmin": xmin,
        "ymin": ymin,
        "cell_width": cell_width,
        "cell_height": cell_height,
        "cell_area": cell_area,
        "chains": chains,
        "tune": tune,
        "draws": draws,
        "target_accept": target_accept,
        "seed": seed,
        "predictive_replicates": predictive_replicates,
        "prediction_seed": prediction_seed,
        "maximum_total_points": maximum_total_points,
        "maximum_output_bytes": maximum_output_bytes,
        "prior_draws": integer(policy["prior_predictive_draws"], "policy.prior_draws", 1, 100_000),
        "maximum_r_hat": number(policy["maximum_r_hat"], "policy.rhat"),
        "minimum_bulk_ess": number(policy["minimum_bulk_ess"], "policy.bulk"),
        "minimum_tail_ess": number(policy["minimum_tail_ess"], "policy.tail"),
        "minimum_ebfmi": number(policy["minimum_ebfmi"], "policy.ebfmi"),
        "maximum_divergences": integer(
            policy["maximum_divergences"], "policy.divergences", 0, 2**63 - 1
        ),
        "maximum_tree_depth_hits": integer(
            policy["maximum_tree_depth_hits"], "policy.depth_hits", 0, 2**63 - 1
        ),
        "maximum_tree_depth": integer(policy["maximum_tree_depth"], "policy.depth", 1, 32),
    }


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
