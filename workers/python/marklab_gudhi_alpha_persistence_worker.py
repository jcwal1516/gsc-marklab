#!/usr/bin/env python3
"""GUDHI worker for exact 2-D alpha-complex persistence and its vectorizations."""

import json
import math
import sys
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parent))

import gudhi
import scipy
from scipy.special import ndtr

from marklab_worker_support import versions
from marklab_worker_support.protocol import (
    CONTRACT_ERRORS,
    MIB,
    ContractError,
    exact,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    write_result,
)
from marklab_worker_support.topology import (
    boundary_of_boundary_residual,
    filtration_simplices,
    persistence_by_dimension,
    sorted_filtration,
)


def strict_sha256(value, label):
    """Require a 64-character string that parses as hexadecimal."""
    if not isinstance(value, str) or len(value) != 64:
        raise ContractError(f"{label} must be a SHA-256 hex digest")
    try:
        int(value, 16)
    except ValueError as error:
        raise ContractError(f"{label} must be a SHA-256 hex digest") from error


def validate_request(request):
    exact(request["format"], "marklab.gudhi_alpha_persistence_request", "format")
    exact(request["version"], 1, "version")
    backend = request["backend"]
    exact(backend["name"], "gudhi", "backend.name")
    exact(backend["version"], versions.GUDHI, "backend.version")
    exact(backend["python_version"], versions.PYTHON, "backend.python_version")
    exact(
        backend["license"],
        "MIT_with_GPLv3_CGAL_alpha_complex_dependency",
        "backend.license",
    )
    strict_sha256(backend["environment_lock_sha256"], "environment lock")
    strict_sha256(backend["worker_sha256"], "worker")
    if len(request["points"]) < 3 or len(request["points"]) > 10_000:
        raise ContractError("point count is outside the worker bound")
    if request["maximum_simplices"] < 7 or request["maximum_simplices"] > 1_000_000:
        raise ContractError("simplex bound is invalid")


def build_filtration(request):
    coordinates = [point["coordinates_um"] for point in request["points"]]
    alpha = gudhi.AlphaComplex(points=coordinates, precision="exact")
    tree = alpha.create_simplex_tree(max_alpha_square=request["max_alpha_square"])
    raw = sorted_filtration(tree, request["maximum_dimension"])
    if len(raw) > request["maximum_simplices"]:
        raise ContractError("alpha filtration exceeds maximum_simplices")
    if len({simplex for simplex, _ in raw}) != len(raw):
        raise ContractError("alpha filtration contains duplicate simplices")
    point_ids = [point["id"] for point in request["points"]]
    simplices, counts = filtration_simplices(raw, point_ids, request["maximum_dimension"], "alpha")
    prime = request["coefficient_field"]
    maximum_residual = boundary_of_boundary_residual(raw, prime)
    if maximum_residual != 0:
        raise ContractError("boundary of boundary is nonzero")
    return (
        tree,
        raw,
        {
            "convention": "alpha_squared_smallest_empty_circumsphere",
            "precision": "gudhi_exact_rounded_to_f64",
            "coefficient_field": prime,
            "tie_break_order": "filtration_then_dimension_then_lexicographic_vertices",
            "validation_status": "passed",
            "simplices": simplices,
            "simplex_counts_by_dimension": counts,
            "boundary_of_boundary_max_abs": float(maximum_residual),
        },
    )


def landscape(persistence_result, request):
    """Persistence landscape (unscaled tent functions) of the transform dimension."""
    dimension = request["transform_dimension"]
    pairs = persistence_result["by_dimension"][dimension]["finite_pairs"]
    grid = request["landscape_grid_alpha_squared"]
    max_k = request["landscape_max_k"]
    values = [[0.0 for _ in grid] for _ in range(max_k)]
    for column, location in enumerate(grid):
        tents = sorted(
            (
                max(0.0, min(location - pair["birth"], pair["death"] - location))
                for pair in pairs
                if pair["death"] > pair["birth"]
            ),
            reverse=True,
        )
        for row, value in enumerate(tents[:max_k]):
            values[row][column] = value
    return {
        "dimension": dimension,
        "grid_alpha_squared": grid,
        "max_k": max_k,
        "norm_convention": "unscaled_tent_alpha_squared_axis",
        "values": values,
    }


def persistence_image(persistence_result, request):
    """Persistence-weighted image with exact Gaussian pixel masses in birth/persistence space."""
    dimension = request["transform_dimension"]
    pairs = persistence_result["by_dimension"][dimension]["finite_pairs"]
    birth_edges = request["image_birth_edges_alpha_squared"]
    persistence_edges = request["image_persistence_edges_alpha_squared"]
    bandwidth = request["image_kernel_bandwidth_alpha_squared"]
    values = [[0.0 for _ in range(len(persistence_edges) - 1)] for _ in range(len(birth_edges) - 1)]
    for pair in pairs:
        persistence_value = pair["death"] - pair["birth"]
        if persistence_value <= 0.0:
            continue
        for birth_pixel in range(len(birth_edges) - 1):
            birth_upper = ndtr((birth_edges[birth_pixel + 1] - pair["birth"]) / bandwidth)
            birth_lower = ndtr((birth_edges[birth_pixel] - pair["birth"]) / bandwidth)
            birth_mass = birth_upper - birth_lower
            for persistence_pixel in range(len(persistence_edges) - 1):
                upper_edge = persistence_edges[persistence_pixel + 1]
                lower_edge = persistence_edges[persistence_pixel]
                persistence_mass = ndtr((upper_edge - persistence_value) / bandwidth) - ndtr(
                    (lower_edge - persistence_value) / bandwidth
                )
                values[birth_pixel][persistence_pixel] += (
                    persistence_value * birth_mass * persistence_mass
                )
    return {
        "dimension": dimension,
        "birth_edges_alpha_squared": birth_edges,
        "persistence_edges_alpha_squared": persistence_edges,
        "kernel_bandwidth_alpha_squared": bandwidth,
        "weight": "persistence",
        "normalization": "none",
        "integration": "exact_axis_aligned_gaussian_pixel_probability_via_scipy_ndtr",
        "values": values,
    }


def euler_curve(raw, request):
    """Euler characteristic of the filtration at each threshold (inclusive)."""
    values = []
    counts_rows = []
    cursor = 0
    counts = [0] * (request["maximum_dimension"] + 1)
    for threshold in request["euler_thresholds_alpha_squared"]:
        while cursor < len(raw) and raw[cursor][1] <= threshold:
            counts[len(raw[cursor][0]) - 1] += 1
            cursor += 1
        values.append(
            sum((1 if dimension % 2 == 0 else -1) * count for dimension, count in enumerate(counts))
        )
        counts_rows.append(list(counts))
    return {
        "thresholds_alpha_squared": request["euler_thresholds_alpha_squared"],
        "values": values,
        "simplex_counts_by_threshold": counts_rows,
    }


def main():
    require_versions(gudhi=gudhi.__version__, scipy=scipy.__version__)
    request_bytes = read_request(16 * MIB)
    request = json.loads(request_bytes)
    validate_request(request)
    tree, raw, filtration_result = build_filtration(request)
    persistence_result = persistence_by_dimension(
        tree, request["coefficient_field"], request["maximum_dimension"]
    )
    result = {
        "format": "marklab.alpha_persistence",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(request_bytes),
        "points": request["points"],
        "max_alpha_um": math.sqrt(request["max_alpha_square"]),
        "filtration": filtration_result,
        "persistence": persistence_result,
        "landscape": landscape(persistence_result, request),
        "persistence_image": persistence_image(persistence_result, request),
        "euler_curve": euler_curve(raw, request),
        "claim_status": "experimental_synthetic_alpha_topology",
    }
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "topology worker contract error", errors=CONTRACT_ERRORS)
