#!/usr/bin/env python3
"""Build bounded projection-free raw CellViT inputs for M4 variograms."""

from __future__ import annotations

import argparse
import csv
import hashlib
import importlib.util
import json
import math
import os
import shutil
from collections import defaultdict
from pathlib import Path

import numpy as np


FIELDS_PER_PATIENT = 4
CELLS_PER_FIELD = 8
FIELD_FRAME_STRIDE_UM = 1_000.0
MAXIMUM_FIELD_SPAN_UM = 300.0
MAXIMUM_ANALYZED_DISTANCE_UM = 100.0


def select_rows(rows: list[dict[str, str]], maximum: int) -> list[dict[str, str]]:
    if maximum < 1 or any(not row.get("cell_id") for row in rows):
        raise ValueError("raw spatial selection requires bounded identified cells")
    selected = sorted(
        rows,
        key=lambda row: (hashlib.sha256(row["cell_id"].encode()).digest(), row["cell_id"]),
    )[:maximum]
    return sorted(selected, key=lambda row: row["cell_id"])


def frame_field_coordinates(
    rows: list[dict[str, str]], field_index: int
) -> list[tuple[str, float, float]]:
    if not rows or field_index < 0:
        raise ValueError("field framing requires nonempty rows and a nonnegative index")
    coordinates = [
        (row["cell_id"], float(row["x_um"]), float(row["y_um"])) for row in rows
    ]
    if any(not math.isfinite(value) for _, x, y in coordinates for value in (x, y)):
        raise ValueError("field coordinates must be finite")
    minimum_x = min(x for _, x, _ in coordinates)
    minimum_y = min(y for _, _, y in coordinates)
    span_x = max(x for _, x, _ in coordinates) - minimum_x
    span_y = max(y for _, _, y in coordinates) - minimum_y
    if max(span_x, span_y) > MAXIMUM_FIELD_SPAN_UM:
        raise ValueError("selected field exceeds the admitted physical span")
    if FIELD_FRAME_STRIDE_UM - MAXIMUM_FIELD_SPAN_UM <= MAXIMUM_ANALYZED_DISTANCE_UM:
        raise RuntimeError("field frame stride does not exclude cross-field pairs")
    offset_x = field_index * FIELD_FRAME_STRIDE_UM
    return [
        (cell_id, x - minimum_x + offset_x, y - minimum_y)
        for cell_id, x, y in coordinates
    ]


def raw_support_module():
    path = Path(__file__).with_name("marklab_tcga_crc_raw_embedding_summary.py")
    spec = importlib.util.spec_from_file_location("marklab_raw_summary_support", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def read_csv(path: Path) -> tuple[list[str], list[dict[str, str]]]:
    with path.open(newline="", encoding="utf-8") as source:
        reader = csv.DictReader(source)
        if reader.fieldnames is None:
            raise ValueError(f"CSV has no header: {path}")
        return list(reader.fieldnames), list(reader)


def build(args: argparse.Namespace) -> None:
    support = raw_support_module()
    _admitted_headers, admitted_rows = read_csv(args.admitted_patients)
    admitted = {row["patient_id"]: row for row in admitted_rows}
    _, phenotype_rows = read_csv(args.phenotype_manifest)
    phenotype = {row["patient_id"]: row for row in phenotype_rows}
    _, field_rows = read_csv(args.stability_fields_manifest)
    selected_fields = defaultdict(list)
    for row in field_rows:
        patient = row["patient_id"]
        if patient in admitted and len(selected_fields[patient]) < FIELDS_PER_PATIENT:
            selected_fields[patient].append(row["field_id"])
    if set(selected_fields) != set(admitted):
        raise ValueError("M4 fields do not cover every admitted patient")
    sources = {
        str(args.admitted_patients.resolve()): support.sha256(args.admitted_patients),
        str(args.phenotype_manifest.resolve()): support.sha256(args.phenotype_manifest),
        str(args.stability_fields_manifest.resolve()): support.sha256(args.stability_fields_manifest),
        str(args.environment_lock.resolve()): support.sha256(args.environment_lock),
    }
    feature_names = [f"embedding_raw_{index:04d}" for index in range(1280)]

    out = args.out.resolve()
    if out.exists():
        raise ValueError(f"output already exists: {out}")
    out.parent.mkdir(parents=True, exist_ok=True)
    staging = out.with_name(f".{out.name}.{os.getpid()}.tmp")
    (staging / "patients").mkdir(parents=True)
    try:
        patient_manifest = []
        for patient in sorted(admitted):
            if patient not in phenotype:
                raise ValueError(f"patient lacks phenotype identity: {patient}")
            roi = phenotype[patient]["roi_id"]
            slide_root = args.inference_root / roi
            tensors = sorted(slide_root.glob("*_cells.pt"))
            inference_manifest = slide_root / "inference_manifest.json"
            if len(tensors) != 1 or not inference_manifest.is_file():
                raise ValueError(f"patient lacks raw tensor identity: {patient}")
            raw = support.load_raw_tensor(tensors[0], args.cellvit_source_root)
            selected = []
            for field_index, field in enumerate(selected_fields[patient]):
                field_path = args.field_cells_root / f"{field}.csv"
                _headers, rows = read_csv(field_path)
                bounded_rows = select_rows(rows, CELLS_PER_FIELD)
                framed_coordinates = {
                    cell_id: (x_um, y_um)
                    for cell_id, x_um, y_um in frame_field_coordinates(
                        bounded_rows, field_index
                    )
                }
                for row in bounded_rows:
                    prefix = f"{roi}:"
                    cell_id = row["cell_id"]
                    if not cell_id.startswith(prefix) or not cell_id[len(prefix) :].isdigit():
                        raise ValueError(f"invalid raw cell identity: {cell_id}")
                    index = int(cell_id[len(prefix) :])
                    if index >= len(raw):
                        raise ValueError(f"raw cell index is out of range: {cell_id}")
                    x_um, y_um = framed_coordinates[cell_id]
                    selected.append((f"{field}:{cell_id}", x_um, y_um, raw[index]))
                sources[str(field_path.resolve())] = support.sha256(field_path)
            if len(selected) < 8:
                raise ValueError(f"patient has too few bounded raw spatial cells: {patient}")
            path = staging / "patients" / f"{patient}.csv"
            with path.open("w", newline="", encoding="utf-8") as target:
                writer = csv.writer(target, lineterminator="\n")
                writer.writerow(["object_id", "x_um", "y_um", *feature_names])
                for cell_id, x, y, vector in selected:
                    writer.writerow([cell_id, repr(x), repr(y), *(repr(float(value)) for value in vector)])
            patient_manifest.append(
                {
                    "patient_id": patient,
                    "site_id": admitted[patient]["site_id"],
                    "label": admitted[patient]["label"],
                    "field_count": len(selected_fields[patient]),
                    "cell_count": len(selected),
                    "input": f"patients/{patient}.csv",
                    "input_sha256": support.sha256(path),
                }
            )
            for source_path in (tensors[0], inference_manifest):
                sources[str(source_path.resolve())] = support.sha256(source_path)
        with (staging / "patients.csv").open("w", newline="", encoding="utf-8") as target:
            writer = csv.DictWriter(target, fieldnames=list(patient_manifest[0]), lineterminator="\n")
            writer.writeheader()
            writer.writerows(patient_manifest)
        (staging / "bins.csv").write_text(
            "bin_id,lower_um,upper_um\nnear,0,25\nintermediate,25,50\nfar,50,100\n",
            encoding="utf-8",
        )
        (staging / "admission.json").write_text(
            json.dumps(
                {
                    "schema_name": "marklab_tcga_crc_m4_raw_spatial_admission",
                    "schema_version": "1.0",
                    "population_unit": "patient",
                    "patient_count": len(patient_manifest),
                    "fields_per_patient_ceiling": FIELDS_PER_PATIENT,
                    "cells_per_field_ceiling": CELLS_PER_FIELD,
                    "raw_embedding_width": 1280,
                    "projection": "none",
                    "spatial_scales_um": [25.0, 50.0, 100.0],
                    "coordinate_frame": "within_field_distances_preserved_fields_separated_on_x_axis",
                    "field_frame_stride_um": FIELD_FRAME_STRIDE_UM,
                    "maximum_field_span_um": MAXIMUM_FIELD_SPAN_UM,
                    "cross_field_pairs_within_analyzed_distance": False,
                    "molecular_labels_used_for_features": False,
                    "source_sha256": dict(sorted(sources.items())),
                },
                indent=2,
                sort_keys=True,
            )
            + "\n",
            encoding="utf-8",
        )
        os.rename(staging, out)
    except BaseException:
        shutil.rmtree(staging, ignore_errors=True)
        raise


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--admitted-patients", required=True, type=Path)
    parser.add_argument("--phenotype-manifest", required=True, type=Path)
    parser.add_argument("--stability-fields-manifest", required=True, type=Path)
    parser.add_argument("--field-cells-root", required=True, type=Path)
    parser.add_argument("--inference-root", required=True, type=Path)
    parser.add_argument("--cellvit-source-root", required=True, type=Path)
    parser.add_argument("--environment-lock", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    return parser.parse_args()


if __name__ == "__main__":
    build(parse_args())
