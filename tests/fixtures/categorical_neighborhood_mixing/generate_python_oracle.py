#!/usr/bin/env python3
"""Independent standard-library oracle for the three-class radius graph."""

import json
import math


POINTS = [(0.0, 0.0), (1.0, 0.0), (2.5, 0.0), (4.0, 0.0)]
LABELS = [0, 1, 1, 2]
CLASSES = 3
RADIUS = 1.6


def main() -> None:
    matrix = [0] * (CLASSES * CLASSES)
    zero_neighbors = [0] * CLASSES
    for source, (x_source, y_source) in enumerate(POINTS):
        neighbors = []
        for target, (x_target, y_target) in enumerate(POINTS):
            if source == target:
                continue
            distance = math.hypot(x_source - x_target, y_source - y_target)
            if distance <= RADIUS:
                neighbors.append(target)
                matrix[LABELS[source] * CLASSES + LABELS[target]] += 1
        if not neighbors:
            zero_neighbors[LABELS[source]] += 1

    visits = sum(matrix)
    edges = visits // 2
    counts = [LABELS.count(index) for index in range(CLASSES)]
    population_pairs = len(POINTS) * (len(POINTS) - 1)
    observed = [value / visits for value in matrix]
    expected = []
    for source in range(CLASSES):
        for target in range(CLASSES):
            numerator = (
                counts[source] * (counts[source] - 1)
                if source == target
                else counts[source] * counts[target]
            )
            expected.append(numerator / population_pairs)
    cross_directed = sum(
        matrix[source * CLASSES + target]
        for source in range(CLASSES)
        for target in range(CLASSES)
        if source != target
    )
    summaries = []
    for source in range(CLASSES):
        incidences = matrix[source * CLASSES : (source + 1) * CLASSES]
        total = sum(incidences)
        probabilities = [value / total for value in incidences] if total else None
        entropy = (
            -sum(value * math.log(value) for value in probabilities if value > 0.0)
            if probabilities
            else None
        )
        if entropy == 0.0:
            entropy = 0.0
        summaries.append(
            {
                "neighbor_incidences": incidences,
                "neighbor_label_entropy_nats": entropy,
                "neighbor_probabilities": probabilities,
                "normalized_neighbor_label_entropy": (
                    entropy / math.log(CLASSES) if entropy is not None else None
                ),
                "zero_neighbor_cell_count": zero_neighbors[source],
            }
        )
    cross_fraction = (cross_directed // 2) / edges
    random_cross = 1.0 - sum(
        expected[index * CLASSES + index] for index in range(CLASSES)
    )
    print(
        json.dumps(
            {
                "cell_counts": counts,
                "class_summaries": summaries,
                "cross_class_edge_count": cross_directed // 2,
                "cross_class_edge_fraction": cross_fraction,
                "cross_edge_fraction_minus_expectation": cross_fraction
                - random_cross,
                "directed_pair_counts": matrix,
                "directed_pair_visits": visits,
                "observed_pair_fractions": observed,
                "pair_fraction_excess": [
                    actual - null for actual, null in zip(observed, expected)
                ],
                "random_label_cross_edge_expectation": random_cross,
                "random_label_pair_expectations": expected,
                "undirected_edge_count": edges,
                "zero_neighbor_cell_count": sum(zero_neighbors),
            },
            indent=2,
            sort_keys=True,
        )
    )


if __name__ == "__main__":
    main()
