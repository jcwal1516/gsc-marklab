#!/usr/bin/env python3
"""Weighted exact-window adapter for Marklab's pinned dense PyMC LGCP."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import math
from pathlib import Path
import sys
from typing import Any

import numpy as np
import pymc as pm


REQUEST_FORMAT = "marklab.pymc_arbitrary_window_lgcp_request"
RESULT_FORMAT = "marklab.bayesian_arbitrary_window_lgcp_fit"
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


def exact(value: Any, expected: Any, path: str) -> None:
    if value != expected:
        raise ContractError(f"{path} must equal {expected!r}")


def number(value: Any, path: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ContractError(f"{path} must be numeric")
    result = float(value)
    if not math.isfinite(result):
        raise ContractError(f"{path} must be finite")
    return result


def integer(value: Any, path: str, low: int, high: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not low <= value <= high:
        raise ContractError(f"{path} must be an integer in [{low},{high}]")
    return value


def digest(value: Any, path: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise ContractError(f"{path} must be a lowercase SHA-256 digest")
    return value


def load_source_worker() -> Any:
    path = Path(__file__).with_name("marklab_pymc_gridded_lgcp_worker.py")
    specification = importlib.util.spec_from_file_location("marklab_pymc_gridded_lgcp", path)
    if specification is None or specification.loader is None:
        raise ContractError("cannot load the PyMC gridded LGCP worker")
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


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
    digest(request["source_request_sha256"], "source_request_sha256")
    identity = obj(
        request["input"],
        {
            "events_digest",
            "event_membership_digest",
            "quadrature_digest",
            "window_logical_digest",
        },
        "input",
    )
    for name, value in identity.items():
        digest(value, f"input.{name}")
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
    source = load_source_worker()
    config = source.validate(request["source_request"], lock_digest, source_worker_digest)
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
        if (
            not math.isclose(config["covariate"][index], parsed_node["covariate"], rel_tol=1e-12, abs_tol=1e-12)
            or not math.isclose(
                config["offset"][index],
                parsed_node["offset"] + math.log(parsed_node["weight_um2"]),
                rel_tol=1e-12,
                abs_tol=1e-12,
            )
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
    expected = np.empty_like(covariance)
    for row in range(dimension):
        for column in range(dimension):
            distance = math.hypot(
                parsed[row]["x_um"] - parsed[column]["x_um"],
                parsed[row]["y_um"] - parsed[column]["y_um"],
            )
            scaled = math.sqrt(3.0) * distance / length_scale
            expected[row, column] = amplitude**2 * (1.0 + scaled) * math.exp(-scaled)
            if row == column:
                expected[row, column] += jitter
    if (
        not np.allclose(covariance, expected, rtol=1e-12, atol=1e-12)
        or np.max(np.abs(np.triu(cholesky, k=1))) > 1e-14
        or np.any(np.diag(cholesky) <= 0.0)
        or not np.allclose(cholesky @ cholesky.T, covariance, rtol=1e-10, atol=1e-12)
    ):
        raise ContractError("physical covariance or Cholesky factor differs")
    raw_pairs = request["neighbor_pairs"]
    if not isinstance(raw_pairs, list) or not raw_pairs:
        raise ContractError("at least one physical neighbor pair is required")
    pairs = []
    previous = None
    for raw in raw_pairs:
        if not isinstance(raw, list) or len(raw) != 2:
            raise ContractError("physical neighbor pairs must contain two indices")
        pair = (
            integer(raw[0], "neighbor_pairs[].left", 0, dimension - 1),
            integer(raw[1], "neighbor_pairs[].right", 0, dimension - 1),
        )
        if pair[0] >= pair[1] or (previous is not None and pair <= previous):
            raise ContractError("physical neighbor pairs must be unique and canonical")
        pairs.append(pair)
        previous = pair
    maximum_pairs = integer(
        request["maximum_neighbor_pairs"], "maximum_neighbor_pairs", 1, 1_000_000
    )
    if len(pairs) > maximum_pairs:
        raise ContractError("physical neighbor-pair ceiling exceeded")
    if len(nodes) > integer(
        request["maximum_quadrature_nodes"], "maximum_quadrature_nodes", 4, 36
    ) or sum(node["count"] for node in parsed) > integer(
        request["maximum_events"], "maximum_events", 1, 100_000
    ):
        raise ContractError("physical event or node ceiling exceeded")
    if number(request["neighbor_radius_um"], "neighbor_radius_um") <= 0.0:
        raise ContractError("physical neighbor radius must be positive")
    digest(request["covariance_sha256"], "covariance_sha256")
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


def spatial_statistics(
    counts: np.ndarray, weights: np.ndarray, pairs: list[tuple[int, int]]
) -> tuple[np.ndarray, np.ndarray]:
    density = counts / weights
    variance = np.var(density, axis=-1)
    contrast = np.mean(
        np.stack(
            [np.abs(density[..., left] - density[..., right]) for left, right in pairs]
        ),
        axis=0,
    )
    return variance, contrast


def summarize_spatial(observed: float, replicated: np.ndarray) -> dict[str, float]:
    return {
        "observed": observed,
        "replicated_mean": float(replicated.mean()),
        "replicated_sd": float(replicated.std(ddof=1)),
        "probability_replicated_at_least_observed": float(
            np.mean(replicated >= observed)
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
    source = load_source_worker()
    result = source.fit(config, request_sha256, lock_digest, worker_digest)
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
        weights = np.asarray([node["weight_um2"] for node in nodes], dtype=np.float64)
        observed = np.asarray([node["count"] for node in nodes], dtype=np.int64)
        replicated = np.asarray(
            [pattern["cell_counts"] for pattern in result["patterns"]], dtype=np.int64
        )
        observed_variance, observed_contrast = spatial_statistics(observed, weights, pairs)
        replicated_variance, replicated_contrast = spatial_statistics(
            replicated, weights, pairs
        )
        spatial_ppc = {
            "replicate_count": len(result["patterns"]),
            "prediction_seed": config["prediction_seed"],
            "neighbor_radius_um": request["neighbor_radius_um"],
            "neighbor_pair_count": len(pairs),
            "node_density_variance": summarize_spatial(
                float(observed_variance), replicated_variance
            ),
            "neighbor_density_mean_absolute_difference": summarize_spatial(
                float(observed_contrast), replicated_contrast
            ),
        }
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
            "backend_adapter": "equal_area_synthetic_grid_with_log_weight_offsets_and_physical_covariance_override",
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
    if pm.__version__ != PYMC_VERSION or sys.version_info[:2] != (3, 12):
        raise ContractError("PyMC or Python version drift")
    script = Path(__file__)
    lock_digest = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_digest = hashlib.sha256(script.read_bytes()).hexdigest()
    source_worker_digest = hashlib.sha256(
        script.with_name("marklab_pymc_gridded_lgcp_worker.py").read_bytes()
    ).hexdigest()
    raw = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if len(raw) > 16 * 1024 * 1024:
        raise ContractError("request exceeds 16 MiB")
    request_sha256 = hashlib.sha256(raw).hexdigest()
    request, config, nodes, _, pairs = validate(
        json.loads(raw), lock_digest, worker_digest, source_worker_digest
    )
    result = fit(
        request,
        config,
        nodes,
        pairs,
        request_sha256,
        lock_digest,
        worker_digest,
    )
    sys.stdout.write(json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")))
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(
            f"marklab PyMC arbitrary-window LGCP failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
