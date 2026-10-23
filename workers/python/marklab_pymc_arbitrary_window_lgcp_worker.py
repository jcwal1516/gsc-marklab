#!/usr/bin/env python3
"""Weighted exact-window adapter for Marklab's pinned dense PyMC LGCP."""

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

import marklab_pymc_gridded_lgcp_worker as gridded
from marklab_worker_support.arbitrary_window import (
    density_statistics,
    statistic_check,
)
from marklab_worker_support.arbitrary_window_lgcp import validate_request as validate
from marklab_worker_support.protocol import (
    MIB,
    file_sha256,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    worker_digests,
    write_result,
)

RESULT_FORMAT = "marklab.bayesian_arbitrary_window_lgcp_fit"


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
