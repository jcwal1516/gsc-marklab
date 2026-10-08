#!/usr/bin/env python3

import hashlib
import json
import math
import sys

import gudhi

GUDHI_VERSION = "3.13.0"


class ContractError(Exception):
    pass


def exact(actual, expected, label):
    if actual != expected:
        raise ContractError(f"{label} mismatch")


def squared_distance(left, right):
    return sum((a - b) ** 2 for a, b in zip(left, right))


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


def oriented_boundary(simplex):
    return [
        (simplex[:index] + simplex[index + 1 :], 1 if index % 2 == 0 else -1)
        for index in range(len(simplex))
    ]


def filtration_result(tree, request, landmark_ids):
    raw = [
        (tuple(sorted(int(vertex) for vertex in simplex)), float(value))
        for simplex, value in tree.get_filtration()
        if len(simplex) - 1 <= request["maximum_dimension"]
    ]
    raw.sort(key=lambda item: (item[1], len(item[0]), item[0]))
    if len(raw) > request["maximum_simplices"]:
        raise ContractError("witness filtration exceeds maximum_simplices")
    positions = {simplex: index for index, (simplex, _) in enumerate(raw)}
    counts = [0] * (request["maximum_dimension"] + 1)
    simplices = []
    for position, (simplex, value) in enumerate(raw):
        if not math.isfinite(value):
            raise ContractError("witness filtration value is non-finite")
        boundary = []
        if len(simplex) > 1:
            for face, _ in oriented_boundary(simplex):
                if face not in positions or positions[face] >= position or raw[positions[face]][1] > value:
                    raise ContractError("witness face order is invalid")
                boundary.append(positions[face])
        dimension = len(simplex) - 1
        counts[dimension] += 1
        simplices.append(
            {
                "vertex_ids": [landmark_ids[index] for index in simplex],
                "dimension": dimension,
                "filtration_alpha_squared": value,
                "boundary_indices": boundary,
            }
        )
    prime = request["coefficient_field"]
    maximum_residual = 0
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


def persistence_result(tree, request):
    tree.compute_persistence(
        homology_coeff_field=request["coefficient_field"],
        min_persistence=-1.0,
        persistence_dim_max=True,
    )
    dimensions = []
    for dimension in range(request["maximum_dimension"] + 1):
        finite_pairs = []
        essential_births = []
        for birth, death in tree.persistence_intervals_in_dimension(dimension):
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
        dimensions.append(
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
        "by_dimension": dimensions,
    }


def main():
    if gudhi.__version__ != GUDHI_VERSION or sys.version_info[:2] != (3, 12):
        raise ContractError("GUDHI or Python version drift")
    request_bytes = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if not request_bytes or len(request_bytes) > 16 * 1024 * 1024:
        raise ContractError("request is empty or too large")
    request = json.loads(request_bytes)
    exact(request["format"], "marklab.gudhi_witness_persistence_request", "format")
    exact(request["version"], 1, "version")
    exact(request["backend"]["name"], "gudhi", "backend.name")
    exact(request["backend"]["version"], GUDHI_VERSION, "backend.version")
    exact(request["backend"]["python_version"], "3.12", "backend.python_version")
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
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "points": request["points"],
        "landmark_ids": landmark_ids,
        "approximation": {
            "landmark_count": len(landmark_ids),
            "coverage_radius_um": coverage_radius,
            "nu": request["nu"],
            "max_scale_um": math.sqrt(request["max_scale_square"]),
        },
        "filtration": filtration_result(tree, request, landmark_ids),
        "persistence": persistence_result(tree, request),
        "claim_status": "experimental_witness_approximation",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError) as error:
        print(f"topology worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
