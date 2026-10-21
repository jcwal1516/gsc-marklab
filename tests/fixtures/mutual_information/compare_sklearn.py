"""Compare `marklab numerics mutual-information` with scikit-learn and calibrate its null.

Run from the repository root with a Python that has numpy, scipy and scikit-learn:
    python -I tests/fixtures/mutual_information/compare_sklearn.py \
        /path/to/marklab /path/to/CRC_clusters_neighborhoods_markers.csv /path/to/new/output

Fixed before any comparison was run (2026-10-10):

Part A, estimate agreement. Data: Schurch et al. (2020, CC BY 4.0), the first ten TMA A spots
by file name. Pairs: Ki67 with beta-catenin, PD-L1 with HLA-DR, CD44 with vimentin; the
population is the cells with both values above zero (zero-inflated intensities are ties the
continuous estimator excludes). k = 3 and 5; 60 cases. Reference:
sklearn.feature_selection.mutual_info_regression with n_neighbors = k and random_state = 0.
It scales each variable by its standard deviation (as Marklab does), adds noise of 1e-10
times the mean absolute value (expected to change no neighbor count) and clips negative
estimates to 0. Tolerances: where sklearn reports a positive value, |Marklab - sklearn| <=
1e-9 max(1, |sklearn|); where it reports 0, Marklab's unclipped estimate <= 1e-9. Every case
must be computed by Marklab.

Part B, null calibration under spatial dependence. Positions: all cells of the first spot.
Marks: two independent zero-mean Gaussian fields with exponential covariance exp(-d / range)
at the cell positions, range 10, 50 and 200 um; for power, the second mark is
0.4 first + sqrt(0.84) independent at range 50. k = 3, 99 permutations, 200 replicates per
scenario, numpy seed 20261010, Marklab seed = replicate index. Designs: 100 um blocks from
the frame corner, and one block (the ordinary permutation test). Report the rejection rate at
p <= 0.05 with a Clopper-Pearson 95% interval. A design is read as controlling size in a
scenario when its rate is at most 0.081 (0.05 + 2 binomial standard errors); no design is
required to pass, and the rates are reported whatever they are.

Revision after run 01 (no tolerance or criterion changed): run 01 showed that neither the
block design nor the ordinary permutation controls size for fields with ranges of 50 and
200 um. Run 02 adds a third design fixed before it ran, variogram-matching surrogates with
smoothing neighbor counts 4, 8, 16, 32, 64 and 128 and lag edges 0 to 240 um in 20 um steps,
judged by the same 0.081 rule. Part A and the other two designs are unchanged and, being
deterministic, are expected to reproduce run 01.
"""

import json
import pathlib
import subprocess
import sys
import tempfile

import numpy as np
import scipy
import sklearn
from scipy.stats import beta
from sklearn.feature_selection import mutual_info_regression

UM_PER_PIXEL = 0.377442
FRAME_ORIGIN = -0.5 * UM_PER_PIXEL
PAIRS = [
    ("Ki67 - proliferation:Cyc_5_ch_4", "beta-catenin - Wnt signaling:Cyc_4_ch_4"),
    ("PD-L1 - checkpoint:Cyc_5_ch_3", "HLA-DR - MHC-II:Cyc_5_ch_2"),
    ("CD44 - stroma:Cyc_2_ch_2", "Vimentin - cytoplasm:Cyc_8_ch_2"),
]
TOLERANCE = 1e-9
RANGES = (10.0, 50.0, 200.0)
REPLICATES = 200
SIZE_LIMIT = 0.081


def read_cells(csv_path):
    import csv

    rows = []
    with open(csv_path, newline="") as handle:
        for row in csv.DictReader(handle):
            if row["TMA_AB"] == "A":
                rows.append(row)
    spots = sorted({row["File Name"] for row in rows})[:10]
    return spots, rows


VARIOGRAM_NULL = {"kind": "variogram_matching", "smoothing_neighbors": [4, 8, 16, 32, 64, 128],
                  "lag_edges_um": [20.0 * step for step in range(13)]}


def blocks(width):
    return {"kind": "block_permutation", "origin_um": [FRAME_ORIGIN, FRAME_ORIGIN],
            "width_um": width}


def request(cells, first_name, second_name, k, null, permutations, seed):
    return {
        "format": "marklab.mutual_information_request",
        "version": 1,
        "coordinate_frame_id": "schurch-um",
        "first_name": first_name,
        "second_name": second_name,
        "cells": cells,
        "neighbors_k": k,
        "null": null,
        "permutations": permutations,
        "seed": seed,
        "limits": {"maximum_cells": 5000, "maximum_distance_evaluations": 4_000_000_000,
                   "maximum_variogram_pairs": 2_000_000},
    }


def run(executable, payload, input_path, output_path):
    input_path.write_text(json.dumps(payload))
    completed = subprocess.run(
        [executable, "numerics", "mutual-information", "--input", str(input_path),
         "--out", str(output_path)],
        capture_output=True, text=True, check=False)
    if completed.returncode != 0:
        return None, completed.stderr.strip()
    return json.loads(output_path.read_text()), None


def part_a(executable, spots, rows, output):
    cases = []
    for spot in spots:
        here = [row for row in rows if row["File Name"] == spot]
        for first, second in PAIRS:
            chosen = [row for row in here if float(row[first]) > 0 and float(row[second]) > 0]
            x = np.array([float(row[first]) for row in chosen])
            y = np.array([float(row[second]) for row in chosen])
            cells = [
                {"id": f"c{row['CellID']}", "x_um": float(row["X:X"]) * UM_PER_PIXEL,
                 "y_um": float(row["Y:Y"]) * UM_PER_PIXEL, "first": float(a), "second": float(b)}
                for row, a, b in zip(chosen, x, y)
            ]
            for k in (3, 5):
                stem = f"{spot}-{first.split(' ')[0]}-{second.split(' ')[0]}-k{k}"
                result, error = run(
                    executable, request(cells, first, second, k, blocks(1e6), 0, 0),
                    output / f"{stem}-input.json", output / f"{stem}-result.json")
                reference = float(mutual_info_regression(
                    x.reshape(-1, 1), y, n_neighbors=k, random_state=0)[0])
                if result is None:
                    cases.append({"case": stem, "cells": len(chosen), "reference": reference,
                                  "error": error, "passed": False})
                    continue
                estimate = result["estimate_nats"]
                if reference > 0:
                    difference = abs(estimate - reference) / max(1.0, abs(reference))
                    passed = difference <= TOLERANCE
                else:
                    difference = max(0.0, estimate)
                    passed = estimate <= TOLERANCE
                cases.append({"case": stem, "cells": len(chosen), "reference": reference,
                              "estimate": estimate, "difference": difference,
                              "passed": passed})
                print(stem, len(chosen), f"{reference:.6f}", f"{estimate:.6f}", passed)
    return cases


def gaussian_field(points, correlation_range, rng):
    distance = np.hypot(*(points[:, None, :] - points[None, :, :]).transpose(2, 0, 1))
    covariance = np.exp(-distance / correlation_range) + 1e-10 * np.eye(len(points))
    return np.linalg.cholesky(covariance)


def interval(successes, trials):
    lower = 0.0 if successes == 0 else beta.ppf(0.025, successes, trials - successes + 1)
    upper = 1.0 if successes == trials else beta.ppf(0.975, successes + 1, trials - successes)
    return [float(lower), float(upper)]


def part_b(executable, spots, rows, output):
    here = [row for row in rows if row["File Name"] == spots[0]]
    points = np.array([[float(row["X:X"]) * UM_PER_PIXEL, float(row["Y:Y"]) * UM_PER_PIXEL]
                       for row in here])
    rng = np.random.default_rng(20261010)
    scenarios = [(f"independent-range-{int(r)}", r, 0.0) for r in RANGES]
    scenarios.append(("dependent-0.4-range-50", 50.0, 0.4))
    summary = []
    with tempfile.TemporaryDirectory() as scratch:
        scratch = pathlib.Path(scratch)
        for name, correlation_range, coupling in scenarios:
            factor = gaussian_field(points, correlation_range, rng)
            rejections = {"blocks_100um": 0, "one_block": 0, "variogram_matching": 0}
            for replicate in range(REPLICATES):
                first = factor @ rng.standard_normal(len(points))
                other = factor @ rng.standard_normal(len(points))
                second = coupling * first + np.sqrt(1 - coupling**2) * other
                cells = [{"id": f"c{i}", "x_um": float(p[0]), "y_um": float(p[1]),
                          "first": float(a), "second": float(b)}
                         for i, (p, a, b) in enumerate(zip(points, first, second))]
                designs = (("blocks_100um", blocks(100.0)), ("one_block", blocks(1e6)),
                           ("variogram_matching", VARIOGRAM_NULL))
                for design, null in designs:
                    result_path = scratch / f"{name}-{design}-{replicate}.json"
                    result, error = run(
                        executable, request(cells, "field_1", "field_2", 3, null, 99,
                                            replicate),
                        scratch / "input.json", result_path)
                    if result is None:
                        raise SystemExit(f"{name} {design} {replicate}: {error}")
                    rejections[design] += result["upper_p_value"] <= 0.05
            for design, count in rejections.items():
                rate = count / REPLICATES
                row = {"scenario": name, "range_um": correlation_range, "coupling": coupling,
                       "design": design, "rejections": count, "replicates": REPLICATES,
                       "rate": rate, "interval_95": interval(count, REPLICATES)}
                if coupling == 0.0:
                    row["controls_size"] = rate <= SIZE_LIMIT
                summary.append(row)
                print(name, design, count, rate)
    return summary


def main():
    if len(sys.argv) != 4:
        raise SystemExit("expected Marklab executable, Schurch CSV and a new output directory")
    executable = str(pathlib.Path(sys.argv[1]).resolve(strict=True))
    output = pathlib.Path(sys.argv[3])
    output.mkdir(parents=True, exist_ok=False)
    spots, rows = read_cells(sys.argv[2])
    cases = part_a(executable, spots, rows, output)
    calibration = part_b(executable, spots, rows, output)
    differences = [case["difference"] for case in cases if "difference" in case]
    summary = {
        "format": "marklab.mutual_information_reference_comparison",
        "version": 1,
        "versions": {"scikit-learn": sklearn.__version__, "scipy": scipy.__version__,
                     "numpy": np.__version__},
        "tolerance": TOLERANCE,
        "estimate_cases": cases,
        "max_difference": max(differences) if differences else None,
        "estimates_passed": all(case["passed"] for case in cases),
        "calibration": calibration,
    }
    (output / "summary.json").write_text(json.dumps(summary, indent=2))
    print("estimates passed", summary["estimates_passed"], "max difference",
          summary["max_difference"])
    if not summary["estimates_passed"]:
        raise SystemExit("mutual-information estimate comparison failed; see summary.json")


if __name__ == "__main__":
    main()
