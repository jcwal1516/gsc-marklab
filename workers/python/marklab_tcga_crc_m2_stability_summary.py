#!/usr/bin/env python3
"""Summarize bounded patient-level stability for TCGA CRC M2 coordinates."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import os
import random
import shutil
from collections import defaultdict
from pathlib import Path


RADII = (25.0, 50.0, 75.0, 100.0)
BOOTSTRAPS = 500
FIELDS_PER_PATIENT = 4


def ranks(values: list[float]) -> list[float]:
    order = sorted(range(len(values)), key=lambda index: (values[index], index))
    result = [0.0] * len(values)
    start = 0
    while start < len(order):
        end = start + 1
        while end < len(order) and values[order[end]] == values[order[start]]:
            end += 1
        rank = (start + 1 + end) / 2.0
        for index in order[start:end]:
            result[index] = rank
        start = end
    return result


def spearman(left: list[float], right: list[float]) -> float:
    if len(left) != len(right) or len(left) < 2:
        raise ValueError("Spearman correlation requires paired nontrivial values")
    x = ranks(left)
    y = ranks(right)
    mx = math.fsum(x) / len(x)
    my = math.fsum(y) / len(y)
    numerator = math.fsum((a - mx) * (b - my) for a, b in zip(x, y))
    denominator = math.sqrt(
        math.fsum((a - mx) ** 2 for a in x) * math.fsum((b - my) ** 2 for b in y)
    )
    if denominator == 0.0:
        raise ValueError("Spearman correlation is undefined for a constant vector")
    return numerator / denominator


def median(values: list[float]) -> float:
    ordered = sorted(values)
    middle = len(ordered) // 2
    if len(ordered) % 2:
        return ordered[middle]
    return (ordered[middle - 1] + ordered[middle]) / 2.0


def quantile(values: list[float], probability: float) -> float:
    ordered = sorted(values)
    position = probability * (len(ordered) - 1)
    lower = math.floor(position)
    upper = math.ceil(position)
    if lower == upper:
        return ordered[lower]
    return ordered[lower] + (position - lower) * (ordered[upper] - ordered[lower])


def roi_bootstrap_absolute_deviations(
    values: list[float], replicates: int, seed: int
) -> list[float]:
    if len(values) < 2 or replicates < 1 or any(not math.isfinite(value) for value in values):
        raise ValueError("ROI bootstrap requires finite field values")
    observed = median(values)
    generator = random.Random(seed)
    return [
        abs(median([values[generator.randrange(len(values))] for _ in values]) - observed)
        for _ in range(replicates)
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


def field_curve(path: Path) -> dict[float, float]:
    document = json.loads(path.read_text(encoding="utf-8"))
    if document.get("format") != "marklab.classical_spatial":
        raise ValueError(f"invalid classical field result: {path}")
    curve = {}
    for point in document["analysis"]["curve"]:
        radius = float(point["radius_um"])
        if point["status"] == "available" and point.get("l") is not None:
            curve[radius] = float(point["l"]) / radius - 1.0
    return curve


def graph_bands(path: Path) -> dict[str, float]:
    document = json.loads(path.read_text(encoding="utf-8"))
    if document.get("format") != "marklab.graph_spectral":
        raise ValueError(f"invalid graph result: {path}")
    return {row["id"]: float(row["energy_fraction"]) for row in document["frequency_bands"]}


def build(args: argparse.Namespace) -> None:
    fields = read_csv(args.fields_manifest)
    selected = defaultdict(list)
    for row in fields:
        if len(selected[row["patient_id"]]) < FIELDS_PER_PATIENT:
            selected[row["patient_id"]].append(row["field_id"])
    if not selected or any(len(values) < 2 for values in selected.values()):
        raise ValueError("bounded field selection lacks patient support")
    sources = {str(args.fields_manifest.resolve()): sha256(args.fields_manifest)}
    patient_scale = []
    unavailable = []
    by_radius = {radius: {"base": {}, "subsample": {}, "bootstrap95": {}} for radius in RADII}
    for patient in sorted(selected):
        base_values = defaultdict(list)
        subsample_values = defaultdict(list)
        for field in selected[patient]:
            base_path = args.base_results / patient / field / "result.json"
            subsample_path = args.subsample_results / patient / field / "result.json"
            if not base_path.is_file() or not subsample_path.is_file():
                raise ValueError(f"selected stability field is incomplete: {patient}/{field}")
            base = field_curve(base_path)
            subsample = field_curve(subsample_path)
            sources[str(base_path.resolve())] = sha256(base_path)
            sources[str(subsample_path.resolve())] = sha256(subsample_path)
            for radius in RADII:
                if radius in base:
                    base_values[radius].append(base[radius])
                if radius in subsample:
                    subsample_values[radius].append(subsample[radius])
        for radius in RADII:
            if len(base_values[radius]) < 2 or len(subsample_values[radius]) < 2:
                unavailable.append(
                    {
                        "patient_id": patient,
                        "radius_um": radius,
                        "base_field_count": len(base_values[radius]),
                        "subsample_field_count": len(subsample_values[radius]),
                        "blocker": "fewer than two selected fields have an available exact L endpoint",
                    }
                )
                continue
            base_median = median(base_values[radius])
            subsample_median = median(subsample_values[radius])
            seed = int.from_bytes(hashlib.sha256(f"{patient}:{radius}".encode()).digest()[:8], "big")
            deviations = roi_bootstrap_absolute_deviations(base_values[radius], BOOTSTRAPS, seed)
            bootstrap95 = quantile(deviations, 0.95)
            by_radius[radius]["base"][patient] = base_median
            by_radius[radius]["subsample"][patient] = subsample_median
            by_radius[radius]["bootstrap95"][patient] = bootstrap95
            patient_scale.append(
                {
                    "patient_id": patient,
                    "radius_um": radius,
                    "base_field_median_l_relative": base_median,
                    "subsample_field_median_l_relative": subsample_median,
                    "absolute_subsample_difference": abs(subsample_median - base_median),
                    "roi_bootstrap_absolute_deviation_95": bootstrap95,
                }
            )
    scale_summary = {}
    for radius in RADII:
        patients = sorted(set(by_radius[radius]["base"]) & set(by_radius[radius]["subsample"]))
        base = [by_radius[radius]["base"][patient] for patient in patients]
        subsample = [by_radius[radius]["subsample"][patient] for patient in patients]
        scale_summary[str(int(radius))] = {
            "patient_count": len(patients),
            "cell_subsample_patient_rank_spearman": spearman(base, subsample),
            "median_absolute_cell_subsample_difference": median(
                [abs(left - right) for left, right in zip(base, subsample)]
            ),
            "median_roi_bootstrap_absolute_deviation_95": median(
                [by_radius[radius]["bootstrap95"][patient] for patient in patients]
            ),
        }
    adjacent = {}
    for left, right in zip(RADII, RADII[1:]):
        patients = sorted(set(by_radius[left]["base"]) & set(by_radius[right]["base"]))
        adjacent[f"{int(left)}_{int(right)}"] = {
            "patient_count": len(patients),
            "patient_rank_spearman": spearman(
                [by_radius[left]["base"][patient] for patient in patients],
                [by_radius[right]["base"][patient] for patient in patients],
            ),
        }
    graph_summary = {}
    graph_values = {2: defaultdict(dict), 3: defaultdict(dict)}
    for patient in sorted(selected):
        for grid, root in ((2, args.graph_2x2_results), (3, args.graph_3x3_results)):
            path = root / f"{patient}.json"
            sources[str(path.resolve())] = sha256(path)
            for band, value in graph_bands(path).items():
                graph_values[grid][band][patient] = value
    for band in ("low", "middle", "high"):
        patients = sorted(set(graph_values[2][band]) & set(graph_values[3][band]))
        graph_summary[band] = {
            "patient_count": len(patients),
            "grid_2x2_vs_3x3_patient_rank_spearman": spearman(
                [graph_values[2][band][patient] for patient in patients],
                [graph_values[3][band][patient] for patient in patients],
            ),
        }

    out = args.out.resolve()
    if out.exists():
        raise ValueError(f"output already exists: {out}")
    out.parent.mkdir(parents=True, exist_ok=True)
    staging = out.with_name(f".{out.name}.{os.getpid()}.tmp")
    staging.mkdir()
    try:
        with (staging / "patient_scale_stability.csv").open("w", newline="", encoding="utf-8") as target:
            writer = csv.DictWriter(target, fieldnames=list(patient_scale[0]), lineterminator="\n")
            writer.writeheader()
            writer.writerows(patient_scale)
        (staging / "summary.json").write_text(
            json.dumps(
                {
                    "schema_name": "marklab_tcga_crc_m2_stability_summary",
                    "schema_version": "1.0",
                    "population_unit": "patient",
                    "resample_unit": "sampled_field",
                    "patient_count": len(selected),
                    "fields_per_patient_ceiling": FIELDS_PER_PATIENT,
                    "roi_bootstrap_replicates": BOOTSTRAPS,
                    "scale_summary": scale_summary,
                    "nearby_scale_summary": adjacent,
                    "graph_grid_summary": graph_summary,
                    "unavailable_patient_scales": unavailable,
                    "source_sha256": dict(sorted(sources.items())),
                    "claim_status": "stability_diagnostic_not_independent_field_inference",
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
    parser.add_argument("--fields-manifest", required=True, type=Path)
    parser.add_argument("--base-results", required=True, type=Path)
    parser.add_argument("--subsample-results", required=True, type=Path)
    parser.add_argument("--graph-2x2-results", required=True, type=Path)
    parser.add_argument("--graph-3x3-results", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    return parser.parse_args()


if __name__ == "__main__":
    build(parse_args())
