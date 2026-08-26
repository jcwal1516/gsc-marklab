#!/usr/bin/env python3
"""Static PyMC worker for a rectangular log-linear Poisson point process."""

from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path
import sys
from typing import Any

import numpy as np
import pymc as pm
import pytensor.tensor as pt

PYMC_VERSION = "6.3.0"


class ContractError(ValueError):
    pass


def obj(value: Any, keys: set[str], path: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        actual = set(value) if isinstance(value, dict) else set()
        raise ContractError(
            f"{path} fields differ: missing={sorted(keys - actual)}, unknown={sorted(actual - keys)}"
        )
    return value


def number(value: Any, path: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ContractError(f"{path} must be numeric")
    result = float(value)
    if not math.isfinite(result):
        raise ContractError(f"{path} must be finite")
    return result


def integer(value: Any, path: str, low: int, high: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not low <= value <= high:
        raise ContractError(f"{path} must be an integer in [{low}, {high}]")
    return value


def exact(value: Any, expected: Any, path: str) -> None:
    if value != expected:
        raise ContractError(f"{path} must equal {expected!r}")


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format", "version", "backend", "model", "window", "grid_x", "grid_y",
            "cell_width_um", "cell_height_um", "cell_area_um2", "events", "quadrature",
            "observed_cell_counts", "sampling", "resources", "diagnostic_policy",
        },
        "request",
    )
    exact(request["format"], "marklab.pymc_worker_request", "request.format")
    integer(request["version"], "request.version", 1, 1)
    backend = obj(
        request["backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "backend",
    )
    exact(backend["name"], "pymc", "backend.name")
    exact(backend["version"], PYMC_VERSION, "backend.version")
    exact(backend["python_version"], "3.12", "backend.python")
    exact(backend["environment_lock_sha256"], lock_digest, "backend.lock")
    exact(backend["worker_sha256"], worker_digest, "backend.worker")
    model = obj(
        request["model"],
        {
            "format", "version", "family", "coordinate_unit", "window", "quadrature",
            "covariates", "intercept_prior_mean", "intercept_prior_sd",
            "coefficient_prior_mean", "coefficient_prior_sd", "likelihood",
            "posterior_predictive", "backend_capability", "maturity",
        },
        "model",
    )
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    integer(model["version"], "model.version", 1, 1)
    exact(model["family"], "log_linear_inhomogeneous_poisson_process", "model.family")
    exact(model["coordinate_unit"], "micrometer", "model.unit")
    exact(model["window"], "half_open_rectangle", "model.window")
    exact(model["quadrature"], "complete_regular_midpoint_equal_area", "model.quadrature")
    exact(
        model["covariates"],
        "one_event_and_grid_covariate_plus_offset",
        "model.covariates",
    )
    intercept_prior_mean = number(model["intercept_prior_mean"], "model.intercept_mean")
    intercept_prior_sd = number(model["intercept_prior_sd"], "model.intercept_sd")
    coefficient_prior_mean = number(model["coefficient_prior_mean"], "model.coefficient_mean")
    coefficient_prior_sd = number(model["coefficient_prior_sd"], "model.coefficient_sd")
    exact(
        model["likelihood"],
        "event_linear_predictor_sum_minus_complete_grid_integral",
        "model.likelihood",
    )
    exact(
        model["posterior_predictive"],
        "piecewise_constant_grid_poisson_counts",
        "model.predictive",
    )
    exact(model["backend_capability"], "nuts", "model.capability")
    exact(model["maturity"], "experimental", "model.maturity")
    if min(intercept_prior_sd, coefficient_prior_sd) <= 0.0:
        raise ContractError("prior SDs must be positive")

    window = obj(request["window"], {"xmin_um", "ymin_um", "xmax_um", "ymax_um"}, "window")
    xmin = number(window["xmin_um"], "window.xmin")
    ymin = number(window["ymin_um"], "window.ymin")
    xmax = number(window["xmax_um"], "window.xmax")
    ymax = number(window["ymax_um"], "window.ymax")
    grid_x = integer(request["grid_x"], "grid_x", 1, 1_024)
    grid_y = integer(request["grid_y"], "grid_y", 1, 1_024)
    node_count = grid_x * grid_y
    if not 4 <= node_count <= 4_096 or xmin >= xmax or ymin >= ymax:
        raise ContractError("window or fitted grid is invalid")
    cell_width = number(request["cell_width_um"], "cell_width")
    cell_height = number(request["cell_height_um"], "cell_height")
    cell_area = number(request["cell_area_um2"], "cell_area")
    if (
        cell_area <= 0.0
        or not math.isclose(cell_width, (xmax - xmin) / grid_x, rel_tol=1e-12, abs_tol=1e-12)
        or not math.isclose(cell_height, (ymax - ymin) / grid_y, rel_tol=1e-12, abs_tol=1e-12)
        or not math.isclose(cell_area, cell_width * cell_height, rel_tol=1e-12, abs_tol=1e-12)
    ):
        raise ContractError("derived cell geometry mismatch")

    events = request["events"]
    if not isinstance(events, list) or not 20 <= len(events) <= 100_000:
        raise ContractError("events require 20-100000 entries")
    event_covariate = []
    event_offset = []
    prior_id = ""
    for index, raw in enumerate(events):
        event = obj(raw, {"event_id", "x_um", "y_um", "covariate", "offset"}, f"events[{index}]")
        event_id = event["event_id"]
        x = number(event["x_um"], "event.x")
        y = number(event["y_um"], "event.y")
        if (
            not isinstance(event_id, str)
            or not event_id
            or event_id.strip() != event_id
            or event_id <= prior_id
            or not xmin <= x < xmax
            or not ymin <= y < ymax
        ):
            raise ContractError("event identity or window membership is invalid")
        prior_id = event_id
        event_covariate.append(number(event["covariate"], "event.covariate"))
        event_offset.append(number(event["offset"], "event.offset"))

    quadrature = request["quadrature"]
    if not isinstance(quadrature, list) or len(quadrature) != node_count:
        raise ContractError("quadrature dimensions mismatch")
    grid_covariate = []
    grid_offset = []
    for index, raw in enumerate(quadrature):
        node = obj(raw, {"ix", "iy", "covariate", "offset"}, f"quadrature[{index}]")
        exact(node["ix"], index % grid_x, "quadrature.ix")
        exact(node["iy"], index // grid_x, "quadrature.iy")
        grid_covariate.append(number(node["covariate"], "quadrature.covariate"))
        grid_offset.append(number(node["offset"], "quadrature.offset"))
    grid_covariate = np.asarray(grid_covariate, dtype=np.float64)
    if np.ptp(grid_covariate) <= math.sqrt(np.finfo(np.float64).eps) * max(
        1.0, float(np.abs(grid_covariate).max())
    ):
        raise ContractError("quadrature covariate does not vary materially")
    observed_counts = request["observed_cell_counts"]
    if not isinstance(observed_counts, list) or len(observed_counts) != node_count:
        raise ContractError("observed cell count dimensions mismatch")
    observed_counts = np.asarray(
        [integer(value, "observed_cell_counts[]", 0, 2**63 - 1) for value in observed_counts],
        dtype=np.int64,
    )
    if int(observed_counts.sum()) != len(events):
        raise ContractError("observed cell counts do not sum to events")

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
            "maximum_events", "maximum_quadrature_nodes", "maximum_total_iterations",
            "maximum_draw_cell_work", "maximum_output_bytes", "timeout_seconds",
        },
        "resources",
    )
    if len(events) > integer(resources["maximum_events"], "resources.events", 1, 100_000):
        raise ContractError("event limit exceeded")
    if node_count > integer(resources["maximum_quadrature_nodes"], "resources.nodes", 1, 4_096):
        raise ContractError("node limit exceeded")
    maximum_iterations = integer(
        resources["maximum_total_iterations"], "resources.iterations", 1, 400_000
    )
    if chains * (tune + draws) > maximum_iterations:
        raise ContractError("iteration limit exceeded")
    maximum_work = integer(
        resources["maximum_draw_cell_work"], "resources.work", 1, 20_000_000
    )
    if chains * draws * node_count > maximum_work:
        raise ContractError("draw-cell work limit exceeded")
    integer(resources["maximum_output_bytes"], "resources.output", 1, 4 * 1_048_576)
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3_600)
    policy = obj(
        request["diagnostic_policy"],
        {
            "prior_predictive_draws", "maximum_r_hat", "minimum_bulk_ess",
            "minimum_tail_ess", "minimum_ebfmi", "maximum_divergences",
            "maximum_tree_depth_hits", "maximum_tree_depth",
        },
        "policy",
    )
    return {
        "intercept_prior_mean": intercept_prior_mean,
        "intercept_prior_sd": intercept_prior_sd,
        "coefficient_prior_mean": coefficient_prior_mean,
        "coefficient_prior_sd": coefficient_prior_sd,
        "event_covariate": np.asarray(event_covariate, dtype=np.float64),
        "event_offset": np.asarray(event_offset, dtype=np.float64),
        "grid_covariate": grid_covariate,
        "grid_offset": np.asarray(grid_offset, dtype=np.float64),
        "observed_counts": observed_counts,
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
        "prior_draws": integer(policy["prior_predictive_draws"], "policy.prior_draws", 1, 100_000),
        "maximum_r_hat": number(policy["maximum_r_hat"], "policy.rhat"),
        "minimum_bulk_ess": number(policy["minimum_bulk_ess"], "policy.bulk"),
        "minimum_tail_ess": number(policy["minimum_tail_ess"], "policy.tail"),
        "minimum_ebfmi": number(policy["minimum_ebfmi"], "policy.ebfmi"),
        "maximum_divergences": integer(policy["maximum_divergences"], "policy.divergences", 0, 2**63 - 1),
        "maximum_tree_depth_hits": integer(policy["maximum_tree_depth_hits"], "policy.depth_hits", 0, 2**63 - 1),
        "maximum_tree_depth": integer(policy["maximum_tree_depth"], "policy.depth", 1, 32),
    }


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    digest = hashlib.sha256(f"marklab-pymc-ipp-v1\0{seed}\0{purpose}\0{index}".encode()).digest()
    return int.from_bytes(digest[:4], "little")


def summary(draws: np.ndarray) -> dict[str, float]:
    return {
        "mean": float(draws.mean()),
        "sd": float(draws.std(ddof=1)),
        "interval_lower": float(np.quantile(draws, 0.025)),
        "interval_upper": float(np.quantile(draws, 0.975)),
    }


def tree_values(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
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
            random_seed=[seed_for(config["seed"], "chain", i) for i in range(config["chains"])],
            target_accept=config["target_accept"],
            nuts_sampler="pymc",
            nuts={"max_treedepth": config["maximum_tree_depth"]},
            progressbar=False,
            quiet=True,
            compute_convergence_checks=False,
        )

    intercept_draws = np.asarray(posterior["posterior"]["intercept"].values, dtype=np.float64).reshape(-1)
    coefficient_draws = np.asarray(posterior["posterior"]["coefficient"].values, dtype=np.float64).reshape(-1)
    eta = (
        intercept_draws[:, None]
        + coefficient_draws[:, None] * config["grid_covariate"][None, :]
        + config["grid_offset"][None, :]
    )
    intensity = np.exp(eta)
    expected = config["cell_area"] * intensity
    pearson = (config["observed_counts"][None, :] - expected) / np.sqrt(expected)
    rng = np.random.default_rng(seed_for(config["seed"], "predictive"))
    replicated = rng.poisson(expected)
    prior_intercept = np.asarray(prior["prior"]["intercept"].values, dtype=np.float64).reshape(-1)
    prior_coefficient = np.asarray(prior["prior"]["coefficient"].values, dtype=np.float64).reshape(-1)
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
    monitored = ["intercept", "coefficient"]
    r_hat = float(tree_values(pm.stats.rhat(posterior, var_names=monitored, method="rank"), monitored).max())
    bulk = float(tree_values(pm.stats.ess(posterior, var_names=monitored, method="bulk"), monitored).min())
    tail = float(tree_values(pm.stats.ess(posterior, var_names=monitored, method="tail"), monitored).min())
    mcse_mean = float(tree_values(pm.stats.mcse(posterior, var_names=monitored, method="mean"), monitored).max())
    mcse_sd = float(tree_values(pm.stats.mcse(posterior, var_names=monitored, method="sd"), monitored).max())
    energy = np.asarray(posterior["sample_stats"]["energy"].values, dtype=np.float64)
    ebfmi = float(np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1)))
    divergences = int(np.asarray(posterior["sample_stats"]["divergences"].values).sum())
    depth_hits = int(np.asarray(posterior["sample_stats"]["reached_max_treedepth"].values).sum())
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
        "backend": {
            "name": "pymc",
            "version": pm.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha,
            "worker_sha256": worker_sha,
        },
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


def main() -> None:
    if pm.__version__ != PYMC_VERSION or sys.version_info[:2] != (3, 12):
        raise ContractError("PyMC or Python version drift")
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    raw = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if len(raw) > 16 * 1024 * 1024:
        raise ContractError("request exceeds 16 MiB")
    request_sha = hashlib.sha256(raw).hexdigest()
    result = fit(validate(json.loads(raw), lock_sha, worker_sha), request_sha, lock_sha, worker_sha)
    sys.stdout.write(json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")))
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"marklab PyMC IPP worker failed: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from error
