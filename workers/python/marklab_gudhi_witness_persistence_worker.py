#!/usr/bin/env python3
"""GUDHI worker for witness-complex persistence on farthest-point landmarks."""

import json
import math
import sys
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parent))

import gudhi

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


def squared_distance(left, right):
    return sum((a - b) ** 2 for a, b in zip(left, right, strict=False))


def select_landmarks(points, count):
    selected = [0]
    while len(selected) < count:
        best_index = None
        best_distance = -1.0
        for index, point in enumerate(points):
            if index in selected:
                continue
            distance = min(squared_distance(point, points[chosen]) for chosen in selected)
            if distance > best_distance:
                best_index = index
                best_distance = distance
        selected.append(best_index)
    coverage_square = max(
        min(squared_distance(point, points[chosen]) for chosen in selected) for point in points
    )
    return selected, math.sqrt(coverage_square)


def filtration_result(tree, request, landmark_ids):
    raw = sorted_filtration(tree, request["maximum_dimension"])
    if len(raw) > request["maximum_simplices"]:
        raise ContractError("witness filtration exceeds maximum_simplices")
    simplices, counts = filtration_simplices(
        raw, landmark_ids, request["maximum_dimension"], "witness"
    )
    prime = request["coefficient_field"]
    maximum_residual = boundary_of_boundary_residual(raw, prime)
    if maximum_residual != 0:
        raise ContractError("boundary of boundary is nonzero")
    return {
        "convention": "gudhi_weak_euclidean_witness_alpha_squared",
        "precision": "gudhi_f64",
        "coefficient_field": prime,
        "tie_break_order": "filtration_then_dimension_then_lexicographic_landmark_index",
        "validation_status": "passed",
        "simplices": simplices,
        "simplex_counts_by_dimension": counts,
        "boundary_of_boundary_max_abs": float(maximum_residual),
    }


def main():
    require_versions(gudhi=gudhi.__version__)
    request_bytes = read_request(16 * MIB)
    request = json.loads(request_bytes)
    exact(request["format"], "marklab.gudhi_witness_persistence_request", "format")
    exact(request["version"], 1, "version")
    exact(request["backend"]["name"], "gudhi", "backend.name")
    exact(request["backend"]["version"], versions.GUDHI, "backend.version")
    exact(request["backend"]["python_version"], versions.PYTHON, "backend.python_version")
    exact(request["landmark_method"], "farthest_point", "landmark_method")
    exact(request["nu"], 0, "nu")
    points = [point["coordinates_um"] for point in request["points"]]
    selected, coverage_radius = select_landmarks(points, request["landmark_count"])
    landmarks = [points[index] for index in selected]
    landmark_ids = [request["points"][index]["id"] for index in selected]
    tree = gudhi.EuclideanWitnessComplex(
        landmarks=landmarks,
        witnesses=points,
    ).create_simplex_tree(
        max_alpha_square=request["max_scale_square"],
        limit_dimension=request["maximum_dimension"],
    )
    result = {
        "format": "marklab.witness_persistence",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(request_bytes),
        "points": request["points"],
        "landmark_ids": landmark_ids,
        "approximation": {
            "landmark_count": len(landmark_ids),
            "coverage_radius_um": coverage_radius,
            "nu": request["nu"],
            "max_scale_um": math.sqrt(request["max_scale_square"]),
        },
        "filtration": filtration_result(tree, request, landmark_ids),
        "persistence": persistence_by_dimension(
            tree, request["coefficient_field"], request["maximum_dimension"]
        ),
        "claim_status": "experimental_witness_approximation",
    }
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "topology worker contract error", errors=CONTRACT_ERRORS)
