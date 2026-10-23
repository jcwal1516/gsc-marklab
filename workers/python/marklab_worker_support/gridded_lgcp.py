"""Backend-neutral request contract for the dense gridded LGCP."""

from __future__ import annotations

import math
from typing import Any

import numpy as np

from . import versions
from .protocol import (
    MIB,
    ContractError,
    exact,
    integer,
    number,
    obj,
    sha256_digest,
    validate_backend,
)

SOURCE_WORKER = "marklab_pymc_gridded_lgcp_worker.py"


def close(left: float, right: float) -> bool:
    return math.isclose(left, right, rel_tol=1e-12, abs_tol=1e-12)


def validate_request(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
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
