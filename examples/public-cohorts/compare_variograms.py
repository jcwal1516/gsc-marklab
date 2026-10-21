#!/usr/bin/env python3
"""Compare native empirical variograms with SciKit-GStat on observed CODEX cells.

Uses the preserved CC BY 4.0 Schurch table downloaded by schurch.py. The feature
CSV is a declared numerical-vector profile, not an image-model embedding export.
"""

from __future__ import annotations

import argparse
import csv
from importlib.metadata import version
import json
from pathlib import Path
import subprocess

import numpy as np
from scipy.spatial.distance import pdist
from skgstat import Variogram

from schurch import DATASET, DESIGN, FILENAME


REPOSITORY = Path(__file__).resolve().parents[2]
# Prespecified before execution; the full source field is kept without sampling.
EDGES_UM = np.array([0.0, 10.0, 25.0, 50.0, 100.0])
SCALED_ERROR_LIMIT = 1e-12
EXPECTED_CELLS = 1107


def write_json(path: Path, value) -> None:
    with path.open("x") as stream:
        json.dump(value, stream, indent=2, allow_nan=False)
        stream.write("\n")


def selected_cells(source: Path):
    rows = []
    with (source / FILENAME).open(newline="") as stream:
        for row in csv.DictReader(stream):
            if row["TMA_AB"] == "A" and row["Region"] == "reg001" and row["ClusterName"] != "dirt":
                rows.append(row)
    rows.sort(key=lambda row: row["CellID"])
    if len(rows) != EXPECTED_CELLS or len({row["CellID"] for row in rows}) != len(rows):
        raise ValueError("the declared field must contain 1107 distinct non-dirt cells")
    coordinates = np.array([[float(row["X:X"]), float(row["Y:Y"])] for row in rows])
    coordinates *= DESIGN["micrometres_per_pixel"]
    values = np.array([[float(row[column]) for column in DESIGN["marker_columns"].values()] for row in rows])
    if not np.isfinite(coordinates).all() or not np.isfinite(values).all():
        raise ValueError("complete finite physical coordinates and compensated marker values are required")
    return rows, coordinates, values


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="preserved Schurch download directory")
    parser.add_argument("out", type=Path, help="new external output directory")
    parser.add_argument("--binary", type=Path, default=REPOSITORY / "target/debug/marklab")
    args = parser.parse_args()
    if version("scikit-gstat") != "1.0.23":
        raise ValueError("this comparison is pinned to scikit-gstat 1.0.23")
    out = args.out.resolve()
    if out == REPOSITORY or REPOSITORY in out.parents or out.exists():
        raise ValueError("output must be a new directory outside the checkout")
    rows, coordinates, values = selected_cells(args.source)
    # Marklab closes its last bin; SciKit-GStat uses upper-open bins throughout.
    # This field must exercise the same membership without changing either rule.
    if np.any(pdist(coordinates) == EDGES_UM[-1]):
        raise ValueError("last-edge ties require a separate convention comparison")
    out.mkdir(parents=True)
    write_json(out / "design.json", {
        "dataset": DATASET, "license": "CC BY 4.0", "field": "A-reg001",
        "source_cells": len(rows), "excluded_class": "dirt",
        "coordinate_scale_um_per_pixel": DESIGN["micrometres_per_pixel"],
        "features": DESIGN["marker_columns"], "feature_units": "compensated fluorescence AU",
        "edges_um": EDGES_UM.tolist(), "estimator": "Matheron",
        "pair_weights": "unit", "pair_count_acceptance": "exact",
        "scaled_semivariance_error_limit": SCALED_ERROR_LIMIT,
        "fit": "none; empirical variogram only", "patient_inference": False,
    })
    bins_path = out / "bins.csv"
    with bins_path.open("x", newline="") as stream:
        writer = csv.writer(stream)
        writer.writerow(["bin_id", "lower_um", "upper_um"])
        for index, (lower, upper) in enumerate(zip(EDGES_UM[:-1], EDGES_UM[1:])):
            writer.writerow([f"lag-{index}", lower, upper])
    references = [
        Variogram(coordinates, values[:, column], estimator="matheron", bin_func=EDGES_UM[1:], fit_method=None)
        for column in range(values.shape[1])
    ]
    defaults = [Variogram(coordinates, values[:, column], fit_method=None) for column in range(values.shape[1])]
    write_json(out / "scikit-gstat-defaults.json", [{
        "marker": marker, "bins_um": ref.bins.tolist(),
        "pair_counts": ref.bin_count.tolist(), "semivariance": ref.experimental.tolist(),
        "comparison": "descriptive; default 10-bin full-distance design differs from the fixed tissue scales",
    } for marker, ref in zip(DESIGN["marker_columns"], defaults)])
    reports = []
    label, columns = "CD3-CD20", [0, 1]
    input_path = out / f"{label}.csv"
    with input_path.open("x", newline="") as stream:
        writer = csv.writer(stream)
        writer.writerow(["object_id", "x_um", "y_um", *[f"embedding_{i}" for i in range(len(columns))]])
        for row, xy, vector in zip(rows, coordinates, values):
            writer.writerow([row["CellID"], *xy, *vector[columns]])
    result_path = out / f"{label}.json"
    command = [str(args.binary.resolve()), "bayes", "vector-semivariogram", "--input", str(input_path),
               "--bins", str(bins_path), "--maximum-pair-visits", str(len(rows) * (len(rows) - 1) // 2),
               "--out", str(result_path)]
    run = subprocess.run(command, capture_output=True, text=True)
    (out / f"{label}.log").write_text(run.stdout + run.stderr)
    run.check_returncode()
    native = json.loads(result_path.read_text())
    observed_counts = np.array([row["pair_count"] for row in native["curve"]])
    observed_values = np.array([row["semivariance"] for row in native["curve"]])
    expected_counts = references[columns[0]].bin_count
    expected_values = sum((references[column].experimental for column in columns), start=np.zeros(len(EDGES_UM) - 1))
    errors = np.abs(observed_values - expected_values) / np.maximum(1.0, np.abs(expected_values))
    passed = bool(np.array_equal(observed_counts, expected_counts) and np.isfinite(errors).all() and np.all(errors <= SCALED_ERROR_LIMIT))
    reports.append({"features": label, "native_counts": observed_counts.tolist(),
                    "reference_counts": expected_counts.tolist(), "native_semivariance": observed_values.tolist(),
                    "reference_semivariance": expected_values.tolist(), "maximum_scaled_error": float(errors.max()), "passed": passed})
    summary = {"versions": {name: version(name) for name in ["numpy", "scipy", "scikit-gstat"]},
               "tolerance": SCALED_ERROR_LIMIT, "comparisons": reports, "passed": all(row["passed"] for row in reports)}
    write_json(out / "summary.json", summary)
    print(json.dumps(summary, indent=2))
    if not summary["passed"]:
        raise SystemExit("variogram comparison failed its fixed acceptance criteria")


if __name__ == "__main__":
    main()
