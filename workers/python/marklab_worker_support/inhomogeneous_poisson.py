"""Backend-neutral request contract for the rectangular Poisson point process."""

from __future__ import annotations

import math
from typing import Any

import numpy as np

from . import versions
from .protocol import MIB, ContractError, exact, integer, number, obj, validate_backend

SOURCE_WORKER = "marklab_pymc_inhomogeneous_poisson_worker.py"


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
            "cell_width_um",
            "cell_height_um",
            "cell_area_um2",
            "events",
            "quadrature",
            "observed_cell_counts",
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
            "coordinate_unit",
            "window",
            "quadrature",
            "covariates",
            "intercept_prior_mean",
            "intercept_prior_sd",
            "coefficient_prior_mean",
            "coefficient_prior_sd",
            "likelihood",
            "posterior_predictive",
            "backend_capability",
            "maturity",
        },
        "model",
    )
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    integer(model["version"], "model.version", 1, 1)
    exact(model["family"], "log_linear_inhomogeneous_poisson_process", "model.family")
    exact(model["coordinate_unit"], "micrometer", "model.unit")
    exact(model["window"], "half_open_rectangle", "model.window")
    exact(model["quadrature"], "complete_regular_midpoint_equal_area", "model.quadrature")
    exact(model["covariates"], "one_event_and_grid_covariate_plus_offset", "model.covariates")
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
        or not close(cell_width, (xmax - xmin) / grid_x)
        or not close(cell_height, (ymax - ymin) / grid_y)
        or not close(cell_area, cell_width * cell_height)
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
    covariate_floor = math.sqrt(np.finfo(np.float64).eps) * max(
        1.0, float(np.abs(grid_covariate).max())
    )
    if np.ptp(grid_covariate) <= covariate_floor:
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
            "maximum_events",
            "maximum_quadrature_nodes",
            "maximum_total_iterations",
            "maximum_draw_cell_work",
            "maximum_output_bytes",
            "timeout_seconds",
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
    maximum_work = integer(resources["maximum_draw_cell_work"], "resources.work", 1, 20_000_000)
    if chains * draws * node_count > maximum_work:
        raise ContractError("draw-cell work limit exceeded")
    maximum_output_bytes = integer(
        resources["maximum_output_bytes"], "resources.output", 1, 4 * MIB
    )
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3_600)
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
