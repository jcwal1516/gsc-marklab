#!/usr/bin/env python3
"""Independent soft-composition oracle over complete probability rows."""

import json
import math


rows = [
    [1.0, 0.0, 0.0],
    [0.5, 0.5, 0.0],
    [0.0, 0.25, 0.75],
    [0.0, 0.0, 1.0],
]


def entropy(values: list[float]) -> float:
    return -sum(value * math.log(value) for value in values if value > 0.0)


means = [sum(row[index] for row in rows) / len(rows) for index in range(len(rows[0]))]
aggregate_entropy = entropy(means)
result = {
    "oracle": "Python standard-library complete simplex rows",
    "mean_probabilities": means,
    "mean_row_entropy_nats": sum(entropy(row) for row in rows) / len(rows),
    "aggregate_composition_entropy_nats": aggregate_entropy,
    "effective_class_count": math.exp(aggregate_entropy),
    "maximum_row_sum_absolute_error": max(abs(sum(row) - 1.0) for row in rows),
}
print(json.dumps(result, indent=2, sort_keys=True))
