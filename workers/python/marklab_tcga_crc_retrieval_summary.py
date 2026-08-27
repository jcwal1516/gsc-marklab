#!/usr/bin/env python3
"""Summarize two durable TCGA CRC held-out fingerprint retrieval lanes."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import os
from pathlib import Path
import random
import tempfile
from typing import Any, Iterable


SCHEMA_VERSION = "1.0"
BOOTSTRAP_REPLICATES = 2_000
BOOTSTRAP_SEED = 20_260_827
COMMON_COHORT_FEATURE_MAP = {
    "embedding_raw_*": "raw_summary_unchanged",
    "embedding_stage_ordinal": "stage_ordinal_divided_by_4",
    "embedding_log1p_all_cell_count": "log1p_all_cell_count_divided_by_10",
    "embedding_tumor_fraction": "fraction_unchanged",
    "embedding_inflammatory_fraction": "fraction_unchanged",
    "embedding_connective_fraction": "fraction_unchanged",
    "embedding_cell_density_per_mm2": "log1p_cells_per_mm2_divided_by_10",
    "embedding_median_inflammatory_count_50um": "log1p_count_divided_by_5",
    "embedding_median_stromal_distance_um": "distance_divided_by_100_micrometres",
    "embedding_tumor_inflammatory_relative_excess_0_50": "dimensionless_unchanged",
    "embedding_tumor_connective_relative_excess_0_50": "dimensionless_unchanged",
    "embedding_stromal_context_residual_organization": "dimensionless_unchanged",
    "embedding_inflammatory_context_residual_organization": "dimensionless_unchanged",
    "embedding_combined_context_residual_organization": "dimensionless_unchanged",
    "embedding_coordinate_l_relative_25um": "dimensionless_unchanged",
    "embedding_coordinate_l_relative_50um": "dimensionless_unchanged",
    "embedding_coordinate_l_relative_75um": "dimensionless_unchanged",
    "embedding_coordinate_l_relative_100um": "dimensionless_unchanged",
    "embedding_coordinate_graph_2x2_low_energy_fraction": "fraction_unchanged",
    "embedding_coordinate_graph_2x2_middle_energy_fraction": "fraction_unchanged",
    "embedding_coordinate_graph_2x2_high_energy_fraction": "fraction_unchanged",
    "embedding_coordinate_graph_3x3_low_energy_fraction": "fraction_unchanged",
    "embedding_coordinate_graph_3x3_middle_energy_fraction": "fraction_unchanged",
    "embedding_coordinate_graph_3x3_high_energy_fraction": "fraction_unchanged",
    "embedding_raw_variogram_0_25um": "raw_squared_distance_unchanged",
    "embedding_raw_variogram_25_50um": "raw_squared_distance_unchanged",
    "embedding_raw_variogram_50_100um": "raw_squared_distance_unchanged",
}


class SummaryError(ValueError):
    """A durable result or admitted identity differs from the fixed contract."""


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while True:
            block = stream.read(1024 * 1024)
            if not block:
                break
            digest.update(block)
    return digest.hexdigest()


def read_csv(path: Path) -> list[dict[str, str]]:
    with path.open(encoding="utf-8", newline="") as stream:
        rows = list(csv.DictReader(stream))
    if not rows:
        raise SummaryError(f"{path} contains no rows")
    return rows


def write_csv(path: Path, fields: list[str], rows: Iterable[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=fields, lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def write_json(path: Path, value: Any) -> None:
    encoded = json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    path.write_text(encoded + "\n", encoding="utf-8")


def mean(values: list[float]) -> float:
    if not values or any(not math.isfinite(value) for value in values):
        raise SummaryError("summary requires nonempty finite patient values")
    return math.fsum(values) / len(values)


def quantile(values: list[float], probability: float) -> float:
    ordered = sorted(values)
    position = probability * (len(ordered) - 1)
    lower = int(math.floor(position))
    upper = int(math.ceil(position))
    if lower == upper:
        return ordered[lower]
    fraction = position - lower
    return ordered[lower] + fraction * (ordered[upper] - ordered[lower])


def bootstrap_mean(values: list[float], replicates: int, seed: int) -> dict[str, float]:
    generator = random.Random(seed)
    estimates = [
        mean([values[generator.randrange(len(values))] for _ in values])
        for _ in range(replicates)
    ]
    return {"lower": quantile(estimates, 0.025), "upper": quantile(estimates, 0.975)}


def common_cohort_value(name: str, raw_value: float) -> float:
    raw_embedding_summary = name.startswith("embedding_raw_")
    if (
        name not in COMMON_COHORT_FEATURE_MAP
        and not raw_embedding_summary
    ) or not math.isfinite(raw_value):
        raise SummaryError("common cohort feature or value is invalid")
    if name == "embedding_stage_ordinal":
        value = raw_value / 4.0
    elif name == "embedding_log1p_all_cell_count":
        value = raw_value / 10.0
    elif name == "embedding_cell_density_per_mm2":
        if raw_value < 0.0:
            raise SummaryError("cell density is negative")
        value = math.log1p(raw_value) / 10.0
    elif name == "embedding_median_inflammatory_count_50um":
        if raw_value < 0.0:
            raise SummaryError("local inflammatory count is negative")
        value = math.log1p(raw_value) / 5.0
    elif name == "embedding_median_stromal_distance_um":
        if raw_value < 0.0:
            raise SummaryError("stromal distance is negative")
        value = raw_value / 100.0
    else:
        value = raw_value
    if not math.isfinite(value):
        raise SummaryError("common cohort feature is non-finite")
    return value


def summarize_block(
    rows: list[dict[str, Any]], bootstrap_replicates: int, seed: int
) -> dict[str, Any]:
    if len(rows) < 4 or bootstrap_replicates < 1:
        raise SummaryError("retrieval summary dimensions are invalid")
    effects: list[float] = []
    top1: list[float] = []
    top5: list[float] = []
    first_same_ranks: list[float] = []
    by_group: dict[str, list[float]] = {}
    for row in rows:
        matches = row["matches"]
        if not matches:
            raise SummaryError("retrieval row has no matches")
        within = [distance for _, label, distance in matches if label == row["label"]]
        between = [distance for _, label, distance in matches if label != row["label"]]
        if not within or not between:
            raise SummaryError("retrieval row lacks within- or between-class support")
        effects.append(mean(between) - mean(within))
        top1_correct = float(matches[0][1] == row["label"])
        top1.append(top1_correct)
        by_group.setdefault(row["label"], []).append(top1_correct)
        nearest = matches[: min(5, len(matches))]
        counts: dict[str, int] = {}
        for _, label, _ in nearest:
            counts[label] = counts.get(label, 0) + 1
        predicted = sorted(counts, key=lambda label: (-counts[label], label))[0]
        top5.append(float(predicted == row["label"]))
        first_same_ranks.append(
            float(next(index for index, (_, label, _) in enumerate(matches, 1) if label == row["label"]))
        )
    group_recalls = {group: mean(values) for group, values in sorted(by_group.items())}
    return {
        "patient_count": len(rows),
        "top1_accuracy": mean(top1),
        "top1_accuracy_bootstrap_95": bootstrap_mean(
            top1, bootstrap_replicates, seed + 1
        ),
        "top5_majority_accuracy": mean(top5),
        "top5_majority_accuracy_bootstrap_95": bootstrap_mean(
            top5, bootstrap_replicates, seed + 2
        ),
        "top1_recall_by_group": group_recalls,
        "balanced_top1_accuracy": mean(list(group_recalls.values())),
        "mean_first_same_class_rank": mean(first_same_ranks),
        "mean_between_minus_within_distance": mean(effects),
        "mean_between_minus_within_distance_bootstrap_95": bootstrap_mean(
            effects, bootstrap_replicates, seed + 3
        ),
    }


def paired_increment(
    baseline: dict[str, dict[str, float]],
    comparison: dict[str, dict[str, float]],
    bootstrap_replicates: int,
    seed: int,
    baseline_name: str,
    comparison_name: str,
) -> dict[str, Any]:
    if set(baseline) != set(comparison):
        raise SummaryError("baseline/comparison patient identities differ")
    patients = sorted(baseline)
    top1 = [
        comparison[patient]["top1_correct"] - baseline[patient]["top1_correct"]
        for patient in patients
    ]
    top5 = [
        comparison[patient]["top5_correct"] - baseline[patient]["top5_correct"]
        for patient in patients
    ]
    effect = [
        comparison[patient]["between_minus_within"] - baseline[patient]["between_minus_within"]
        for patient in patients
    ]
    direction = f"{comparison_name}_minus_{baseline_name}"
    return {
        "patient_count": len(patients),
        f"top1_accuracy_difference_{direction}": mean(top1),
        "top1_accuracy_difference_bootstrap_95": bootstrap_mean(
            top1, bootstrap_replicates, seed + 1
        ),
        f"top5_accuracy_difference_{direction}": mean(top5),
        "top5_accuracy_difference_bootstrap_95": bootstrap_mean(
            top5, bootstrap_replicates, seed + 2
        ),
        f"between_minus_within_effect_difference_{direction}": mean(effect),
        "between_minus_within_effect_difference_bootstrap_95": bootstrap_mean(
            effect, bootstrap_replicates, seed + 3
        ),
    }


def build(args: argparse.Namespace) -> None:
    admission_root = Path(args.admission)
    result_root = Path(args.results)
    output = Path(args.out)
    baseline_lane = args.baseline_lane
    comparison_lane = args.comparison_lane
    if not baseline_lane or not comparison_lane or baseline_lane == comparison_lane:
        raise SummaryError("baseline and comparison lanes must be distinct nonempty names")
    if output.exists() or output.is_symlink():
        raise SummaryError(f"output already exists: {output}")
    admission = json.loads((admission_root / "admission.json").read_text(encoding="utf-8"))
    patients = read_csv(admission_root / "admitted_patients.csv")
    labels = {row["patient_id"]: row["label"] for row in patients}
    sites = {row["patient_id"]: row["site_id"] for row in patients}
    folds = read_csv(admission_root / "folds.csv")
    if len(labels) != admission["patient_count"] or len(folds) != admission["fold_count"]:
        raise SummaryError("admission dimensions differ")
    output.parent.mkdir(parents=True, exist_ok=True)
    staging = Path(tempfile.mkdtemp(prefix=f".{output.name}.part-", dir=output.parent))
    blocks: dict[tuple[str, str], list[dict[str, Any]]] = {}
    patient_metrics: dict[tuple[str, str], dict[str, dict[str, float]]] = {}
    distance_rows: dict[tuple[str, str], list[dict[str, Any]]] = {}
    fingerprint_rows: dict[tuple[str, str], list[dict[str, Any]]] = {}
    prediction_rows: list[dict[str, Any]] = []
    input_result_sha256: dict[str, str] = {}
    for fold in folds:
        lane = fold["lane"]
        holdout = fold["holdout_policy"]
        patient = fold["patient_id"]
        key = (lane, holdout)
        training_path = admission_root / fold["training_path"]
        query_path = admission_root / fold["query_path"]
        if sha256(training_path) != fold["training_sha256"] or sha256(query_path) != fold["query_sha256"]:
            raise SummaryError(f"admitted fold digest differs for {patient}")
        result_path = result_root / lane / holdout / f"{patient}.json"
        result_digest = sha256(result_path)
        input_result_sha256[result_path.relative_to(result_root).as_posix()] = result_digest
        result = json.loads(result_path.read_text(encoding="utf-8"))
        expected_policy = (
            "exclude_same_patient_and_site"
            if holdout == "site_held_out"
            else "exclude_same_patient"
        )
        candidate_count = int(fold["candidate_count"])
        if (
            result["format"] != "marklab.region_retrieval"
            or result["training_sha256"] != fold["training_sha256"]
            or result["query_sha256"] != fold["query_sha256"]
            or result["query_region_id"] != patient
            or result["leakage_policy"] != expected_policy
            or result["k"] != candidate_count
            or result["eligible_candidate_count"] != candidate_count
            or len(result["matches"]) != candidate_count
        ):
            raise SummaryError(f"durable result identity differs for {patient}")
        matches: list[tuple[str, str, float]] = []
        for rank, matched in enumerate(result["matches"], 1):
            candidate = matched["patient_id"]
            distance = float(matched["distance"])
            if (
                matched["rank"] != rank
                or candidate not in labels
                or candidate == patient
                or (holdout == "site_held_out" and sites[candidate] == sites[patient])
                or not math.isfinite(distance)
                or distance < 0.0
            ):
                raise SummaryError(f"durable match differs for {patient} rank {rank}")
            matches.append((candidate, labels[candidate], distance))
            distance_rows.setdefault(key, []).append(
                {
                    "query_patient_id": patient,
                    "candidate_patient_id": candidate,
                    "query_label": labels[patient],
                    "candidate_label": labels[candidate],
                    "query_site_id": sites[patient],
                    "candidate_site_id": sites[candidate],
                    "rank": rank,
                    "distance": distance,
                    "same_class": labels[patient] == labels[candidate],
                    "same_site": sites[patient] == sites[candidate],
                }
            )
        row = {"patient_id": patient, "label": labels[patient], "matches": matches}
        blocks.setdefault(key, []).append(row)
        within = [distance for _, label, distance in matches if label == labels[patient]]
        between = [distance for _, label, distance in matches if label != labels[patient]]
        nearest = matches[:5]
        counts: dict[str, int] = {}
        for _, label, _ in nearest:
            counts[label] = counts.get(label, 0) + 1
        predicted_top5 = sorted(counts, key=lambda label: (-counts[label], label))[0]
        details = {
            "top1_correct": float(matches[0][1] == labels[patient]),
            "top5_correct": float(predicted_top5 == labels[patient]),
            "between_minus_within": mean(between) - mean(within),
        }
        patient_metrics.setdefault(key, {})[patient] = details
        prediction_rows.append(
            {
                "lane": lane,
                "holdout_policy": holdout,
                "patient_id": patient,
                "label": labels[patient],
                "site_id": sites[patient],
                "candidate_count": candidate_count,
                "top1_prediction": matches[0][1],
                "top1_correct": int(details["top1_correct"]),
                "top5_majority_prediction": predicted_top5,
                "top5_correct": int(details["top5_correct"]),
                "first_same_class_rank": next(
                    rank for rank, (_, label, _) in enumerate(matches, 1) if label == labels[patient]
                ),
                "between_minus_within_distance": details["between_minus_within"],
                "ood_score": result["ood_score"],
            }
        )
        query_rows = read_csv(query_path)
        if len(query_rows) != 1:
            raise SummaryError(f"query input differs for {patient}")
        query = query_rows[0]
        names = result["index"]["feature_names"]
        centers = result["index"]["training_mean"]
        scales = result["index"]["training_population_sd"]
        if len(names) != len(centers) or len(names) != len(scales):
            raise SummaryError(f"training transform differs for {patient}")
        for name in names:
            value = common_cohort_value(name, float(query[name]))
            fingerprint_rows.setdefault(key, []).append(
                {"patient_id": patient, "group": labels[patient], "feature": name, "value": value}
            )
    summary_blocks: dict[str, Any] = {}
    for index, key in enumerate(sorted(blocks)):
        name = f"{key[0]}_{key[1]}"
        summary_blocks[name] = summarize_block(
            blocks[key], BOOTSTRAP_REPLICATES, BOOTSTRAP_SEED + index * 100
        )
        write_csv(
            staging / "distance_matrices" / f"{name}.csv",
            [
                "query_patient_id",
                "candidate_patient_id",
                "query_label",
                "candidate_label",
                "query_site_id",
                "candidate_site_id",
                "rank",
                "distance",
                "same_class",
                "same_site",
            ],
            distance_rows[key],
        )
        write_csv(
            staging / "cohort_inputs" / f"{name}.csv",
            ["patient_id", "group", "feature", "value"],
            fingerprint_rows[key],
        )
    increments = {
        holdout: paired_increment(
            patient_metrics[(baseline_lane, holdout)],
            patient_metrics[(comparison_lane, holdout)],
            BOOTSTRAP_REPLICATES,
            BOOTSTRAP_SEED + 1_000 + index * 100,
            baseline_lane,
            comparison_lane,
        )
        for index, holdout in enumerate(["patient_held_out", "site_held_out"])
    }
    write_csv(
        staging / "heldout_predictions.csv",
        [
            "lane",
            "holdout_policy",
            "patient_id",
            "label",
            "site_id",
            "candidate_count",
            "top1_prediction",
            "top1_correct",
            "top5_majority_prediction",
            "top5_correct",
            "first_same_class_rank",
            "between_minus_within_distance",
            "ood_score",
        ],
        prediction_rows,
    )
    write_json(
        staging / "summary.json",
        {
            "schema_name": "marklab_crc_fingerprint_retrieval_summary",
            "schema_version": SCHEMA_VERSION,
            "patient_count": len(labels),
            "population_unit": "patient",
            "bootstrap_replicates": BOOTSTRAP_REPLICATES,
            "bootstrap_seed": BOOTSTRAP_SEED,
            "blocks": summary_blocks,
            f"{comparison_lane}_minus_{baseline_lane}": increments,
            "cohort_test_feature_map": COMMON_COHORT_FEATURE_MAP,
            "cohort_test_normalization": "fixed_label_free_unit_map_shared_by_every_patient",
            "interpretation_policy": "report_positive_null_negative_and_site_unstable_results_without_significance_optimization",
            "claim_status": "exploratory_patient_held_out_retrieval",
        },
    )
    output_files = sorted(
        path for path in staging.rglob("*") if path.is_file() and path.name != "summary_manifest.json"
    )
    write_json(
        staging / "summary_manifest.json",
        {
            "schema_name": "marklab_crc_fingerprint_retrieval_summary_manifest",
            "schema_version": SCHEMA_VERSION,
            "admission_sha256": sha256(admission_root / "admission.json"),
            "input_result_sha256": input_result_sha256,
            "output_sha256": {
                path.relative_to(staging).as_posix(): sha256(path) for path in output_files
            },
        },
    )
    os.replace(staging, output)


def parser() -> argparse.ArgumentParser:
    value = argparse.ArgumentParser()
    value.add_argument("--admission", required=True)
    value.add_argument("--results", required=True)
    value.add_argument("--out", required=True)
    value.add_argument("--baseline-lane", default="m0")
    value.add_argument("--comparison-lane", default="m1")
    return value


if __name__ == "__main__":
    try:
        build(parser().parse_args())
    except Exception as error:
        print(f"TCGA CRC retrieval summary failed: {type(error).__name__}: {error}", file=os.sys.stderr)
        raise SystemExit(2) from error
