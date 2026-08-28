#!/usr/bin/env python3
"""Independent direct-loop oracle for the binary physical-radius mixing graph."""

import json
import math


points = [(2.0, 5.0), (4.0, 5.0), (6.0, 5.0), (8.0, 5.0)]
labels = [0, 0, 1, 1]
radius = 2.1
edges = []
for left in range(len(points)):
    for right in range(left + 1, len(points)):
        distance = math.hypot(
            points[left][0] - points[right][0],
            points[left][1] - points[right][1],
        )
        if distance <= radius:
            edges.append((left, right))

cross = sum(labels[left] != labels[right] for left, right in edges)
cross_fraction = cross / len(edges)


def binary_entropy(probability: float) -> float:
    terms = []
    if probability > 0.0:
        terms.append(-probability * math.log(probability))
    if probability < 1.0:
        complement = 1.0 - probability
        terms.append(-complement * math.log(complement))
    return sum(terms)


negative_count = labels.count(0)
positive_count = labels.count(1)
expectation = (
    2.0 * negative_count * positive_count / (len(labels) * (len(labels) - 1))
)
result = {
    "oracle": "Python standard-library direct undirected pair loop",
    "radius_um": radius,
    "edges": edges,
    "undirected_edge_count": len(edges),
    "directed_pair_visits": 2 * len(edges),
    "cross_compartment_edge_count": cross,
    "cross_compartment_edge_fraction": cross_fraction,
    "random_label_cross_edge_expectation": expectation,
    "cross_edge_fraction_minus_expectation": cross_fraction - expectation,
    "edge_type_entropy_nats": binary_entropy(cross_fraction),
    "normalized_edge_type_entropy": binary_entropy(cross_fraction) / math.log(2.0),
}
print(json.dumps(result, indent=2, sort_keys=True))
