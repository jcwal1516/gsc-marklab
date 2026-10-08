#!/usr/bin/env python3
import json
import math

intensities = [
    0.06589474481377681,
    0.05976753571179758,
    0.03728287154067532,
    0.04968278901946622,
]
eligible = [0, 1, 2]
radius_um = 1.0
pair_bandwidth_um = 0.5
kernel_at_one = 0.75 / pair_bandwidth_um
kernel_sum = 2.0 * kernel_at_one / (intensities[0] * intensities[1])
center_sum = math.fsum(1.0 / intensities[row] for row in eligible)
result = {
    "directed_pairs_in_support": 2,
    "eligible_center_inverse_intensity_sum": center_sum,
    "eligible_centers": len(eligible),
    "g": kernel_sum / (2.0 * math.pi * radius_um * center_sum),
    "intensity_bandwidth_um": 2.0,
    "inverse_intensity_kernel_sum": kernel_sum,
    "kernel": "epanechnikov",
    "pair_bandwidth_um": pair_bandwidth_um,
    "radius_um": radius_um,
}
print(json.dumps(result, indent=2, sort_keys=True))
