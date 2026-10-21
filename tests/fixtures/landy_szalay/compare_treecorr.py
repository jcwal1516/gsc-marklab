"""Compare `marklab numerics landy-szalay` with TreeCorr pair counts on public cells.

Run from the repository root with a Python that has numpy and treecorr:
    python -I tests/fixtures/landy_szalay/compare_treecorr.py \
        /path/to/marklab /path/to/CRC_clusters_neighborhoods_markers.csv /path/to/new/output

Fixed before any comparison was run (2026-10-10):
* Data: Schurch et al. (2020, CC BY 4.0), the first five TMA A spots by file name; tumor cells,
  CD8+ T cells and CD68+CD163+ macrophages with at least 20 cells. Positions are pixels x
  0.377442 um; the window is the published frame, edges at -0.5 and 1919.5 / 1439.5 pixels.
* Random catalog: max(2000, 20 n) points uniform in the frame from numpy default_rng with seed
  20261010 + case index.
* Bins: [0, 10), [10, 20), ..., [190, 200) um.
* Reference: TreeCorr 5.1.4 NNCorrelation with linear bins, bin_slop = 0 and brute force, so
  pair counts are exact. TreeCorr skips pairs at zero separation; coincident cells are counted
  independently here and added to its first bin, because they are distinct pairs at distance
  0. TreeCorr normalizes auto-pair totals by n^2 / 2, whereas Landy and Szalay (1993) use
  n(n - 1) / 2, as Marklab does, so the reference xi is computed from TreeCorr's counts with the
  1993 normalization; TreeCorr's own calculateXi is recorded with its difference.
* Criteria: identical DD, DR and RR counts in every bin; xi within 1e-12 scaled error
  max(1, |reference|).
"""

import json
import pathlib
import subprocess
import sys

import numpy as np
import treecorr

UM_PER_PIXEL = 0.377442
FRAME = np.array([-0.5, 1919.5, -0.5, 1439.5]) * UM_PER_PIXEL
TYPES = ["tumor cells", "CD8+ T cells", "CD68+CD163+ macrophages"]
EDGES = [10.0 * step for step in range(21)]
TOLERANCE = 1e-12


def read_cases(csv_path):
    import collections
    import csv

    spots = collections.defaultdict(list)
    with open(csv_path, newline="") as handle:
        for row in csv.DictReader(handle):
            if row["TMA_AB"] == "A":
                spots[row["File Name"]].append(row)
    for spot in sorted(spots)[:5]:
        for kind in TYPES:
            rows = [row for row in spots[spot] if row["ClusterName"] == kind]
            if len(rows) >= 20:
                yield spot, kind, rows


def correlation():
    return treecorr.NNCorrelation(min_sep=EDGES[0], max_sep=EDGES[-1], nbins=len(EDGES) - 1,
                                  bin_type="Linear", bin_slop=0, brute=True)


def main():
    if len(sys.argv) != 4:
        raise SystemExit("expected Marklab executable, Schurch CSV and a new output directory")
    executable = str(pathlib.Path(sys.argv[1]).resolve(strict=True))
    output = pathlib.Path(sys.argv[3])
    output.mkdir(parents=True, exist_ok=False)
    window = {"type": "MultiPolygon", "coordinates": [[[
        [FRAME[0], FRAME[2]], [FRAME[1], FRAME[2]], [FRAME[1], FRAME[3]], [FRAME[0], FRAME[3]],
        [FRAME[0], FRAME[2]]]]]}
    cases = []
    for index, (spot, kind, rows) in enumerate(read_cases(sys.argv[2])):
        data = np.array([[float(r["X:X"]), float(r["Y:Y"])] for r in rows]) * UM_PER_PIXEL
        rng = np.random.default_rng(20261010 + index)
        count = max(2000, 20 * len(data))
        randoms = np.column_stack([rng.uniform(FRAME[0], FRAME[1], count),
                                   rng.uniform(FRAME[2], FRAME[3], count)])
        stem = f"{spot}-{kind.replace(' ', '_').replace('+', 'p')}"
        request = {
            "format": "marklab.landy_szalay_request", "version": 1,
            "coordinate_frame_id": f"{spot}-um", "window": window,
            "cells": [{"id": f"c{r['CellID']}", "x_um": float(p[0]), "y_um": float(p[1])}
                      for r, p in zip(rows, data)],
            "random_catalog_id": f"uniform-frame-seed-{20261010 + index}",
            "random_points": randoms.tolist(), "bin_edges_um": EDGES,
            "limits": {"maximum_cells": 100000, "maximum_random_points": 1000000,
                       "maximum_pair_evaluations": 10_000_000_000},
        }
        input_path = output / f"{stem}-input.json"
        result_path = output / f"{stem}-result.json"
        input_path.write_text(json.dumps(request))
        subprocess.run([executable, "numerics", "landy-szalay", "--input", str(input_path),
                        "--out", str(result_path)], check=True)
        result = json.loads(result_path.read_text())

        data_catalog = treecorr.Catalog(x=data[:, 0], y=data[:, 1])
        random_catalog = treecorr.Catalog(x=randoms[:, 0], y=randoms[:, 1])
        dd, dr, rr = correlation(), correlation(), correlation()
        dd.process(data_catalog)
        dr.process(data_catalog, random_catalog)
        rr.process(random_catalog)
        _, counts = np.unique(data, axis=0, return_counts=True)
        coincident = int(np.sum(counts * (counts - 1) // 2))
        dd_counts = dd.npairs.copy()
        dd_counts[0] += coincident
        n, n_random = len(data), len(randoms)
        dd_n = dd_counts / (n * (n - 1) / 2)
        dr_n = dr.npairs / (n * n_random)
        rr_n = rr.npairs / (n_random * (n_random - 1) / 2)
        reference = (dd_n - 2 * dr_n + rr_n) / rr_n
        treecorr_xi, _ = dd.calculateXi(rr=rr, dr=dr)
        marklab_xi = np.array([b["xi"] for b in result["bins"]], dtype=float)
        counts_equal = (
            [b["data_data_pairs"] for b in result["bins"]] == dd_counts.astype(int).tolist()
            and [b["data_random_pairs"] for b in result["bins"]] == dr.npairs.astype(int).tolist()
            and [b["random_random_pairs"] for b in result["bins"]]
            == rr.npairs.astype(int).tolist())
        errors = np.abs(marklab_xi - reference) / np.maximum(1.0, np.abs(reference))
        passed = bool(counts_equal and np.all(errors <= TOLERANCE))
        cases.append({
            "case": f"{spot} {kind}", "cells": n, "random_points": n_random,
            "coincident_cell_pairs": coincident, "counts_equal": counts_equal,
            "max_xi_error": float(errors.max()),
            "max_difference_from_treecorr_normalization": float(
                np.max(np.abs(marklab_xi - treecorr_xi))),
            "xi_first_bin": float(reference[0]), "xi_at_50um": float(reference[5]),
            "passed": passed})
        print(spot, kind, n, coincident, counts_equal, f"{errors.max():.2e}",
              f"{reference[0]:.3f}", passed)
    summary = {"format": "marklab.landy_szalay_reference_comparison", "version": 1,
               "versions": {"treecorr": treecorr.__version__, "numpy": np.__version__},
               "tolerance": TOLERANCE, "cases": cases,
               "passed": all(case["passed"] for case in cases)}
    (output / "summary.json").write_text(json.dumps(summary, indent=2))
    print("passed", summary["passed"])
    if not summary["passed"]:
        raise SystemExit("Landy-Szalay comparison failed; see summary.json")


if __name__ == "__main__":
    main()
