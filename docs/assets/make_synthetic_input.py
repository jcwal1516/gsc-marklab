"""Generate the synthetic marked pattern used by the README overview figure.

Usage:  python docs/assets/make_synthetic_input.py <out_dir>
Writes <out_dir>/cells.csv and <out_dir>/mask.geojson. Requires numpy + matplotlib.
The pattern is SYNTHETIC: two planted high-mark-rate territories on a homogeneous
background, inside an irregular polygonal window. It is not patient data.
"""
import json, math, csv, sys
import numpy as np

OUT = sys.argv[1] if len(sys.argv) > 1 else "."

rng = np.random.default_rng(20260828)

# Irregular tumor blob, ~2400 x 1700 um, star-shaped radial perturbation.
cx, cy, n_v = 1200.0, 850.0, 220
th = np.linspace(0, 2*math.pi, n_v, endpoint=False)
r = (760 + 190*np.sin(3*th + 0.7) + 110*np.sin(5*th + 2.1)
        + 55*np.sin(9*th + 0.3))
px, py = cx + r*np.cos(th)*1.35, cy + r*np.sin(th)
ring = [[float(a), float(b)] for a, b in zip(px, py)]
ring.append(ring[0])
mask = {"type": "MultiPolygon", "coordinates": [[ring]]}
json.dump(mask, open(f"{OUT}/mask.geojson", "w"))

from matplotlib.path import Path
poly = Path(np.column_stack([px, py]))

# Cells: homogeneous base + a few gland-like clusters, rejection sampled into mask.
pts = []
while len(pts) < 5200:
    batch = np.column_stack([rng.uniform(cx-1150, cx+1150, 4000),
                             rng.uniform(cy-820,  cy+820,  4000)])
    pts.extend(batch[poly.contains_points(batch)].tolist())
pts = np.array(pts[:5200])

clusters = np.array([[820., 640.], [1690., 1010.], [1310., 380.]])
for c in clusters:
    loc = c + rng.normal(0, 95, size=(420, 2))
    pts = np.vstack([pts, loc[poly.contains_points(loc)]])

# Marks: base rate 0.28, elevated inside two territories -> real residual signal.
terr = np.array([[880., 720., 300.], [1660., 1020., 260.]])
p = np.full(len(pts), 0.28)
for tx, ty, tr in terr:
    d = np.hypot(pts[:,0]-tx, pts[:,1]-ty)
    p = np.maximum(p, 0.86*np.exp(-(d/tr)**2))
mark = (rng.random(len(pts)) < p).astype(int)

with open(f"{OUT}/cells.csv", "w", newline="") as fh:
    w = csv.writer(fh)
    w.writerow(["x_um","y_um","mark","case_id","timepoint","protein","valid_tumor","valid_ihc","qc_bin","component_id"])
    for (x, y), m in zip(pts, mark):
        w.writerow([f"{x:.3f}", f"{y:.3f}", m, "synthetic_demo", "post", "MSH6", "true", "true",
                    10 if y < 850 else 20, 1])

print("cells:", len(pts), "marked:", int(mark.sum()), "p:", round(mark.mean(), 3))
