#!/usr/bin/env python3
"""Build patient-held-out Schuerch CODEX neighborhood fingerprints."""

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
NEIGHBORHOODS = {
    "1": ("T cell enriched", "embedding_microenvironment_t_cell_enriched_fraction"),
    "2": ("Bulk tumor", "embedding_microenvironment_bulk_tumor_fraction"),
    "3": (
        "Immune-infiltrated stroma",
        "embedding_microenvironment_immune_infiltrated_stroma_fraction",
    ),
    "4": (
        "Macrophage enriched",
        "embedding_microenvironment_macrophage_enriched_fraction",
    ),
    "5": ("Follicle", "embedding_microenvironment_follicle_fraction"),
    "6": ("Tumor boundary", "embedding_microenvironment_tumor_boundary_fraction"),
    "7": (
        "Vascularized smooth muscle",
        "embedding_microenvironment_vascularized_smooth_muscle_fraction",
    ),
    "8": ("Smooth muscle", "embedding_microenvironment_smooth_muscle_fraction"),
    "9": (
        "Granulocyte enriched",
        "embedding_microenvironment_granulocyte_enriched_fraction",
    ),
}
FEATURES = [value[1] for value in NEIGHBORHOODS.values()]


def admit_cell(cluster_name: str, neighborhood_number: str, excluded_clusters: set[str]) -> bool:
    return cluster_name not in excluded_clusters and neighborhood_number in NEIGHBORHOODS


def cell_subsample_admitted(object_id: str) -> bool:
    if not object_id:
        raise ValueError("CODEX cell subsample identity is empty")
    value = int.from_bytes(
        hashlib.sha256(("schurch-codex-cell-stability-v1:" + object_id).encode()).digest()[:8],
        "big",
    )
    return value < int(CELL_SUBSAMPLE_FRACTION * (1 << 64))


def canonical_patient_order(patient_ids: list[str]) -> list[str]:
    if any(not patient_id for patient_id in patient_ids):
        raise ValueError("CODEX patient identity is empty")
    return sorted(patient_ids)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_fingerprint(
    path: Path,
    rows: list[dict],
    *,
    query: bool,
    provenance: str,
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fields = ["region_id", "patient_id", "site_id"]
    if not query:
        fields.append("split")
    fields.extend(["domain", "provenance_sha256", *FEATURES])
    with path.open("w", newline="", encoding="utf-8") as target:
        writer = csv.writer(target, lineterminator="\n")
        writer.writerow(fields)
        for row in rows:
            prefix = [row["patient_id"], row["patient_id"], "schurch_codex_2020"]
            if not query:
                prefix.append("train")
            writer.writerow(
                [
                    *prefix,
                    "schurch_codex_neighborhood_patient_held_out",
                    provenance,
                    *(repr(row[feature]) for feature in FEATURES),
                ]
            )


def fractions(counts: dict[str, int]) -> dict[str, float]:
    total = sum(counts.values())
    if total < 1:
        raise ValueError("CODEX neighborhood fingerprint has no admitted cells")
    values = {
        NEIGHBORHOODS[number][1]: counts.get(number, 0) / total
        for number in NEIGHBORHOODS
    }
    if not math.isclose(sum(values.values()), 1.0, rel_tol=0.0, abs_tol=1e-12):
        raise ValueError("CODEX neighborhood fractions do not sum to one")
    return values


def build(args: argparse.Namespace) -> None:
    provenance_document = json.loads(args.provenance.read_text(encoding="utf-8"))
    if (
        provenance_document.get("source_file") != args.cells.name
        or provenance_document.get("source_sha256") != sha256(args.cells)
        or provenance_document.get("license") != "CC BY 4.0"
        or float(provenance_document.get("coordinate_scale_um_per_pixel")) != 0.37744
    ):
        raise ValueError("Schuerch CODEX source provenance differs")
    with args.molecular_labels.open(newline="", encoding="utf-8") as source:
        label_rows = list(csv.DictReader(source))
    labels = {row["patient_id"]: row["class_name"] for row in label_rows}
    if len(labels) != len(label_rows):
        raise ValueError("Schuerch molecular patient identity is duplicated")
    patient_counts: dict[str, dict[str, int]] = {}
    patient_subsample_counts: dict[str, dict[str, int]] = {}
    core_counts: dict[tuple[str, str], dict[str, int]] = {}
    original_names: dict[str, set[str]] = {number: set() for number in NEIGHBORHOODS}
    source_rows = 0
    provenance_retained_rows = 0
    admitted_rows = 0
    excluded_clusters = set(provenance_document["excluded_cluster_names"])
    with args.cells.open(newline="", encoding="utf-8-sig") as source:
        reader = csv.DictReader(source)
        required = {
            "CellID",
            "File Name",
            "patients",
            "spots",
            "neighborhood number final",
            "neighborhood name",
            "ClusterName",
            "X:X",
            "Y:Y",
        }
        if reader.fieldnames is None or not required.issubset(reader.fieldnames):
            raise ValueError("Schuerch CODEX cells lack required identity or neighborhood fields")
        for row in reader:
            source_rows += 1
            patient = row["patients"]
            number = row["neighborhood number final"]
            if patient not in labels:
                raise ValueError(f"Schuerch CODEX patient lacks molecular identity: {patient}")
            if row["ClusterName"] not in excluded_clusters:
                provenance_retained_rows += 1
            if not admit_cell(row["ClusterName"], number, excluded_clusters):
                continue
            if row["neighborhood name"] != NEIGHBORHOODS[number][0]:
                raise ValueError("Schuerch neighborhood number/name mapping differs")
            if not row["File Name"] or not row["CellID"]:
                raise ValueError("Schuerch CODEX cell/core identity is empty")
            original_names[number].add(row["neighborhood name"])
            patient_counts.setdefault(patient, {})[number] = (
                patient_counts.setdefault(patient, {}).get(number, 0) + 1
            )
            key = (patient, row["spots"])
            core_counts.setdefault(key, {})[number] = core_counts.setdefault(key, {}).get(number, 0) + 1
            if cell_subsample_admitted(f"{row['File Name']}:{row['CellID']}"):
                patient_subsample_counts.setdefault(patient, {})[number] = (
                    patient_subsample_counts.setdefault(patient, {}).get(number, 0) + 1
                )
            admitted_rows += 1
    if source_rows != provenance_document["summary"]["source_rows"]:
        raise ValueError("Schuerch CODEX source row count differs")
    if provenance_retained_rows != provenance_document["summary"]["retained_rows"]:
        raise ValueError("Schuerch CODEX retained row count differs")
    patients = []
    exclusions = []
    for patient in canonical_patient_order(list(patient_counts)):
        label = labels[patient]
        if label not in {"MSI", "MSS"}:
            exclusions.append(
                {
                    "patient_id": patient,
                    "label": label,
                    "blocker": "molecular class is outside the prespecified MSI/MSS contrast",
                }
            )
            continue
        cores = sorted(core for current_patient, core in core_counts if current_patient == patient)
        if len(cores) != 4:
            exclusions.append(
                {
                    "patient_id": patient,
                    "label": label,
                    "blocker": f"expected four tissue cores but observed {len(cores)}",
                }
            )
            continue
        patients.append(
            {
                "patient_id": patient,
                "label": label,
                "site_id": "schurch_codex_2020",
                "project_id": "Schurch-CODEX",
                "core_count": len(cores),
                "cell_count": sum(patient_counts[patient].values()),
                **fractions(patient_counts[patient]),
            }
        )
    if len(patients) != 34 or sum(row["label"] == "MSI" for row in patients) != 4:
        raise ValueError("Schuerch CODEX molecular admission dimensions differ")
    source_sha256 = {
        str(args.cells.resolve()): sha256(args.cells),
        str(args.provenance.resolve()): sha256(args.provenance),
        str(args.molecular_labels.resolve()): sha256(args.molecular_labels),
    }
    provenance = hashlib.sha256(
        json.dumps(
            {
                "sources": source_sha256,
                "features": FEATURES,
                "vocabulary": NEIGHBORHOODS,
                "feature_construction": "protein_blind_declared_neighborhood_fractions",
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
        patient_fields = [
            "patient_id",
            "label",
            "site_id",
            "project_id",
            "core_count",
            "cell_count",
            *FEATURES,
        ]
        with (staging / "patient_fingerprints.csv").open(
            "w", newline="", encoding="utf-8"
        ) as target:
            writer = csv.DictWriter(target, fieldnames=patient_fields, lineterminator="\n")
            writer.writeheader()
            writer.writerows(patients)
        subsample_rows = [
            {
                "patient_id": row["patient_id"],
                "label": row["label"],
                "cell_count": sum(patient_subsample_counts[row["patient_id"]].values()),
                **fractions(patient_subsample_counts[row["patient_id"]]),
            }
            for row in patients
        ]
        with (staging / "patient_subsample_80_fingerprints.csv").open(
            "w", newline="", encoding="utf-8"
        ) as target:
            writer = csv.DictWriter(
                target, fieldnames=list(subsample_rows[0]), lineterminator="\n"
            )
            writer.writeheader()
            writer.writerows(subsample_rows)
        core_rows = []
        included = {row["patient_id"] for row in patients}
        for (patient, core), counts in sorted(
            core_counts.items(), key=lambda item: (item[0][0], item[0][1])
        ):
            if patient in included:
                core_rows.append(
                    {
                        "patient_id": patient,
                        "core_id": core,
                        "label": labels[patient],
                        "cell_count": sum(counts.values()),
                        **fractions(counts),
                    }
                )
        with (staging / "core_fingerprints.csv").open(
            "w", newline="", encoding="utf-8"
        ) as target:
            writer = csv.DictWriter(
                target, fieldnames=list(core_rows[0]), lineterminator="\n"
            )
            writer.writeheader()
            writer.writerows(core_rows)
        folds = []
        for query in patients:
            training = [row for row in patients if row["patient_id"] != query["patient_id"]]
            fold = staging / "folds" / "codex_neighborhood" / "patient_held_out" / query["patient_id"]
            write_fingerprint(fold / "training.csv", training, query=False, provenance=provenance)
            write_fingerprint(fold / "query.csv", [query], query=True, provenance=provenance)
            folds.append(
                {
                    "lane": "codex_neighborhood",
                    "holdout_policy": "patient_held_out",
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
                    "schema_name": "marklab_schurch_codex_neighborhood_fingerprint_admission",
                    "schema_version": SCHEMA_VERSION,
                    "population_unit": "patient",
                    "patient_count": len(patients),
                    "core_count": len(core_rows),
                    "source_cell_count": source_rows,
                    "provenance_retained_cell_count": provenance_retained_rows,
                    "admitted_neighborhood_cell_count": admitted_rows,
                    "provenance_excluded_cell_count": source_rows - provenance_retained_rows,
                    "unassigned_neighborhood_cell_count": provenance_retained_rows - admitted_rows,
                    "cell_subsample_fraction": CELL_SUBSAMPLE_FRACTION,
                    "cell_subsample_count": sum(row["cell_count"] for row in subsample_rows),
                    "group_counts": {
                        label: sum(row["label"] == label for row in patients)
                        for label in ("MSI", "MSS")
                    },
                    "excluded_patients": exclusions,
                    "feature_names": FEATURES,
                    "neighborhood_vocabulary": {
                        number: {"source": source, "harmonized": feature}
                        for number, (source, feature) in NEIGHBORHOODS.items()
                    },
                    "raw_protein_feature_space_used": False,
                    "raw_feature_space_shared_with_cellvit_claimed": False,
                    "coordinate_scale_um_per_pixel": 0.37744,
                    "license": "CC BY 4.0",
                    "normalization": "inside_patient_held_out_training_fold_by_marklab_region_retrieval",
                    "fold_count": len(folds),
                    "provenance_sha256": provenance,
                    "source_sha256": source_sha256,
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
    parser.add_argument("--cells", required=True, type=Path)
    parser.add_argument("--provenance", required=True, type=Path)
    parser.add_argument("--molecular-labels", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    return parser.parse_args()


if __name__ == "__main__":
    build(parse_args())
