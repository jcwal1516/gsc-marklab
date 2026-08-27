#!/usr/bin/env python3
"""Join existing TCGA CRC coordinate workflows into cumulative M2 folds."""

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
RADII_UM = (25.0, 50.0, 75.0, 100.0)
M2_FEATURES = [
    *(f"embedding_coordinate_l_relative_{int(radius)}um" for radius in RADII_UM),
    *(f"embedding_coordinate_graph_{grid}x{grid}_{band}_energy_fraction" for grid in (2, 3) for band in ("low", "middle", "high")),
]


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_csv(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as source:
        return list(csv.DictReader(source))


def finite(value: object, label: str) -> float:
    parsed = float(value)
    if not math.isfinite(parsed):
        raise ValueError(f"{label} must be finite")
    return parsed


def classical_features(path: Path, patient: str) -> list[float]:
    document = json.loads(path.read_text(encoding="utf-8"))
    analysis = document.get("analysis")
    if document.get("format") != "marklab.classical_spatial" or not isinstance(analysis, dict):
        raise ValueError(f"invalid classical result: {path}")
    if analysis.get("case_id") != patient or analysis.get("status") != "available":
        raise ValueError(f"classical result is unavailable or belongs to another patient: {path}")
    curve = analysis.get("curve")
    if not isinstance(curve, list):
        raise ValueError(f"classical result lacks a curve: {path}")
    by_radius = {finite(point.get("radius_um"), "classical radius"): point for point in curve}
    values = []
    for radius in RADII_UM:
        point = by_radius.get(radius)
        if not isinstance(point, dict) or point.get("status") != "available":
            raise ValueError(f"classical radius {radius} is unavailable: {path}")
        values.append(finite(point.get("l"), "classical L") / radius - 1.0)
    return values


def graph_features(path: Path) -> list[float]:
    document = json.loads(path.read_text(encoding="utf-8"))
    if document.get("format") != "marklab.graph_spectral" or document.get("version") != 1:
        raise ValueError(f"invalid graph-spectral result: {path}")
    bands = document.get("frequency_bands")
    if not isinstance(bands, list):
        raise ValueError(f"graph-spectral result lacks bands: {path}")
    by_id = {band.get("id"): band for band in bands if isinstance(band, dict)}
    values = [finite(by_id.get(band, {}).get("energy_fraction"), f"{band} energy") for band in ("low", "middle", "high")]
    if any(value < 0.0 or value > 1.0 for value in values) or abs(math.fsum(values) - 1.0) > 1e-9:
        raise ValueError(f"graph-spectral energy fractions are invalid: {path}")
    return values


def classical_result_path(root: Path, patient: str) -> Path:
    canonical = root / patient / "result.json"
    if canonical.is_file():
        return canonical
    cli_directory = root / f"{patient}.json" / "result.json"
    return cli_directory


def write_fold(path: Path, rows: list[dict], feature_names: list[str], *, query: bool, domain: str, provenance: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    metadata = ["region_id", "patient_id", "site_id"]
    if not query:
        metadata.append("split")
    metadata.extend(["domain", "provenance_sha256"])
    with path.open("w", newline="", encoding="utf-8") as target:
        writer = csv.writer(target, lineterminator="\n")
        writer.writerow([*metadata, *feature_names])
        for row in rows:
            prefix = [row["patient_id"], row["patient_id"], row["site_id"]]
            if not query:
                prefix.append("train")
            prefix.extend([domain, provenance])
            writer.writerow([*prefix, *(repr(value) for value in row["features"])])


def build(args: argparse.Namespace) -> None:
    admitted_path = args.admitted_patients.resolve()
    admitted_rows = read_csv(admitted_path)
    if not admitted_rows:
        raise ValueError("admitted patient table is empty")
    admitted = {}
    for row in admitted_rows:
        patient = row["patient_id"]
        site = row["site_id"]
        if not patient or not site or patient in admitted:
            raise ValueError("admitted patient/site identities must be unique and nonempty")
        admitted[patient] = row

    sources = {str(admitted_path): sha256(admitted_path)}
    patient_rows = []
    excluded_patients = []
    feature_names = [*M0_FEATURES, *M2_FEATURES]
    for patient in sorted(admitted):
        query_path = args.m0_folds_root / "patient_held_out" / patient / "query.csv"
        classical_path = classical_result_path(args.classical_results, patient)
        graph2_path = args.graph_2x2_results / f"{patient}.json"
        graph3_path = args.graph_3x3_results / f"{patient}.json"
        paths = [query_path, classical_path, graph2_path, graph3_path]
        if any(not path.is_file() for path in paths):
            raise ValueError(f"patient {patient} lacks a complete M0/M2 source set")
        query = read_csv(query_path)
        if len(query) != 1 or query[0].get("patient_id") != patient:
            raise ValueError(f"M0 query has the wrong patient identity: {query_path}")
        m0 = [finite(query[0].get(name), name) for name in M0_FEATURES]
        try:
            coordinate = classical_features(classical_path, patient)
        except ValueError as error:
            excluded_patients.append(
                {"patient_id": patient, "lane": "m2", "blocker": str(error)}
            )
            sources[str(classical_path.resolve())] = sha256(classical_path)
            continue
        features = [*m0, *coordinate, *graph_features(graph2_path), *graph_features(graph3_path)]
        if len(features) != len(feature_names):
            raise AssertionError("M2 feature shape mismatch")
        patient_rows.append(
            {
                "patient_id": patient,
                "site_id": admitted[patient]["site_id"],
                "molecular_group": admitted[patient].get("label", ""),
                "features": features,
            }
        )
        for path in paths:
            sources[str(path.resolve())] = sha256(path)

    provenance = hashlib.sha256(
        json.dumps(
            {"sources": dict(sorted(sources.items())), "features": feature_names},
            separators=(",", ":"),
            sort_keys=True,
        ).encode()
    ).hexdigest()
    out = args.out.resolve()
    if out.exists():
        raise ValueError(f"output already exists: {out}")
    out.parent.mkdir(parents=True, exist_ok=True)
    staging = out.with_name(f".{out.name}.{os.getpid()}.tmp")
    staging.mkdir()
    try:
        fold_manifest = []
        admitted_headers = list(admitted_rows[0])
        included = {row["patient_id"] for row in patient_rows}
        with (staging / "admitted_patients.csv").open("w", newline="", encoding="utf-8") as target:
            writer = csv.DictWriter(target, fieldnames=admitted_headers, lineterminator="\n")
            writer.writeheader()
            writer.writerows(row for row in admitted_rows if row["patient_id"] in included)
        with (staging / "patient_features.csv").open("w", newline="", encoding="utf-8") as target:
            writer = csv.writer(target, lineterminator="\n")
            writer.writerow(["patient_id", "site_id", "molecular_group", *feature_names])
            for row in patient_rows:
                writer.writerow(
                    [
                        row["patient_id"],
                        row["site_id"],
                        row["molecular_group"],
                        *(repr(value) for value in row["features"]),
                    ]
                )
        for policy in ("patient_held_out", "site_held_out"):
            for query in patient_rows:
                training = [
                    row
                    for row in patient_rows
                    if row["patient_id"] != query["patient_id"]
                    and (policy != "site_held_out" or row["site_id"] != query["site_id"])
                ]
                if not training:
                    raise ValueError(f"{policy} leaves no candidates for {query['patient_id']}")
                fold = staging / "folds" / "m2" / policy / query["patient_id"]
                domain = f"tcga_crc_m2_{policy}"
                write_fold(
                    fold / "query.csv",
                    [query],
                    feature_names,
                    query=True,
                    domain=domain,
                    provenance=provenance,
                )
                write_fold(
                    fold / "training.csv",
                    training,
                    feature_names,
                    query=False,
                    domain=domain,
                    provenance=provenance,
                )
                training_path = fold / "training.csv"
                query_path = fold / "query.csv"
                fold_manifest.append(
                    {
                        "lane": "m2",
                        "holdout_policy": policy,
                        "patient_id": query["patient_id"],
                        "label": query["molecular_group"],
                        "site_id": query["site_id"],
                        "project_id": admitted[query["patient_id"]].get("project_id", ""),
                        "candidate_count": len(training),
                        "training_path": training_path.relative_to(staging).as_posix(),
                        "query_path": query_path.relative_to(staging).as_posix(),
                        "training_sha256": sha256(training_path),
                        "query_sha256": sha256(query_path),
                        "provenance_sha256": provenance,
                    }
                )
                baseline_fold = staging / "folds" / "m0" / policy / query["patient_id"]
                baseline_query_row = {**query, "features": query["features"][: len(M0_FEATURES)]}
                baseline_training_rows = [
                    {**row, "features": row["features"][: len(M0_FEATURES)]} for row in training
                ]
                baseline_domain = f"tcga_crc_m0_{policy}"
                write_fold(
                    baseline_fold / "query.csv",
                    [baseline_query_row],
                    M0_FEATURES,
                    query=True,
                    domain=baseline_domain,
                    provenance=provenance,
                )
                write_fold(
                    baseline_fold / "training.csv",
                    baseline_training_rows,
                    M0_FEATURES,
                    query=False,
                    domain=baseline_domain,
                    provenance=provenance,
                )
                baseline_training = baseline_fold / "training.csv"
                baseline_query = baseline_fold / "query.csv"
                fold_manifest.append(
                    {
                        "lane": "m0",
                        "holdout_policy": policy,
                        "patient_id": query["patient_id"],
                        "label": query["molecular_group"],
                        "site_id": query["site_id"],
                        "project_id": admitted[query["patient_id"]].get("project_id", ""),
                        "candidate_count": len(training),
                        "training_path": baseline_training.relative_to(staging).as_posix(),
                        "query_path": baseline_query.relative_to(staging).as_posix(),
                        "training_sha256": sha256(baseline_training),
                        "query_sha256": sha256(baseline_query),
                        "provenance_sha256": provenance,
                    }
                )
        with (staging / "folds.csv").open("w", newline="", encoding="utf-8") as target:
            writer = csv.DictWriter(target, fieldnames=list(fold_manifest[0]), lineterminator="\n")
            writer.writeheader()
            writer.writerows(fold_manifest)
        (staging / "admission.json").write_text(
            json.dumps(
                {
                    "schema_name": "marklab_tcga_crc_m2_fingerprint_admission",
                    "schema_version": "1.0",
                    "population_unit": "patient",
                    "patient_count": len(patient_rows),
                    "excluded_patient_count": len(excluded_patients),
                    "excluded_patients": excluded_patients,
                    "feature_count": len(feature_names),
                    "feature_names": feature_names,
                    "molecular_labels_used_for_features": False,
                    "classical_scales_um": list(RADII_UM),
                    "graph_grid_scales": ["2x2", "3x3"],
                    "fold_policies": ["patient_held_out", "site_held_out"],
                    "fold_count": len(fold_manifest),
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
    parser.add_argument("--m0-folds-root", required=True, type=Path)
    parser.add_argument("--admitted-patients", required=True, type=Path)
    parser.add_argument("--classical-results", required=True, type=Path)
    parser.add_argument("--graph-2x2-results", required=True, type=Path)
    parser.add_argument("--graph-3x3-results", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    return parser.parse_args()


if __name__ == "__main__":
    build(parse_args())
