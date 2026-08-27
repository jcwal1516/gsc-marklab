#!/usr/bin/env python3
"""Split admitted patient coordinate inputs into exact field stability inputs."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import os
import re
import shutil
from collections import defaultdict
from pathlib import Path


SAFE_ID = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")
SUBSAMPLE_FRACTION = 0.8


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_csv(path: Path) -> tuple[list[str], list[dict[str, str]]]:
    with path.open(newline="", encoding="utf-8") as source:
        reader = csv.DictReader(source)
        if reader.fieldnames is None:
            raise ValueError(f"CSV has no header: {path}")
        return list(reader.fieldnames), list(reader)


def rectangle(polygon: object) -> tuple[float, float, float, float]:
    if not isinstance(polygon, list) or len(polygon) != 1 or not isinstance(polygon[0], list):
        raise ValueError("coordinate field window must be one exterior rectangular ring")
    ring = polygon[0]
    if len(ring) != 5 or ring[0] != ring[-1]:
        raise ValueError("coordinate field window must be a closed five-vertex rectangle")
    points = [(float(point[0]), float(point[1])) for point in ring]
    if any(not math.isfinite(value) for point in points for value in point):
        raise ValueError("coordinate field window contains a non-finite vertex")
    xs = {point[0] for point in points[:-1]}
    ys = {point[1] for point in points[:-1]}
    if len(xs) != 2 or len(ys) != 2:
        raise ValueError("coordinate field window is not axis aligned")
    return min(xs), min(ys), max(xs), max(ys)


def write_rows(path: Path, headers: list[str], rows: list[dict[str, str]]) -> None:
    with path.open("w", newline="", encoding="utf-8") as target:
        writer = csv.DictWriter(target, fieldnames=headers, lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def build(args: argparse.Namespace) -> None:
    coordinate_input = args.coordinate_input.resolve()
    manifest_path = coordinate_input / "patients.csv"
    _, patients = read_csv(manifest_path)
    if not patients:
        raise ValueError("coordinate patient manifest is empty")
    sources = {str(manifest_path): sha256(manifest_path)}
    prepared = []
    total_cells = 0
    total_subsampled = 0
    for patient_row in patients:
        patient = patient_row["patient_id"]
        if not SAFE_ID.fullmatch(patient):
            raise ValueError(f"unsafe patient identity: {patient}")
        patient_root = coordinate_input / "patients" / patient
        cells_path = patient_root / "cells.csv"
        window_path = patient_root / "window.geojson"
        headers, rows = read_csv(cells_path)
        required = {"x_um", "y_um", "case_id", "region_id"}
        if not required.issubset(headers):
            raise ValueError(f"patient cell input lacks required columns: {cells_path}")
        if any(row["case_id"] != patient for row in rows):
            raise ValueError(f"patient cell identity mismatch: {cells_path}")
        window = json.loads(window_path.read_text(encoding="utf-8"))
        geometry = window.get("geometry", {})
        polygons = geometry.get("coordinates")
        if geometry.get("type") != "MultiPolygon" or not isinstance(polygons, list):
            raise ValueError(f"patient window is not a MultiPolygon: {window_path}")
        rectangles = [rectangle(polygon) for polygon in polygons]
        grouped = defaultdict(list)
        for row in rows:
            grouped[row["region_id"]].append(row)
        if len(grouped) != len(rectangles) or int(patient_row["field_count"]) != len(grouped):
            raise ValueError(f"patient field identities and exact window components disagree: {patient}")
        used = set()
        for region in sorted(grouped):
            if not SAFE_ID.fullmatch(region):
                raise ValueError(f"unsafe field identity: {region}")
            field_rows = grouped[region]
            candidates = []
            for index, (x0, y0, x1, y1) in enumerate(rectangles):
                if index in used:
                    continue
                if all(
                    x0 <= float(row["x_um"]) <= x1 and y0 <= float(row["y_um"]) <= y1
                    for row in field_rows
                ):
                    candidates.append(index)
            if len(candidates) != 1:
                raise ValueError(f"field {region} does not map uniquely to one exact window")
            polygon_index = candidates[0]
            used.add(polygon_index)
            ranked = sorted(
                enumerate(field_rows),
                key=lambda item: hashlib.sha256(
                    f"{patient}\0{region}\0{item[0]}\0{item[1]['x_um']}\0{item[1]['y_um']}".encode()
                ).digest(),
            )
            count = min(len(field_rows), max(2, math.floor(len(field_rows) * SUBSAMPLE_FRACTION)))
            selected_indices = {index for index, _row in ranked[:count]}
            subsample = [row for index, row in enumerate(field_rows) if index in selected_indices]
            prepared.append(
                {
                    "patient_id": patient,
                    "field_id": region,
                    "headers": headers,
                    "rows": field_rows,
                    "subsample": subsample,
                    "polygon": polygons[polygon_index],
                }
            )
            total_cells += len(field_rows)
            total_subsampled += len(subsample)
        sources[str(cells_path)] = sha256(cells_path)
        sources[str(window_path)] = sha256(window_path)

    out = args.out.resolve()
    if out.exists():
        raise ValueError(f"output already exists: {out}")
    out.parent.mkdir(parents=True, exist_ok=True)
    staging = out.with_name(f".{out.name}.{os.getpid()}.tmp")
    staging.mkdir()
    try:
        manifest = []
        for field in prepared:
            root = staging / "fields" / field["patient_id"] / field["field_id"]
            root.mkdir(parents=True)
            write_rows(root / "cells.csv", field["headers"], field["rows"])
            write_rows(root / "cells_subsample_80.csv", field["headers"], field["subsample"])
            (root / "window.geojson").write_text(
                json.dumps(
                    {
                        "type": "Feature",
                        "properties": {
                            "patient_id": field["patient_id"],
                            "field_id": field["field_id"],
                            "population_unit": "patient",
                            "role": "roi_stability_resample_unit",
                        },
                        "geometry": {"type": "MultiPolygon", "coordinates": [field["polygon"]]},
                    },
                    indent=2,
                    sort_keys=True,
                )
                + "\n",
                encoding="utf-8",
            )
            manifest.append(
                {
                    "patient_id": field["patient_id"],
                    "field_id": field["field_id"],
                    "cell_count": len(field["rows"]),
                    "subsample_cell_count": len(field["subsample"]),
                    "root": f"fields/{field['patient_id']}/{field['field_id']}",
                }
            )
        with (staging / "fields.csv").open("w", newline="", encoding="utf-8") as target:
            writer = csv.DictWriter(target, fieldnames=list(manifest[0]), lineterminator="\n")
            writer.writeheader()
            writer.writerows(manifest)
        (staging / "admission.json").write_text(
            json.dumps(
                {
                    "schema_name": "marklab_tcga_crc_coordinate_stability_admission",
                    "schema_version": "1.0",
                    "population_unit": "patient",
                    "resample_unit": "sampled_field",
                    "patient_count": len(patients),
                    "field_count": len(prepared),
                    "cell_count": total_cells,
                    "subsample_cell_count": total_subsampled,
                    "cell_subsample_fraction": SUBSAMPLE_FRACTION,
                    "subsample_seed_rule": "sha256 patient, field, source row, x, y rank",
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
    parser.add_argument("--coordinate-input", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    return parser.parse_args()


if __name__ == "__main__":
    build(parse_args())
