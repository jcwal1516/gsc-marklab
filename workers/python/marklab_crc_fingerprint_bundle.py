#!/usr/bin/env python3
"""Seal the canonical real-data CRC spatial fingerprint result bundle."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import os
from pathlib import Path
import shutil
from typing import Any, Iterable


SCHEMA_VERSION = "1.0"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def read_csv(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as source:
        rows = list(csv.DictReader(source))
    if not rows:
        raise ValueError(f"CSV has no rows: {path}")
    return rows


def interval_status(mean: float, lower: float, upper: float) -> str:
    if not all(math.isfinite(value) for value in (mean, lower, upper)) or lower > upper:
        raise ValueError("effect interval must be finite and ordered")
    if lower > 0.0:
        return "positive"
    if upper < 0.0:
        return "negative"
    return "uncertain"


def write_csv(path: Path, fields: list[str], rows: Iterable[dict[str, Any]]) -> None:
    with path.open("w", newline="", encoding="utf-8") as target:
        writer = csv.DictWriter(target, fieldnames=fields, lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def tcga_lane_summary(root: Path, lane: str) -> dict[str, Any]:
    analysis = root / "analysis" if lane == "m1" else root / lane / "analysis"
    retrieval_root = analysis if lane == "m1" else analysis / "retrieval"
    summary = read_json(retrieval_root / "summary.json")
    increment = summary[f"{lane}_minus_m0"]["patient_held_out"]
    top1_key = f"top1_accuracy_difference_{lane}_minus_m0"
    effect_key = f"between_minus_within_effect_difference_{lane}_minus_m0"
    top1 = float(increment[top1_key])
    top1_interval = increment["top1_accuracy_difference_bootstrap_95"]
    effect = float(increment[effect_key])
    effect_interval = increment["between_minus_within_effect_difference_bootstrap_95"]
    cohort_root = analysis / "cohort_tests"
    energy = read_json(cohort_root / f"{lane}_patient_held_out_energy.json")
    mmd = read_json(cohort_root / f"{lane}_patient_held_out_mmd.json")
    return {
        "patient_count": summary["blocks"][f"{lane}_patient_held_out"]["patient_count"],
        "patient_held_out": summary["blocks"][f"{lane}_patient_held_out"],
        "site_held_out": summary["blocks"][f"{lane}_site_held_out"],
        "top1_increment_vs_m0": {
            "mean": top1,
            "interval_95": top1_interval,
            "status": interval_status(
                top1, float(top1_interval["lower"]), float(top1_interval["upper"])
            ),
        },
        "between_minus_within_increment_vs_m0": {
            "mean": effect,
            "interval_95": effect_interval,
            "status": interval_status(
                effect, float(effect_interval["lower"]), float(effect_interval["upper"])
            ),
        },
        "patient_label_energy": {
            "effect": energy["energy_distance"],
            "p_value": energy["p_value"],
        },
        "patient_label_linear_mmd": {
            "effect": mmd["mmd_squared"],
            "p_value": mmd["p_value"],
        },
    }


def patient_fingerprint_rows(root: Path) -> list[dict[str, Any]]:
    specs = [
        ("m0", root / "admission"),
        ("m1", root / "admission"),
        ("m2", root / "m2" / "fingerprint_admission"),
        ("m3", root / "m3" / "admission"),
        ("m4", root / "m4" / "admission"),
        ("m6", root / "m6" / "admission"),
    ]
    rows = []
    for lane, admission in specs:
        for patient in read_csv(admission / "admitted_patients.csv"):
            patient_id = patient["patient_id"]
            query = read_csv(
                admission / "folds" / lane / "patient_held_out" / patient_id / "query.csv"
            )
            if len(query) != 1:
                raise ValueError(f"patient fingerprint query differs: {lane}/{patient_id}")
            for feature, value in query[0].items():
                if feature.startswith("embedding_"):
                    rows.append(
                        {
                            "cohort": "TCGA_CRC_CellViT",
                            "lane": lane,
                            "patient_id": patient_id,
                            "label": patient["label"],
                            "site_id": patient["site_id"],
                            "feature": feature,
                            "value": value,
                        }
                    )
    codex_root = root / "external" / "schurch_codex" / "admission"
    codex_admission = read_json(codex_root / "admission.json")
    for patient in read_csv(codex_root / "patient_fingerprints.csv"):
        for feature in codex_admission["feature_names"]:
            rows.append(
                {
                    "cohort": "Schurch_CODEX_2020",
                    "lane": "orthogonal_neighborhood",
                    "patient_id": patient["patient_id"],
                    "label": patient["label"],
                    "site_id": patient["site_id"],
                    "feature": feature,
                    "value": patient[feature],
                }
            )
    return rows


def classical_curve(path: Path) -> dict[float, float]:
    document = read_json(path)
    if document.get("format") != "marklab.classical_spatial":
        raise ValueError(f"invalid classical field result: {path}")
    return {
        float(point["radius_um"]): float(point["l"]) / float(point["radius_um"]) - 1.0
        for point in document["analysis"]["curve"]
        if point["status"] == "available" and point.get("l") is not None
    }


def specimen_fingerprint_rows(root: Path) -> list[dict[str, Any]]:
    result = []
    selected: dict[str, int] = {}
    for field in read_csv(root / "m2" / "stability_input" / "fields.csv"):
        patient = field["patient_id"]
        if selected.get(patient, 0) >= 4:
            continue
        selected[patient] = selected.get(patient, 0) + 1
        curve = classical_curve(
            root
            / "m2"
            / "results"
            / "classical_field_base"
            / patient
            / field["field_id"]
            / "result.json"
        )
        for radius, value in sorted(curve.items()):
            result.append(
                {
                    "cohort": "TCGA_CRC_CellViT",
                    "lane": "m2_classical_coordinate",
                    "patient_id": patient,
                    "specimen_id": field["field_id"],
                    "feature": f"coordinate_l_relative_{int(radius)}um",
                    "value": value,
                }
            )
    for field in read_csv(root / "m4" / "stability_input" / "fields.csv"):
        path = (
            root
            / "m4"
            / "results"
            / "variogram_fields"
            / field["patient_id"]
            / f"{field['field_id']}.json"
        )
        document = read_json(path)
        for point in document["curve"]:
            if int(point["pair_count"]) >= 2 and point.get("semivariance") is not None:
                result.append(
                    {
                        "cohort": "TCGA_CRC_CellViT",
                        "lane": "m4_raw_spatial",
                        "patient_id": field["patient_id"],
                        "specimen_id": field["field_id"],
                        "feature": f"raw_variogram_{point['bin_id']}",
                        "value": point["semivariance"],
                    }
                )
    codex = root / "external" / "schurch_codex" / "admission"
    codex_admission = read_json(codex / "admission.json")
    for core in read_csv(codex / "core_fingerprints.csv"):
        for feature in codex_admission["feature_names"]:
            result.append(
                {
                    "cohort": "Schurch_CODEX_2020",
                    "lane": "orthogonal_neighborhood",
                    "patient_id": core["patient_id"],
                    "specimen_id": core["core_id"],
                    "feature": feature,
                    "value": core[feature],
                }
            )
    return result


def required_artifacts(root: Path) -> list[Path]:
    paths = [
        root / "admission" / "admission.json",
        root / "admission" / "admitted_patients.csv",
        root / "analysis" / "summary.json",
        root / "analysis" / "summary_manifest.json",
        root / "m2" / "stability_input" / "fields.csv",
    ]
    for directory in (
        root / "analysis",
        root / "m2" / "analysis",
        root / "m3" / "analysis",
        root / "m4" / "analysis",
        root / "m6" / "analysis",
        root / "external" / "schurch_codex" / "analysis",
        root / "external" / "schurch_he",
        root / "m7",
    ):
        paths.extend(path for path in directory.rglob("*") if path.is_file())
    for admission in (
        root / "m2" / "fingerprint_admission",
        root / "m3" / "admission",
        root / "m4" / "admission",
        root / "m6" / "admission",
        root / "external" / "schurch_codex" / "admission",
    ):
        paths.extend(path for path in admission.iterdir() if path.is_file())
    for pattern in (
        "m2/projects/region_retrieval_shard_*/executions.jsonl",
        "m3/projects/region_retrieval_shard_*/executions.jsonl",
        "m4/projects/region_retrieval_shard_*/executions.jsonl",
        "m6/projects/region_retrieval_shard_*/executions.jsonl",
        "external/schurch_codex/projects/region_retrieval_shard_*/executions.jsonl",
        "m2/replay_proof/*.json",
        "m3/replay_proof/*.json",
        "m4/replay_proof/*.json",
        "m6/replay_proof/*.json",
        "external/schurch_codex/replay_proof/*.json",
    ):
        paths.extend(root.glob(pattern))
    unique = sorted(set(path.resolve() for path in paths))
    missing = [str(path) for path in unique if not path.is_file()]
    if missing:
        raise ValueError(f"canonical bundle artifact is missing: {missing[:3]}")
    return unique


def build(args: argparse.Namespace) -> None:
    root = args.root.resolve()
    out = args.out.resolve()
    if out.exists() or out.is_symlink():
        raise ValueError(f"output already exists: {out}")
    lanes = {lane: tcga_lane_summary(root, lane) for lane in ("m1", "m2", "m3", "m4", "m6")}
    m2_stability = read_json(root / "m2" / "analysis" / "stability" / "summary.json")
    m3_stability = read_json(root / "m3" / "analysis" / "stability" / "summary.json")
    m4_stability = read_json(root / "m4" / "analysis" / "stability" / "summary.json")
    m6_admission = read_json(root / "m6" / "admission" / "admission.json")
    codex = read_json(
        root / "external" / "schurch_codex" / "analysis" / "retrieval" / "summary.json"
    )
    codex_energy = read_json(
        root / "external" / "schurch_codex" / "analysis" / "cohort_tests" / "energy.json"
    )
    codex_mmd = read_json(
        root / "external" / "schurch_codex" / "analysis" / "cohort_tests" / "mmd.json"
    )
    schurch_he = read_json(root / "external" / "schurch_he" / "scientific_summary.json")
    m7_path = (
        root
        / "m7"
        / "results"
        / "beta_binomial_group_gender_slide_hierarchy_cellvit_msi_vs_mss_final.json"
    )
    m7 = read_json(m7_path)
    m7_effect = m7["posterior"]["marginal_probability_difference_comparison_minus_reference"]
    m7_miss = (
        root
        / "m7"
        / "results"
        / "beta_binomial_group_gender_slide_hierarchy_durable_miss.json"
    )
    m7_hit = (
        root
        / "m7"
        / "results"
        / "beta_binomial_group_gender_slide_hierarchy_durable_hit.json"
    )
    if sha256(m7_miss) != sha256(m7_hit):
        raise ValueError("M7 durable miss/hit outputs differ")
    unavailable = [
        {
            "lane": "M1 annotation spatial fusion component",
            "blocker": "no bounded four-field cell-subsample and ROI-bootstrap stability result was available",
        },
        {
            "lane": "M2 graph/Fourier grid fusion component",
            "blocker": "2x2-vs-3x3 patient-rank Spearman stability ranged from 0.2701 to 0.6090",
        },
        {
            "lane": "M4 near raw-vector variogram fusion component",
            "blocker": "only 87 of 169 patients had at least two fields with two eligible 0-25 um pairs",
        },
        {
            "lane": "M4 raw-vector kernel and neighborhood summaries",
            "blocker": "no admitted fold-safe raw-vector kernel or neighborhood output exists for this cohort",
        },
        {
            "lane": "M5 genuine patch/multiscale embedding",
            "blocker": "TCGA and CPTAC CellViT graphs contain x cell vectors, positions, and patch-selection metadata but no independent patch embedding tensor; 366 CPTAC .pt graphs and the admitted TCGA roots contained no patch/tile embedding file",
        },
        {
            "lane": "M7 cohort and distinct ROI variance components",
            "blocker": "the admitted pinned model contains one CPTAC cohort and slide-within-patient counts without a separate ROI-within-slide level",
        },
        {
            "lane": "direct M6 external feature-space transfer",
            "blocker": "Schuerch CODEX protein/neighborhood space and TCGA CellViT raw vectors are not a shared raw feature space; no second cohort has all 67 M6 features under the same typed schema",
        },
    ]
    scientific = {
        "schema_name": "marklab_crc_spatial_fingerprint_canonical_summary",
        "schema_version": SCHEMA_VERSION,
        "scientific_objective": "determine whether CRC tumors form reproducible spatial microenvironments and whether a patient-level spatial fingerprint recurs within molecular class across independent patients",
        "interpretation_policy": "null_negative_confounded_and_unstable_results_are_reported_without_significance_optimization",
        "population_unit": "patient",
        "tcga_discovery_and_internal_validation": lanes,
        "stability": {
            "m2_coordinate": {
                "scale_summary": m2_stability["scale_summary"],
                "nearby_scale_summary": m2_stability["nearby_scale_summary"],
                "graph_grid_summary": m2_stability["graph_grid_summary"],
            },
            "m3_raw_nonspatial": {
                "comparisons": m3_stability["comparisons"],
                "fusion_eligible": m3_stability["fusion_eligible"],
            },
            "m4_raw_spatial": {
                "bands": m4_stability["bands"],
                "nearby_scale_summary": m4_stability["nearby_scale_summary"],
                "fusion_eligible": m4_stability["fusion_eligible"],
            },
        },
        "m6_fusion": {
            "patient_count": m6_admission["patient_count"],
            "feature_count": m6_admission["m6_feature_count"],
            "included_components": m6_admission["included_components"],
            "selection_policy": m6_admission["selection_policy"],
        },
        "m7_bayesian": {
            "cohort": "CPTAC CRC H&E CellViT",
            "patients": m7["input"]["patients"],
            "slides": m7["input"]["slides"],
            "repeated_patients": m7["input"]["repeated_patients"],
            "backend": m7["backend"],
            "diagnostics": m7["diagnostics"],
            "marginal_msi_minus_mss_probability_difference": {
                **m7_effect,
                "status": interval_status(
                    float(m7_effect["mean"]),
                    float(m7_effect["interval_lower"]),
                    float(m7_effect["interval_upper"]),
                ),
            },
            "posterior_predictive_group_tail_probability": m7["posterior_predictive"][
                "probability_replicated_patient_group_difference_at_least_observed"
            ],
            "durable_miss_hit_sha256": sha256(m7_miss),
        },
        "external_validation": {
            "schurch_he_cellvit": {
                "patient_count": schurch_he["patient_count"],
                "molecular_primary": schurch_he["molecular_primary"],
                "role": "independent H&E CellViT direction-generating cohort; its cosine-excess score is not the 67-feature M6 vector",
            },
            "schurch_codex": {
                "patient_count": codex["patient_count"],
                "balanced_top1_accuracy": codex["balanced_top1_accuracy"],
                "top1_recall_by_group": codex["top1_recall_by_group"],
                "mean_between_minus_within_distance": codex[
                    "mean_between_minus_within_distance"
                ],
                "stability": codex["stability"],
                "energy": {
                    "effect": codex_energy["energy_distance"],
                    "p_value": codex_energy["p_value"],
                },
                "linear_mmd": {
                    "effect": codex_mmd["mmd_squared"],
                    "p_value": codex_mmd["p_value"],
                },
                "role": codex["validation_role"],
                "raw_feature_space_shared_with_cellvit_claimed": False,
            },
        },
        "overall_result": "The admitted fingerprints are often technically stable, but this checkpoint does not establish a reproducible cross-patient molecular-class spatial fingerprint. M1 showed a modest uncertain internal gain; M2 and M3 were null-to-negative; M4 reduced top-1 retrieval; and stability-selected M6 changed patient-held-out top-1 by +0.012 with an interval spanning zero, changed site-held-out top-1 by -0.018, and had null patient-label energy/MMD. Schuerch CODEX neighborhoods were stable but had zero MSI top-1 recall and null energy/MMD. The Bayesian MSI-minus-MSS interval narrowly included zero.",
        "unavailable_lane_count": len(unavailable),
        "claim_limitations": [
            "retrospective exploratory cohorts only",
            "no clinical utility, causal, prospective, or performance claim",
            "slides, cores, fields, cells, and edges were not treated as independent patient replicates",
            "CODEX protein vectors and CellViT embeddings were never treated as one raw feature space",
        ],
    }
    root_artifacts = required_artifacts(root)
    out.parent.mkdir(parents=True, exist_ok=True)
    staging = out.with_name(f".{out.name}.{os.getpid()}.tmp")
    staging.mkdir()
    try:
        patient_rows = patient_fingerprint_rows(root)
        write_csv(
            staging / "patient_fingerprints.csv",
            ["cohort", "lane", "patient_id", "label", "site_id", "feature", "value"],
            patient_rows,
        )
        specimen_rows = specimen_fingerprint_rows(root)
        write_csv(
            staging / "specimen_fingerprints.csv",
            ["cohort", "lane", "patient_id", "specimen_id", "feature", "value"],
            specimen_rows,
        )
        (staging / "scientific_summary.json").write_text(
            json.dumps(scientific, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        (staging / "unavailable_lanes.json").write_text(
            json.dumps(
                {
                    "schema_name": "marklab_crc_spatial_fingerprint_unavailable_lanes",
                    "schema_version": SCHEMA_VERSION,
                    "lanes": unavailable,
                },
                indent=2,
                sort_keys=True,
            )
            + "\n",
            encoding="utf-8",
        )
        generated = sorted(path for path in staging.iterdir() if path.is_file())
        artifact_hashes = {
            path.relative_to(root).as_posix(): sha256(path) for path in root_artifacts
        }
        stability_hashes = {
            relative: digest
            for relative, digest in artifact_hashes.items()
            if "/stability/" in relative or relative.endswith("stability.json")
        }
        ledgers = {
            relative: sum(1 for line in (root / relative).read_text(encoding="utf-8").splitlines() if line)
            for relative in artifact_hashes
            if relative.endswith("executions.jsonl")
        }
        manifest = {
            "schema_name": "marklab_crc_spatial_fingerprint_canonical_bundle",
            "schema_version": SCHEMA_VERSION,
            "population_unit": "patient",
            "patient_fingerprint_row_count": len(patient_rows),
            "specimen_fingerprint_row_count": len(specimen_rows),
            "artifact_sha256": artifact_hashes,
            "stability_output_sha256": stability_hashes,
            "durable_ledger_rows": ledgers,
            "generated_sha256": {path.name: sha256(path) for path in generated},
            "result_format_compatibility": "0.3_preserved",
        }
        (staging / "manifest.json").write_text(
            json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        os.rename(staging, out)
    except BaseException:
        shutil.rmtree(staging, ignore_errors=True)
        raise


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    return parser.parse_args()


if __name__ == "__main__":
    build(parse_args())
