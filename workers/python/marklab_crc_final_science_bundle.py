#!/usr/bin/env python3
"""Seal the final canonical CRC scientific result bundle."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
from typing import Any


class BundleError(ValueError):
    """A required final scientific artifact is missing or inconsistent."""


def read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def write_json(path: Path, document: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def graph_topology_interpretation(summary: dict[str, Any]) -> dict[str, object]:
    graph_gate = summary["fusion"]["graph"]
    topology_gate = summary["fusion"]["topology"]
    graph_increment = summary["incremental_information"]["graph"][
        "balanced_accuracy_increment"
    ]
    topology_increment = summary["incremental_information"]["topology"][
        "balanced_accuracy_increment"
    ]
    if graph_gate["eligible"] or topology_gate["eligible"]:
        status = "at_least_one_prespecified_block_admitted"
    elif graph_increment <= 0.0 and topology_increment <= 0.0:
        status = "unstable_and_nonincremental"
    else:
        status = "failed_prespecified_stability_or_incremental_gate"
    return {
        "status": status,
        "graph_balanced_accuracy_increment": graph_increment,
        "topology_balanced_accuracy_increment": topology_increment,
        "graph_failed_checks": graph_gate["failed_checks"],
        "topology_failed_checks": topology_gate["failed_checks"],
        "new_blocks_included": summary["fusion"]["new_blocks_included"],
    }


def _copy_file(source: Path, staging: Path, relative: str) -> None:
    if not source.is_file() or source.is_symlink():
        raise BundleError(f"required regular artifact is absent: {source}")
    destination = staging / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)


def _copy_tree(source: Path, staging: Path, relative: str) -> None:
    if not source.is_dir() or source.is_symlink():
        raise BundleError(f"required artifact tree is absent: {source}")
    shutil.copytree(source, staging / relative)


def _copy_existing_science(canonical: Path, staging: Path) -> None:
    for directory in (
        "bundle",
        "analysis",
        "m2/analysis",
        "m3/analysis",
        "m4/analysis",
        "m6/analysis",
        "m7",
        "external/schurch_codex/admission",
        "external/schurch_codex/analysis",
        "external/schurch_he",
    ):
        _copy_tree(canonical / directory, staging, f"existing/{directory}")
    for relative in (
        "admission/admission.json",
        "admission/admitted_patients.csv",
        "admission/folds.csv",
        "m2/fingerprint_admission/admission.json",
        "m2/fingerprint_admission/admitted_patients.csv",
        "m2/fingerprint_admission/folds.csv",
        "m2/fingerprint_admission/patient_features.csv",
        "m3/admission/admission.json",
        "m3/admission/admitted_patients.csv",
        "m3/admission/folds.csv",
        "m4/admission/admission.json",
        "m4/admission/admitted_patients.csv",
        "m4/admission/folds.csv",
        "m6/admission/admission.json",
        "m6/admission/admitted_patients.csv",
        "m6/admission/folds.csv",
    ):
        _copy_file(canonical / relative, staging, f"existing/{relative}")


def _cohort_test_summary(root: Path, model: str) -> dict[str, object]:
    energy = read_json(root / f"{model}_energy.json")
    mmd = read_json(root / f"{model}_mmd.json")
    return {
        "population_unit": "patient",
        "energy_distance": energy["energy_distance"],
        "energy_p_value": energy["p_value"],
        "linear_unbiased_mmd_squared": mmd["mmd_squared"],
        "linear_mmd_p_value": mmd["p_value"],
        "permutations": energy["permutations"]["completed"],
    }


def build(arguments: argparse.Namespace) -> None:
    canonical = arguments.canonical.resolve()
    graph = arguments.graph.resolve()
    outcome = arguments.outcome.resolve()
    output = arguments.out.resolve()
    if output.exists() or output.is_symlink():
        raise BundleError(f"output already exists: {output}")
    canonical_summary = read_json(canonical / "bundle" / "scientific_summary.json")
    graph_summary = read_json(graph / "patient-analysis-v1" / "summary.json")
    outcome_summary = read_json(outcome / "scientific_summary.json")
    replay = read_json(graph / "graph-topology-execution-manifest-v1.json")
    if (
        canonical_summary.get("population_unit") != "patient"
        or graph_summary.get("population_unit") != "patient"
        or outcome_summary.get("population_unit") != "patient"
        or not replay.get("all_result_replay_bytes_equal")
        or not replay.get("all_replays_backend_execution_disabled")
    ):
        raise BundleError("source population unit or durable replay proof differs")
    graph_interpretation = graph_topology_interpretation(graph_summary)
    cohort_tests = {
        model: _cohort_test_summary(graph / "cohort-tests-v1", model)
        for model in (
            "graph_only",
            "topology_only",
            "m0_m3_nonspatial",
            "m0_m3_graph",
            "m0_m3_topology",
            "final_fused_admitted_blocks",
        )
    }
    canonical_unavailable = read_json(canonical / "bundle" / "unavailable_lanes.json")[
        "lanes"
    ]
    unavailable = [
        *canonical_unavailable,
        {
            "lane": "CPTAC graph/topology acquisition-site-held-out validation",
            "blocker": "the admitted CPTAC case/slide manifest has no acquisition-site field; patient-held-out and single-cohort leakage checks completed without inventing site identities",
        },
        {
            "lane": "graph/topology addition to the fused fingerprint",
            "blocker": "both blocks failed their frozen individual stability and strictly-positive held-out incremental-information gates",
        },
    ]
    m1 = canonical_summary["tcga_discovery_and_internal_validation"]["m1"]
    m6 = canonical_summary["tcga_discovery_and_internal_validation"]["m6"]
    m7 = canonical_summary["m7_bayesian"]
    codex = canonical_summary["external_validation"]["schurch_codex"]
    schurch_he = canonical_summary["external_validation"]["schurch_he_cellvit"]
    interpretation = {
        "schema_name": "marklab_crc_final_scientific_interpretation",
        "schema_version": "1.0",
        "objective": "SCIENCE-CRC-FINAL-01",
        "scientific_question": "whether CRC tumors contain reproducible spatial microenvironments and whether patient-level spatial fingerprints recur within molecular or tumor classes",
        "population_unit": "patient",
        "interpretation_policy": "positive_null_confounded_unstable_and_unavailable_results_reported_without_significance_optimization",
        "supported": [
            {
                "finding": "classifier-derived short-range tumor organization is descriptively present across admitted CRC H&E cohorts",
                "evidence": {
                    "tcga_positive_patients": outcome_summary["relationship_summaries"]["TCGA_CRC"]["tumor_tumor"]["short_range_magnitude"]["positive_count"],
                    "tcga_patients": outcome_summary["relationship_summaries"]["TCGA_CRC"]["tumor_tumor"]["short_range_magnitude"]["count"],
                    "schurch_he_positive_patients": outcome_summary["relationship_summaries"]["Schurch_CRC"]["he_tumor_tumor_organization"]["positive_count"],
                    "schurch_he_patients": outcome_summary["relationship_summaries"]["Schurch_CRC"]["he_tumor_tumor_organization"]["count"],
                    "cptac_positive_patients": outcome_summary["relationship_summaries"]["CPTAC_CRC"]["tumor_tumor_organization"]["positive_count"],
                    "cptac_patients": outcome_summary["relationship_summaries"]["CPTAC_CRC"]["tumor_tumor_organization"]["count"],
                },
                "scope": "descriptive organization, not proof of a transferable molecular-class fingerprint",
            },
            {
                "finding": "the independent Schuerch H&E CellViT cohort reproduced the frozen MSI-versus-MSS cosine-excess direction",
                "effect": schurch_he["molecular_primary"]["median_difference"],
                "interval_95": schurch_he["molecular_primary"]["median_difference_confidence_interval"],
                "rank_biserial": schurch_he["molecular_primary"]["rank_biserial"],
                "scope": schurch_he["role"],
            },
            {
                "finding": "TCGA annotation-neighborhood M1 increased the between-minus-within patient distance effect beyond M0",
                "effect": m1["between_minus_within_increment_vs_m0"],
                "qualification": "its held-out top-1 increment remained uncertain and this is internal rather than external validation",
            },
        ],
        "null": [
            {
                "finding": "the stability-selected TCGA M6 fingerprint did not establish molecular-class recurrence",
                "heldout_top1_increment": m6["top1_increment_vs_m0"],
                "energy": m6["patient_label_energy"],
                "linear_mmd": m6["patient_label_linear_mmd"],
            },
            {
                "finding": "new CPTAC graph and witness-topology blocks added no held-out balanced-accuracy increment beyond composition, technical covariates, and nonspatial CellViT embeddings",
                "graph_topology": graph_interpretation,
                "cohort_tests": cohort_tests,
            },
            {
                "finding": "the retained M7 Bayesian MSI-minus-MSS interval includes zero",
                "effect": m7["marginal_msi_minus_mss_probability_difference"],
                "diagnostics": m7["diagnostics"],
            },
            {
                "finding": "Schuerch CODEX neighborhoods did not reproduce an MSI retrieval or distributional separation",
                "balanced_top1_accuracy": codex["balanced_top1_accuracy"],
                "top1_recall_by_group": codex["top1_recall_by_group"],
                "energy": codex["energy"],
                "linear_mmd": codex["linear_mmd"],
            },
        ],
        "unstable": [
            {
                "finding": "graph scattering was robust to cell subsampling, 1-um coordinate perturbation, and nearby radii but failed the two-slide patient stability gate",
                "stability": read_json(graph / "patient-analysis-v1" / "stability.json")["graph"],
            },
            {
                "finding": "witness topology failed coordinate, lower-tail cell-subsample, nearby-scale, and two-slide stability checks",
                "stability": read_json(graph / "patient-analysis-v1" / "stability.json")["topology"],
            },
            {
                "finding": "the existing M2 graph-grid patient ranks were unstable across 2x2 and 3x3 grids",
                "stability": canonical_summary["stability"]["m2_coordinate"]["graph_grid_summary"],
            },
        ],
        "confounded": [
            "CellViT hard classes and embeddings are model outputs and may retain slide, acquisition, and tissue-composition effects",
            "the eight-patient CPTAC graph/topology subset is exploratory and has no acquisition-site variable for site-held-out validation",
            "the Schuerch H&E cosine-excess score is not the 67-feature TCGA M6 vector, so it is direction validation rather than direct full-fingerprint transfer",
            "repeatability distances can exceed within-patient distances while leave-one-specimen-out patient retrieval remains low; neither alone establishes a recurrent molecular-class fingerprint",
        ],
        "unavailable": unavailable,
        "overall_conclusion": "CRC H&E data support descriptive, recurring local tumor spatial organization and one independent direction-level MSI/MSS H&E association, but the prespecified patient-level analyses do not establish a stable transferable molecular-class spatial fingerprint. Graph and witness-topology additions were excluded because they were nonincremental and unstable across nested slides; M6, CODEX class recurrence, and M7 remain null-compatible.",
        "claim_limitations": [
            "retrospective exploratory cohorts only",
            "no clinical utility, causal, prospective, calibration, equivalence, or performance claim",
            "slides, ROIs, fields, cells, patches, and edges were never independent patient replicates",
            "CODEX protein/neighborhood features and CellViT embeddings were never treated as a shared raw feature space",
        ],
        "durable_replay": {
            "graph_unique_executions": replay["graph_unique_executions"],
            "topology_unique_project_executions": replay[
                "topology_unique_project_executions"
            ],
            "topology_external_backend_executions": replay[
                "topology_external_backend_executions"
            ],
            "retrieval_unique_executions": replay["retrieval_unique_executions"],
            "all_result_replay_bytes_equal": replay["all_result_replay_bytes_equal"],
            "all_replays_backend_execution_disabled": replay[
                "all_replays_backend_execution_disabled"
            ],
            "m7_durable_miss_hit_sha256": m7["durable_miss_hit_sha256"],
        },
    }
    staging = output.with_name(f".{output.name}.{os.getpid()}.tmp")
    if staging.exists() or staging.is_symlink():
        raise BundleError(f"staging path already exists: {staging}")
    staging.mkdir(parents=True)
    _copy_existing_science(canonical, staging)
    _copy_tree(outcome, staging, "outcome")
    for directory in (
        "patient-analysis-v1",
        "cohort-tests-v1",
        "retrieval-results-v1",
        "retrieval-replay-v1",
        "project-ledgers-v1",
        "provenance",
    ):
        _copy_tree(graph / directory, staging, f"graph_topology/{directory}")
    for relative in (
        "graph-topology-execution-manifest-v1.json",
        "prepared-design.json",
        "prepared-manifest.csv",
        "nonspatial-provenance.json",
        "nonspatial-manifest.csv",
    ):
        _copy_file(graph / relative, staging, f"graph_topology/{relative}")
    write_json(staging / "scientific_interpretation.json", interpretation)
    files = sorted(path for path in staging.rglob("*") if path.is_file())
    manifest = {
        "schema_name": "marklab_crc_final_scientific_bundle",
        "schema_version": "1.0",
        "objective": "SCIENCE-CRC-FINAL-01",
        "population_unit": "patient",
        "result_format_compatibility": "0.3_preserved",
        "artifact_count": len(files),
        "artifact_sha256": {
            path.relative_to(staging).as_posix(): sha256(path) for path in files
        },
    }
    write_json(staging / "manifest.json", manifest)
    os.rename(staging, output)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--canonical", required=True, type=Path)
    parser.add_argument("--graph", required=True, type=Path)
    parser.add_argument("--outcome", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    return parser.parse_args()


if __name__ == "__main__":
    build(parse_args())
