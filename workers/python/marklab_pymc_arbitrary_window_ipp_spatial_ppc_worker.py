#!/usr/bin/env python3
"""Spatial posterior-predictive check for the weighted exact-window IPP."""

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

import marklab_pymc_inhomogeneous_poisson_worker as ipp
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
    backend_identity,
    derive_seed,
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

REQUEST_FORMAT = "marklab.pymc_arbitrary_window_ipp_spatial_ppc_request"
RESULT_FORMAT = "marklab.arbitrary_window_ipp_spatial_ppc"


def validate(
    request: Any, lock_digest: str, worker_digest: str, source_worker_digest: str
) -> tuple[dict[str, Any], dict[str, Any], np.ndarray, np.ndarray, list[tuple[int, int]]]:
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "source_request_sha256",
            "source_request",
            "input",
            "observed_node_counts",
            "node_weights_um2",
            "neighbor_pairs",
            "neighbor_radius_um",
            "prediction_seed",
            "maximum_predictive_work",
            "maximum_neighbor_pairs",
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
    config = ipp.validate(request["source_request"], lock_digest, source_worker_digest)
    node_count = config["grid_covariate"].size
    observed = request["observed_node_counts"]
    if not isinstance(observed, list) or len(observed) != node_count:
        raise ContractError("observed node-count dimensions differ")
    observed_counts = np.asarray(
        [integer(value, "observed_node_counts[]", 0, 2**63 - 1) for value in observed],
        dtype=np.int64,
    )
    if int(observed_counts.sum()) != config["event_covariate"].size:
        raise ContractError("observed node counts do not sum to events")
    weights = request["node_weights_um2"]
    if not isinstance(weights, list) or len(weights) != node_count:
        raise ContractError("node-weight dimensions differ")
    node_weights = np.asarray(
        [number(value, "node_weights_um2[]") for value in weights], dtype=np.float64
    )
    if np.any(node_weights <= 0.0):
        raise ContractError("node weights must be positive")
    pairs = neighbor_pairs(request["neighbor_pairs"], node_count)
    if number(request["neighbor_radius_um"], "neighbor_radius_um") <= 0.0:
        raise ContractError("neighbor_radius_um must be positive")
    integer(request["prediction_seed"], "prediction_seed", 0, 2**64 - 1)
    maximum_work = integer(
        request["maximum_predictive_work"], "maximum_predictive_work", 1, 100_000_000
    )
    maximum_pairs = integer(
        request["maximum_neighbor_pairs"], "maximum_neighbor_pairs", 1, 1_000_000
    )
    if len(pairs) > maximum_pairs:
        raise ContractError("spatial PPC neighbor-pair ceiling exceeded")
    work = config["chains"] * config["draws"] * (node_count + len(pairs))
    if work > maximum_work:
        raise ContractError("spatial PPC work exceeds the declared maximum")
    config["observed_counts"] = observed_counts
    return request, config, observed_counts, node_weights, pairs


def seed_for(seed: int) -> int:
    return derive_seed("marklab-pymc-arbitrary-window-ipp-spatial-ppc-v1", seed)


def run_ppc(
    request: dict[str, Any],
    config: dict[str, Any],
    observed_counts: np.ndarray,
    weights: np.ndarray,
    pairs: list[tuple[int, int]],
    request_sha256: str,
    lock_digest: str,
    worker_digest: str,
    source_worker_digest: str,
) -> dict[str, Any]:
    """Fit the source IPP and compare observed and replicated node-density statistics.

    Replicated node counts are Poisson draws from every posterior draw of the expected node
    counts; the statistics are the node-density variance and the mean absolute density
    difference over the fixed neighbor pairs.
    """
    sampled = ipp.sample_model(config)
    source_result = ipp.result_from_sample(
        config,
        sampled,
        request["source_request_sha256"],
        lock_digest,
        source_worker_digest,
    )
    expected = sampled["expected"]
    rng = np.random.default_rng(seed_for(request["prediction_seed"]))
    replicated = rng.poisson(expected)
    observed_variance, observed_contrast = density_statistics(observed_counts, weights, pairs)
    replicated_variance, replicated_contrast = density_statistics(replicated, weights, pairs)
    values = np.concatenate(
        [
            np.asarray([observed_variance, observed_contrast]),
            replicated_variance,
            replicated_contrast,
        ]
    )
    if not np.isfinite(values).all():
        raise ContractError("spatial PPC produced a non-finite result")
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": backend_identity("pymc", pm.__version__, lock_digest, worker_digest),
        "input": request["input"],
        "request_sha256": request_sha256,
        "source_request_sha256": request["source_request_sha256"],
        "fit_state": source_result["fit_state"],
        "sampling": source_result["sampling"],
        "diagnostics": source_result["diagnostics"],
        "observed_event_count": int(observed_counts.sum()),
        "quadrature_node_count": int(observed_counts.size),
        "neighbor_radius_um": request["neighbor_radius_um"],
        "neighbor_pair_count": len(pairs),
        "posterior_predictive_replicates": int(replicated.shape[0]),
        "prediction_seed": request["prediction_seed"],
        "total_predictive_work": int(replicated.shape[0] * (observed_counts.size + len(pairs))),
        "maximum_predictive_work": request["maximum_predictive_work"],
        "maximum_neighbor_pairs": request["maximum_neighbor_pairs"],
        "summaries": {
            "node_density_variance": statistic_check(float(observed_variance), replicated_variance),
            "neighbor_density_mean_absolute_difference": statistic_check(
                float(observed_contrast), replicated_contrast
            ),
        },
        "statistical_unit": "one_observed_point_pattern",
        "null_model": (
            "fitted_log_linear_inhomogeneous_poisson_process_conditional_on_fixed_covariate"
        ),
        "assumptions": [
            "event_to_quadrature_membership_is_exact_and_complete",
            "quadrature_weights_partition_the_exact_window_area",
            "neighbor_pairs_use_fixed_physical_representative_distance",
            "node_density_is_count_per_square_micrometer",
        ],
        "finite_result_policy": "reject_non_finite_nonconverged_is_diagnostic_only",
        "claim_status": (
            "experimental_single_pattern_spatial_ppc"
            if source_result["fit_state"] == "complete"
            else "diagnostic_only_nonconverged"
        ),
    }


def main() -> None:
    require_versions(pymc=pm.__version__)
    lock_digest, worker_digest = worker_digests(__file__)
    source_worker_digest = file_sha256(ipp.__file__)
    raw = read_request(16 * MIB)
    request, config, observed, weights, pairs = validate(
        json.loads(raw), lock_digest, worker_digest, source_worker_digest
    )
    result = run_ppc(
        request,
        config,
        observed,
        weights,
        pairs,
        sha256_hex(raw),
        lock_digest,
        worker_digest,
        source_worker_digest,
    )
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "marklab PyMC arbitrary-window IPP spatial PPC failed")
