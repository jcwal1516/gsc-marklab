"""Compare `marklab numerics bond-orientational-order` with freud 3.6.1 `order.Hexatic`.

Run from the repository root with an isolated interpreter that has freud-analysis 3.6.1:

    target/method-reference-venv/bin/python -I tests/fixtures/bond_order/compare_freud.py \
        /path/to/marklab /path/to/CRC_clusters_neighborhoods_markers.csv /path/to/new/output

Fixed before any comparison was run (2026-10-10):

* Data: Schurch et al. (2020, CC BY 4.0) "tumor cells" in the first three TMA A spots by file
  name, coordinates x 0.377442 um per pixel, window = the published 1920 x 1440 pixel frame.
* Order: k = 6 with (a) the 6 nearest neighbors and (b) all neighbors within 25 um.
* freud: a 4000 um box centred on the frame; neighbors from AABBQuery with exclude_ii;
  unweighted Hexatic(k=6). freud 3.6.1 rejects pair queries in non-periodic boxes (found on
  the first run, before any comparison), so the box stays periodic: the nearest periodic image
  of a cell is at least 4000 - 725 = 3275 um away, far beyond any neighbor distance used here.
* Compared cells: those Marklab includes (border rule) and, for (a), without a distance tie at
  the sixth neighbor, where tie-breaking legitimately differs.
* Tolerances: identical neighbor counts; real and imaginary parts within 1e-12 absolute.

Revision after the first run, which stopped before reporting any error: freud 3.6.1 stores
positions and order parameters in single precision (its results are float32), so the 1e-12
criterion against freud's own values was unattainable by construction. Without inspecting
any comparison value, the criteria became:

1. psi_6 recomputed in float64 from freud's own neighbor lists must equal Marklab's within
   1e-12 absolute (this also confirms identical neighbor sets);
2. freud's float32 psi_6 must lie within the a priori single-precision bound
   B_i = k * 2 sqrt(2) * M * 2^-24 / d_i + 1e-6, with M the largest centred coordinate
   magnitude and d_i the cell's nearest-neighbor distance (bond-angle error from rounding both
   endpoints, scaled by k, plus float32 summation error).
"""

import csv
import json
import subprocess
import sys
from pathlib import Path

import freud
import numpy as np

TOLERANCE = 1e-12
FLOAT32_EPSILON = 2.0**-24
UM_PER_PIXEL = 0.377442
FRAME = np.asarray([-0.5, 1919.5, -0.5, 1439.5]) * UM_PER_PIXEL


def main():
    executable, source, output = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
    if output.exists():
        raise SystemExit("output path must not exist")
    if freud.__version__ != "3.6.1":
        raise SystemExit(f"freud 3.6.1 required, found {freud.__version__}")
    output.mkdir(parents=True)
    spots = {}
    with source.open(newline="") as handle:
        for row in csv.DictReader(handle):
            if row["TMA_AB"] == "A" and row["ClusterName"] == "tumor cells":
                spots.setdefault(row["File Name"], []).append(row)
    window = {"type": "MultiPolygon", "coordinates": [[[
        [FRAME[0], FRAME[2]], [FRAME[1], FRAME[2]], [FRAME[1], FRAME[3]], [FRAME[0], FRAME[3]],
        [FRAME[0], FRAME[2]]]]]}
    rules = {
        "nearest6": ({"kind": "nearest_count", "count": 6}, {"num_neighbors": 6}),
        "radius25": ({"kind": "radius", "radius_um": 25.0}, {"r_max": 25.0}),
    }
    cases = []
    for spot in sorted(spots)[:3]:
        rows = spots[spot]
        points = np.asarray([[float(r["X:X"]), float(r["Y:Y"])] for r in rows]) * UM_PER_PIXEL
        center = np.asarray([(FRAME[0] + FRAME[1]) / 2.0, (FRAME[2] + FRAME[3]) / 2.0])
        box = freud.box.Box(Lx=4000.0, Ly=4000.0, is2D=True)
        system = (box, np.column_stack([points - center, np.zeros(len(points))]))
        for name, (marklab_rule, freud_query) in rules.items():
            request = {
                "format": "marklab.bond_orientational_order_request", "version": 1,
                "coordinate_frame_id": f"{spot}-um", "window": window,
                "cells": [{"id": f"c{r['CellID']}", "x_um": float(p[0]), "y_um": float(p[1])}
                          for r, p in zip(rows, points, strict=True)],
                "symmetry": 6, "neighbors": marklab_rule, "limits": {"maximum_cells": 100000},
            }
            stem = f"{spot}-{name}"
            (output / f"{stem}-request.json").write_text(json.dumps(request))
            subprocess.run([str(executable), "numerics", "bond-orientational-order", "--input",
                            str(output / f"{stem}-request.json"), "--out",
                            str(output / f"{stem}-result.json")], check=True)
            result = json.loads((output / f"{stem}-result.json").read_text())
            neighbors = freud.locality.AABBQuery(*system).query(
                system[1], {**freud_query, "exclude_ii": True}).toNeighborList()
            hexatic = freud.order.Hexatic(k=6).compute(system, neighbors=neighbors)
            reference = np.asarray(hexatic.particle_order, dtype=np.complex128)
            counts = np.bincount(neighbors.query_point_indices, minlength=len(points))
            # Float64 psi_6 from freud's neighbor lists, independently of both programs' sums.
            exact = np.zeros(len(points), dtype=np.complex128)
            bonds = points[neighbors.point_indices] - points[neighbors.query_point_indices]
            np.add.at(exact, neighbors.query_point_indices,
                      np.exp(6j * np.arctan2(bonds[:, 1], bonds[:, 0])))
            exact /= np.maximum(counts, 1)
            nearest = np.full(len(points), np.inf)
            np.minimum.at(nearest, neighbors.query_point_indices, np.hypot(bonds[:, 0], bonds[:, 1]))
            scale = float(np.max(np.abs(points - center)))
            compared, count_mismatch, worst_exact, worst_ratio = 0, 0, 0.0, 0.0
            for index, cell in enumerate(result["cells"]):
                if not cell["included"] or cell["tie_at_cutoff"]:
                    continue
                compared += 1
                if cell["neighbors"] != counts[index]:
                    count_mismatch += 1
                    continue
                psi = complex(cell["psi_real"], cell["psi_imaginary"])
                worst_exact = max(worst_exact, abs(psi.real - exact[index].real),
                                  abs(psi.imag - exact[index].imag))
                bound = 6 * 2 * np.sqrt(2) * scale * FLOAT32_EPSILON / nearest[index] + 1e-6
                error = max(abs(psi.real - reference[index].real),
                            abs(psi.imag - reference[index].imag))
                worst_ratio = max(worst_ratio, error / bound)
            # Diagnostics independent of Marklab: freud's float32 output, and float64 arithmetic on
            # float32-rounded positions, each against the float64 evaluation.
            rounded = np.asarray(system[1], np.float32).astype(np.float64)[:, :2]
            rounded_bonds = rounded[neighbors.point_indices] - rounded[neighbors.query_point_indices]
            from_rounded = np.zeros(len(points), dtype=np.complex128)
            np.add.at(from_rounded, neighbors.query_point_indices,
                      np.exp(6j * np.arctan2(rounded_bonds[:, 1], rounded_bonds[:, 0])))
            from_rounded /= np.maximum(counts, 1)
            has = counts > 0
            freud_vs_exact = float(np.max(np.abs(reference[has] - exact[has])))
            rounding_vs_exact = float(np.max(np.abs(from_rounded[has] - exact[has])))
            passed = bool(compared > 0 and count_mismatch == 0 and worst_exact <= TOLERANCE
                          and worst_ratio <= 1.0)
            worst_exact, worst_ratio = float(worst_exact), float(worst_ratio)
            cases.append({"case": stem, "cells": len(points), "included": result["included_cells"],
                          "compared": compared, "neighbor_count_mismatches": count_mismatch,
                          "maximum_float64_recomputation_error": worst_exact,
                          "maximum_float32_error_over_bound": worst_ratio,
                          "freud_float32_vs_float64_evaluation": freud_vs_exact,
                          "float32_rounded_positions_vs_float64_evaluation": rounding_vs_exact,
                          "mean_magnitude": result["mean_magnitude"], "passed": passed})
            print(json.dumps(cases[-1]))
    report = {"format": "marklab.bond_order_freud_comparison", "version": 1,
              "freud": freud.__version__, "float64_tolerance_absolute": TOLERANCE,
              "float32_bound": "6 * 2 sqrt(2) * M * 2^-24 / d_i + 1e-6", "cases": cases,
              "passed": all(case["passed"] for case in cases)}
    (output / "summary.json").write_text(json.dumps(report, indent=1))
    if not report["passed"]:
        raise SystemExit("bond-orientational order comparison failed; see summary.json")


if __name__ == "__main__":
    main()
