#!/usr/bin/env python3
"""Build the TCGA CRC fusion lane from fingerprint components that passed QC."""

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
M0_FEATURES = [
    "embedding_stage_ordinal",
    "embedding_log1p_all_cell_count",
    "embedding_tumor_fraction",
    "embedding_inflammatory_fraction",
    "embedding_connective_fraction",
    "embedding_cell_density_per_mm2",
]
M2_STABLE_FEATURES = [
    "embedding_coordinate_l_relative_25um",
    "embedding_coordinate_l_relative_50um",
    "embedding_coordinate_l_relative_75um",
    "embedding_coordinate_l_relative_100um",
]
M3_STABLE_FEATURES = [
    f"embedding_raw_{summary}_q{quantile:03d}"
    for summary in (
        "channel_mean",
        "channel_population_sd",
        "cell_root_mean_square",
        "cell_mean",
        "cell_population_sd",
    )
    for quantile in range(0, 101, 10)
]
M4_STABLE_FEATURES = [
    "embedding_raw_variogram_25_50um",
    "embedding_raw_variogram_50_100um",
]
M6_FEATURES = [
    *M0_FEATURES,
    *M2_STABLE_FEATURES,
    *M3_STABLE_FEATURES,
    *M4_STABLE_FEATURES,
]


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
        rows = list(reader)
    if not rows:
        raise ValueError(f"CSV has no rows: {path}")
    return list(reader.fieldnames), rows


def finite(row: dict[str, str], name: str) -> float:
    value = float(row[name])
    if not math.isfinite(value):
        raise ValueError(f"fusion feature is non-finite: {name}")
    return value


def query_values(path: Path, patient_id: str, features: list[str]) -> list[float]:
    _, rows = read_csv(path)
    if len(rows) != 1 or rows[0].get("patient_id") != patient_id:
        raise ValueError(f"fusion query identity differs: {path}")
    return [finite(rows[0], feature) for feature in features]


def stable_m4_values(document: dict) -> list[float]:
    if (
        document.get("format") != "marklab.vector_semivariogram"
        or document.get("version") != 1
        or document.get("embedding_dimension") != 1280
    ):
        raise ValueError("invalid M4 raw-vector semivariogram")
    curve = {row["bin_id"]: row for row in document["curve"]}
    values = []
    for band in ("intermediate", "far"):
        row = curve.get(band, {})
        value = row.get("semivariance")
        if int(row.get("pair_count", 0)) < 2 or value is None or not math.isfinite(float(value)):
            raise ValueError(f"stable M4 band lacks two finite pairs: {band}")
        values.append(float(value))
    return values


def write_fold(
    path: Path,
    rows: list[dict],
    features: list[str],
    *,
    query: bool,
    domain: str,
    provenance: str,
) -> None:
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
            writer.writerow(
                [*prefix, domain, provenance, *(repr(value) for value in row["features"])]
            )


def validate_selection(args: argparse.Namespace) -> dict[str, str]:
    paths = {
        "m2_stability": args.m2_stability.resolve(),
        "m3_stability": args.m3_stability.resolve(),
        "m4_stability": args.m4_stability.resolve(),
    }
    documents = {
        name: json.loads(path.read_text(encoding="utf-8")) for name, path in paths.items()
    }
    m2 = documents["m2_stability"]
    m3 = documents["m3_stability"]
    m4 = documents["m4_stability"]
    if (
        min(
            row["cell_subsample_patient_rank_spearman"]
            for row in m2["scale_summary"].values()
        )
        < 0.90
        or min(
            row["patient_rank_spearman"]
            for row in m2["nearby_scale_summary"].values()
        )
        < 0.50
        or m3.get("fusion_eligible") is not True
        or m4["bands"]["intermediate"].get("fusion_eligible") is not True
        or m4["bands"]["far"].get("fusion_eligible") is not True
        or m4["bands"]["near"].get("fusion_eligible") is not False
    ):
        raise ValueError("fusion QC selection does not match the admitted component set")
    return {str(path): sha256(path) for path in paths.values()}


def build(args: argparse.Namespace) -> None:
    patient_headers, admitted = read_csv(args.admitted_patients)
    stability_sources = validate_selection(args)
    patient_rows = []
    excluded = []
    sources = {
        str(args.admitted_patients.resolve()): sha256(args.admitted_patients),
        **stability_sources,
    }
    for patient in admitted:
        patient_id = patient["patient_id"]
        m0_path = args.m0_folds_root / "patient_held_out" / patient_id / "query.csv"
        m2_path = (
            args.m2_admission
            / "folds"
            / "m2"
            / "patient_held_out"
            / patient_id
            / "query.csv"
        )
        m3_path = (
            args.m3_admission
            / "folds"
            / "m3"
            / "patient_held_out"
            / patient_id
            / "query.csv"
        )
        m4_path = args.m4_results / f"{patient_id}.json"
        missing = [
            name
            for name, path in (("m0", m0_path), ("m2", m2_path), ("m3", m3_path), ("m4", m4_path))
            if not path.is_file()
        ]
        if missing:
            excluded.append(
                {
                    "patient_id": patient_id,
                    "blocker": "missing fusion component identity: " + ",".join(missing),
                }
            )
            continue
        try:
            m0 = query_values(m0_path, patient_id, M0_FEATURES)
            m2 = query_values(m2_path, patient_id, M2_STABLE_FEATURES)
            m3 = query_values(m3_path, patient_id, M3_STABLE_FEATURES)
            m4_document = json.loads(m4_path.read_text(encoding="utf-8"))
            m4 = stable_m4_values(m4_document)
        except (ValueError, KeyError) as error:
            excluded.append({"patient_id": patient_id, "blocker": str(error)})
            continue
        patient_rows.append(
            {
                **patient,
                "m0": m0,
                "m6": [*m0, *m2, *m3, *m4],
            }
        )
        for path in (m0_path, m2_path, m3_path, m4_path):
            sources[str(path.resolve())] = sha256(path)
    if len(patient_rows) < 24:
        raise ValueError("fewer than 24 patients have complete stable fusion components")
    lanes = {"m0": M0_FEATURES, "m6": M6_FEATURES}
    provenance = hashlib.sha256(
        json.dumps(
            {
                "sources": dict(sorted(sources.items())),
                "features": lanes,
                "selection": "stability_qc_not_molecular_association",
            },
            sort_keys=True,
            separators=(",", ":"),
        ).encode()
    ).hexdigest()
    out = args.out.resolve()
    if out.exists() or out.is_symlink():
        raise ValueError(f"output already exists: {out}")
    out.parent.mkdir(parents=True, exist_ok=True)
    staging = out.with_name(f".{out.name}.{os.getpid()}.tmp")
    staging.mkdir()
    try:
        included = {row["patient_id"] for row in patient_rows}
        with (staging / "admitted_patients.csv").open(
            "w", newline="", encoding="utf-8"
        ) as target:
            writer = csv.DictWriter(
                target, fieldnames=patient_headers, lineterminator="\n"
            )
            writer.writeheader()
            writer.writerows(row for row in admitted if row["patient_id"] in included)
        folds = []
        for lane, features in lanes.items():
            for policy in ("patient_held_out", "site_held_out"):
                for query in patient_rows:
                    training = [
                        row
                        for row in patient_rows
                        if row["patient_id"] != query["patient_id"]
                        and (policy != "site_held_out" or row["site_id"] != query["site_id"])
                    ]
                    fold = staging / "folds" / lane / policy / query["patient_id"]
                    domain = f"tcga_crc_{lane}_{policy}"
                    write_fold(
                        fold / "query.csv",
                        [{**query, "features": query[lane]}],
                        features,
                        query=True,
                        domain=domain,
                        provenance=provenance,
                    )
                    write_fold(
                        fold / "training.csv",
                        [{**row, "features": row[lane]} for row in training],
                        features,
                        query=False,
                        domain=domain,
                        provenance=provenance,
                    )
                    folds.append(
                        {
                            "lane": lane,
                            "holdout_policy": policy,
                            "patient_id": query["patient_id"],
                            "label": query["label"],
                            "site_id": query["site_id"],
                            "project_id": query.get("project_id", ""),
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
                    "schema_name": "marklab_tcga_crc_m6_fusion_admission",
                    "schema_version": SCHEMA_VERSION,
                    "population_unit": "patient",
                    "patient_count": len(patient_rows),
                    "excluded_patient_count": len(excluded),
                    "excluded_patients": excluded,
                    "m6_feature_count": len(M6_FEATURES),
                    "m6_feature_names": M6_FEATURES,
                    "included_components": {
                        "m0": M0_FEATURES,
                        "m2_classical_coordinate": M2_STABLE_FEATURES,
                        "m3_raw_nonspatial": M3_STABLE_FEATURES,
                        "m4_raw_spatial_intermediate_far": M4_STABLE_FEATURES,
                    },
                    "excluded_components": {
                        "m1_annotation_spatial": "four-field cell/ROI stability was not available in this bounded checkpoint",
                        "m2_graph": "2x2-vs-3x3 patient-rank stability was 0.27-0.61",
                        "m4_near": "only 87 patients had at least two field-level near-bin estimates",
                        "m5_patch": "no independent genuine patch embedding tensor was admitted",
                    },
                    "selection_policy": "component_stability_only_not_molecular_effect_or_p_value",
                    "normalization": "inside_each_held_out_training_fold_by_marklab_region_retrieval",
                    "fold_count": len(folds),
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
    parser.add_argument("--m2-admission", required=True, type=Path)
    parser.add_argument("--m3-admission", required=True, type=Path)
    parser.add_argument("--m4-results", required=True, type=Path)
    parser.add_argument("--m2-stability", required=True, type=Path)
    parser.add_argument("--m3-stability", required=True, type=Path)
    parser.add_argument("--m4-stability", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    return parser.parse_args()


if __name__ == "__main__":
    build(parse_args())
