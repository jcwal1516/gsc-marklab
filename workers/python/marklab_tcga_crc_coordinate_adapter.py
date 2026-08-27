#!/usr/bin/env python3
"""Build label-free TCGA CRC coordinate inputs at the patient population unit."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import os
import shutil
from collections import defaultdict
from pathlib import Path


MAXIMUM_FIELDS_PER_PATIENT = 14
MAXIMUM_CELLS_PER_FIELD = 100_000
GRAPH_GRIDS = (2, 3)
COORDINATE_BOUNDARY_SNAP_TOLERANCE_UM = 0.001


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_rows(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as source:
        return list(csv.DictReader(source))


def finite(value: str, label: str) -> float:
    parsed = float(value)
    if not math.isfinite(parsed):
        raise ValueError(f"{label} must be finite")
    return parsed


def field_key(patient: str, combined_roi: str) -> tuple[str, str, str]:
    try:
        slide, field = combined_roi.rsplit("__", 1)
    except ValueError as error:
        raise ValueError(f"field ROI lacks the exact slide__field identity: {combined_roi}") from error
    return patient, slide, field


def polygon(x0: float, y0: float, side: float) -> list[list[list[float]]]:
    x1 = x0 + side
    y1 = y0 + side
    return [[[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]]]


def graph_input(fields: list[dict], grid: int) -> dict:
    side = fields[0]["side"]
    bin_side = side / grid
    nodes = []
    for field in fields:
        counts = [[0 for _ in range(grid)] for _ in range(grid)]
        for cell in field["cells"]:
            column = min(grid - 1, int((cell["x"] - field["x0"]) / bin_side))
            row = min(grid - 1, int((cell["y"] - field["y0"]) / bin_side))
            counts[row][column] += 1
        bin_area_mm2 = bin_side * bin_side / 1_000_000.0
        for row in range(grid):
            for column in range(grid):
                nodes.append(
                    {
                        "id": f'{field["combined_roi"]}:g{grid}:r{row}:c{column}',
                        "coordinates_um": [
                            field["x0"] + (column + 0.5) * bin_side,
                            field["y0"] + (row + 0.5) * bin_side,
                        ],
                        "signal": math.log1p(counts[row][column] / bin_area_mm2),
                    }
                )
    mean = math.fsum(node["signal"] for node in nodes) / len(nodes)
    for node in nodes:
        node["signal"] -= mean
    return {
        "nodes": nodes,
        "radius_um": bin_side * 1.01,
        "weight": "binary",
        "laplacian": "combinatorial",
        "bands": [
            {"id": "low", "minimum": 0.0, "maximum": 1.0},
            {"id": "middle", "minimum": 1.0, "maximum": 3.0},
            {"id": "high", "minimum": 3.0, "maximum": 1_000_000.0},
        ],
        "maximum_pairs": len(nodes) * (len(nodes) - 1) // 2,
    }


def write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def build(args: argparse.Namespace) -> None:
    sources = {
        "field_manifest": args.field_manifest.resolve(),
        "field_qc": args.field_qc.resolve(),
        "slide_qc": args.slide_qc.resolve(),
        "admitted_patients": args.admitted_patients.resolve(),
    }
    admitted_rows = read_rows(args.admitted_patients)
    admitted = [row["patient_id"] for row in admitted_rows]
    if not admitted or len(set(admitted)) != len(admitted):
        raise ValueError("admitted patients require unique nonempty patient_id values")
    admitted_set = set(admitted)

    qc = {}
    for row in read_rows(args.field_qc):
        key = (row["patient_id"], row["roi_id"], row["field_id"])
        if key in qc:
            raise ValueError(f"duplicate field QC identity: {key}")
        patch_row = int(row["patch_row"])
        patch_column = int(row["patch_column"])
        if patch_row < 0 or patch_column < 0:
            raise ValueError(f"negative patch grid identity: {key}")
        qc[key] = (patch_row, patch_column, int(row["tumor_cell_count"]))

    slide_mpp = {}
    for row in read_rows(args.slide_qc):
        key = (row["patient_id"], row["roi_id"])
        if key in slide_mpp:
            raise ValueError(f"duplicate slide QC identity: {key}")
        mpp = finite(row["target_mpp"], "target_mpp")
        if mpp <= 0.0:
            raise ValueError(f"slide target_mpp must be positive: {key}")
        slide_mpp[key] = mpp

    by_patient: dict[str, list[dict]] = defaultdict(list)
    file_hashes = {}
    snapped_boundary_cell_count = 0
    for row in read_rows(args.field_manifest):
        patient = row["patient_id"]
        if patient not in admitted_set:
            continue
        key = field_key(patient, row["roi_id"])
        if key not in qc:
            raise ValueError(f"field manifest lacks exact QC row: {key}")
        if (patient, key[1]) not in slide_mpp:
            raise ValueError(f"field manifest lacks exact slide QC row: {(patient, key[1])}")
        patch_row, patch_column, expected_count = qc[key]
        cell_path = (args.field_cells_root / Path(row["cells"]).name).resolve()
        if not cell_path.is_file():
            raise ValueError(f"field cell table is unavailable: {cell_path}")
        cell_rows = read_rows(cell_path)
        if len(cell_rows) != expected_count or not (1 <= len(cell_rows) <= MAXIMUM_CELLS_PER_FIELD):
            raise ValueError(
                f"field {row['roi_id']} has {len(cell_rows)} cells; expected {expected_count} within bounds"
            )
        mpp = slide_mpp[(patient, key[1])]
        side = args.patch_pixels * mpp
        stride = (args.patch_pixels - args.patch_overlap_pixels) * mpp
        half_overlap = args.patch_overlap_pixels * mpp / 2.0
        # CellViT's frozen postprocessor places a patch at
        # index * (size - overlap) - overlap / 2 in level-zero coordinates.
        x0 = patch_column * stride - half_overlap
        y0 = patch_row * stride - half_overlap
        cells = []
        seen = set()
        for cell in cell_rows:
            cell_id = cell["cell_id"]
            if not cell_id or cell_id in seen:
                raise ValueError(f"field {row['roi_id']} has duplicate/empty cell_id")
            seen.add(cell_id)
            x = finite(cell["x_um"], "x_um")
            y = finite(cell["y_um"], "y_um")
            outside = max(x0 - x, x - (x0 + side), y0 - y, y - (y0 + side), 0.0)
            if outside > COORDINATE_BOUNDARY_SNAP_TOLERANCE_UM:
                raise ValueError(
                    f"cell {cell_id} lies outside exact field window {row['roi_id']}"
                )
            snapped_x = min(max(x, x0), x0 + side)
            snapped_y = min(max(y, y0), y0 + side)
            if snapped_x != x or snapped_y != y:
                snapped_boundary_cell_count += 1
                x, y = snapped_x, snapped_y
            cells.append({"id": cell_id, "x": x, "y": y})
        file_hashes[str(cell_path)] = sha256(cell_path)
        by_patient[patient].append(
            {
                "combined_roi": row["roi_id"],
                "slide": key[1],
                "field": key[2],
                "x0": x0,
                "y0": y0,
                "side": side,
                "mpp": mpp,
                "cells": cells,
            }
        )

    missing = sorted(admitted_set - by_patient.keys())
    if missing:
        raise ValueError(f"admitted patients lack nonempty coordinate fields: {missing}")
    excluded_overlapping_fields = []
    for patient, fields in by_patient.items():
        fields.sort(key=lambda field: field["combined_roi"])
        if any(field["side"] != fields[0]["side"] for field in fields):
            raise ValueError(f"patient {patient} has inconsistent slide physical scales")
        retained = []
        for field in fields:
            patch_row, patch_column, _ = qc[(patient, field["slide"], field["field"])]
            overlaps = any(
                abs(patch_row - prior["patch_row"]) <= 1
                and abs(patch_column - prior["patch_column"]) <= 1
                for prior in retained
            )
            if overlaps:
                excluded_overlapping_fields.append(field["combined_roi"])
                continue
            field["patch_row"] = patch_row
            field["patch_column"] = patch_column
            retained.append(field)
        fields[:] = retained
        if not fields:
            raise ValueError(f"patient {patient} has no disjoint nonempty fields")
        if len(fields) > MAXIMUM_FIELDS_PER_PATIENT:
            raise ValueError(
                f"patient {patient} has {len(fields)} fields; maximum is {MAXIMUM_FIELDS_PER_PATIENT}"
            )

    out = args.out.resolve()
    if out.exists():
        raise ValueError(f"output already exists: {out}")
    out.parent.mkdir(parents=True, exist_ok=True)
    staging = out.with_name(f".{out.name}.{os.getpid()}.tmp")
    if staging.exists():
        raise ValueError(f"staging output already exists: {staging}")
    staging.mkdir()
    try:
        patient_manifest = []
        total_cells = 0
        total_fields = 0
        field_sides = []
        for patient in sorted(admitted):
            fields = by_patient[patient]
            side = fields[0]["side"]
            field_sides.append(side)
            patient_dir = staging / "patients" / patient
            patient_dir.mkdir(parents=True)
            cell_path = patient_dir / "cells.csv"
            with cell_path.open("w", newline="", encoding="utf-8") as target:
                writer = csv.writer(target, lineterminator="\n")
                writer.writerow(
                    [
                        "x_um",
                        "y_um",
                        "mark",
                        "case_id",
                        "timepoint",
                        "protein",
                        "valid_tumor",
                        "valid_ihc",
                        "slide_id",
                        "region_id",
                    ]
                )
                for field in fields:
                    for cell in field["cells"]:
                        writer.writerow(
                            [
                                repr(cell["x"]),
                                repr(cell["y"]),
                                0,
                                patient,
                                "baseline",
                                "coordinate_only",
                                "true",
                                "true",
                                field["slide"],
                                field["combined_roi"],
                            ]
                        )
            window = {
                "type": "Feature",
                "properties": {
                    "patient_id": patient,
                    "population_unit": "patient",
                    "field_side_um": side,
                    "field_count": len(fields),
                },
                "geometry": {
                    "type": "MultiPolygon",
                    "coordinates": [polygon(field["x0"], field["y0"], side) for field in fields],
                },
            }
            write_json(patient_dir / "window.geojson", window)
            for grid in GRAPH_GRIDS:
                write_json(patient_dir / f"graph_{grid}x{grid}.json", graph_input(fields, grid))
            patient_cells = sum(len(field["cells"]) for field in fields)
            total_cells += patient_cells
            total_fields += len(fields)
            patient_manifest.append(
                {
                    "patient_id": patient,
                    "field_count": len(fields),
                    "cell_count": patient_cells,
                    "target_mpp": fields[0]["mpp"],
                    "field_side_um": side,
                    "cells": f"patients/{patient}/cells.csv",
                    "window": f"patients/{patient}/window.geojson",
                    "graph_2x2": f"patients/{patient}/graph_2x2.json",
                    "graph_3x3": f"patients/{patient}/graph_3x3.json",
                }
            )
        with (staging / "patients.csv").open("w", newline="", encoding="utf-8") as target:
            writer = csv.DictWriter(target, fieldnames=list(patient_manifest[0]))
            writer.writeheader()
            writer.writerows(patient_manifest)
        write_json(
            staging / "admission.json",
            {
                "schema_name": "marklab_tcga_crc_coordinate_admission",
                "schema_version": "1.0",
                "cohort": "TCGA CRC H&E CellViT tumor-cell coordinates",
                "population_unit": "patient",
                "patient_count": len(patient_manifest),
                "field_count": total_fields,
                "cell_count": total_cells,
                "field_side_um_range": [min(field_sides), max(field_sides)],
                "field_stride_um_range": [
                    min(side * (args.patch_pixels - args.patch_overlap_pixels) / args.patch_pixels for side in field_sides),
                    max(side * (args.patch_pixels - args.patch_overlap_pixels) / args.patch_pixels for side in field_sides),
                ],
                "coordinate_units": "micrometres",
                "analyzed_window": "exact union of admitted 1024-pixel sampled field rectangles",
                "field_origin_rule": "index * (patch_size - patch_overlap) - patch_overlap / 2",
                "coordinate_boundary_snap_tolerance_um": COORDINATE_BOUNDARY_SNAP_TOLERANCE_UM,
                "snapped_boundary_cell_count": snapped_boundary_cell_count,
                "molecular_labels_used_for_features": False,
                "graph_grid_sizes": list(GRAPH_GRIDS),
                "graph_signal": "patient-centered log1p cell density per equal-area field bin",
                "excluded_overlapping_field_count": len(excluded_overlapping_fields),
                "excluded_overlapping_fields": sorted(excluded_overlapping_fields),
                "source_sha256": {
                    **{str(path): sha256(path) for path in sources.values()},
                    **dict(sorted(file_hashes.items())),
                },
            },
        )
        os.rename(staging, out)
    except BaseException:
        shutil.rmtree(staging, ignore_errors=True)
        raise


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--field-manifest", required=True, type=Path)
    parser.add_argument("--field-qc", required=True, type=Path)
    parser.add_argument("--slide-qc", required=True, type=Path)
    parser.add_argument("--field-cells-root", required=True, type=Path)
    parser.add_argument("--admitted-patients", required=True, type=Path)
    parser.add_argument("--patch-pixels", required=True, type=int)
    parser.add_argument("--patch-overlap-pixels", required=True, type=int)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    if (
        args.patch_pixels <= 0
        or args.patch_overlap_pixels < 0
        or args.patch_overlap_pixels >= args.patch_pixels
    ):
        parser.error("physical field dimensions must be finite and positive")
    return args


if __name__ == "__main__":
    build(parse_args())
