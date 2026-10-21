"""Compare `marklab numerics ordinary-kriging` with SciKit-GStat 1.0.23 ordinary kriging.

Run from the repository root with an isolated interpreter that has scikit-gstat 1.0.23:

    target/spatial-reference-venv/bin/python -I tests/fixtures/kriging/compare_skgstat.py \
        /path/to/marklab /path/to/CRC_clusters_neighborhoods_markers.csv /path/to/new/output

Fixed before any comparison was run (2026-10-10):

* Data: Schurch et al. (2020, CC BY 4.0), spot reg001_A, the 600 non-"dirt" cells with the
  smallest CellID; value log1p(CD45 intensity); coordinates x 0.377442 um per pixel.
* Variogram (revised before run 02; see below): exponential with SciKit-GStat effective range
  r = 150 um, partial sill c0 = the sample variance of the values and nugget b = 0.1 c0. Marklab
  receives range parameter a = r / 3, partial sill c0 and nugget b, which define the same
  function. Run 01 instead used SciKit-GStat's default fit (12 lags, maxlag 150 um), which gave
  r = 5.9 um and b = 0; within that radius almost no cell had five neighbors, so both programs
  reported every prediction unavailable and nothing was compared. A declared model is enough
  for this implementation comparison, which does not depend on a fit. Run 02 omitted
  SciKit-GStat's `use_nugget=True`; its Variogram then drops a manually supplied nugget from the
  model its kriging uses (every semivariance was exactly b lower), so run 02 compared
  different models. The script now sets the flag and checks the adopted parameters.
* Neighborhood: SciKit-GStat searches within its effective range r for 5 to 15 neighbors;
  Marklab gets search radius r, minimum 5 and maximum 15.
* Held-out check: 100 um blocks from the image corner, each left out in turn; SciKit-GStat
  kriges each block from a manually fixed variogram with the same parameters on the remaining
  cells. Targets: a 20 x 15 grid at 36 um spacing.
* Tolerances: identical availability; predictions, kriging variances and summary statistics
  within 1e-8 scaled error max(1, |reference|).
"""

import csv
import json
import math
import subprocess
import sys
from pathlib import Path

import numpy as np
import skgstat

TOLERANCE = 1e-8
UM_PER_PIXEL = 0.377442
FRAME = (-0.5 * UM_PER_PIXEL, -0.5 * UM_PER_PIXEL)
BLOCK = 100.0
Z_975 = 1.959963984540054


def scaled(observed, reference):
    return abs(observed - reference) / max(1.0, abs(reference))


def main():
    executable, source, output = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
    if output.exists():
        raise SystemExit("output path must not exist")
    if skgstat.__version__ != "1.0.23":
        raise SystemExit(f"SciKit-GStat 1.0.23 required, found {skgstat.__version__}")
    output.mkdir(parents=True)
    rows = []
    with source.open(newline="") as handle:
        for row in csv.DictReader(handle):
            if row["File Name"] == "reg001_A" and row["ClusterName"] != "dirt":
                rows.append(row)
    rows.sort(key=lambda row: int(row["CellID"]))
    rows = rows[:600]
    coords = np.asarray(
        [[float(r["X:X"]) * UM_PER_PIXEL, float(r["Y:Y"]) * UM_PER_PIXEL] for r in rows]
    )
    values = np.log1p(np.asarray([float(r["CD45 - hematopoietic cells:Cyc_4_ch_2"]) for r in rows]))
    if len({tuple(c) for c in coords}) != len(coords):
        raise SystemExit("duplicate coordinates; SciKit-GStat would drop them")
    effective_range = 150.0
    partial_sill = float(np.var(values))
    nugget = 0.1 * partial_sill
    grid = np.asarray([[18.0 + 36.0 * i, 18.0 + 36.0 * j] for j in range(15) for i in range(20)])

    def reference_kriging(train, target):
        variogram = skgstat.Variogram(
            coords[train], values[train], model="exponential", n_lags=12, maxlag=150.0,
            fit_method="manual", fit_range=effective_range, fit_sill=partial_sill,
            fit_nugget=nugget, use_nugget=True,
        )
        if not np.allclose(variogram.parameters, [effective_range, partial_sill, nugget]):
            raise SystemExit(f"SciKit-GStat did not adopt the declared model: {variogram.parameters}")
        kriging = skgstat.OrdinaryKriging(variogram, min_points=5, max_points=15, mode="exact")
        estimate = kriging.transform(target[:, 0], target[:, 1])
        return np.asarray(estimate, float), np.asarray(kriging.sigma, float)

    target_estimate, target_variance = reference_kriging(np.arange(len(coords)), grid)
    blocks = np.floor((coords - np.asarray(FRAME)) / BLOCK).astype(int)
    held_estimate = np.full(len(coords), math.nan)
    held_variance = np.full(len(coords), math.nan)
    for block in {tuple(b) for b in blocks}:
        inside = np.all(blocks == block, axis=1)
        estimate, variance = reference_kriging(np.flatnonzero(~inside), coords[inside])
        held_estimate[inside] = estimate
        held_variance[inside] = variance

    request = {
        "format": "marklab.ordinary_kriging_request", "version": 1,
        "coordinate_frame_id": "reg001_A-um", "value_name": "log1p_cd45_intensity",
        "value_unit": "log1p_arbitrary_intensity",
        "observations": [
            {"id": f"c{r['CellID']}", "x_um": float(c[0]), "y_um": float(c[1]), "value": float(v)}
            for r, c, v in zip(rows, coords, values, strict=True)
        ],
        "variogram": {"model": "exponential", "range_um": effective_range / 3.0,
                      "partial_sill": partial_sill, "nugget": nugget},
        "neighborhood": {"search_radius_um": effective_range, "minimum_neighbors": 5,
                         "maximum_neighbors": 15},
        "cross_validation": {"block_width_um": BLOCK, "origin_um": list(FRAME)},
        "targets": [{"id": f"g{i}", "x_um": float(x), "y_um": float(y)}
                    for i, (x, y) in enumerate(grid)],
        "limits": {"maximum_observations": 1000, "maximum_targets": 1000,
                   "maximum_distance_evaluations": 10_000_000},
    }
    (output / "request.json").write_text(json.dumps(request))
    subprocess.run([str(executable), "numerics", "ordinary-kriging", "--input",
                    str(output / "request.json"), "--out", str(output / "result.json")],
                   check=True)
    result = json.loads((output / "result.json").read_text())

    errors = {"target_prediction": 0.0, "target_variance": 0.0,
              "held_out_prediction": 0.0, "held_out_variance": 0.0, "summary": 0.0}
    availability_matches = True

    def compare(rows_out, estimate, variance, prefix):
        nonlocal availability_matches
        for row, ref_estimate, ref_variance in zip(rows_out, estimate, variance, strict=True):
            available = row["prediction"] is not None
            if available != bool(np.isfinite(ref_estimate)):
                availability_matches = False
                continue
            if available:
                errors[f"{prefix}_prediction"] = max(
                    errors[f"{prefix}_prediction"], scaled(row["prediction"], ref_estimate))
                errors[f"{prefix}_variance"] = max(
                    errors[f"{prefix}_variance"], scaled(row["kriging_variance"], ref_variance))

    compare(result["targets"], target_estimate, target_variance, "target")
    held = result["cross_validation"]["held_out"]
    compare([row["prediction"] for row in held], held_estimate, held_variance, "held_out")
    ok = np.isfinite(held_estimate) & (held_variance > 0)
    error = values[ok] - held_estimate[ok]
    standardized = error / np.sqrt(held_variance[ok])
    reference_summary = {
        "available": int(ok.sum()), "rmse": float(np.sqrt(np.mean(error**2))),
        "mean_error": float(np.mean(error)),
        "mean_standardized_error": float(np.mean(standardized)),
        "mean_squared_standardized_error": float(np.mean(standardized**2)),
        "coverage_95": float(np.mean(np.abs(standardized) <= Z_975)),
    }
    summary = result["cross_validation"]["summary"]
    for key, value in reference_summary.items():
        errors["summary"] = max(errors["summary"], scaled(summary[key], value))
    passed = availability_matches and all(value <= TOLERANCE for value in errors.values())
    report = {
        "format": "marklab.kriging_skgstat_comparison", "version": 1,
        "skgstat": skgstat.__version__, "tolerance_scaled": TOLERANCE,
        "variogram": {"effective_range_um": effective_range, "partial_sill": partial_sill,
                      "nugget": nugget},
        "observations": len(coords), "targets": len(grid),
        "held_out_available": reference_summary["available"],
        "reference_summary": reference_summary, "marklab_summary": summary,
        "availability_matches": availability_matches, "maximum_errors": errors,
        "passed": passed,
    }
    (output / "summary.json").write_text(json.dumps(report, indent=1))
    print(json.dumps(report, indent=1))
    if not passed:
        raise SystemExit("kriging comparison failed; see summary.json")


if __name__ == "__main__":
    main()
