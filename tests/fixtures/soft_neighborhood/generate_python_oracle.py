#!/usr/bin/env python3
"""Independent direct-pair soft-neighborhood oracle."""

import json
import math


points = [(0.0, 0.0), (1.0, 0.0), (3.0, 0.0), (10.0, 0.0)]
simplex = [[1.0, 0.0], [0.0, 1.0], [0.5, 0.5], [0.25, 0.75]]
def evaluate(radius):
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
    return {
        "radius_um": radius,
        "directed_pair_visits": visits,
        "zero_neighbor_cell_count": sum(row["neighbor_count"] == 0 for row in rows),
        "mean_neighbor_class_mass": (
            [value / visits for value in total] if visits else None
        ),
        "rows": rows,
    }


scales = [evaluate(radius) for radius in (1.5, 2.5)]
left = scales[0]["mean_neighbor_class_mass"]
right = scales[1]["mean_neighbor_class_mass"]
adjacent_tv = 0.5 * sum(abs(a - b) for a, b in zip(left, right))

result = {
    "oracle": "Python standard-library direct physical-radius pair loop",
    **scales[0],
    "multiscale": {
        "total_directed_pair_visits": sum(scale["directed_pair_visits"] for scale in scales),
        "scales": scales,
        "adjacent_scale_total_variation_distance": [adjacent_tv],
    },
}
print(json.dumps(result, indent=2, sort_keys=True))
