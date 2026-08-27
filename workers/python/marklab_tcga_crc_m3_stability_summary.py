#!/usr/bin/env python3
"""Summarize bounded patient-level stability of raw CellViT embedding summaries."""

from __future__ import annotations

import argparse
import csv
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import shutil
import statistics
from typing import Iterable

import numpy as np


SCHEMA_VERSION = "1.0"
CELL_SUBSAMPLE_FRACTION = 0.8
FUSION_THRESHOLDS = {
    "full_source_vs_four_field_median": 0.70,
    "full_source_vs_four_field_q10": 0.50,
    "four_field_vs_cell_subsample_median": 0.90,
    "four_field_vs_cell_subsample_q10": 0.75,
    "four_field_vs_leave_one_field_median": 0.80,
    "four_field_vs_leave_one_field_q10": 0.60,
}


def support_module():
    path = Path(__file__).with_name("marklab_tcga_crc_raw_embedding_summary.py")
    spec = importlib.util.spec_from_file_location("marklab_raw_summary_support", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def rankdata(values: list[float]) -> list[float]:
    if not values or any(not math.isfinite(value) for value in values):
        raise ValueError("ranks require nonempty finite values")
    ordered = sorted(range(len(values)), key=lambda index: (values[index], index))
    ranks = [0.0] * len(values)
    start = 0
    while start < len(ordered):
        end = start + 1
        while end < len(ordered) and values[ordered[end]] == values[ordered[start]]:
            end += 1
        midrank = (start + 1 + end) / 2.0
        for position in range(start, end):
            ranks[ordered[position]] = midrank
        start = end
    return ranks


def spearman(left: list[float], right: list[float]) -> float:
    if len(left) != len(right) or len(left) < 3:
        raise ValueError("Spearman correlation requires equal vectors with at least three values")
    left_rank = rankdata(left)
    right_rank = rankdata(right)
    left_mean = statistics.fmean(left_rank)
    right_mean = statistics.fmean(right_rank)
    numerator = math.fsum(
        (left_value - left_mean) * (right_value - right_mean)
        for left_value, right_value in zip(left_rank, right_rank)
    )
    left_ss = math.fsum((value - left_mean) ** 2 for value in left_rank)
    right_ss = math.fsum((value - right_mean) ** 2 for value in right_rank)
    denominator = math.sqrt(left_ss * right_ss)
    if denominator == 0.0:
        raise ValueError("Spearman correlation is undefined for a constant vector")
    return numerator / denominator


def field_id(object_id: str) -> str:
    field, separator, remainder = object_id.partition(":")
    if not separator or not field or not remainder:
        raise ValueError("bounded raw object identity lacks its field")
    return field


def cell_subsample_indices(object_ids: list[str], fraction: float) -> list[int]:
    if not 0.0 < fraction < 1.0 or len(set(object_ids)) != len(object_ids):
        raise ValueError("cell subsampling requires unique identities and a proper fraction")
    grouped: dict[str, list[int]] = {}
    for index, object_id in enumerate(object_ids):
        grouped.setdefault(field_id(object_id), []).append(index)
    selected = []
    for field in sorted(grouped):
        indices = grouped[field]
        retain = max(1, int(math.floor(len(indices) * fraction)))
        if len(indices) > 1 and retain >= len(indices):
            raise ValueError("each field must lose at least one cell in the stability subsample")
        selected.extend(
            sorted(
                indices,
                key=lambda index: (
                    hashlib.sha256(
                        ("m3-cell-stability-v1:" + object_ids[index]).encode()
                    ).digest(),
                    object_ids[index],
                ),
            )[:retain]
        )
    return sorted(selected)


def quantile(values: list[float], probability: float) -> float:
    return float(np.quantile(np.asarray(values, dtype=np.float64), probability))


def write_csv(path: Path, fields: list[str], rows: Iterable[dict]) -> None:
    with path.open("w", newline="", encoding="utf-8") as target:
        writer = csv.DictWriter(target, fieldnames=fields, lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def build(args: argparse.Namespace) -> None:
    support = support_module()
    m3_root = args.m3_admission.resolve()
    bounded_root = args.bounded_raw_input.resolve()
    admission = json.loads((m3_root / "admission.json").read_text(encoding="utf-8"))
    bounded = json.loads((bounded_root / "admission.json").read_text(encoding="utf-8"))
    if (
        admission["patient_count"] != bounded["patient_count"]
        or bounded.get("cross_field_pairs_within_analyzed_distance") is not False
    ):
        raise ValueError("M3 and bounded raw admissions differ or field frames are not isolated")
    _, patients = support.read_csv(bounded_root / "patients.csv")
    raw_feature_names = admission["raw_summary_feature_names"]
    comparisons = {
        "full_source_vs_four_field": ([], []),
        "four_field_vs_cell_subsample": ([], []),
        "four_field_vs_leave_one_field": ([], []),
    }
    sources = {
        str((m3_root / "admission.json")): support.sha256(m3_root / "admission.json"),
        str((bounded_root / "admission.json")): support.sha256(
            bounded_root / "admission.json"
        ),
        str((bounded_root / "patients.csv")): support.sha256(
            bounded_root / "patients.csv"
        ),
    }
    patient_rows = []
    unavailable_leave_one_field = []
    for patient in patients:
        patient_id = patient["patient_id"]
        path = bounded_root / patient["input"]
        if support.sha256(path) != patient["input_sha256"]:
            raise ValueError(f"bounded raw digest differs for {patient_id}")
        headers, rows = support.read_csv(path)
        embedding_names = headers[3:]
        if embedding_names != [f"embedding_raw_{index:04d}" for index in range(1280)]:
            raise ValueError(f"raw vector schema differs for {patient_id}")
        object_ids = [row["object_id"] for row in rows]
        matrix = np.asarray(
            [[float(row[name]) for name in embedding_names] for row in rows],
            dtype=np.float64,
        )
        bounded_names, four_field = support.summarize_raw_embeddings(matrix)
        if bounded_names != raw_feature_names:
            raise ValueError("raw summary feature identity changed")
        subsample = cell_subsample_indices(object_ids, CELL_SUBSAMPLE_FRACTION)
        _, cell_subsample = support.summarize_raw_embeddings(matrix[subsample])
        fields = sorted({field_id(object_id) for object_id in object_ids})
        leave_one_out = []
        if len(fields) >= 2:
            for omitted in fields:
                indices = [
                    index
                    for index, object_id in enumerate(object_ids)
                    if field_id(object_id) != omitted
                ]
                if len(indices) >= 2:
                    _, summary = support.summarize_raw_embeddings(matrix[indices])
                    leave_one_out.append(summary)
        leave_one_field = (
            [
                statistics.median(summary[index] for summary in leave_one_out)
                for index in range(len(raw_feature_names))
            ]
            if leave_one_out
            else None
        )
        if leave_one_field is None:
            unavailable_leave_one_field.append(
                {
                    "patient_id": patient_id,
                    "field_count": len(fields),
                    "blocker": "fewer than two provenance-sorted fields have sufficient raw cells",
                }
            )
        query_path = m3_root / "folds" / "m3" / "patient_held_out" / patient_id / "query.csv"
        _, query_rows = support.read_csv(query_path)
        if len(query_rows) != 1 or query_rows[0]["patient_id"] != patient_id:
            raise ValueError(f"full-source M3 query identity differs for {patient_id}")
        full_source = [float(query_rows[0][name]) for name in raw_feature_names]
        comparison_values = [
            ("full_source_vs_four_field", full_source, four_field),
            ("four_field_vs_cell_subsample", four_field, cell_subsample),
        ]
        if leave_one_field is not None:
            comparison_values.append(
                ("four_field_vs_leave_one_field", four_field, leave_one_field)
            )
        for name, left, right in comparison_values:
            comparisons[name][0].append(left)
            comparisons[name][1].append(right)
        patient_rows.append(
            {
                "patient_id": patient_id,
                "field_count": len(fields),
                "cell_count": len(rows),
                "cell_subsample_count": len(subsample),
                "full_source_vs_four_field_median_absolute_difference": statistics.median(
                    abs(left - right) for left, right in zip(full_source, four_field)
                ),
                "four_field_vs_cell_subsample_median_absolute_difference": statistics.median(
                    abs(left - right) for left, right in zip(four_field, cell_subsample)
                ),
                "four_field_vs_leave_one_field_median_absolute_difference": (
                    statistics.median(
                        abs(left - right)
                        for left, right in zip(four_field, leave_one_field)
                    )
                    if leave_one_field is not None
                    else ""
                ),
            }
        )
        sources[str(path)] = support.sha256(path)
        sources[str(query_path)] = support.sha256(query_path)
    feature_rows = []
    aggregate = {}
    for comparison, (left_rows, right_rows) in comparisons.items():
        correlations = []
        for index, feature in enumerate(raw_feature_names):
            correlation = spearman(
                [row[index] for row in left_rows],
                [row[index] for row in right_rows],
            )
            correlations.append(correlation)
            feature_rows.append(
                {
                    "comparison": comparison,
                    "feature": feature,
                    "patient_rank_spearman": correlation,
                }
            )
        aggregate[comparison] = {
            "feature_count": len(correlations),
            "median_feature_patient_rank_spearman": statistics.median(correlations),
            "q10_feature_patient_rank_spearman": quantile(correlations, 0.10),
            "minimum_feature_patient_rank_spearman": min(correlations),
        }
    fusion_eligible = all(
        aggregate[comparison][f"{statistic}_feature_patient_rank_spearman"] >= threshold
        for key, threshold in FUSION_THRESHOLDS.items()
        for comparison, statistic in [key.rsplit("_", 1)]
    )
    out = args.out.resolve()
    if out.exists() or out.is_symlink():
        raise ValueError(f"output already exists: {out}")
    out.parent.mkdir(parents=True, exist_ok=True)
    staging = out.with_name(f".{out.name}.{os.getpid()}.tmp")
    staging.mkdir()
    try:
        write_csv(
            staging / "patient_diagnostics.csv",
            list(patient_rows[0]),
            patient_rows,
        )
        write_csv(
            staging / "feature_stability.csv",
            ["comparison", "feature", "patient_rank_spearman"],
            feature_rows,
        )
        summary = {
            "schema_name": "marklab_tcga_crc_m3_stability_summary",
            "schema_version": SCHEMA_VERSION,
            "population_unit": "patient",
            "patient_count": len(patient_rows),
            "fields_per_patient_ceiling": bounded["fields_per_patient_ceiling"],
            "cell_subsample_fraction": CELL_SUBSAMPLE_FRACTION,
            "comparisons": aggregate,
            "fusion_thresholds_prespecified_before_real_summary": FUSION_THRESHOLDS,
            "fusion_eligible": fusion_eligible,
            "unavailable_leave_one_field": unavailable_leave_one_field,
            "claim_status": "bounded_stability_diagnostic_not_independent_cell_or_field_inference",
            "source_sha256": dict(sorted(sources.items())),
        }
        (staging / "summary.json").write_text(
            json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        output_files = sorted(path for path in staging.iterdir() if path.is_file())
        (staging / "manifest.json").write_text(
            json.dumps(
                {
                    "schema_name": "marklab_tcga_crc_m3_stability_manifest",
                    "schema_version": SCHEMA_VERSION,
                    "output_sha256": {
                        path.name: support.sha256(path) for path in output_files
                    },
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
    parser.add_argument("--m3-admission", required=True, type=Path)
    parser.add_argument("--bounded-raw-input", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    return parser.parse_args()


if __name__ == "__main__":
    build(parse_args())
