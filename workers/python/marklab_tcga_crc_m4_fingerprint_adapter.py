#!/usr/bin/env python3
"""Build cumulative M0/M4 folds from raw-vector spatial variograms."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import os
import shutil
from pathlib import Path


M0_FEATURES = [
    "embedding_stage_ordinal",
    "embedding_log1p_all_cell_count",
    "embedding_tumor_fraction",
    "embedding_inflammatory_fraction",
    "embedding_connective_fraction",
    "embedding_cell_density_per_mm2",
]
M4_FEATURES = [
    "embedding_raw_variogram_0_25um",
    "embedding_raw_variogram_25_50um",
    "embedding_raw_variogram_50_100um",
]
BANDS = ("near", "intermediate", "far")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_csv(path: Path) -> tuple[list[str], list[dict[str, str]]]:
    with path.open(newline="", encoding="utf-8") as source:
        reader = csv.DictReader(source)
        return list(reader.fieldnames or []), list(reader)


def finite(value: object, label: str) -> float:
    parsed = float(value)
    if not math.isfinite(parsed):
        raise ValueError(f"{label} must be finite")
    return parsed


def variogram_features(path: Path) -> list[float]:
    document = json.loads(path.read_text(encoding="utf-8"))
    if (
        document.get("format") != "marklab.vector_semivariogram"
        or document.get("version") != 1
        or document.get("embedding_dimension") != 1280
    ):
        raise ValueError(f"invalid raw-vector variogram: {path}")
    curve = {row["bin_id"]: row for row in document["curve"]}
    values = []
    for band in BANDS:
        row = curve.get(band, {})
        if int(row.get("pair_count", 0)) < 2 or row.get("semivariance") is None:
            raise ValueError(f"variogram band {band} lacks two raw-vector pairs: {path}")
        values.append(finite(row["semivariance"], f"{band} semivariance"))
    return values


def write_fold(path: Path, rows: list[dict], features: list[str], query: bool, domain: str, provenance: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fields = ["region_id", "patient_id", "site_id"]
    if not query:
        fields.append("split")
    fields.extend(["domain", "provenance_sha256", *features])
    with path.open("w", newline="", encoding="utf-8") as target:
        writer = csv.writer(target, lineterminator="\n")
        writer.writerow(fields)
        for row in rows:
            prefix = [row["patient_id"], row["patient_id"], row["site_id"]]
            if not query:
                prefix.append("train")
            writer.writerow([*prefix, domain, provenance, *(repr(value) for value in row["features"])])


def build(args: argparse.Namespace) -> None:
    admitted_headers, admitted_rows = read_csv(args.admitted_patients)
    sources = {str(args.admitted_patients.resolve()): sha256(args.admitted_patients)}
    patient_rows = []
    excluded = []
    for admitted in admitted_rows:
        patient = admitted["patient_id"]
        result_path = args.variogram_results / f"{patient}.json"
        m0_path = args.m0_folds_root / "patient_held_out" / patient / "query.csv"
        if not result_path.is_file() or not m0_path.is_file():
            raise ValueError(f"patient lacks M0/M4 source identity: {patient}")
        try:
            spatial = variogram_features(result_path)
        except ValueError as error:
            excluded.append({"patient_id": patient, "lane": "m4", "blocker": str(error)})
            sources[str(result_path.resolve())] = sha256(result_path)
            continue
        _headers, m0_rows = read_csv(m0_path)
        if len(m0_rows) != 1:
            raise ValueError(f"patient M0 query is invalid: {patient}")
        m0 = [finite(m0_rows[0][name], name) for name in M0_FEATURES]
        patient_rows.append(
            {
                "patient_id": patient,
                "site_id": admitted["site_id"],
                "label": admitted["label"],
                "project_id": admitted.get("project_id", ""),
                "m0": m0,
                "m4": [*m0, *spatial],
            }
        )
        for path in (result_path, m0_path):
            sources[str(path.resolve())] = sha256(path)
    lane_features = {"m0": M0_FEATURES, "m4": [*M0_FEATURES, *M4_FEATURES]}
    provenance = hashlib.sha256(
        json.dumps({"sources": dict(sorted(sources.items())), "features": lane_features}, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()
    out = args.out.resolve()
    if out.exists():
        raise ValueError(f"output already exists: {out}")
    out.parent.mkdir(parents=True, exist_ok=True)
    staging = out.with_name(f".{out.name}.{os.getpid()}.tmp")
    staging.mkdir()
    try:
        included = {row["patient_id"] for row in patient_rows}
        with (staging / "admitted_patients.csv").open("w", newline="", encoding="utf-8") as target:
            writer = csv.DictWriter(target, fieldnames=admitted_headers, lineterminator="\n")
            writer.writeheader()
            writer.writerows(row for row in admitted_rows if row["patient_id"] in included)
        folds = []
        for lane, features in lane_features.items():
            for policy in ("patient_held_out", "site_held_out"):
                for query in patient_rows:
                    training = [
                        row for row in patient_rows
                        if row["patient_id"] != query["patient_id"]
                        and (policy != "site_held_out" or row["site_id"] != query["site_id"])
                    ]
                    fold = staging / "folds" / lane / policy / query["patient_id"]
                    domain = f"tcga_crc_{lane}_{policy}"
                    write_fold(fold / "query.csv", [{**query, "features": query[lane]}], features, True, domain, provenance)
                    write_fold(fold / "training.csv", [{**row, "features": row[lane]} for row in training], features, False, domain, provenance)
                    folds.append(
                        {
                            "lane": lane,
                            "holdout_policy": policy,
                            "patient_id": query["patient_id"],
                            "label": query["label"],
                            "site_id": query["site_id"],
                            "project_id": query["project_id"],
                            "candidate_count": len(training),
                            "training_path": (fold / "training.csv").relative_to(staging).as_posix(),
                            "query_path": (fold / "query.csv").relative_to(staging).as_posix(),
                            "training_sha256": sha256(fold / "training.csv"),
                            "query_sha256": sha256(fold / "query.csv"),
                            "provenance_sha256": provenance,
                        }
                    )
        with (staging / "folds.csv").open("w", newline="", encoding="utf-8") as target:
            writer = csv.DictWriter(target, fieldnames=list(folds[0]), lineterminator="\n")
            writer.writeheader()
            writer.writerows(folds)
        (staging / "admission.json").write_text(
            json.dumps(
                {
                    "schema_name": "marklab_tcga_crc_m4_fingerprint_admission",
                    "schema_version": "1.0",
                    "population_unit": "patient",
                    "patient_count": len(patient_rows),
                    "excluded_patient_count": len(excluded),
                    "excluded_patients": excluded,
                    "m4_feature_count": len(lane_features["m4"]),
                    "spatial_embedding_features": M4_FEATURES,
                    "projection": "none",
                    "fold_count": len(folds),
                    "unavailable_components": {
                        "kernel": "no admitted fold-safe raw-vector kernel output for the current patient subset",
                        "neighborhood": "no admitted fold-safe raw-vector neighborhood output for the current patient subset",
                    },
                    "provenance_sha256": provenance,
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
    parser.add_argument("--m0-folds-root", required=True, type=Path)
    parser.add_argument("--variogram-results", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    return parser.parse_args()


if __name__ == "__main__":
    build(parse_args())
