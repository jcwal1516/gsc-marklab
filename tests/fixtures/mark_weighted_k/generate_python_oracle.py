#!/usr/bin/env python3
import json
import math


POINTS = [(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (3.0, 0.0)]
MARKS = [1.0, 2.0, 3.0, 4.0]
RADII = [0.5, 1.1]
WINDOW = (-2.0, 5.0, -2.0, 2.0)
AREA = (WINDOW[1] - WINDOW[0]) * (WINDOW[3] - WINDOW[2])


def boundary_distance(point):
    x, y = point
    xmin, xmax, ymin, ymax = WINDOW
    return min(x - xmin, xmax - x, y - ymin, ymax - y)


mean = math.fsum(MARKS) / len(MARKS)
ordered_products = math.fsum(
    MARKS[left] * MARKS[right]
    for left in range(len(MARKS))
    for right in range(len(MARKS))
    if left != right
)
expected_weight = ordered_products / (len(MARKS) * (len(MARKS) - 1)) / mean**2
curve = []
for radius in RADII:
    eligible = [
        index
        for index, point in enumerate(POINTS)
        if boundary_distance(point) >= radius
    ]
    products = []
    for source in eligible:
        for target in range(len(POINTS)):
            if source != target and math.dist(POINTS[source], POINTS[target]) <= radius:
                products.append(MARKS[source] * MARKS[target])
    product_sum = math.fsum(products)
    denominator = len(MARKS) * len(eligible)
    unweighted = AREA * len(products) / denominator if denominator else None
    normalized_sum = product_sum / mean**2
    curve.append(
        {
            "directed_pairs": len(products),
            "eligible_centers": len(eligible),
            "expected_random_label_weighted_k": (
                unweighted * expected_weight if unweighted is not None else None
            ),
            "mark_product_sum": product_sum,
            "normalized_weight_sum": normalized_sum,
            "radius_um": radius,
            "unweighted_k": unweighted,
            "weighted_k": AREA * normalized_sum / denominator if denominator else None,
        }
    )

print(
    json.dumps(
        {
            "curve": curve,
            "expected_random_label_weight": expected_weight,
            "global_mark_mean": mean,
        },
        indent=2,
        sort_keys=True,
    )
)
