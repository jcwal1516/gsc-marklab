#!/usr/bin/env python3
import json
import math

points = [(2.0, 2.0), (3.0, 2.0), (7.0, 7.0), (9.0, 7.0)]
boundary_distances = [2.0, 2.0, 3.0, 1.0]
bandwidth_um = 2.0
radius_um = 1.1
area_um2 = 100.0
probes = [(x + 0.5, y + 0.5) for y in range(10) for x in range(10)]


def kernel(left, right):
    squared = (left[0] - right[0]) ** 2 + (left[1] - right[1]) ** 2
    return math.exp(-squared / (2.0 * bandwidth_um**2)) / (
        2.0 * math.pi * bandwidth_um**2
    )


boundary_masses = [math.fsum(kernel(point, probe) for probe in probes) for point in points]
intensities = []
for row, point in enumerate(points):
    raw = math.fsum(
        kernel(point, other) for other_row, other in enumerate(points) if other_row != row
    )
    intensities.append(len(points) / (len(points) - 1) * raw / boundary_masses[row])

eligible = [row for row, distance in enumerate(boundary_distances) if distance >= radius_um]
pairs = [
    (source, target)
    for source in eligible
    for target in range(len(points))
    if source != target and math.dist(points[source], points[target]) <= radius_um
]
pair_sum = math.fsum(1.0 / (intensities[i] * intensities[j]) for i, j in pairs)
center_sum = math.fsum(1.0 / intensities[row] for row in eligible)
k_value = pair_sum / center_sum
result = {
    "area_um2": area_um2,
    "bandwidth_um": bandwidth_um,
    "boundary_masses": boundary_masses,
    "directed_pairs": len(pairs),
    "eligible_center_inverse_intensity_sum": center_sum,
    "eligible_centers": len(eligible),
    "integration_grid": [10, 10],
    "intensities": intensities,
    "inverse_intensity_pair_sum": pair_sum,
    "k": k_value,
    "l": math.sqrt(k_value / math.pi),
    "radius_um": radius_um,
}
print(json.dumps(result, indent=2, sort_keys=True))
