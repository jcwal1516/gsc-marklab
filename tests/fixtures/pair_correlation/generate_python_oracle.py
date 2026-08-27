#!/usr/bin/env python3
import json
import math

points = [(3.0, 5.0), (4.0, 5.0), (7.0, 5.0)]
radius_um = 1.0
bandwidth_um = 0.5
area_um2 = 100.0
boundary_distances = [3.0, 4.0, 3.0]

eligible = [
    index
    for index, distance in enumerate(boundary_distances)
    if distance >= radius_um + bandwidth_um
]
weights = []
for source in eligible:
    for target in range(len(points)):
        if source == target:
            continue
        dx = points[source][0] - points[target][0]
        dy = points[source][1] - points[target][1]
        distance = math.hypot(dx, dy)
        scaled = (radius_um - distance) / bandwidth_um
        if abs(scaled) <= 1.0:
            weights.append(0.75 * (1.0 - scaled * scaled) / bandwidth_um)

weight_sum = math.fsum(weights)
result = {
    "area_um2": area_um2,
    "bandwidth_um": bandwidth_um,
    "directed_pairs_in_support": len(weights),
    "eligible_centers": len(eligible),
    "g": area_um2
    * weight_sum
    / (2.0 * math.pi * radius_um * len(points) * len(eligible)),
    "kernel": "epanechnikov",
    "kernel_weight_sum": weight_sum,
    "radius_um": radius_um,
}
print(json.dumps(result, indent=2, sort_keys=True))
