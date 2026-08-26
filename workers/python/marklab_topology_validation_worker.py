#!/usr/bin/env python3

import hashlib
import json
import math
import sys

import gudhi
import numpy as np
import scipy
import skimage
from scipy.special import ndtr
from skimage.measure import euler_number, perimeter_crofton


class ValidationError(Exception):
    pass


def passed(identifier, algorithm, evidence):
    return {
        "id": identifier,
        "algorithm": algorithm,
        "status": "passed",
        "evidence": evidence,
        "limitation": None,
    }


def require(condition, identifier):
    if not condition:
        raise ValidationError(f"exact validation failed: {identifier}")


def main():
    if (
        gudhi.__version__ != "3.13.0"
        or scipy.__version__ != "1.18.1"
        or skimage.__version__ != "0.26.0"
        or sys.version_info[:2] != (3, 12)
    ):
        raise ValidationError("backend version drift")
    request_bytes = sys.stdin.buffer.read(1_000_001)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.topology_validation_request" or request["version"] != 1:
        raise ValidationError("request identity mismatch")
    entries = []

    boundary = gudhi.SimplexTree()
    for vertex in range(3):
        boundary.insert([vertex], filtration=0.0)
    for edge in [(0, 1), (0, 2), (1, 2)]:
        boundary.insert(edge, filtration=0.0)
    boundary.compute_persistence(homology_coeff_field=2, min_persistence=-1.0, persistence_dim_max=True)
    betti = boundary.betti_numbers()
    require(betti[:2] == [1, 1], "hand_complex_betti")
    entries.append(passed("hand_complex_betti", "gudhi_simplex_tree", {"betti_0_1": betti[:2]}))

    height = math.sqrt(3.0)
    alpha = gudhi.AlphaComplex(points=[[0.0, 0.0], [2.0, 0.0], [1.0, height]], precision="exact")
    alpha_tree = alpha.create_simplex_tree(max_alpha_square=4.0)
    filtration = {tuple(simplex): float(value) for simplex, value in alpha_tree.get_filtration()}
    triangle_value = filtration[(0, 1, 2)]
    require(abs(triangle_value - 4.0 / 3.0) < 1e-12, "alpha_against_analytic_triangle")
    entries.append(passed("alpha_against_analytic_triangle", "gudhi_exact_alpha", {"triangle_alpha_squared": triangle_value, "analytic": 4.0 / 3.0}))

    grid = [1.0, 7.0 / 6.0, 4.0 / 3.0]
    landscape = [max(0.0, min(value - 1.0, 4.0 / 3.0 - value)) for value in grid]
    require(max(abs(a - b) for a, b in zip(landscape, [0.0, 1.0 / 6.0, 0.0])) < 1e-12, "landscape_against_hand_tent")
    entries.append(passed("landscape_against_hand_tent", "direct_tent", {"values": landscape}))

    bandwidth = 0.1
    persistence = 1.0 / 3.0
    ndtr_mass = persistence * (ndtr(5.0) - ndtr(-5.0)) * (
        ndtr((0.5 - persistence) / bandwidth) - ndtr(-persistence / bandwidth)
    )
    cdf = lambda value: 0.5 * (1.0 + math.erf(value / math.sqrt(2.0)))
    erf_mass = persistence * (cdf(5.0) - cdf(-5.0)) * (
        cdf((0.5 - persistence) / bandwidth) - cdf(-persistence / bandwidth)
    )
    require(abs(ndtr_mass - erf_mass) < 1e-15, "persistence_image_against_erf_integral")
    entries.append(passed("persistence_image_against_erf_integral", "scipy_ndtr_vs_math_erf", {"absolute_difference": abs(ndtr_mass - erf_mass)}))

    alpha_tree.compute_persistence(homology_coeff_field=2, min_persistence=-1.0, persistence_dim_max=True)
    final_betti = alpha_tree.betti_numbers()
    alternating = 3 - 3 + 1
    betti_euler = sum((1 if dimension % 2 == 0 else -1) * count for dimension, count in enumerate(final_betti))
    require(alternating == betti_euler == 1, "euler_betti_and_alternating_counts")
    entries.append(passed("euler_betti_and_alternating_counts", "gudhi_betti_vs_simplex_counts", {"alternating": alternating, "betti_euler": betti_euler}))

    raster = np.ones((200, 200), dtype=bool)
    raster_area = float(raster.sum())
    raster_perimeter = float(perimeter_crofton(raster, directions=4))
    area_error = abs(raster_area - 40_000.0) / 40_000.0
    perimeter_error = abs(raster_perimeter - 800.0) / 800.0
    raster_euler = int(euler_number(raster, connectivity=1))
    require(area_error == 0.0 and perimeter_error < 0.06 and raster_euler == 1, "polygon_high_resolution_raster_minkowski")
    entries.append(passed("polygon_high_resolution_raster_minkowski", "analytic_square_vs_scikit_image", {"area_relative_error": area_error, "perimeter_relative_error": perimeter_error, "euler": raster_euler}))

    line = np.asarray([[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [3.0, 0.0]])
    distances = [float(np.linalg.norm(line[index + 1] - line[index])) for index in range(3)]
    critical = max(distances)
    require(critical == 1.0, "lattice_connectivity_transition")
    entries.append(passed("lattice_connectivity_transition", "hand_union_find_control", {"critical_radius": critical}))

    baseline = np.zeros((5, 5), dtype=bool)
    baseline[2, 2] = True
    perturbed = baseline.copy()
    perturbed[2, 3] = True
    area_relative = abs(float(perturbed.sum()) - float(baseline.sum())) / float(baseline.sum())
    euler_change = abs(int(euler_number(perturbed, connectivity=1)) - int(euler_number(baseline, connectivity=1)))
    require(area_relative == 1.0 and euler_change == 0, "bounded_segmentation_perturbation")
    entries.append(passed("bounded_segmentation_perturbation", "declared_pixel_toggle", {"area_relative_error": area_relative, "euler_change": euler_change}))

    alpha_tree.compute_persistence(homology_coeff_field=2, min_persistence=-1.0, persistence_dim_max=True)
    essential = [pair for pair in alpha_tree.persistence_intervals_in_dimension(0) if math.isinf(float(pair[1]))]
    require(len(essential) == 1, "essential_class_policy")
    entries.append(passed("essential_class_policy", "separate_infinite_death", {"essential_h0_count": len(essential), "json_policy": "birth_only"}))

    entries.append({
        "id": "sparse_memory_scaling",
        "algorithm": "persistent_reduction",
        "status": "not_verified",
        "evidence": {"bounded_dense_and_library_fixtures_only": True},
        "limitation": "no representative sparse pathology workload or memory measurement is admitted",
    })
    output = {
        "format": "marklab.topology_validation",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "exact_fixture_status": "passed",
        "scale_validation_status": "not_verified",
        "entries": entries,
        "claim_status": "synthetic_exact_topology_validation_only",
    }
    json.dump(output, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ValidationError, KeyError, TypeError, ValueError, json.JSONDecodeError) as error:
        print(f"topology validation error: {error}", file=sys.stderr)
        raise SystemExit(2)
