"""Compare `marklab numerics hyperuniformity` with the structure-factor package.

Run from the repository root:
    R_HOME=$(R RHOME) python -I tests/fixtures/hyperuniformity/compare_structure_factor.py \
        /path/to/marklab /path/to/unpacked/structure_factor-2.2.1-wheel \
        /path/to/CRC_clusters_neighborhoods_markers.csv /path/to/new/output

structure-factor 2.2.1 (MIT; Hawat, Gautier, Bardenet and Lachieze-Rey) declares Python < 3.10
and numpy < 2. It runs here from its unpacked wheel on Python 3.13 and numpy 2 with two
adaptations, neither touching the compared code: numba's njit is replaced by an identity
decorator (the package compiles an isotropic Bessel helper at import, which the compared
estimators never call), and the allowed wavevectors are passed explicitly because the
package's own generator fails under numpy 2. The script reports the wheel's SHA-256 so the
exact reference is identified.

Fixed before any comparison was run (2026-10-10):
* Data: Schurch et al. (2020, CC BY 4.0), the first five TMA A spots by file name, all cells
  except the "dirt" cluster; positions are pixels x 0.377442 um.
* Window: the 300 um square centred on the frame centre (959.5, 719.5 pixels); cells inside it.
* Wavevectors: k = 2 pi m / 300 um, m != 0, |m_x|, |m_y| <= floor(1.0 * 300 / 2 pi) = 47.
* Bins: |k| edges 4 pi j / 300 for j = 0..23; fit stop at the centre of bin 5, so bins 0-4
  are fitted (the package fits the points before the bin closest to the stop).
* Reference: StructureFactor(PointPattern(points, BoxWindow)).scattering_intensity(k=...);
  bin_data(|k|, S, bins=edges); effective_hyperuniformity and hyperuniformity_class on the
  binned means without standard deviations (unweighted), which fit by scipy curve_fit.
* Revision after run 02 (no tolerance changed): the edges 4 pi j / 300 equal the wavenumbers
  of allowed wavevectors with |m| = 2j, so bin membership at an edge depended on boundary
  conventions. Run 02 failed because scipy closes the last bin on the right while Marklab's
  bins are half-open: the four wavevectors with |m| = 46 changed the last bin's mean by up to
  2.5e-3 (per-wavevector S and the fits agreed). Run 03 uses edges 4 pi (j + 1/4) / 300,
  j = 0..23, whose squares are never (2 pi / 300)^2 times an integer, so no wavevector lies on
  an edge; the fit stop is again the centre of bin 5.
* Tolerances: S at every wavevector and every bin mean within 1e-9 absolute; identical bin
  counts and first-peak bin; intercept, slope and H within 1e-7 scaled error
  max(1, |reference|) (curve_fit stops at a relative tolerance of 1.5e-8); alpha and c within
  1e-5 scaled, since the reference minimizes from (1, 1) by Levenberg-Marquardt and Marklab by
  a profiled grid and golden-section search.
"""

import hashlib
import json
import pathlib
import subprocess
import sys
import types

import numpy as np

UM_PER_PIXEL = 0.377442
SIDE = 300.0
CENTER = np.array([959.5, 719.5]) * UM_PER_PIXEL
K_MAX = 1.0
EDGES = [4 * np.pi * (j + 0.25) / SIDE for j in range(24)]
STOP = (EDGES[5] + EDGES[6]) / 2
TOLERANCES = {"values": 1e-9, "line": 1e-7, "power_law": 1e-5}


def load_reference(unpacked):
    stub = types.ModuleType("numba")
    stub.njit = lambda *args, **kwargs: (lambda function: function)
    sys.modules["numba"] = stub
    sys.path.insert(0, str(unpacked))
    from structure_factor.hyperuniformity import (bin_data, effective_hyperuniformity,
                                                  hyperuniformity_class)
    from structure_factor.point_pattern import PointPattern
    from structure_factor.spatial_windows import BoxWindow
    from structure_factor.structure_factor import StructureFactor
    return BoxWindow, PointPattern, StructureFactor, bin_data, effective_hyperuniformity, \
        hyperuniformity_class


def scaled(observed, reference):
    return abs(observed - reference) / max(1.0, abs(reference))


def main():
    if len(sys.argv) != 5:
        raise SystemExit("expected Marklab, the unpacked wheel, the Schurch CSV and an output")
    executable = str(pathlib.Path(sys.argv[1]).resolve(strict=True))
    unpacked = pathlib.Path(sys.argv[2]).resolve(strict=True)
    output = pathlib.Path(sys.argv[4])
    output.mkdir(parents=True, exist_ok=False)
    (BoxWindow, PointPattern, StructureFactor, bin_data, effective_hyperuniformity,
     hyperuniformity_class) = load_reference(unpacked)
    origin = CENTER - SIDE / 2
    import collections
    import csv

    spots = collections.defaultdict(list)
    with open(sys.argv[3], newline="") as handle:
        for row in csv.DictReader(handle):
            if row["TMA_AB"] == "A" and row["ClusterName"] != "dirt":
                spots[row["File Name"]].append(row)
    reach = int(np.floor(K_MAX * SIDE / (2 * np.pi)))
    m = np.array([(a, b) for a in range(-reach, reach + 1) for b in range(-reach, reach + 1)
                  if (a, b) != (0, 0)], dtype=float)
    k = 2 * np.pi * m / SIDE
    cases = []
    for spot in sorted(spots)[:5]:
        rows = spots[spot]
        points = np.array([[float(r["X:X"]), float(r["Y:Y"])] for r in rows]) * UM_PER_PIXEL
        keep = np.all((points >= origin) & (points < origin + SIDE), axis=1)
        points = points[keep]
        request = {
            "format": "marklab.hyperuniformity_request", "version": 1,
            "coordinate_frame_id": f"{spot}-um",
            "window": {"origin_um": origin.tolist(), "side_um": [SIDE, SIDE]},
            "cells": [{"id": f"c{i}", "x_um": float(p[0]), "y_um": float(p[1])}
                      for i, p in enumerate(points)],
            "maximum_wavenumber_per_axis": K_MAX, "wavenumber_bin_edges": EDGES,
            "fit_wavenumber_stop": STOP,
            "limits": {"maximum_cells": 100000, "maximum_wavevector_evaluations": 100_000_000},
        }
        input_path = output / f"{spot}-input.json"
        result_path = output / f"{spot}-result.json"
        input_path.write_text(json.dumps(request))
        subprocess.run([executable, "numerics", "hyperuniformity", "--input", str(input_path),
                        "--out", str(result_path)], check=True)
        result = json.loads(result_path.read_text())

        window = BoxWindow(bounds=np.array([[origin[0], origin[0] + SIDE],
                                            [origin[1], origin[1] + SIDE]]))
        _, s_reference = StructureFactor(PointPattern(points=points, window=window)) \
            .scattering_intensity(k=k)
        lookup = {tuple(v["m"]): v["structure_factor"] for v in result["wavevectors"]}
        s_marklab = np.array([lookup[(int(a), int(b))] for a, b in m])
        norms = np.linalg.norm(k, axis=1)
        centers, means, _ = bin_data(norms, s_reference, bins=EDGES)
        counts = np.histogram(norms, bins=EDGES)[0]
        line = effective_hyperuniformity(centers, means, k_norm_stop=STOP)
        power = hyperuniformity_class(centers, means, k_norm_stop=STOP)
        intercept, slope = line["fitted_line"](0.0), line["fitted_line"](1.0) - line["fitted_line"](0.0)
        bin_means = np.array([b["mean"] for b in result["bins"]], dtype=float)
        errors = {
            "structure_factor": float(np.max(np.abs(s_marklab - s_reference))),
            "bin_means": float(np.max(np.abs(bin_means - means))),
            "intercept": scaled(result["s0_intercept"], intercept),
            "slope": scaled(result["line_slope"], slope),
            "h_index": scaled(result["h_index"], line["H"]),
            "alpha": scaled(result["power_law_alpha"], power["alpha"]),
            "c": scaled(result["power_law_c"], power["c"]),
        }
        same_counts = [b["wavevectors"] for b in result["bins"]] == counts.tolist()
        same_peak = result["first_peak_bin"] == line["idx_first_peak"]
        passed = bool(
            same_counts and same_peak
            and errors["structure_factor"] <= TOLERANCES["values"]
            and errors["bin_means"] <= TOLERANCES["values"]
            and max(errors["intercept"], errors["slope"], errors["h_index"])
            <= TOLERANCES["line"]
            and max(errors["alpha"], errors["c"]) <= TOLERANCES["power_law"])
        peak = line["idx_first_peak"]
        cases.append({"spot": spot, "cells": int(len(points)), "h_index": float(line["H"]),
                      "alpha": float(power["alpha"]),
                      "first_peak_bin": None if peak is None else int(peak),
                      "marklab_first_peak_bin": result["first_peak_bin"],
                      "same_counts": same_counts, "errors": errors, "passed": passed})
        print(spot, len(points), f"H={line['H']:.4f}", f"alpha={power['alpha']:.4f}",
              line["idx_first_peak"], result["first_peak_bin"],
              {key: f"{value:.1e}" for key, value in errors.items()}, passed)
    wheel = next(unpacked.parent.glob("*.whl"), None)
    summary = {"format": "marklab.hyperuniformity_reference_comparison", "version": 1,
               "structure_factor": "2.2.1",
               "wheel_sha256": hashlib.sha256(wheel.read_bytes()).hexdigest() if wheel else None,
               "numpy": np.__version__, "tolerances": TOLERANCES, "cases": cases,
               "passed": all(case["passed"] for case in cases)}
    (output / "summary.json").write_text(json.dumps(summary, indent=2))
    print("passed", summary["passed"])
    if not summary["passed"]:
        raise SystemExit("hyperuniformity comparison failed; see summary.json")


if __name__ == "__main__":
    main()
