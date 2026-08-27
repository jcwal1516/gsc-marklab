#!/usr/bin/env python3
import json
import math


POINTS = [(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (3.0, 0.0)]
MARKS = [1.0, 2.0, 3.0, 4.0]
RADII = [0.5, 1.1]
WINDOW = (-2.0, 5.0, -2.0, 2.0)


def boundary_distance(point):
    x, y = point
    xmin, xmax, ymin, ymax = WINDOW
    return min(x - xmin, xmax - x, y - ymin, ymax - y)


mean = math.fsum(MARKS) / len(MARKS)
variance = math.fsum((value - mean) ** 2 for value in MARKS) / len(MARKS)
ordered_products = math.fsum(
    MARKS[left] * MARKS[right]
    for left in range(len(MARKS))
    for right in range(len(MARKS))
    if left != right
)
expected = ordered_products / (len(MARKS) * (len(MARKS) - 1)) / mean**2
curve = []
lower = 0.0
for radius in RADII:
    products = []
    for source, source_point in enumerate(POINTS):
        if boundary_distance(source_point) < radius:
            continue
        for target, target_point in enumerate(POINTS):
            if source == target:
                continue
            distance = math.dist(source_point, target_point)
            if lower < distance <= radius:
                products.append(MARKS[source] * MARKS[target])
    product_sum = math.fsum(products)
    curve.append(
        {
            "correlation": product_sum / len(products) / mean**2 if products else None,
            "directed_pairs": len(products),
            "mark_product_sum": product_sum,
            "radius_um": radius,
            "shell_lower_um": lower,
        }
    )
    lower = radius

print(
    json.dumps(
        {
            "curve": curve,
            "expected_random_label_correlation": expected,
            "global_mark_mean": mean,
            "global_mark_population_variance": variance,
        },
        indent=2,
        sort_keys=True,
    )
)
