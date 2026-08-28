#!/usr/bin/env python3
"""Independent standard-library ordinal composition oracle."""

import json
import math


levels = ["negative", "weak", "moderate", "strong"]
values = [0, 1, 1, 3]
counts = [values.count(index) for index in range(len(levels))]
proportions = [count / len(values) for count in counts]
cumulative = []
running = 0
for count in counts:
    running += count
    cumulative.append(running / len(values))
ordered = sorted(values)
entropy = -sum(value * math.log(value) for value in proportions if value > 0)

print(json.dumps({
    "oracle": "Python standard-library ordinal counts and order statistics",
    "levels": levels,
    "counts": counts,
    "proportions": proportions,
    "cumulative_proportions": cumulative,
    "lower_median_level": levels[ordered[(len(values) - 1) // 2]],
    "upper_median_level": levels[ordered[len(values) // 2]],
    "entropy_nats": entropy,
    "normalized_entropy": entropy / math.log(len(levels)),
    "effective_level_count": math.exp(entropy),
}, indent=2, sort_keys=True))
