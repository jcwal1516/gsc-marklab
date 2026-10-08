#!/usr/bin/env python3

import hashlib
import json
import math
import random
import sys

import gudhi
import numpy as np
import scipy
import skimage
from scipy import ndimage
from skimage.measure import euler_number, perimeter_crofton
from skimage.morphology import binary_dilation, disk


class ContractError(Exception):
    pass


def cubical_diagram(mask, pixel_size):
    signed = (
        ndimage.distance_transform_edt(~mask) - ndimage.distance_transform_edt(mask)
    ) * pixel_size
    complex_ = gudhi.CubicalComplex(top_dimensional_cells=signed)
    complex_.compute_persistence(homology_coeff_field=2, min_persistence=-1.0)
    diagrams = []
    for dimension in range(2):
        finite = []
        for birth, death in complex_.persistence_intervals_in_dimension(dimension):
            if math.isfinite(death) and death > birth:
                finite.append([float(birth), float(death)])
        diagrams.append(finite)
    return diagrams


def landscape(diagram, grid):
    output = []
    for location in grid:
        output.append(
            max(
                [max(0.0, min(location - birth, death - location)) for birth, death in diagram]
                or [0.0]
            )
        )
    return output


def morphology(mask, pixel_size):
    area = float(mask.sum()) * pixel_size**2
    perimeter = float(perimeter_crofton(mask, directions=4)) * pixel_size
    euler = int(euler_number(mask, connectivity=1))
    return area, perimeter, euler


def euler_curve(mask, pixel_size, scales):
    values = []
    for scale in scales:
        radius = int(round(scale / pixel_size))
        transformed = mask if radius == 0 else binary_dilation(mask, disk(radius), mode="ignore")
        values.append(int(euler_number(transformed, connectivity=1)))
    return values


def critical_radius(mask, pixel_size):
    points = np.argwhere(mask)
    if len(points) <= 1:
        return 0.0
    visited = {0}
    maximum_edge = 0.0
    while len(visited) < len(points):
        best = None
        for left in visited:
            for right in range(len(points)):
                if right in visited:
                    continue
                distance = float(np.linalg.norm(points[left] - points[right])) * pixel_size
                candidate = (distance, right)
                if best is None or candidate < best:
                    best = candidate
        maximum_edge = max(maximum_edge, best[0])
        visited.add(best[1])
    return maximum_edge


def relative_error(baseline, candidate):
    errors = []
    for reference, changed in zip(baseline, candidate):
        errors.append(abs(changed - reference) / max(abs(reference), 1.0))
    return max(errors)


def main():
    if (
        gudhi.__version__ != "3.13.0"
        or scipy.__version__ != "1.18.1"
        or skimage.__version__ != "0.26.0"
        or sys.version_info[:2] != (3, 12)
    ):
        raise ContractError("backend version drift")
    request_bytes = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.topology_stability_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    mask = np.asarray(request["base_mask"], dtype=bool)
    pixel_size = float(request["pixel_size_um"])
    scales = request["scales_um"]
    generator = request["perturbation_generator"]
    if generator["kind"] != "toggle_declared_pixels":
        raise ContractError("unsupported perturbation generator")
    candidates = [tuple(int(value) for value in pair) for pair in generator["candidate_pixels"]]
    toggles = int(generator["toggles_per_repetition"])
    if toggles < 1 or toggles > len(candidates):
        raise ContractError("toggle count is invalid")
    for row, column in candidates:
        if row < 0 or column < 0 or row >= mask.shape[0] or column >= mask.shape[1]:
            raise ContractError("candidate pixel is outside the mask")
    baseline_diagram = cubical_diagram(mask, pixel_size)
    baseline_landscape = landscape(baseline_diagram[0], scales)
    baseline_euler = euler_curve(mask, pixel_size, scales)
    baseline_morphology = morphology(mask, pixel_size)
    baseline_critical = critical_radius(mask, pixel_size)
    results = []
    for repetition in range(request["repetitions"]):
        rng = random.Random(request["seed"] + repetition)
        selected = sorted(rng.sample(candidates, toggles))
        perturbed = mask.copy()
        for row, column in selected:
            perturbed[row, column] = ~perturbed[row, column]
        diagram = cubical_diagram(perturbed, pixel_size)
        changed_landscape = landscape(diagram[0], scales)
        changed_euler = euler_curve(perturbed, pixel_size, scales)
        diagram_bottleneck = max(
            float(gudhi.bottleneck_distance(baseline_diagram[dimension], diagram[dimension], e=0.0))
            for dimension in range(2)
        )
        landscape_l2 = math.sqrt(
            sum((left - right) ** 2 for left, right in zip(baseline_landscape, changed_landscape))
        )
        results.append(
            {
                "repetition": repetition,
                "toggled_pixels": [list(pair) for pair in selected],
                "diagram_bottleneck": diagram_bottleneck,
                "landscape_l2": landscape_l2,
                "euler_curve_linf": float(max(abs(a - b) for a, b in zip(baseline_euler, changed_euler))),
                "minkowski_relative_error": relative_error(baseline_morphology, morphology(perturbed, pixel_size)),
                "critical_radius_shift_um": critical_radius(perturbed, pixel_size) - baseline_critical,
            }
        )
    maximums = {
        key: max(result[key] for result in results)
        for key in [
            "diagram_bottleneck",
            "landscape_l2",
            "euler_curve_linf",
            "minkowski_relative_error",
            "critical_radius_shift_um",
        ]
    }
    output = {
        "format": "marklab.topology_stability",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "seed": request["seed"],
        "repetitions": request["repetitions"],
        "scales_um": scales,
        "results": results,
        "maximum_observed_sensitivity": maximums,
        "failure_rate": 0.0,
        "sensitivity_flags": [],
        "claim_status": "experimental_declared_segmentation_perturbations",
    }
    json.dump(output, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError) as error:
        print(f"stability worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
