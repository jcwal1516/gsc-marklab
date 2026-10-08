#!/usr/bin/env python3
import json
import math

points = [(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (3.0, 0.0)]
codes = [0, 0, 1, 1]
source_code = 0
target_code = 1
radius_um = 1.0
bandwidth_um = 0.5
area_um2 = 28.0
boundary_distances = [2.0, 2.0, 2.0, 2.0]

eligible = [
    row
    for row, code in enumerate(codes)
    if code == source_code
    and boundary_distances[row] >= radius_um + bandwidth_um
]
weights = []
for source in eligible:
    for target, code in enumerate(codes):
        if code != target_code:
            continue
        distance = math.dist(points[source], points[target])
        scaled = (radius_um - distance) / bandwidth_um
        if abs(scaled) <= 1.0:
            weights.append(0.75 * (1.0 - scaled * scaled) / bandwidth_um)

weight_sum = math.fsum(weights)
result = {
    "area_um2": area_um2,
    "bandwidth_um": bandwidth_um,
    "cross_g": area_um2
    * weight_sum
    / (2.0 * math.pi * radius_um * len(eligible) * codes.count(target_code)),
    "directed_source_target_pairs_in_support": len(weights),
    "eligible_source_centers": len(eligible),
    "kernel": "epanechnikov",
    "kernel_weight_sum": weight_sum,
    "radius_um": radius_um,
}
print(json.dumps(result, indent=2, sort_keys=True))
