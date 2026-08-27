#!/usr/bin/env python3
"""Split bounded M4 raw vectors into cell-subsample and field stability inputs."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import os
from pathlib import Path
import shutil


SCHEMA_VERSION = "1.0"
CELL_SUBSAMPLE_FRACTION = 0.8


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def field_id(object_id: str) -> str:
    field, separator, remainder = object_id.partition(":")
    if (
        not separator
        or not field
        or not remainder
        or "/" in field
        or field in {".", ".."}
    ):
        raise ValueError("bounded raw object identity lacks a safe field identity")
    return field


def cell_subsample(rows: list[dict[str, str]]) -> list[dict[str, str]]:
    grouped: dict[str, list[dict[str, str]]] = {}
    identities = [row.get("object_id", "") for row in rows]
    if not rows or len(set(identities)) != len(identities):
        raise ValueError("M4 cell subsampling requires unique identified rows")
    for row in rows:
        grouped.setdefault(field_id(row["object_id"]), []).append(row)
    selected = []
    for field in sorted(grouped):
        field_rows = grouped[field]
        retain = max(1, int(math.floor(len(field_rows) * CELL_SUBSAMPLE_FRACTION)))
        selected.extend(
            sorted(
                field_rows,
                key=lambda row: (
                    hashlib.sha256(
                        ("m4-cell-stability-v1:" + row["object_id"]).encode()
                    ).digest(),
                    row["object_id"],
                ),
            )[:retain]
        )
    return sorted(selected, key=lambda row: row["object_id"])


def read_csv(path: Path) -> tuple[list[str], list[dict[str, str]]]:
    with path.open(newline="", encoding="utf-8") as source:
        reader = csv.DictReader(source)
        if reader.fieldnames is None:
            raise ValueError(f"CSV has no header: {path}")
        return list(reader.fieldnames), list(reader)


def write_csv(path: Path, fields: list[str], rows: list[dict[str, str]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as target:
        writer = csv.DictWriter(target, fieldnames=fields, lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def build(args: argparse.Namespace) -> None:
    source_root = args.input.resolve()
    admission_path = source_root / "admission.json"
    admission = json.loads(admission_path.read_text(encoding="utf-8"))
    if admission.get("cross_field_pairs_within_analyzed_distance") is not False:
        raise ValueError("M4 stability requires isolated physical field frames")
    patient_headers, patients = read_csv(source_root / "patients.csv")
    if len(patients) != admission["patient_count"]:
        raise ValueError("M4 patient manifest differs from admission")
    out = args.out.resolve()
    if out.exists() or out.is_symlink():
        raise ValueError(f"output already exists: {out}")
    out.parent.mkdir(parents=True, exist_ok=True)
    staging = out.with_name(f".{out.name}.{os.getpid()}.tmp")
    staging.mkdir()
    try:
        patient_manifest = []
        field_manifest = []
        blockers = []
        sources = {
            str(admission_path): sha256(admission_path),
            str(source_root / "patients.csv"): sha256(source_root / "patients.csv"),
        }
        for patient in patients:
            patient_id = patient["patient_id"]
            source_path = source_root / patient["input"]
            if sha256(source_path) != patient["input_sha256"]:
                raise ValueError(f"M4 input digest differs for {patient_id}")
            headers, rows = read_csv(source_path)
            if headers[:3] != ["object_id", "x_um", "y_um"] or len(headers) != 1283:
                raise ValueError(f"M4 raw vector schema differs for {patient_id}")
            grouped: dict[str, list[dict[str, str]]] = {}
            for row in rows:
                grouped.setdefault(field_id(row["object_id"]), []).append(row)
            subsample_rows = cell_subsample(rows)
            subsample_path = staging / "patients_subsample_80" / f"{patient_id}.csv"
            write_csv(subsample_path, headers, subsample_rows)
            patient_manifest.append(
                {
                    **{name: patient[name] for name in patient_headers if name not in {"input", "input_sha256"}},
                    "cell_count": str(len(subsample_rows)),
                    "input": f"patients_subsample_80/{patient_id}.csv",
                    "input_sha256": sha256(subsample_path),
                }
            )
            for field in sorted(grouped):
                field_rows = grouped[field]
                if len(field_rows) < 2:
                    blockers.append(
                        {
                            "patient_id": patient_id,
                            "field_id": field,
                            "cell_count": len(field_rows),
                            "blocker": "fewer than two admitted raw-vector cells",
                        }
                    )
                    continue
                field_path = staging / "fields" / patient_id / f"{field}.csv"
                write_csv(field_path, headers, field_rows)
                field_manifest.append(
                    {
                        "patient_id": patient_id,
                        "field_id": field,
                        "cell_count": len(field_rows),
                        "input": field_path.relative_to(staging).as_posix(),
                        "input_sha256": sha256(field_path),
                    }
                )
            sources[str(source_path)] = sha256(source_path)
        write_csv(staging / "patients.csv", list(patient_manifest[0]), patient_manifest)
        write_csv(staging / "fields.csv", list(field_manifest[0]), field_manifest)
        (staging / "admission.json").write_text(
            json.dumps(
                {
                    "schema_name": "marklab_tcga_crc_m4_stability_input",
                    "schema_version": SCHEMA_VERSION,
                    "population_unit": "patient",
                    "patient_count": len(patient_manifest),
                    "field_count": len(field_manifest),
                    "cell_subsample_fraction": CELL_SUBSAMPLE_FRACTION,
                    "source_coordinate_frame": admission["coordinate_frame"],
                    "unavailable_fields": blockers,
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
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    return parser.parse_args()


if __name__ == "__main__":
    build(parse_args())
