"""Validate the MCMICRO quantification adapter on the nf-core tonsil CyCIF test files.

Downloads, into a new directory outside the checkout, an MCQuant single-cell table and its
segmentation mask from nf-core/test-datasets at a pinned commit (MIT licence), then checks:

* provenance: the mask's label set equals the table's CellID set, every label's pixel count
  equals its Area, and every label's centroid (mean row and column index) equals its
  Y_centroid and X_centroid within 1e-6 pixels, so the table was measured on this mask;
* calibration: the mask's OME-XML PhysicalSizeX and PhysicalSizeY are equal, in micrometres,
  and supply the adapter's scale;
* adapter: `mcmicro_csv_slide` returns the CellIDs, the centroids times that scale (within
  1e-12 relative error) and every marker value exactly as an independent reading of the CSV.

These criteria were fixed before the first run (2026-10-10). Run with a Python that has numpy
and tifffile:

    target/spatial-reference-venv/bin/python -I examples/public-cohorts/validate_mcmicro.py \
        /absolute/new/path/mcmicro-raw /absolute/new/path/results
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import sys
import urllib.request
import xml.etree.ElementTree as ElementTree
from pathlib import Path

import numpy as np
import tifffile

REPOSITORY = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPOSITORY / "clients/python"))

from upstream_formats import mcmicro_csv_slide  # noqa: E402

COMMIT = "58806b114b0c6e21f1b346053039511a8d6769d3"
BASE = f"https://raw.githubusercontent.com/nf-core/test-datasets/{COMMIT}"
SOURCES = {
    "cell.csv": ("data/imaging/downstream/cycif_tonsil_cell.csv", 467_756),
    "cell.ome.tif": ("data/imaging/quantification/cell.ome.tif", 1_137_766),
    "LICENSE": ("LICENSE", 1_064),
}
MARKERS = ["CD3", "CD45RO", "Antigen Ki67", "Pan-cytokeratin", "Aortic smooth muscle actin"]


def download(directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=False)
    for name, (path, size) in SOURCES.items():
        with urllib.request.urlopen(f"{BASE}/{path}", timeout=60) as response:
            data = response.read(size + 1)
        if len(data) != size:
            raise ValueError(f"{name}: expected {size} bytes, received {len(data)}")
        (directory / name).write_bytes(data)
    if "MIT License" not in (directory / "LICENSE").read_text():
        raise ValueError("nf-core/test-datasets licence is not the expected MIT licence")
    (directory / "source.json").write_text(json.dumps(
        {"repository": "nf-core/test-datasets", "commit": COMMIT,
         "files": {name: path for name, (path, _) in SOURCES.items()}, "license": "MIT"},
        indent=2))


def physical_size(mask_path: Path) -> tuple[float, float, str | None]:
    with tifffile.TiffFile(mask_path) as tiff:
        description = tiff.pages[0].description
    # Downloaded XML: refuse DTDs and entity declarations rather than expand them.
    if "<!DOCTYPE" in description or "<!ENTITY" in description or len(description) > 1 << 20:
        raise ValueError("mask OME-XML declares a DTD or entity, or is unexpectedly large")
    root = ElementTree.fromstring(description)
    pixels = next(element for element in root.iter() if element.tag.endswith("Pixels"))
    unit = pixels.get("PhysicalSizeXUnit", "µm")
    if pixels.get("PhysicalSizeYUnit", "µm") != unit or unit != "µm":
        raise ValueError(f"mask physical size unit is {unit!r}, not micrometres")
    size_x, size_y = pixels.get("PhysicalSizeX"), pixels.get("PhysicalSizeY")
    if size_x is None or size_y is None:
        raise ValueError("mask OME-XML does not declare PhysicalSizeX and PhysicalSizeY")
    return float(size_x), float(size_y), root.get("Creator")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if not args.source.exists():
        download(args.source)
    args.output.mkdir(parents=True, exist_ok=False)
    with (args.source / "cell.csv").open(newline="") as stream:
        rows = list(csv.DictReader(stream))
    mask = tifffile.imread(args.source / "cell.ome.tif")
    size_x, size_y, creator = physical_size(args.source / "cell.ome.tif")
    if size_x != size_y or not math.isfinite(size_x) or size_x <= 0:
        raise ValueError("mask pixels are not square with a positive physical size")

    labels = mask.ravel()
    rows_index, columns_index = np.indices(mask.shape)
    counts = np.bincount(labels)
    row_means = np.bincount(labels, weights=rows_index.ravel()) / np.maximum(counts, 1)
    column_means = np.bincount(labels, weights=columns_index.ravel()) / np.maximum(counts, 1)
    mask_ids = set(np.flatnonzero(counts[1:]) + 1)
    table_ids = {int(row["CellID"]) for row in rows}
    centroid_error = max(
        max(abs(float(row["Y_centroid"]) - row_means[int(row["CellID"])]),
            abs(float(row["X_centroid"]) - column_means[int(row["CellID"])]))
        for row in rows)
    area_equal = all(int(float(row["Area"])) == counts[int(row["CellID"])] for row in rows)

    height, width = mask.shape
    window = {"type": "MultiPolygon", "coordinates": [[[
        [-0.5 * size_x, -0.5 * size_y], [(width - 0.5) * size_x, -0.5 * size_y],
        [(width - 0.5) * size_x, (height - 0.5) * size_y], [-0.5 * size_x, (height - 0.5) * size_y],
        [-0.5 * size_x, -0.5 * size_y]]]]}
    channels = [{"id": marker, "label": marker, "kind": "continuous",
                 "unit": "fluorescence_au", "measurement_status": "measured",
                 "provenance": "MCQuant mean intensity, nf-core tonsil CyCIF test data"}
                for marker in MARKERS]
    slide = mcmicro_csv_slide(
        args.source / "cell.csv", slide_id="tonsil", patient_id="tonsil", group="test",
        coordinate_frame_id="tonsil-um", window=window, channels=channels,
        coordinate_scale_um=(size_x, size_y), channel_columns={m: m for m in MARKERS})

    oracle = sorted(rows, key=lambda row: row["CellID"])
    expected_ids = [f"{len('tonsil')}:tonsil:{row['CellID']}" for row in oracle]
    coordinate_error = max(
        max(abs(x - float(row["X_centroid"]) * size_x) / max(1.0, abs(x)),
            abs(y - float(row["Y_centroid"]) * size_y) / max(1.0, abs(y)))
        for (x, y), row in zip(slide["coordinates_um"], oracle))
    values_equal = all(
        slide["observations"][marker] == [float(row[marker]) for row in oracle]
        for marker in MARKERS)
    checks = {
        "label_set_equals_cell_ids": mask_ids == table_ids,
        "area_equals_label_pixels": bool(area_equal),
        "centroid_error_px": float(centroid_error),
        "centroids_within_1e-6_px": bool(centroid_error <= 1e-6),
        "ids_equal": slide["cell_ids"] == expected_ids,
        "coordinate_relative_error": float(coordinate_error),
        "coordinates_within_1e-12": bool(coordinate_error <= 1e-12),
        "marker_values_equal": bool(values_equal),
    }
    passed = all(value for value in checks.values() if isinstance(value, bool))
    summary = {
        "format": "marklab.mcmicro_adapter_validation", "version": 1,
        "source": {"repository": "nf-core/test-datasets", "commit": COMMIT, "license": "MIT"},
        "cells": len(rows), "mask_shape": [height, width],
        "physical_size_um": [size_x, size_y], "mask_creator": creator,
        "checks": checks, "passed": passed,
    }
    (args.output / "summary.json").write_text(json.dumps(summary, indent=2))
    print(json.dumps(summary, indent=2))
    if not passed:
        raise SystemExit("MCMICRO adapter validation failed; see summary.json")


if __name__ == "__main__":
    main()
