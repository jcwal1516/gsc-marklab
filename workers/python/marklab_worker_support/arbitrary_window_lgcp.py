"""Backend-neutral request contract for the exact-window dense LGCP adapter."""

from __future__ import annotations

import math
from typing import Any

import numpy as np

from . import versions
from .arbitrary_window import neighbor_pairs, validate_input_identity
from .gridded_lgcp import validate_request as validate_gridded_request
from .protocol import (
    ContractError,
    exact,
    integer,
    number,
    obj,
    sha256_digest,
    validate_backend,
)

SOURCE_WORKER = "marklab_pymc_arbitrary_window_lgcp_worker.py"

REQUEST_FORMAT = "marklab.pymc_arbitrary_window_lgcp_request"


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


def validate_request(
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
    config = validate_gridded_request(request["source_request"], lock_digest, source_worker_digest)
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
