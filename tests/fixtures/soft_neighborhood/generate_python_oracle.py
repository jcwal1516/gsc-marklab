#!/usr/bin/env python3
"""Independent direct-pair soft-neighborhood oracle."""

import json
import math


points = [(0.0, 0.0), (1.0, 0.0), (3.0, 0.0), (10.0, 0.0)]
simplex = [[1.0, 0.0], [0.0, 1.0], [0.5, 0.5], [0.25, 0.75]]
radius = 1.5
rows = []
total = [0.0, 0.0]
visits = 0
for source, point in enumerate(points):
    neighbors = [
        target
        for target, candidate in enumerate(points)
        if target != source and math.dist(point, candidate) <= radius
    ]
    if neighbors:
        mean = [
            sum(simplex[target][class_index] for target in neighbors) / len(neighbors)
            for class_index in range(len(simplex[0]))
        ]
        for target in neighbors:
            for class_index, value in enumerate(simplex[target]):
                total[class_index] += value
        visits += len(neighbors)
    else:
        mean = None
    rows.append({"neighbor_count": len(neighbors), "mean_neighbor_probabilities": mean})

result = {
    "oracle": "Python standard-library direct physical-radius pair loop",
    "radius_um": radius,
    "directed_pair_visits": visits,
    "zero_neighbor_cell_count": sum(row["neighbor_count"] == 0 for row in rows),
    "mean_neighbor_class_mass": [value / visits for value in total],
    "rows": rows,
}
print(json.dumps(result, indent=2, sort_keys=True))
