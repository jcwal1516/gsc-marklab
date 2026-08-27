#!/usr/bin/env python3
"""Prepare leakage-safe TCGA CRC M0/M1 patient retrieval inputs."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import os
from pathlib import Path
import tempfile
from typing import Any, Iterable


SCHEMA_VERSION = "1.0"
M0_FEATURES = [
    "embedding_stage_ordinal",
    "embedding_log1p_all_cell_count",
    "embedding_tumor_fraction",
    "embedding_inflammatory_fraction",
    "embedding_connective_fraction",
    "embedding_cell_density_per_mm2",
]
M1_SPATIAL_FEATURES = [
    "embedding_median_inflammatory_count_50um",
    "embedding_median_stromal_distance_um",
    "embedding_tumor_inflammatory_relative_excess_0_50",
    "embedding_tumor_connective_relative_excess_0_50",
    "embedding_stromal_context_residual_organization",
    "embedding_inflammatory_context_residual_organization",
    "embedding_combined_context_residual_organization",
]
STAGES = {"Stage I": 1.0, "Stage II": 2.0, "Stage III": 3.0, "Stage IV": 4.0}


class AdapterError(ValueError):
    """A source or prepared fold violates the fixed admission contract."""


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while True:
            block = stream.read(1024 * 1024)
            if not block:
                break
            digest.update(block)
    return digest.hexdigest()


def read_csv(path: Path, required: set[str]) -> list[dict[str, str]]:
    if not path.is_file() or path.is_symlink():
        raise AdapterError(f"{path} must be one regular file")
    with path.open(encoding="utf-8", newline="") as stream:
        reader = csv.DictReader(stream)
        if reader.fieldnames is None or not required.issubset(reader.fieldnames):
            raise AdapterError(f"{path} is missing required fields")
        rows = list(reader)
    if not rows:
        raise AdapterError(f"{path} contains no rows")
    return rows


def keyed(rows: list[dict[str, str]], source: str) -> dict[str, dict[str, str]]:
    result: dict[str, dict[str, str]] = {}
    for row in rows:
        patient = row["patient_id"]
        if not patient or patient.strip() != patient or Path(patient).name != patient:
            raise AdapterError(f"{source} patient identity is invalid")
        if patient in result:
            raise AdapterError(f"{source} patient identity is duplicated")
        result[patient] = row
    return result


def finite(row: dict[str, str], name: str) -> float:
    try:
        value = float(row[name])
    except (KeyError, ValueError) as error:
        raise AdapterError(f"{name} is not numeric") from error
    if not math.isfinite(value):
        raise AdapterError(f"{name} is not finite")
    return value


def patient_values(
    patient: str,
    stage: dict[str, str],
    spatial: dict[str, str],
    microenvironment: dict[str, str],
) -> dict[str, float]:
    stage_value = STAGES.get(stage["stage"])
    if stage_value is None:
        raise AdapterError(f"{patient} has unsupported pathologic stage {stage['stage']!r}")
    project = spatial["project_id"]
    if project not in {"TCGA-COAD", "TCGA-READ"} or project != stage["project_id"]:
        raise AdapterError(f"{patient} project identity differs")
    site = spatial["tissue_source_site"]
    if not site or site.strip() != site or Path(site).name != site:
        raise AdapterError(f"{patient} tissue-source site is invalid")
    all_cells = finite(microenvironment, "all_cell_count")
    tumor = finite(microenvironment, "tumor_cell_count")
    inflammatory = finite(microenvironment, "inflammatory_cell_count")
    connective = finite(microenvironment, "connective_cell_count")
    density = finite(spatial, "cell_density_per_mm2")
    background = finite(spatial, "mean_background_fraction")
    if (
        all_cells <= 0.0
        or min(tumor, inflammatory, connective) < 0.0
        or max(tumor, inflammatory, connective) > all_cells
        or density <= 0.0
        or not 0.0 <= background <= 1.0
    ):
        raise AdapterError(f"{patient} composition or technical value is invalid")
    return {
        "embedding_stage_ordinal": stage_value,
        "embedding_log1p_all_cell_count": math.log1p(all_cells),
        "embedding_tumor_fraction": tumor / all_cells,
        "embedding_inflammatory_fraction": inflammatory / all_cells,
        "embedding_connective_fraction": connective / all_cells,
        "embedding_cell_density_per_mm2": density,
        "embedding_median_inflammatory_count_50um": finite(
            microenvironment, "median_inflammatory_count_50um"
        ),
        "embedding_median_stromal_distance_um": finite(
            microenvironment, "median_stromal_distance_um"
        ),
        "embedding_tumor_inflammatory_relative_excess_0_50": finite(
            microenvironment, "tumor_inflammatory_relative_excess_0_50"
        ),
        "embedding_tumor_connective_relative_excess_0_50": finite(
            microenvironment, "tumor_connective_relative_excess_0_50"
        ),
        "embedding_stromal_context_residual_organization": finite(
            microenvironment, "stromal_context_residual_organization"
        ),
        "embedding_inflammatory_context_residual_organization": finite(
            microenvironment, "inflammatory_context_residual_organization"
        ),
        "embedding_combined_context_residual_organization": finite(
            microenvironment, "combined_context_residual_organization"
        ),
    }


def write_csv(path: Path, fields: list[str], rows: Iterable[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=fields, lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def write_json(path: Path, value: Any) -> None:
    encoded = json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    path.write_text(encoded + "\n", encoding="utf-8")


def provenance_digest(
    adapter_sha256: str,
    source_sha256: dict[str, str],
    lane: str,
    holdout_policy: str,
    features: list[str],
) -> str:
    encoded = json.dumps(
        {
            "schema_name": "marklab_crc_fingerprint_retrieval_input",
            "schema_version": SCHEMA_VERSION,
            "adapter_sha256": adapter_sha256,
            "source_sha256": source_sha256,
            "lane": lane,
            "holdout_policy": holdout_policy,
            "features": features,
            "feature_construction": "prespecified_without_molecular_labels",
        },
        sort_keys=True,
        separators=(",", ":"),
    ).encode()
    return hashlib.sha256(encoded).hexdigest()


def varies(rows: list[dict[str, Any]], features: list[str]) -> bool:
    return all(len({row["values"][name] for row in rows}) > 1 for name in features)


def build(args: argparse.Namespace) -> None:
    sources = {
        "molecular_labels": Path(args.molecular_labels),
        "stage_labels": Path(args.stage_labels),
        "spatial_metrics": Path(args.spatial_metrics),
        "microenvironment_metrics": Path(args.microenvironment_metrics),
    }
    molecular = keyed(
        read_csv(sources["molecular_labels"], {"patient_id", "class_name"}),
        "molecular labels",
    )
    stages = keyed(
        read_csv(sources["stage_labels"], {"patient_id", "project_id", "stage"}),
        "stage labels",
    )
    spatial = keyed(
        read_csv(
            sources["spatial_metrics"],
            {
                "patient_id",
                "project_id",
                "tissue_source_site",
                "cell_density_per_mm2",
                "mean_background_fraction",
            },
        ),
        "spatial metrics",
    )
    microenvironment = keyed(
        read_csv(
            sources["microenvironment_metrics"],
            {
                "patient_id",
                "tumor_cell_count",
                "all_cell_count",
                "inflammatory_cell_count",
                "connective_cell_count",
                "median_inflammatory_count_50um",
                "median_stromal_distance_um",
                "tumor_inflammatory_relative_excess_0_50",
                "tumor_connective_relative_excess_0_50",
                "stromal_context_residual_organization",
                "inflammatory_context_residual_organization",
                "combined_context_residual_organization",
            },
        ),
        "microenvironment metrics",
    )
    exclusions: list[dict[str, Any]] = []
    admitted: list[dict[str, Any]] = []
    all_patients = sorted(set(molecular) | set(stages) | set(spatial) | set(microenvironment))
    for patient in all_patients:
        missing = [
            name
            for name, table in [
                ("molecular_labels", molecular),
                ("stage_labels", stages),
                ("spatial_metrics", spatial),
                ("microenvironment_metrics", microenvironment),
            ]
            if patient not in table
        ]
        if missing:
            exclusions.append({"patient_id": patient, "reasons": missing})
            continue
        label = molecular[patient]["class_name"]
        if label not in {"MSI", "MSS"}:
            exclusions.append(
                {"patient_id": patient, "reasons": [f"unsupported_molecular_label:{label}"]}
            )
            continue
        try:
            values = patient_values(
                patient, stages[patient], spatial[patient], microenvironment[patient]
            )
        except AdapterError as error:
            exclusions.append({"patient_id": patient, "reasons": [str(error)]})
            continue
        admitted.append(
            {
                "patient_id": patient,
                "label": label,
                "stage": stages[patient]["stage"],
                "project_id": spatial[patient]["project_id"],
                "site_id": spatial[patient]["tissue_source_site"],
                "values": values,
            }
        )
    if len(admitted) < 24:
        raise AdapterError("fewer than 24 complete TCGA CRC patients are admitted")
    group_counts = {
        label: sum(patient["label"] == label for patient in admitted)
        for label in ["MSI", "MSS"]
    }
    if min(group_counts.values()) < 4:
        raise AdapterError("each molecular group requires at least four patients")
    source_sha256 = {str(path): sha256(path) for path in sources.values()}
    adapter_sha256 = sha256(Path(__file__))
    output = Path(args.out)
    if output.exists() or output.is_symlink():
        raise AdapterError(f"output already exists: {output}")
    output.parent.mkdir(parents=True, exist_ok=True)
    staging = Path(tempfile.mkdtemp(prefix=f".{output.name}.part-", dir=output.parent))
    file_sha256: dict[str, str] = {}
    fold_rows: list[dict[str, Any]] = []
    lanes = {"m0": M0_FEATURES, "m1": [*M0_FEATURES, *M1_SPATIAL_FEATURES]}
    for lane, features in lanes.items():
        for holdout_policy in ["patient_held_out", "site_held_out"]:
            provenance = provenance_digest(
                adapter_sha256, source_sha256, lane, holdout_policy, features
            )
            domain = f"tcga_crc_{lane}_{holdout_policy}"
            for query_patient in admitted:
                training_patients = [
                    patient
                    for patient in admitted
                    if patient["patient_id"] != query_patient["patient_id"]
                    and (
                        holdout_policy != "site_held_out"
                        or patient["site_id"] != query_patient["site_id"]
                    )
                ]
                if len(training_patients) < 4 or not varies(training_patients, features):
                    raise AdapterError(
                        f"{lane} {holdout_policy} fold for {query_patient['patient_id']} is degenerate"
                    )
                relative_root = Path("folds") / lane / holdout_policy / query_patient["patient_id"]
                training_relative = relative_root / "training.csv"
                query_relative = relative_root / "query.csv"
                training_path = staging / training_relative
                query_path = staging / query_relative
                training_fields = [
                    "region_id",
                    "patient_id",
                    "site_id",
                    "split",
                    "domain",
                    "provenance_sha256",
                    *features,
                ]
                query_fields = [
                    "region_id",
                    "patient_id",
                    "site_id",
                    "domain",
                    "provenance_sha256",
                    *features,
                ]
                write_csv(
                    training_path,
                    training_fields,
                    (
                        {
                            "region_id": patient["patient_id"],
                            "patient_id": patient["patient_id"],
                            "site_id": patient["site_id"],
                            "split": "train",
                            "domain": domain,
                            "provenance_sha256": provenance,
                            **{name: patient["values"][name] for name in features},
                        }
                        for patient in training_patients
                    ),
                )
                write_csv(
                    query_path,
                    query_fields,
                    [
                        {
                            "region_id": query_patient["patient_id"],
                            "patient_id": query_patient["patient_id"],
                            "site_id": query_patient["site_id"],
                            "domain": domain,
                            "provenance_sha256": provenance,
                            **{
                                name: query_patient["values"][name]
                                for name in features
                            },
                        }
                    ],
                )
                training_digest = sha256(training_path)
                query_digest = sha256(query_path)
                file_sha256[training_relative.as_posix()] = training_digest
                file_sha256[query_relative.as_posix()] = query_digest
                fold_rows.append(
                    {
                        "lane": lane,
                        "holdout_policy": holdout_policy,
                        "patient_id": query_patient["patient_id"],
                        "label": query_patient["label"],
                        "site_id": query_patient["site_id"],
                        "project_id": query_patient["project_id"],
                        "candidate_count": len(training_patients),
                        "training_path": training_relative.as_posix(),
                        "query_path": query_relative.as_posix(),
                        "training_sha256": training_digest,
                        "query_sha256": query_digest,
                        "provenance_sha256": provenance,
                    }
                )
    patient_fields = ["patient_id", "label", "stage", "project_id", "site_id"]
    write_csv(
        staging / "admitted_patients.csv",
        patient_fields,
        ({name: patient[name] for name in patient_fields} for patient in admitted),
    )
    file_sha256["admitted_patients.csv"] = sha256(staging / "admitted_patients.csv")
    fold_fields = [
        "lane",
        "holdout_policy",
        "patient_id",
        "label",
        "site_id",
        "project_id",
        "candidate_count",
        "training_path",
        "query_path",
        "training_sha256",
        "query_sha256",
        "provenance_sha256",
    ]
    write_csv(staging / "folds.csv", fold_fields, fold_rows)
    file_sha256["folds.csv"] = sha256(staging / "folds.csv")
    site_counts = {
        site: sum(patient["site_id"] == site for patient in admitted)
        for site in sorted({patient["site_id"] for patient in admitted})
    }
    project_counts = {
        project: sum(patient["project_id"] == project for patient in admitted)
        for project in ["TCGA-COAD", "TCGA-READ"]
    }
    write_json(
        staging / "admission.json",
        {
            "schema_name": "marklab_crc_fingerprint_admission",
            "schema_version": SCHEMA_VERSION,
            "cohort": "TCGA CRC H&E CellViT",
            "patient_count": len(admitted),
            "fold_count": len(fold_rows),
            "group_counts": group_counts,
            "project_counts": project_counts,
            "site_counts": site_counts,
            "m0_feature_names": M0_FEATURES,
            "m1_feature_names": [*M0_FEATURES, *M1_SPATIAL_FEATURES],
            "feature_construction": "prespecified_without_molecular_labels",
            "normalization": "inside_each_held_out_training_set_by_marklab_region_retrieval",
            "holdout_policies": ["patient_held_out", "site_held_out"],
            "unavailable_components": {
                "mean_background_fraction": "constant_zero_across_the_admitted_TCGA_patient_cohort"
            },
            "population_unit": "patient",
            "adapter_sha256": adapter_sha256,
            "source_sha256": source_sha256,
            "file_sha256": file_sha256,
            "exclusions": exclusions,
            "claim_status": "exploratory_patient_held_out_crc_fingerprint_inputs",
        },
    )
    os.replace(staging, output)


def parser() -> argparse.ArgumentParser:
    value = argparse.ArgumentParser()
    value.add_argument("--molecular-labels", required=True)
    value.add_argument("--stage-labels", required=True)
    value.add_argument("--spatial-metrics", required=True)
    value.add_argument("--microenvironment-metrics", required=True)
    value.add_argument("--out", required=True)
    return value


if __name__ == "__main__":
    try:
        build(parser().parse_args())
    except Exception as error:
        print(f"TCGA CRC fingerprint adapter failed: {type(error).__name__}: {error}", file=os.sys.stderr)
        raise SystemExit(2) from error
