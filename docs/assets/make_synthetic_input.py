"""Generate the synthetic marked pattern used by the README overview figure.

Usage:  python docs/assets/make_synthetic_input.py <out_dir>
Writes <out_dir>/cells.csv and <out_dir>/mask.geojson. Requires numpy + matplotlib.
The pattern is SYNTHETIC: two planted high-mark-rate territories on a homogeneous
background, inside an irregular polygonal window. It is not patient data.
"""

import csv
import json
import math
import sys

import numpy as np
from matplotlib.path import Path


def main(out):
    rng = np.random.default_rng(20260828)

    # Irregular tumor blob, ~2400 x 1700 um, star-shaped radial perturbation.
    cx, cy, n_v = 1200.0, 850.0, 220
    th = np.linspace(0, 2 * math.pi, n_v, endpoint=False)
    r = 760 + 190 * np.sin(3 * th + 0.7) + 110 * np.sin(5 * th + 2.1) + 55 * np.sin(9 * th + 0.3)
    px, py = cx + r * np.cos(th) * 1.35, cy + r * np.sin(th)
    ring = [[float(a), float(b)] for a, b in zip(px, py, strict=True)]
    ring.append(ring[0])
    mask = {"type": "MultiPolygon", "coordinates": [[ring]]}
    with open(f"{out}/mask.geojson", "w") as stream:
        json.dump(mask, stream)

    poly = Path(np.column_stack([px, py]))

    # Cells: homogeneous base + a few gland-like clusters, rejection sampled into mask.
    pts = []
    while len(pts) < 5200:
        batch = np.column_stack(
            [rng.uniform(cx - 1150, cx + 1150, 4000), rng.uniform(cy - 820, cy + 820, 4000)]
        )
        pts.extend(batch[poly.contains_points(batch)].tolist())
    pts = np.array(pts[:5200])

    clusters = np.array([[820.0, 640.0], [1690.0, 1010.0], [1310.0, 380.0]])
    for c in clusters:
        loc = c + rng.normal(0, 95, size=(420, 2))
        pts = np.vstack([pts, loc[poly.contains_points(loc)]])

    # Marks: base rate 0.28, elevated inside two territories -> real residual signal.
    territories = np.array([[880.0, 720.0, 300.0], [1660.0, 1020.0, 260.0]])
    p = np.full(len(pts), 0.28)
    for tx, ty, tr in territories:
        d = np.hypot(pts[:, 0] - tx, pts[:, 1] - ty)
        p = np.maximum(p, 0.86 * np.exp(-((d / tr) ** 2)))
    mark = (rng.random(len(pts)) < p).astype(int)

    header = [
        "x_um",
        "y_um",
        "mark",
        "case_id",
        "timepoint",
        "protein",
        "valid_tumor",
        "valid_ihc",
        "qc_bin",
        "component_id",
    ]
    with open(f"{out}/cells.csv", "w", newline="") as fh:
        w = csv.writer(fh)
        w.writerow(header)
        for (x, y), m in zip(pts, mark, strict=True):
            qc_bin = 10 if y < 850 else 20
            w.writerow(
                [f"{x:.3f}", f"{y:.3f}", m, "synthetic_demo", "post", "MSH6", "true", "true"]
                + [qc_bin, 1]
            )

    print("cells:", len(pts), "marked:", int(mark.sum()), "p:", round(mark.mean(), 3))


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else ".")
