#!/usr/bin/env python3
"""Seal the final canonical CRC scientific result bundle."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
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


def witness_bottleneck_addendum(root: Path) -> dict[str, object]:
    miss_path = root / "durable_miss.json"
    hit_path = root / "durable_hit.json"
    if not miss_path.is_file() or not hit_path.is_file():
        raise BundleError("witness bottleneck durable miss or hit is absent")
    miss_bytes = miss_path.read_bytes()
    if miss_bytes != hit_path.read_bytes():
        raise BundleError("witness bottleneck durable replay bytes differ")
    ledger_path = root / "project" / "executions.jsonl"
    if not ledger_path.is_file():
        raise BundleError("witness bottleneck durable ledger is absent")
    ledger_count = sum(
        bool(line.strip()) for line in ledger_path.read_text(encoding="utf-8").splitlines()
    )
    if ledger_count != 1:
        raise BundleError("witness bottleneck durable ledger must contain exactly one execution")
    results_path = root / "RESULTS.md"
    if (
        not results_path.is_file()
        or "MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION=1"
        not in results_path.read_text(encoding="utf-8")
    ):
        raise BundleError("witness bottleneck backend-disabled replay evidence is absent")
    result = json.loads(miss_bytes)
    if (
        result.get("format") != "marklab.witness_persistence_bottleneck_stability"
        or result.get("version") != 1
        or not isinstance(result.get("stable_under_bottleneck_threshold"), bool)
        or not isinstance(result.get("stable_under_all_declared_thresholds"), bool)
        or not isinstance(result.get("has_infinite_essential_mismatch"), bool)
    ):
        raise BundleError("witness bottleneck result identity or stability state differs")
    return {
        "statistical_unit": "one_specimen_point_pattern",
        "role": "supplemental_correctness_and_coordinate_perturbation_diagnostic",
        "maximum_bottleneck_distance_um_squared_allowed": result[
            "maximum_bottleneck_distance_um_squared_allowed"
        ],
        "maximum_finite_bottleneck_distance_um_squared": result[
            "maximum_finite_bottleneck_distance_um_squared"
        ],
        "has_infinite_essential_mismatch": result[
            "has_infinite_essential_mismatch"
        ],
        "stable_under_bottleneck_threshold": result[
            "stable_under_bottleneck_threshold"
        ],
        "stable_under_all_declared_thresholds": result[
            "stable_under_all_declared_thresholds"
        ],
        "bottleneck_comparisons": result["bottleneck_comparisons"],
        "bottleneck_interval_count": result["bottleneck_interval_count"],
        "total_backend_executions_on_miss": result["total_backend_executions"],
        "durable_replay": {
            "result_bytes_equal": True,
            "ledger_execution_count": ledger_count,
            "result_sha256": hashlib.sha256(miss_bytes).hexdigest(),
            "backend_execution_disabled_on_hit": True,
        },
        "claim_limitation": "one-specimen diagnostic; not a patient replicate or fused fingerprint component",
    }


def _csv_rows(path: Path) -> list[dict[str, str]]:
    if not path.is_file() or path.is_symlink():
        raise BundleError(f"required regular artifact is absent: {path}")
    with path.open(newline="", encoding="utf-8") as source:
        rows = list(csv.DictReader(source))
    if not rows:
        raise BundleError(f"required table is empty: {path}")
    return rows


def _relative_regular_file(root: Path, relative: object, role: str) -> Path:
    if not isinstance(relative, str) or not relative:
        raise BundleError(f"patient witness {role} path is absent")
    relative_path = Path(relative)
    if relative_path.is_absolute() or ".." in relative_path.parts:
        raise BundleError(f"patient witness {role} path escapes its source root")
    path = root / relative_path
    if not path.is_file() or path.is_symlink():
        raise BundleError(f"patient witness {role} artifact is absent")
    return path


def patient_witness_bottleneck_addendum(root: Path) -> dict[str, object]:
    """Revalidate the sealed patient-unit exact-bottleneck workflow."""
    prepared = root / "prepared"
    executed = root / "executed"
    summary = read_json(root / "summary" / "summary.json")
    execution = read_json(executed / "execution_manifest.json")
    design = read_json(prepared / "design.json")
    manifest_rows = _csv_rows(prepared / "manifest.csv")
    patient_rows = _csv_rows(root / "summary" / "patient_results.csv")
    if (
        design.get("population_unit") != "patient"
        or design.get("pattern_unit") != "slide_nested_within_patient"
        or design.get("patterns_per_patient") != 2
        or design.get("selection_uses_molecular_label") is not False
        or summary.get("schema_name")
        != "marklab_crc_witness_bottleneck_patient_summary"
        or summary.get("schema_version") != "1.0"
        or summary.get("population_unit") != "patient"
        or summary.get("pattern_unit") != "slide_nested_within_patient"
        or execution.get("schema_name")
        != "marklab_crc_witness_bottleneck_patient_execution"
        or execution.get("schema_version") != "1.0"
        or execution.get("population_unit") != "patient"
    ):
        raise BundleError("patient witness design, summary, or execution identity differs")

    manifest: dict[str, tuple[str, str]] = {}
    patterns_by_patient: dict[str, list[str]] = {}
    manifest_groups: dict[str, str] = {}
    for row in manifest_rows:
        pattern = row.get("pattern_id", "")
        patient = row.get("patient_id", "")
        group = row.get("group", "")
        if (
            not pattern
            or Path(pattern).name != pattern
            or not patient
            or not group
            or pattern in manifest
            or row.get("cell_count") != "512"
        ):
            raise BundleError("patient witness manifest identity is absent or duplicated")
        if patient in manifest_groups and manifest_groups[patient] != group:
            raise BundleError("patient witness manifest has conflicting patient groups")
        manifest_groups[patient] = group
        _relative_regular_file(prepared, row.get("request"), "request")
        manifest[pattern] = (patient, group)
        patterns_by_patient.setdefault(patient, []).append(pattern)
    if any(len(patterns) != 2 for patterns in patterns_by_patient.values()):
        raise BundleError("patient witness manifest must contain two slides per patient")

    patient_identity: dict[str, str] = {}
    for row in patient_rows:
        patient = row.get("patient_id", "")
        group = row.get("group", "")
        if not patient or not group or patient in patient_identity:
            raise BundleError("patient witness summary identity is absent or duplicated")
        patient_identity[patient] = group
        if row.get("pattern_count") != "2":
            raise BundleError("patient witness summary must retain two nested slides")
    if patient_identity != manifest_groups:
        raise BundleError("patient witness manifest and summary identities differ")
    group_counts = {
        group: sum(patient_group == group for patient_group in patient_identity.values())
        for group in set(patient_identity.values())
    }
    if set(group_counts) != {"MSI", "MSS"} or min(group_counts.values()) < 2:
        raise BundleError("patient witness molecular groups differ")

    executions = execution.get("executions")
    if not isinstance(executions, list) or len(executions) != len(manifest):
        raise BundleError("patient witness execution count differs from its manifest")
    seen: set[str] = set()
    total_backends = 0
    for row in executions:
        if not isinstance(row, dict):
            raise BundleError("patient witness execution row is malformed")
        pattern = row.get("pattern_id")
        if not isinstance(pattern, str) or pattern not in manifest or pattern in seen:
            raise BundleError("patient witness execution identity differs")
        seen.add(pattern)
        patient, group = manifest[pattern]
        if row.get("patient_id") != patient or row.get("group") != group:
            raise BundleError("patient witness execution patient identity differs")
        miss = _relative_regular_file(executed, row.get("miss"), "miss")
        hit = _relative_regular_file(executed, row.get("hit"), "hit")
        miss_bytes = miss.read_bytes()
        if miss_bytes != hit.read_bytes():
            raise BundleError(f"patient witness replay bytes differ for {pattern}")
        digest = hashlib.sha256(miss_bytes).hexdigest()
        if (
            row.get("miss_sha256") != digest
            or row.get("hit_sha256") != digest
            or row.get("result_bytes_equal") is not True
            or row.get("ledger_execution_count") != 1
        ):
            raise BundleError("patient witness replay receipt differs from artifacts")
        ledger = executed / "projects" / pattern / "executions.jsonl"
        if not ledger.is_file() or ledger.is_symlink():
            raise BundleError(f"patient witness durable ledger is absent for {pattern}")
        ledger_count = sum(
            bool(line.strip())
            for line in ledger.read_text(encoding="utf-8").splitlines()
        )
        if ledger_count != 1:
            raise BundleError("patient witness durable ledger must contain exactly one execution")
        backend_count = row.get("total_backend_executions_on_miss")
        if not isinstance(backend_count, int) or backend_count < 1:
            raise BundleError("patient witness backend execution count is invalid")
        total_backends += backend_count

    patient_count = len(patient_identity)
    pattern_count = len(manifest)
    if (
        set(manifest) != seen
        or design.get("patient_count") != patient_count
        or design.get("pattern_count") != pattern_count
        or summary.get("patient_count") != patient_count
        or summary.get("pattern_count") != pattern_count
        or execution.get("pattern_count") != pattern_count
        or execution.get("miss_count") != pattern_count
        or execution.get("backend_disabled_hit_count") != pattern_count
        or execution.get("all_result_bytes_equal") is not True
        or execution.get("all_ledgers_one_execution") is not True
        or execution.get("total_backend_executions_on_misses") != total_backends
    ):
        raise BundleError("patient witness counts or durable replay proof differ")

    group_comparison = summary.get("group_comparison")
    replay_summary = summary.get("durable_replay")
    interval = (
        group_comparison.get("whole_patient_bootstrap_interval_95")
        if isinstance(group_comparison, dict)
        else None
    )
    finite_values = (
        summary.get("maximum_patient_finite_bottleneck_distance_um_squared"),
        summary.get("maximum_bottleneck_distance_um_squared_allowed"),
        group_comparison.get("mean_difference_msi_minus_mss")
        if isinstance(group_comparison, dict)
        else None,
        *(interval if isinstance(interval, list) else []),
    )
    p_value = (
        group_comparison.get("exact_two_sided_p_value")
        if isinstance(group_comparison, dict)
        else None
    )
    assignment_count = (
        group_comparison.get("exact_assignment_count")
        if isinstance(group_comparison, dict)
        else None
    )
    if (
        not isinstance(group_comparison, dict)
        or not isinstance(interval, list)
        or len(interval) != 2
        or len(finite_values) != 5
        or any(
            isinstance(value, bool)
            or not isinstance(value, (int, float))
            or not math.isfinite(value)
            for value in finite_values
        )
        or finite_values[0] < 0.0
        or finite_values[1] <= 0.0
        or interval[0] > interval[1]
        or summary.get("stable_patient_count") != 0
        or summary.get("all_patients_stable") is not False
        or summary.get("patients_with_infinite_essential_mismatch") != patient_count
        or summary.get("promotion_status") != "unstable_not_promoted"
        or summary.get("fusion_status") != "not_added_without_prespecified_stability"
        or assignment_count != math.comb(patient_count, group_counts["MSI"])
        or isinstance(p_value, bool)
        or not isinstance(p_value, (int, float))
        or not math.isfinite(p_value)
        or not 0.0 <= p_value <= 1.0
        or not isinstance(replay_summary, dict)
        or replay_summary.get("miss_count") != pattern_count
        or replay_summary.get("backend_disabled_hit_count") != pattern_count
        or replay_summary.get("all_result_bytes_equal") is not True
        or replay_summary.get("all_ledgers_one_execution") is not True
    ):
        raise BundleError("patient witness unstable result or uncertainty differs")

    return {
        "statistical_unit": "patient",
        "pattern_unit": "slide_nested_within_patient",
        "patient_count": patient_count,
        "pattern_count": pattern_count,
        "stable_patient_count": 0,
        "patients_with_infinite_essential_mismatch": patient_count,
        "maximum_bottleneck_distance_um_squared_allowed": summary[
            "maximum_bottleneck_distance_um_squared_allowed"
        ],
        "maximum_patient_finite_bottleneck_distance_um_squared": summary[
            "maximum_patient_finite_bottleneck_distance_um_squared"
        ],
        "group_comparison": group_comparison,
        "promotion_status": summary["promotion_status"],
        "fusion_status": summary["fusion_status"],
        "durable_replay": {
            "verified_miss_count": pattern_count,
            "verified_hit_count": pattern_count,
            "all_result_bytes_equal": True,
            "all_ledgers_one_execution": True,
            "backend_execution_disabled_on_hits": True,
            "total_backend_executions_on_misses": total_backends,
        },
        "claim_limitation": summary["claim_limitation"],
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
    witness_bottleneck = (
        arguments.witness_bottleneck.resolve()
        if arguments.witness_bottleneck is not None
        else None
    )
    patient_witness_bottleneck = (
        arguments.patient_witness_bottleneck.resolve()
        if arguments.patient_witness_bottleneck is not None
        else None
    )
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
    bottleneck_addendum = (
        witness_bottleneck_addendum(witness_bottleneck)
        if witness_bottleneck is not None
        else None
    )
    patient_bottleneck_addendum = (
        patient_witness_bottleneck_addendum(patient_witness_bottleneck)
        if patient_witness_bottleneck is not None
        else None
    )
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
    if bottleneck_addendum is not None:
        interpretation["unstable"].append(
            {
                "finding": "exact witness-diagram bottleneck distance independently confirms one-specimen coordinate instability and is not promoted into the patient fingerprint",
                "stability": bottleneck_addendum,
            }
        )
        interpretation["durable_replay"]["witness_bottleneck"] = bottleneck_addendum[
            "durable_replay"
        ]
    if patient_bottleneck_addendum is not None:
        interpretation["unstable"].append(
            {
                "finding": "patient-replicated exact witness-diagram bottleneck stability failed in every admitted patient and is not promoted into the fused fingerprint",
                "stability": patient_bottleneck_addendum,
            }
        )
        interpretation["null"].append(
            {
                "finding": "patient maximum finite witness-bottleneck distance did not establish MSI-versus-MSS separation",
                "effect_and_uncertainty": patient_bottleneck_addendum[
                    "group_comparison"
                ],
            }
        )
        interpretation["durable_replay"]["patient_witness_bottleneck"] = (
            patient_bottleneck_addendum["durable_replay"]
        )
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
    if witness_bottleneck is not None:
        _copy_tree(
            witness_bottleneck,
            staging,
            "graph_topology/witness_bottleneck",
        )
    if patient_witness_bottleneck is not None:
        _copy_tree(
            patient_witness_bottleneck,
            staging,
            "graph_topology/patient_witness_bottleneck",
        )
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
    parser.add_argument("--witness-bottleneck", type=Path)
    parser.add_argument("--patient-witness-bottleneck", type=Path)
    parser.add_argument("--out", required=True, type=Path)
    return parser.parse_args()


if __name__ == "__main__":
    build(parse_args())
