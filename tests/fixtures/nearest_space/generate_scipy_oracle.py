#!/usr/bin/env python3
"""Regenerate the pinned rectangular F/G/J nearest-distance oracle."""

from __future__ import annotations

import json

import numpy as np
import scipy
from scipy.spatial import cKDTree


if scipy.__version__ != "1.18.1":
    raise SystemExit(f"expected scipy 1.18.1, found {scipy.__version__}")

points = np.asarray([[1, 1], [2, 6], [5, 5], [7, 8], [9, 2]], dtype=float)
probes = np.asarray(
    [[1.25 + 2.5 * x, 1.25 + 2.5 * y] for y in range(4) for x in range(4)],
    dtype=float,
)
event_distances = cKDTree(points).query(points, k=2)[0][:, 1]
probe_distances = cKDTree(points).query(probes, k=1)[0]
rows = []
for radius in (0.5, 1.0, 2.5):
    event_eligible = np.minimum.reduce(
        [points[:, 0], 10 - points[:, 0], points[:, 1], 10 - points[:, 1]]
    ) >= radius
    probe_eligible = np.minimum.reduce(
        [probes[:, 0], 10 - probes[:, 0], probes[:, 1], 10 - probes[:, 1]]
    ) >= radius
    g = float(np.mean(event_distances[event_eligible] <= radius))
    f = float(np.mean(probe_distances[probe_eligible] <= radius))
    rows.append(
        {
            "radius_um": radius,
            "eligible_events": int(event_eligible.sum()),
            "events_with_neighbor": int(
                (event_distances[event_eligible] <= radius).sum()
            ),
            "g": g,
            "eligible_probes": int(probe_eligible.sum()),
            "probes_with_event": int(
                (probe_distances[probe_eligible] <= radius).sum()
            ),
            "f": f,
            "j": (1 - g) / (1 - f) if f < 1 else None,
        }
    )

print(
    json.dumps(
        {
            "oracle": "scipy.spatial.cKDTree",
            "scipy_version": scipy.__version__,
            "points": points.tolist(),
            "probe_grid": [4, 4],
            "radii": rows,
        },
        indent=2,
    )
)
