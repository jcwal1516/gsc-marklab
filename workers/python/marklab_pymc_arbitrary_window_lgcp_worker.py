#!/usr/bin/env python3
"""Weighted exact-window adapter for Marklab's pinned dense PyMC LGCP."""

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

import marklab_pymc_gridded_lgcp_worker as gridded
from marklab_worker_support import versions
from marklab_worker_support.arbitrary_window import (
    density_statistics,
    neighbor_pairs,
    statistic_check,
    validate_input_identity,
)
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
    exact,
    file_sha256,
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

REQUEST_FORMAT = "marklab.pymc_arbitrary_window_lgcp_request"
RESULT_FORMAT = "marklab.bayesian_arbitrary_window_lgcp_fit"


def close(left: float, right: float) -> bool:
    return math.isclose(left, right, rel_tol=1e-12, abs_tol=1e-12)


def matern32_covariance(
    nodes: list[dict[str, Any]], amplitude: float, length_scale: float, jitter: float
) -> np.ndarray:
    """Matern-3/2 covariance between node representatives, plus ``jitter`` on the diagonal."""
    dimension = len(nodes)
    covariance = np.empty((dimension, dimension), dtype=np.float64)
    for row in range(dimension):
        for column in range(dimension):
            distance = math.hypot(
                nodes[row]["x_um"] - nodes[column]["x_um"],
                nodes[row]["y_um"] - nodes[column]["y_um"],
            )
            scaled = math.sqrt(3.0) * distance / length_scale
            covariance[row, column] = amplitude**2 * (1.0 + scaled) * math.exp(-scaled)
            if row == column:
                covariance[row, column] += jitter
    return covariance


def validate(
    request: Any, lock_digest: str, worker_digest: str, source_worker_digest: str
) -> tuple[
    dict[str, Any],
    dict[str, Any],
    list[dict[str, Any]],
    np.ndarray,
    list[tuple[int, int]],
]:
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "source_request_sha256",
            "source_request",
            "input",
            "window",
            "nodes",
            "physical_covariance",
            "physical_cholesky",
            "neighbor_pairs",
            "neighbor_radius_um",
            "maximum_neighbor_pairs",
            "maximum_events",
            "maximum_quadrature_nodes",
            "covariance_sha256",
            "maximum_draw_node_work",
            "dense_factorization_work_units",
        },
        "request",
    )
    exact(request["format"], REQUEST_FORMAT, "request.format")
    exact(request["version"], 1, "request.version")
    validate_backend(
        request["backend"],
        "backend",
        name="pymc",
        version=versions.PYMC,
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )
    sha256_digest(request["source_request_sha256"], "source_request_sha256")
    validate_input_identity(request["input"])
    obj(
        request["window"],
        {
            "area_um2",
            "perimeter_um",
            "bounds_um",
            "component_count",
            "hole_count",
            "ring_count",
            "vertex_count",
            "logical_digest",
        },
        "window",
    )
    config = gridded.validate(request["source_request"], lock_digest, source_worker_digest)
    nodes = request["nodes"]
    if not isinstance(nodes, list) or len(nodes) != config["covariate"].size:
        raise ContractError("physical node dimensions differ")
    parsed = []
    previous_id = ""
    for index, raw in enumerate(nodes):
        node = obj(
            raw,
            {"node_id", "x_um", "y_um", "weight_um2", "covariate", "offset", "count"},
            f"nodes[{index}]",
        )
        node_id = node["node_id"]
        if not isinstance(node_id, str) or not node_id or node_id <= previous_id:
            raise ContractError("physical node identities are not canonical")
        parsed_node = {
            "node_id": node_id,
            "x_um": number(node["x_um"], "node.x_um"),
            "y_um": number(node["y_um"], "node.y_um"),
            "weight_um2": number(node["weight_um2"], "node.weight_um2"),
            "covariate": number(node["covariate"], "node.covariate"),
            "offset": number(node["offset"], "node.offset"),
            "count": integer(node["count"], "node.count", 0, 2**63 - 1),
        }
        if parsed_node["weight_um2"] <= 0.0:
            raise ContractError("physical node weight must be positive")
        backend_offset = parsed_node["offset"] + math.log(parsed_node["weight_um2"])
        if (
            not close(config["covariate"][index], parsed_node["covariate"])
            or not close(config["offset"][index], backend_offset)
            or config["counts"][index] != parsed_node["count"]
        ):
            raise ContractError("physical node differs from its algebraic backend cell")
        parsed.append(parsed_node)
        previous_id = node_id
    dimension = len(parsed)
    covariance = np.asarray(
        [number(value, "physical_covariance[]") for value in request["physical_covariance"]],
        dtype=np.float64,
    ).reshape(dimension, dimension)
    cholesky = np.asarray(
        [number(value, "physical_cholesky[]") for value in request["physical_cholesky"]],
        dtype=np.float64,
    ).reshape(dimension, dimension)
    construction = request["source_request"]["model"]["construction"]
    amplitude = number(construction["field_amplitude"], "model.field_amplitude")
    length_scale = number(construction["field_length_scale_um"], "model.field_length_scale")
    jitter = number(construction["jitter"], "model.jitter")
    expected = matern32_covariance(parsed, amplitude, length_scale, jitter)
    if (
        not np.allclose(covariance, expected, rtol=1e-12, atol=1e-12)
        or np.max(np.abs(np.triu(cholesky, k=1))) > 1e-14
        or np.any(np.diag(cholesky) <= 0.0)
        or not np.allclose(cholesky @ cholesky.T, covariance, rtol=1e-10, atol=1e-12)
    ):
        raise ContractError("physical covariance or Cholesky factor differs")
    pairs = neighbor_pairs(request["neighbor_pairs"], dimension)
    maximum_pairs = integer(
        request["maximum_neighbor_pairs"], "maximum_neighbor_pairs", 1, 1_000_000
    )
    if len(pairs) > maximum_pairs:
        raise ContractError("physical neighbor-pair ceiling exceeded")
    maximum_nodes = integer(request["maximum_quadrature_nodes"], "maximum_quadrature_nodes", 4, 36)
    if len(nodes) > maximum_nodes or sum(node["count"] for node in parsed) > integer(
        request["maximum_events"], "maximum_events", 1, 100_000
    ):
        raise ContractError("physical event or node ceiling exceeded")
    if number(request["neighbor_radius_um"], "neighbor_radius_um") <= 0.0:
        raise ContractError("physical neighbor radius must be positive")
    sha256_digest(request["covariance_sha256"], "covariance_sha256")
    maximum_work = integer(
        request["maximum_draw_node_work"], "maximum_draw_node_work", 1, 1_000_000
    )
    actual_work = config["chains"] * config["draws"] * dimension
    if actual_work > maximum_work:
        raise ContractError("physical LGCP draw-node work exceeds maximum")
    exact(
        request["dense_factorization_work_units"],
        dimension**3,
        "dense_factorization_work_units",
    )
    config["cholesky"] = cholesky
    return request, config, parsed, covariance, pairs


def divide_summary(summary: dict[str, float], divisor: float) -> dict[str, float]:
    return {name: value / divisor for name, value in summary.items()}


def spatial_posterior_predictive(
    request: dict[str, Any],
    config: dict[str, Any],
    nodes: list[dict[str, Any]],
    pairs: list[tuple[int, int]],
    patterns: list[dict[str, Any]],
) -> dict[str, Any]:
    weights = np.asarray([node["weight_um2"] for node in nodes], dtype=np.float64)
    observed = np.asarray([node["count"] for node in nodes], dtype=np.int64)
    replicated = np.asarray([pattern["cell_counts"] for pattern in patterns], dtype=np.int64)
    observed_variance, observed_contrast = density_statistics(observed, weights, pairs)
    replicated_variance, replicated_contrast = density_statistics(replicated, weights, pairs)
    return {
        "replicate_count": len(patterns),
        "prediction_seed": config["prediction_seed"],
        "neighbor_radius_um": request["neighbor_radius_um"],
        "neighbor_pair_count": len(pairs),
        "node_density_variance": statistic_check(float(observed_variance), replicated_variance),
        "neighbor_density_mean_absolute_difference": statistic_check(
            float(observed_contrast), replicated_contrast
        ),
    }


def fit(
    request: dict[str, Any],
    config: dict[str, Any],
    nodes: list[dict[str, Any]],
    pairs: list[tuple[int, int]],
    request_sha256: str,
    lock_digest: str,
    worker_digest: str,
) -> dict[str, Any]:
    """Fit the dense gridded LGCP on the weighted quadrature nodes of an exact window.

    Each node is passed to the gridded worker as a backend cell whose offset includes the log of
    the node weight. The gridded model (Poisson node counts, log intensity ``alpha + beta * x +
    offset + u`` with normal priors on ``alpha`` and ``beta`` and a non-centred latent field
    ``u = L z``) is fitted unchanged, with ``L`` the request's physical Matern-3/2 Cholesky
    factor.
    """
    result = gridded.fit(config, request_sha256, lock_digest, worker_digest)
    physical_nodes = []
    for node, cell in zip(nodes, result["cells"], strict=True):
        physical_nodes.append(
            {
                **node,
                "latent_effect": cell["latent_effect"],
                "intensity_per_um2": divide_summary(cell["intensity"], node["weight_um2"]),
                "expected_count": cell["expected_count"],
                "pearson_residual": cell["pearson_residual"],
            }
        )
    spatial_ppc = None
    if result["patterns"]:
        spatial_ppc = spatial_posterior_predictive(
            request, config, nodes, pairs, result["patterns"]
        )
    construction = request["source_request"]["model"]["construction"]
    draws = config["chains"] * config["draws"]
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": result["backend"],
        "source_backend": request["source_request"]["backend"],
        "model": {
            "family": "log_gaussian_cox_process",
            "coordinate_unit": "micrometer",
            "window": "exact_multipolygon",
            "quadrature": "positive_weighted_area_partition",
            "covariates": "one_fixed_node_covariate_plus_offset",
            "latent_field": "zero_mean_dense_gaussian",
            "kernel": "fixed_matern_3_2_on_physical_quadrature_representatives",
            "field_amplitude": construction["field_amplitude"],
            "field_length_scale_um": construction["field_length_scale_um"],
            "jitter": construction["jitter"],
            "intercept_prior_mean": construction["intercept_prior_mean"],
            "intercept_prior_sd": construction["intercept_prior_sd"],
            "coefficient_prior_mean": construction["coefficient_prior_mean"],
            "coefficient_prior_sd": construction["coefficient_prior_sd"],
            "likelihood": "weighted_node_poisson_log_gaussian_intensity",
            "backend_adapter": (
                "equal_area_synthetic_grid_with_log_weight_offsets_and_physical_covariance_override"
            ),
        },
        "input": request["input"],
        "window": request["window"],
        "observed_event_count": sum(node["count"] for node in nodes),
        "quadrature_node_count": len(nodes),
        "quadrature_weight_um2": sum(node["weight_um2"] for node in nodes),
        "sampling": result["sampling"],
        "fit_state": result["fit_state"],
        "posterior": result["posterior"],
        "nodes": physical_nodes,
        "diagnostics": result["diagnostics"],
        "posterior_predictive": result["posterior_predictive"],
        "spatial_posterior_predictive": spatial_ppc,
        "seed": config["seed"],
        "request_sha256": request_sha256,
        "source_request_sha256": request["source_request_sha256"],
        "covariance_sha256": request["covariance_sha256"],
        "draw_node_work": draws * len(nodes),
        "maximum_draw_node_work": request["maximum_draw_node_work"],
        "maximum_events": request["maximum_events"],
        "maximum_quadrature_nodes": request["maximum_quadrature_nodes"],
        "maximum_predictive_points": config["maximum_total_points"],
        "maximum_neighbor_pairs": request["maximum_neighbor_pairs"],
        "dense_covariance_elements": len(nodes) ** 2,
        "dense_factorization_work_units": request["dense_factorization_work_units"],
        "statistical_unit": "one_observed_point_pattern",
        "null_model": "none_descriptive_latent_intensity_model",
        "assumptions": [
            "event_to_weighted_node_membership_is_exact_and_complete",
            "quadrature_weights_partition_the_exact_window_area",
            "matern_amplitude_and_length_scale_are_fixed_not_inferred",
            "weighted_nodes_are_a_recorded_coarse_window_approximation",
            "latent_intensity_is_not_point_interaction_or_attraction",
        ],
        "finite_result_policy": "nonconverged_is_diagnostic_only_reject_non_finite",
        "claim_status": (
            "experimental_single_pattern_latent_field"
            if result["fit_state"] == "complete"
            else "diagnostic_only_nonconverged"
        ),
    }


def main() -> None:
    require_versions(pymc=pm.__version__)
    lock_digest, worker_digest = worker_digests(__file__)
    source_worker_digest = file_sha256(gridded.__file__)
    raw = read_request(16 * MIB)
    request, config, nodes, _, pairs = validate(
        json.loads(raw), lock_digest, worker_digest, source_worker_digest
    )
    result = fit(request, config, nodes, pairs, sha256_hex(raw), lock_digest, worker_digest)
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "marklab PyMC arbitrary-window LGCP failed")
