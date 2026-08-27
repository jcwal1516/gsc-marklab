#!/usr/bin/env python3
"""Summarize durable Schuerch CODEX patient-neighborhood retrieval."""

from __future__ import annotations

import argparse
import csv
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import random
import shutil
import statistics

import numpy as np


SCHEMA_VERSION = "1.0"
BOOTSTRAP_REPLICATES = 2_000
STABILITY_BOOTSTRAP_REPLICATES = 500
BOOTSTRAP_SEED = 20_260_827


def support_module():
    path = Path(__file__).with_name("marklab_tcga_crc_m3_stability_summary.py")
    spec = importlib.util.spec_from_file_location("marklab_rank_support", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_csv(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as source:
        rows = list(csv.DictReader(source))
    if not rows:
        raise ValueError(f"CSV has no rows: {path}")
    return rows


def mean(values: list[float]) -> float:
    if not values or any(not math.isfinite(value) for value in values):
        raise ValueError("summary requires nonempty finite values")
    return math.fsum(values) / len(values)


def majority_label(matches: list[tuple[str, str, float]], maximum: int) -> str:
    counts: dict[str, int] = {}
    for _, label, _ in matches[:maximum]:
        counts[label] = counts.get(label, 0) + 1
    if not counts:
        raise ValueError("retrieval majority requires at least one match")
    return sorted(counts, key=lambda label: (-counts[label], label))[0]


def bootstrap_mean(values: list[float], seed: int) -> dict[str, float]:
    generator = random.Random(seed)
    estimates = [
        mean([values[generator.randrange(len(values))] for _ in values])
        for _ in range(BOOTSTRAP_REPLICATES)
    ]
    return {
        "lower": float(np.quantile(estimates, 0.025)),
        "upper": float(np.quantile(estimates, 0.975)),
    }


def build(args: argparse.Namespace) -> None:
    support = support_module()
    admission_root = args.admission.resolve()
    result_root = args.results.resolve()
    admission = json.loads((admission_root / "admission.json").read_text(encoding="utf-8"))
    features = admission["feature_names"]
    patients = read_csv(admission_root / "patient_fingerprints.csv")
    subsample = {
        row["patient_id"]: row
        for row in read_csv(admission_root / "patient_subsample_80_fingerprints.csv")
    }
    cores: dict[str, list[dict[str, str]]] = {}
    for row in read_csv(admission_root / "core_fingerprints.csv"):
        cores.setdefault(row["patient_id"], []).append(row)
    folds = read_csv(admission_root / "folds.csv")
    labels = {row["patient_id"]: row["label"] for row in patients}
    if len(labels) != admission["patient_count"] or len(folds) != admission["fold_count"]:
        raise ValueError("Schuerch CODEX admission dimensions differ")
    retrieval_rows = []
    distance_rows = []
    prediction_rows = []
    sources = {
        str(admission_root / "admission.json"): sha256(admission_root / "admission.json"),
        str(admission_root / "patient_fingerprints.csv"): sha256(
            admission_root / "patient_fingerprints.csv"
        ),
        str(admission_root / "patient_subsample_80_fingerprints.csv"): sha256(
            admission_root / "patient_subsample_80_fingerprints.csv"
        ),
        str(admission_root / "core_fingerprints.csv"): sha256(
            admission_root / "core_fingerprints.csv"
        ),
    }
    for fold in folds:
        patient = fold["patient_id"]
        result_path = result_root / f"{patient}.json"
        document = json.loads(result_path.read_text(encoding="utf-8"))
        if (
            document.get("format") != "marklab.region_retrieval"
            or document.get("training_sha256") != fold["training_sha256"]
            or document.get("query_sha256") != fold["query_sha256"]
            or document.get("query_region_id") != patient
            or document.get("leakage_policy") != "exclude_same_patient"
            or int(document.get("k")) != int(fold["candidate_count"])
            or document["index"]["feature_names"] != features
        ):
            raise ValueError(f"Schuerch CODEX durable result identity differs: {patient}")
        matches = []
        for rank, matched in enumerate(document["matches"], 1):
            candidate = matched["patient_id"]
            distance = float(matched["distance"])
            if matched["rank"] != rank or candidate not in labels or candidate == patient:
                raise ValueError(f"Schuerch CODEX durable match differs: {patient}")
            matches.append((candidate, labels[candidate], distance))
            distance_rows.append(
                {
                    "query_patient_id": patient,
                    "candidate_patient_id": candidate,
                    "query_label": labels[patient],
                    "candidate_label": labels[candidate],
                    "rank": rank,
                    "distance": distance,
                    "same_class": labels[patient] == labels[candidate],
                }
            )
        within = [distance for _, label, distance in matches if label == labels[patient]]
        between = [distance for _, label, distance in matches if label != labels[patient]]
        if not within or not between:
            raise ValueError(f"Schuerch CODEX retrieval lacks both molecular classes: {patient}")
        top1 = float(matches[0][1] == labels[patient])
        top5_label = majority_label(matches, 5)
        retrieval_rows.append(
            {
                "patient_id": patient,
                "label": labels[patient],
                "top1": top1,
                "top5": float(top5_label == labels[patient]),
                "effect": mean(between) - mean(within),
                "first_same_rank": next(
                    rank for rank, (_, label, _) in enumerate(matches, 1) if label == labels[patient]
                ),
            }
        )
        prediction_rows.append(
            {
                "patient_id": patient,
                "label": labels[patient],
                "top1_prediction": matches[0][1],
                "top1_correct": int(top1),
                "top5_majority_prediction": top5_label,
                "top5_correct": int(top5_label == labels[patient]),
                "between_minus_within_distance": mean(between) - mean(within),
            }
        )
        sources[str(result_path)] = sha256(result_path)
    recalls = {
        label: mean([row["top1"] for row in retrieval_rows if row["label"] == label])
        for label in ("MSI", "MSS")
    }
    stability_rows = []
    stability_summary = {}
    patient_by_id = {row["patient_id"]: row for row in patients}
    for feature_index, feature in enumerate(features):
        full_values = [float(patient_by_id[patient][feature]) for patient in sorted(labels)]
        subsample_values = [float(subsample[patient][feature]) for patient in sorted(labels)]
        core_medians = [
            statistics.median(float(row[feature]) for row in cores[patient])
            for patient in sorted(labels)
        ]
        core_bootstrap_deviations = []
        for patient in sorted(labels):
            values = [float(row[feature]) for row in cores[patient]]
            center = statistics.median(values)
            generator = random.Random(
                BOOTSTRAP_SEED
                + feature_index * 10_000
                + int(hashlib.sha256(patient.encode()).hexdigest()[:8], 16)
            )
            deviations = []
            for _ in range(STABILITY_BOOTSTRAP_REPLICATES):
                sample = [values[generator.randrange(len(values))] for _ in values]
                deviations.append(abs(statistics.median(sample) - center))
            core_bootstrap_deviations.append(float(np.quantile(deviations, 0.95)))
        cell_correlation = support.spearman(full_values, subsample_values)
        core_correlation = support.spearman(full_values, core_medians)
        stability_rows.append(
            {
                "feature": feature,
                "cell_subsample_patient_rank_spearman": cell_correlation,
                "core_median_patient_rank_spearman": core_correlation,
                "median_core_bootstrap_absolute_deviation_95": statistics.median(
                    core_bootstrap_deviations
                ),
            }
        )
    stability_summary = {
        "feature_count": len(features),
        "median_cell_subsample_patient_rank_spearman": statistics.median(
            row["cell_subsample_patient_rank_spearman"] for row in stability_rows
        ),
        "minimum_cell_subsample_patient_rank_spearman": min(
            row["cell_subsample_patient_rank_spearman"] for row in stability_rows
        ),
        "median_core_median_patient_rank_spearman": statistics.median(
            row["core_median_patient_rank_spearman"] for row in stability_rows
        ),
        "minimum_core_median_patient_rank_spearman": min(
            row["core_median_patient_rank_spearman"] for row in stability_rows
        ),
    }
    out = args.out.resolve()
    if out.exists() or out.is_symlink():
        raise ValueError(f"output already exists: {out}")
    out.parent.mkdir(parents=True, exist_ok=True)
    staging = out.with_name(f".{out.name}.{os.getpid()}.tmp")
    staging.mkdir()
    try:
        for filename, fields, rows in (
            (
                "distance_matrix.csv",
                [
                    "query_patient_id",
                    "candidate_patient_id",
                    "query_label",
                    "candidate_label",
                    "rank",
                    "distance",
                    "same_class",
                ],
                distance_rows,
            ),
            ("heldout_predictions.csv", list(prediction_rows[0]), prediction_rows),
            ("feature_stability.csv", list(stability_rows[0]), stability_rows),
        ):
            with (staging / filename).open("w", newline="", encoding="utf-8") as target:
                writer = csv.DictWriter(target, fieldnames=fields, lineterminator="\n")
                writer.writeheader()
                writer.writerows(rows)
        cohort_rows = [
            {
                "patient_id": row["patient_id"],
                "group": row["label"],
                "feature": feature,
                "value": row[feature],
            }
            for row in patients
            for feature in features
        ]
        with (staging / "cohort_input.csv").open("w", newline="", encoding="utf-8") as target:
            writer = csv.DictWriter(
                target,
                fieldnames=["patient_id", "group", "feature", "value"],
                lineterminator="\n",
            )
            writer.writeheader()
            writer.writerows(cohort_rows)
        summary = {
            "schema_name": "marklab_schurch_codex_retrieval_summary",
            "schema_version": SCHEMA_VERSION,
            "population_unit": "patient",
            "patient_count": len(retrieval_rows),
            "top1_accuracy": mean([row["top1"] for row in retrieval_rows]),
            "top1_accuracy_bootstrap_95": bootstrap_mean(
                [row["top1"] for row in retrieval_rows], BOOTSTRAP_SEED
            ),
            "top1_recall_by_group": recalls,
            "balanced_top1_accuracy": mean(list(recalls.values())),
            "top5_majority_accuracy": mean([row["top5"] for row in retrieval_rows]),
            "mean_between_minus_within_distance": mean(
                [row["effect"] for row in retrieval_rows]
            ),
            "mean_between_minus_within_distance_bootstrap_95": bootstrap_mean(
                [row["effect"] for row in retrieval_rows], BOOTSTRAP_SEED + 1
            ),
            "mean_first_same_class_rank": mean(
                [float(row["first_same_rank"]) for row in retrieval_rows]
            ),
            "stability": stability_summary,
            "raw_protein_feature_space_used": False,
            "raw_feature_space_shared_with_cellvit_claimed": False,
            "validation_role": "orthogonal_biological_neighborhood_validation",
            "claim_status": "exploratory_small_msi_group_external_validation",
            "source_sha256": dict(sorted(sources.items())),
        }
        (staging / "summary.json").write_text(
            json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        files = sorted(path for path in staging.iterdir() if path.is_file())
        (staging / "manifest.json").write_text(
            json.dumps(
                {
                    "schema_name": "marklab_schurch_codex_retrieval_manifest",
                    "schema_version": SCHEMA_VERSION,
                    "output_sha256": {path.name: sha256(path) for path in files},
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
    parser.add_argument("--admission", required=True, type=Path)
    parser.add_argument("--results", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    return parser.parse_args()


if __name__ == "__main__":
    build(parse_args())
