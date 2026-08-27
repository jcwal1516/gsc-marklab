#!/usr/bin/env python3
"""Summarize patient-level stability of bounded raw CellViT semivariograms."""

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
BANDS = ("near", "intermediate", "far")
BOOTSTRAP_REPLICATES = 500
BOOTSTRAP_SEED = 20_260_827
FUSION_THRESHOLDS = {
    "minimum_patient_fraction": 0.80,
    "cell_subsample_patient_rank_spearman": 0.80,
    "field_median_patient_rank_spearman": 0.60,
}


def rank_support():
    path = Path(__file__).with_name("marklab_tcga_crc_m3_stability_summary.py")
    spec = importlib.util.spec_from_file_location("marklab_m3_stability_support", path)
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


def semivariance(document: dict, band: str) -> float | None:
    if (
        document.get("format") != "marklab.vector_semivariogram"
        or document.get("version") != 1
        or document.get("embedding_dimension") != 1280
    ):
        raise ValueError("invalid raw-vector semivariogram result")
    row = next((row for row in document["curve"] if row["bin_id"] == band), None)
    if row is None or int(row.get("pair_count", 0)) < 2 or row.get("semivariance") is None:
        return None
    value = float(row["semivariance"])
    return value if math.isfinite(value) else None


def bootstrap_absolute_deviation_95(values: list[float], seed: int) -> float:
    if len(values) < 2:
        raise ValueError("field bootstrap requires at least two values")
    center = statistics.median(values)
    generator = random.Random(seed)
    deviations = []
    for _ in range(BOOTSTRAP_REPLICATES):
        sample = [values[generator.randrange(len(values))] for _ in values]
        deviations.append(abs(statistics.median(sample) - center))
    return float(np.quantile(np.asarray(deviations), 0.95))


def read_result(path: Path, expected_input_sha256: str) -> tuple[dict, str]:
    digest = sha256(path)
    document = json.loads(path.read_text(encoding="utf-8"))
    if document.get("input_sha256") != expected_input_sha256:
        raise ValueError(f"semivariogram input identity differs: {path}")
    return document, digest


def build(args: argparse.Namespace) -> None:
    support = rank_support()
    input_root = args.input.resolve()
    stability_root = args.stability_input.resolve()
    full_root = args.full_results.resolve()
    subsample_root = args.subsample_results.resolve()
    field_root = args.field_results.resolve()
    patients = read_csv(input_root / "patients.csv")
    subsample_patients = {
        row["patient_id"]: row for row in read_csv(stability_root / "patients.csv")
    }
    fields: dict[str, list[dict[str, str]]] = {}
    for row in read_csv(stability_root / "fields.csv"):
        fields.setdefault(row["patient_id"], []).append(row)
    sources = {
        str(input_root / "admission.json"): sha256(input_root / "admission.json"),
        str(stability_root / "admission.json"): sha256(
            stability_root / "admission.json"
        ),
        str(input_root / "bins.csv"): sha256(input_root / "bins.csv"),
    }
    band_values = {
        band: {"full": {}, "subsample": {}, "field_median": {}, "field_values": {}}
        for band in BANDS
    }
    unavailable = []
    for patient_index, patient in enumerate(patients):
        patient_id = patient["patient_id"]
        subsample_patient = subsample_patients.get(patient_id)
        if subsample_patient is None:
            raise ValueError(f"patient lacks M4 subsample identity: {patient_id}")
        full_path = full_root / f"{patient_id}.json"
        subsample_path = subsample_root / f"{patient_id}.json"
        full, full_digest = read_result(full_path, patient["input_sha256"])
        subsample, subsample_digest = read_result(
            subsample_path, subsample_patient["input_sha256"]
        )
        sources[str(full_path)] = full_digest
        sources[str(subsample_path)] = subsample_digest
        field_documents = []
        for field in fields.get(patient_id, []):
            path = field_root / patient_id / f"{field['field_id']}.json"
            document, digest = read_result(path, field["input_sha256"])
            field_documents.append((field["field_id"], document))
            sources[str(path)] = digest
        for band_index, band in enumerate(BANDS):
            full_value = semivariance(full, band)
            subsample_value = semivariance(subsample, band)
            field_values = [
                value
                for _, document in field_documents
                for value in [semivariance(document, band)]
                if value is not None
            ]
            if full_value is not None:
                band_values[band]["full"][patient_id] = full_value
            if subsample_value is not None:
                band_values[band]["subsample"][patient_id] = subsample_value
            if len(field_values) >= 2:
                band_values[band]["field_median"][patient_id] = statistics.median(
                    field_values
                )
                band_values[band]["field_values"][patient_id] = field_values
            missing = []
            if full_value is None:
                missing.append("full patient result lacks two pairs")
            if subsample_value is None:
                missing.append("80% cell subsample lacks two pairs")
            if len(field_values) < 2:
                missing.append("fewer than two fields have two raw-vector pairs")
            if missing:
                unavailable.append(
                    {
                        "patient_id": patient_id,
                        "band": band,
                        "field_result_count": len(field_values),
                        "blocker": "; ".join(missing),
                    }
                )
    summaries = {}
    diagnostic_rows = []
    for band_index, band in enumerate(BANDS):
        values = band_values[band]
        cell_patients = sorted(set(values["full"]) & set(values["subsample"]))
        field_patients = sorted(set(values["full"]) & set(values["field_median"]))
        if len(cell_patients) < 3 or len(field_patients) < 3:
            raise ValueError(f"M4 stability has insufficient patient support for {band}")
        cell_correlation = support.spearman(
            [values["full"][patient] for patient in cell_patients],
            [values["subsample"][patient] for patient in cell_patients],
        )
        field_correlation = support.spearman(
            [values["full"][patient] for patient in field_patients],
            [values["field_median"][patient] for patient in field_patients],
        )
        bootstrap_deviations = {
            patient: bootstrap_absolute_deviation_95(
                values["field_values"][patient],
                BOOTSTRAP_SEED
                + band_index * 10_000
                + int(hashlib.sha256(patient.encode()).hexdigest()[:8], 16),
            )
            for patient in field_patients
        }
        for patient in sorted(set(cell_patients) | set(field_patients)):
            diagnostic_rows.append(
                {
                    "patient_id": patient,
                    "band": band,
                    "full_semivariance": values["full"].get(patient, ""),
                    "cell_subsample_semivariance": values["subsample"].get(patient, ""),
                    "field_median_semivariance": values["field_median"].get(patient, ""),
                    "field_count": len(values["field_values"].get(patient, [])),
                    "field_bootstrap_absolute_deviation_95": bootstrap_deviations.get(
                        patient, ""
                    ),
                }
            )
        fraction = min(len(cell_patients), len(field_patients)) / len(patients)
        eligible = (
            fraction >= FUSION_THRESHOLDS["minimum_patient_fraction"]
            and cell_correlation
            >= FUSION_THRESHOLDS["cell_subsample_patient_rank_spearman"]
            and field_correlation
            >= FUSION_THRESHOLDS["field_median_patient_rank_spearman"]
        )
        summaries[band] = {
            "cell_subsample_patient_count": len(cell_patients),
            "cell_subsample_patient_rank_spearman": cell_correlation,
            "median_absolute_cell_subsample_difference": statistics.median(
                abs(values["full"][patient] - values["subsample"][patient])
                for patient in cell_patients
            ),
            "field_patient_count": len(field_patients),
            "field_median_patient_rank_spearman": field_correlation,
            "median_field_bootstrap_absolute_deviation_95": statistics.median(
                bootstrap_deviations.values()
            ),
            "fusion_eligible": eligible,
        }
    nearby = {}
    for left, right in (("near", "intermediate"), ("intermediate", "far")):
        common = sorted(
            set(band_values[left]["full"]) & set(band_values[right]["full"])
        )
        nearby[f"{left}_vs_{right}"] = {
            "patient_count": len(common),
            "patient_rank_spearman": support.spearman(
                [band_values[left]["full"][patient] for patient in common],
                [band_values[right]["full"][patient] for patient in common],
            ),
        }
    out = args.out.resolve()
    if out.exists() or out.is_symlink():
        raise ValueError(f"output already exists: {out}")
    out.parent.mkdir(parents=True, exist_ok=True)
    staging = out.with_name(f".{out.name}.{os.getpid()}.tmp")
    staging.mkdir()
    try:
        with (staging / "patient_band_diagnostics.csv").open(
            "w", newline="", encoding="utf-8"
        ) as target:
            writer = csv.DictWriter(
                target, fieldnames=list(diagnostic_rows[0]), lineterminator="\n"
            )
            writer.writeheader()
            writer.writerows(diagnostic_rows)
        summary = {
            "schema_name": "marklab_tcga_crc_m4_stability_summary",
            "schema_version": SCHEMA_VERSION,
            "population_unit": "patient",
            "patient_count": len(patients),
            "resample_unit": "provenance_sorted_field",
            "field_bootstrap_replicates": BOOTSTRAP_REPLICATES,
            "field_bootstrap_seed": BOOTSTRAP_SEED,
            "bands": summaries,
            "nearby_scale_summary": nearby,
            "fusion_thresholds_prespecified_before_real_summary": FUSION_THRESHOLDS,
            "fusion_eligible": all(summary["fusion_eligible"] for summary in summaries.values()),
            "unavailable_patient_bands": unavailable,
            "claim_status": "bounded_stability_diagnostic_not_independent_cell_or_field_inference",
            "source_sha256": dict(sorted(sources.items())),
        }
        (staging / "summary.json").write_text(
            json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        files = sorted(path for path in staging.iterdir() if path.is_file())
        (staging / "manifest.json").write_text(
            json.dumps(
                {
                    "schema_name": "marklab_tcga_crc_m4_stability_manifest",
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
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--stability-input", required=True, type=Path)
    parser.add_argument("--full-results", required=True, type=Path)
    parser.add_argument("--subsample-results", required=True, type=Path)
    parser.add_argument("--field-results", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    return parser.parse_args()


if __name__ == "__main__":
    build(parse_args())
