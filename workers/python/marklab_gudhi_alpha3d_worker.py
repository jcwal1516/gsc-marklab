#!/usr/bin/env python3

import hashlib
import json
import math
import sys

import gudhi
import numpy as np


class ContractError(Exception):
    pass


def boundary_matrix(lower, upper):
    lower_index = {tuple(simplex): index for index, simplex in enumerate(lower)}
    matrix = np.zeros((len(lower), len(upper)), dtype=np.uint8)
    for column, simplex in enumerate(upper):
        for omitted in range(len(simplex)):
            face = tuple(simplex[:omitted] + simplex[omitted + 1 :])
            matrix[lower_index[face], column] = 1
    return matrix


def main():
    if gudhi.__version__ != "3.13.0" or np.__version__ != "2.4.6" or sys.version_info[:2] != (3, 12):
        raise ContractError("3-D alpha backend version drift")
    request_bytes = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.gudhi_alpha3d_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    points = request["points"]
    coordinates = [point["coordinates_um"] for point in points]
    alpha = gudhi.AlphaComplex(points=coordinates, precision="exact")
    tree = alpha.create_simplex_tree(max_alpha_square=request["maximum_squared_alpha_um2"])
    tree.compute_persistence(
        homology_coeff_field=2,
        min_persistence=0.0,
        persistence_dim_max=request["maximum_homology_dimension"] >= 3,
    )
    simplices_by_dimension = [[] for _ in range(4)]
    filtration = []
    tetrahedra = []
    for simplex_raw, value in tree.get_filtration():
        simplex = sorted(simplex_raw)
        dimension = len(simplex) - 1
        simplices_by_dimension[dimension].append(simplex)
        item = {
            "vertices": [points[index]["point_id"] for index in simplex],
            "vertex_indices": simplex,
            "dimension": dimension,
            "squared_alpha_um2": float(value),
        }
        filtration.append(item)
        if dimension == 3:
            tetrahedra.append(item.copy())
    maximum_boundary_squared = 0
    for dimension in range(2, 4):
        lower = [sorted(simplex) for simplex in simplices_by_dimension[dimension - 2]]
        middle = [sorted(simplex) for simplex in simplices_by_dimension[dimension - 1]]
        upper = [sorted(simplex) for simplex in simplices_by_dimension[dimension]]
        if not upper:
            continue
        first = boundary_matrix(lower, middle)
        second = boundary_matrix(middle, upper)
        maximum_boundary_squared = max(maximum_boundary_squared, int(((first @ second) % 2).max()))
    diagrams = {}
    for dimension in range(request["maximum_homology_dimension"] + 1):
        intervals = tree.persistence_intervals_in_dimension(dimension)
        diagrams[str(dimension)] = [
            {
                "birth_squared_alpha_um2": float(birth),
                "death_squared_alpha_um2": None if math.isinf(death) else float(death),
                "essential": bool(math.isinf(death)),
            }
            for birth, death in intervals
        ]
    result = {
        "format": "marklab.alpha_complex_3d",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "coordinate_frame": request["coordinate_frame"],
        "filtration_convention": "squared_alpha_physical_um2",
        "point_ids": [point["point_id"] for point in points],
        "maximum_simplex_dimension": int(tree.dimension()),
        "simplex_counts_by_dimension": [len(values) for values in simplices_by_dimension],
        "filtration": filtration,
        "tetrahedra": tetrahedra,
        "persistence_diagrams": diagrams,
        "boundary_squared_max_absolute": maximum_boundary_squared,
        "claim_status": "experimental_synthetic_exact_3d_alpha_complex",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, RuntimeError, json.JSONDecodeError, np.linalg.LinAlgError) as error:
        print(f"3-D alpha worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
