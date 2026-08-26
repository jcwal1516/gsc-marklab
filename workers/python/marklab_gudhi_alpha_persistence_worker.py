#!/usr/bin/env python3

import hashlib
import json
import math
import sys

import gudhi
import scipy
from scipy.special import ndtr

GUDHI_VERSION = "3.13.0"
SCIPY_VERSION = "1.18.1"


class ContractError(Exception):
    pass


def exact(actual, expected, label):
    if actual != expected:
        raise ContractError(f"{label} mismatch")


def strict_sha256(value, label):
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
    exact(backend["version"], GUDHI_VERSION, "backend.version")
    exact(backend["python_version"], "3.12", "backend.python_version")
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


def simplex_key(simplex):
    return tuple(sorted(int(vertex) for vertex in simplex))


def oriented_boundary(simplex):
    return [
        (simplex[:index] + simplex[index + 1 :], 1 if index % 2 == 0 else -1)
        for index in range(len(simplex))
    ]


def build_filtration(request):
    coordinates = [point["coordinates_um"] for point in request["points"]]
    alpha = gudhi.AlphaComplex(points=coordinates, precision="exact")
    tree = alpha.create_simplex_tree(max_alpha_square=request["max_alpha_square"])
    raw = [
        (simplex_key(simplex), float(value))
        for simplex, value in tree.get_filtration()
        if len(simplex) - 1 <= request["maximum_dimension"]
    ]
    raw.sort(key=lambda item: (item[1], len(item[0]), item[0]))
    if len(raw) > request["maximum_simplices"]:
        raise ContractError("alpha filtration exceeds maximum_simplices")
    index = {simplex: position for position, (simplex, _) in enumerate(raw)}
    if len(index) != len(raw):
        raise ContractError("alpha filtration contains duplicate simplices")
    simplices = []
    counts = [0] * (request["maximum_dimension"] + 1)
    for position, (simplex, value) in enumerate(raw):
        if not math.isfinite(value):
            raise ContractError("alpha filtration value is non-finite")
        boundary = []
        if len(simplex) > 1:
            for face, _ in oriented_boundary(simplex):
                if face not in index or index[face] >= position or raw[index[face]][1] > value:
                    raise ContractError("face is absent, late, or has a larger filtration value")
                boundary.append(index[face])
        dimension = len(simplex) - 1
        counts[dimension] += 1
        simplices.append(
            {
                "vertex_ids": [request["points"][vertex]["id"] for vertex in simplex],
                "dimension": dimension,
                "filtration_alpha_squared": value,
                "boundary_indices": boundary,
            }
        )
    maximum_residual = 0
    prime = request["coefficient_field"]
    for simplex, _ in raw:
        if len(simplex) <= 2:
            continue
        coefficients = {}
        for face, face_coefficient in oriented_boundary(simplex):
            for subface, subface_coefficient in oriented_boundary(face):
                coefficients[subface] = (
                    coefficients.get(subface, 0)
                    + face_coefficient * subface_coefficient
                ) % prime
        maximum_residual = max(maximum_residual, *(abs(value) for value in coefficients.values()))
    if maximum_residual != 0:
        raise ContractError("boundary of boundary is nonzero")
    return tree, raw, {
        "convention": "alpha_squared_smallest_empty_circumsphere",
        "precision": "gudhi_exact_rounded_to_f64",
        "coefficient_field": prime,
        "tie_break_order": "filtration_then_dimension_then_lexicographic_vertices",
        "validation_status": "passed",
        "simplices": simplices,
        "simplex_counts_by_dimension": counts,
        "boundary_of_boundary_max_abs": float(maximum_residual),
    }


def persistence(tree, request):
    tree.compute_persistence(
        homology_coeff_field=request["coefficient_field"],
        min_persistence=-1.0,
        persistence_dim_max=True,
    )
    by_dimension = []
    for dimension in range(request["maximum_dimension"] + 1):
        intervals = tree.persistence_intervals_in_dimension(dimension)
        finite_pairs = []
        essential_births = []
        for birth, death in intervals:
            birth = float(birth)
            death = float(death)
            if math.isinf(death):
                essential_births.append(birth)
            elif death >= birth:
                finite_pairs.append({"birth": birth, "death": death})
            else:
                raise ContractError("persistence pair has death before birth")
        finite_pairs.sort(key=lambda pair: (pair["birth"], pair["death"]))
        essential_births.sort()
        by_dimension.append(
            {
                "dimension": dimension,
                "finite_pairs": finite_pairs,
                "essential_births": essential_births,
                "essential_count": len(essential_births),
            }
        )
    return {
        "algorithm": "gudhi_simplex_tree_persistent_cohomology",
        "essential_death_policy": "separate_birth_list_no_json_infinity",
        "by_dimension": by_dimension,
    }


def landscape(persistence_result, request):
    dimension = request["transform_dimension"]
    pairs = persistence_result["by_dimension"][dimension]["finite_pairs"]
    values = [[0.0 for _ in request["landscape_grid_alpha_squared"]]
              for _ in range(request["landscape_max_k"])]
    for column, location in enumerate(request["landscape_grid_alpha_squared"]):
        tents = sorted(
            (max(0.0, min(location - pair["birth"], pair["death"] - location))
             for pair in pairs if pair["death"] > pair["birth"]),
            reverse=True,
        )
        for row, value in enumerate(tents[: request["landscape_max_k"]]):
            values[row][column] = value
    return {
        "dimension": dimension,
        "grid_alpha_squared": request["landscape_grid_alpha_squared"],
        "max_k": request["landscape_max_k"],
        "norm_convention": "unscaled_tent_alpha_squared_axis",
        "values": values,
    }


def persistence_image(persistence_result, request):
    dimension = request["transform_dimension"]
    pairs = persistence_result["by_dimension"][dimension]["finite_pairs"]
    birth_edges = request["image_birth_edges_alpha_squared"]
    persistence_edges = request["image_persistence_edges_alpha_squared"]
    bandwidth = request["image_kernel_bandwidth_alpha_squared"]
    values = [
        [0.0 for _ in range(len(persistence_edges) - 1)]
        for _ in range(len(birth_edges) - 1)
    ]
    for pair in pairs:
        persistence_value = pair["death"] - pair["birth"]
        if persistence_value <= 0.0:
            continue
        for birth_pixel in range(len(birth_edges) - 1):
            birth_mass = ndtr((birth_edges[birth_pixel + 1] - pair["birth"]) / bandwidth) - ndtr(
                (birth_edges[birth_pixel] - pair["birth"]) / bandwidth
            )
            for persistence_pixel in range(len(persistence_edges) - 1):
                persistence_mass = ndtr(
                    (persistence_edges[persistence_pixel + 1] - persistence_value) / bandwidth
                ) - ndtr(
                    (persistence_edges[persistence_pixel] - persistence_value) / bandwidth
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
    values = []
    counts_rows = []
    cursor = 0
    counts = [0] * (request["maximum_dimension"] + 1)
    for threshold in request["euler_thresholds_alpha_squared"]:
        while cursor < len(raw) and raw[cursor][1] <= threshold + 1e-14:
            counts[len(raw[cursor][0]) - 1] += 1
            cursor += 1
        values.append(sum((1 if dimension % 2 == 0 else -1) * count
                          for dimension, count in enumerate(counts)))
        counts_rows.append(list(counts))
    return {
        "thresholds_alpha_squared": request["euler_thresholds_alpha_squared"],
        "values": values,
        "simplex_counts_by_threshold": counts_rows,
    }


def main():
    if gudhi.__version__ != GUDHI_VERSION or scipy.__version__ != SCIPY_VERSION:
        raise ContractError("GUDHI or SciPy version drift")
    if sys.version_info[:2] != (3, 12):
        raise ContractError("Python version drift")
    request_bytes = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if not request_bytes or len(request_bytes) > 16 * 1024 * 1024:
        raise ContractError("request is empty or too large")
    request = json.loads(request_bytes)
    validate_request(request)
    tree, raw, filtration_result = build_filtration(request)
    persistence_result = persistence(tree, request)
    result = {
        "format": "marklab.alpha_persistence",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "points": request["points"],
        "max_alpha_um": math.sqrt(request["max_alpha_square"]),
        "filtration": filtration_result,
        "persistence": persistence_result,
        "landscape": landscape(persistence_result, request),
        "persistence_image": persistence_image(persistence_result, request),
        "euler_curve": euler_curve(raw, request),
        "claim_status": "experimental_synthetic_alpha_topology",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError) as error:
        print(f"topology worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
