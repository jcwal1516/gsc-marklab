import csv
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_crc_final_science_bundle.py"
)


def load_module():
    spec = importlib.util.spec_from_file_location("crc_final_science_bundle", MODULE_PATH)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class CrcFinalScienceBundleTest(unittest.TestCase):
    def test_graph_topology_interpretation_preserves_failed_fusion_gates(self):
        module = load_module()
        summary = {
            "incremental_information": {
                "graph": {"balanced_accuracy_increment": 0.0},
                "topology": {"balanced_accuracy_increment": -0.125},
            },
            "fusion": {
                "graph": {"eligible": False, "failed_checks": ["specimen_leave_one_out_q10"]},
                "topology": {
                    "eligible": False,
                    "failed_checks": ["coordinate_perturbation_median"],
                },
                "new_blocks_included": [],
            },
        }

        result = module.graph_topology_interpretation(summary)

        self.assertEqual(result["status"], "unstable_and_nonincremental")
        self.assertEqual(result["new_blocks_included"], [])
        self.assertEqual(result["graph_balanced_accuracy_increment"], 0.0)
        self.assertEqual(result["topology_balanced_accuracy_increment"], -0.125)

    def test_witness_bottleneck_addendum_requires_one_execution_and_equal_replay(self):
        module = load_module()
        result = {
            "format": "marklab.witness_persistence_bottleneck_stability",
            "version": 1,
            "maximum_bottleneck_distance_um_squared_allowed": 600.0,
            "maximum_finite_bottleneck_distance_um_squared": 7131.0,
            "has_infinite_essential_mismatch": True,
            "stable_under_bottleneck_threshold": False,
            "stable_under_all_declared_thresholds": False,
            "bottleneck_comparisons": 48,
            "bottleneck_interval_count": 7660,
            "total_backend_executions": 18,
        }
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            encoded = json.dumps(result, sort_keys=True).encode()
            (root / "durable_miss.json").write_bytes(encoded)
            (root / "durable_hit.json").write_bytes(encoded)
            (root / "RESULTS.md").write_text(
                "Replay used `MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION=1`.\n",
                encoding="utf-8",
            )
            (root / "project").mkdir()
            (root / "project" / "executions.jsonl").write_text("{}\n", encoding="utf-8")

            addendum = module.witness_bottleneck_addendum(root)

            self.assertEqual(addendum["maximum_finite_bottleneck_distance_um_squared"], 7131.0)
            self.assertTrue(addendum["has_infinite_essential_mismatch"])
            self.assertTrue(addendum["durable_replay"]["result_bytes_equal"])
            self.assertEqual(addendum["durable_replay"]["ledger_execution_count"], 1)
            (root / "project" / "executions.jsonl").write_text("{}\n{}\n", encoding="utf-8")
            with self.assertRaisesRegex(module.BundleError, "exactly one execution"):
                module.witness_bottleneck_addendum(root)

    def test_patient_witness_addendum_revalidates_patient_replay_and_unstable_result(self):
        module = load_module()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "prepared").mkdir()
            (root / "executed" / "results").mkdir(parents=True)
            (root / "executed" / "projects").mkdir()
            (root / "summary").mkdir()
            manifest_rows = []
            patient_rows = []
            executions = []
            for patient_index in range(4):
                patient = f"P{patient_index + 1}"
                group = "MSI" if patient_index < 2 else "MSS"
                patient_rows.append(
                    {
                        "patient_id": patient,
                        "group": group,
                        "pattern_count": 2,
                        "maximum_finite_bottleneck_distance_um_squared": 700.0
                        + patient_index,
                        "has_infinite_essential_mismatch": True,
                        "stable_across_both_patterns": False,
                    }
                )
                for slide_index in range(2):
                    pattern = f"{patient}-S{slide_index + 1}"
                    manifest_rows.append(
                        {
                            "pattern_id": pattern,
                            "patient_id": patient,
                            "group": group,
                            "cell_count": 512,
                            "request": f"requests/{pattern}/request.json",
                        }
                    )
                    request_root = root / "prepared" / "requests" / pattern
                    request_root.mkdir(parents=True)
                    (request_root / "request.json").write_text(
                        "{}\n", encoding="utf-8"
                    )
                    result = (
                        json.dumps({"pattern_id": pattern}, sort_keys=True) + "\n"
                    ).encode()
                    result_root = root / "executed" / "results" / pattern
                    result_root.mkdir()
                    (result_root / "miss.json").write_bytes(result)
                    (result_root / "hit.json").write_bytes(result)
                    project_root = root / "executed" / "projects" / pattern
                    project_root.mkdir()
                    (project_root / "executions.jsonl").write_text(
                        "{}\n", encoding="utf-8"
                    )
                    digest = hashlib.sha256(result).hexdigest()
                    executions.append(
                        {
                            "pattern_id": pattern,
                            "patient_id": patient,
                            "group": group,
                            "miss": f"results/{pattern}/miss.json",
                            "hit": f"results/{pattern}/hit.json",
                            "miss_sha256": digest,
                            "hit_sha256": digest,
                            "result_bytes_equal": True,
                            "ledger_execution_count": 1,
                            "total_backend_executions_on_miss": 6,
                        }
                    )
            for path, rows in (
                (root / "prepared" / "manifest.csv", manifest_rows),
                (root / "summary" / "patient_results.csv", patient_rows),
            ):
                with path.open("w", newline="", encoding="utf-8") as target:
                    writer = csv.DictWriter(target, fieldnames=list(rows[0]))
                    writer.writeheader()
                    writer.writerows(rows)
            (root / "prepared" / "design.json").write_text(
                json.dumps(
                    {
                        "population_unit": "patient",
                        "pattern_unit": "slide_nested_within_patient",
                        "patient_count": 4,
                        "pattern_count": 8,
                        "patterns_per_patient": 2,
                        "selection_uses_molecular_label": False,
                    },
                    sort_keys=True,
                ),
                encoding="utf-8",
            )
            (root / "summary" / "summary.json").write_text(
                json.dumps(
                    {
                        "schema_name": "marklab_crc_witness_bottleneck_patient_summary",
                        "schema_version": "1.0",
                        "population_unit": "patient",
                        "pattern_unit": "slide_nested_within_patient",
                        "patient_count": 4,
                        "pattern_count": 8,
                        "stable_patient_count": 0,
                        "stable_patient_fraction": 0.0,
                        "all_patients_stable": False,
                        "patients_with_infinite_essential_mismatch": 4,
                        "maximum_patient_finite_bottleneck_distance_um_squared": 703.0,
                        "maximum_bottleneck_distance_um_squared_allowed": 600.0,
                        "group_comparison": {
                            "mean_difference_msi_minus_mss": -2.0,
                            "whole_patient_bootstrap_interval_95": [-4.0, 1.0],
                            "exact_assignment_count": 6,
                            "exact_two_sided_p_value": 0.5,
                        },
                        "promotion_status": "unstable_not_promoted",
                        "fusion_status": "not_added_without_prespecified_stability",
                        "claim_limitation": "slides remain nested inside patients",
                        "durable_replay": {
                            "miss_count": 8,
                            "backend_disabled_hit_count": 8,
                            "all_result_bytes_equal": True,
                            "all_ledgers_one_execution": True,
                        },
                    },
                    sort_keys=True,
                ),
                encoding="utf-8",
            )
            (root / "executed" / "execution_manifest.json").write_text(
                json.dumps(
                    {
                        "schema_name": "marklab_crc_witness_bottleneck_patient_execution",
                        "schema_version": "1.0",
                        "population_unit": "patient",
                        "pattern_count": 8,
                        "miss_count": 8,
                        "backend_disabled_hit_count": 8,
                        "all_result_bytes_equal": True,
                        "all_ledgers_one_execution": True,
                        "total_backend_executions_on_misses": 48,
                        "maximum_processes": 2,
                        "executions": executions,
                    },
                    sort_keys=True,
                ),
                encoding="utf-8",
            )

            addendum = module.patient_witness_bottleneck_addendum(root)

            self.assertEqual(addendum["patient_count"], 4)
            self.assertEqual(addendum["pattern_count"], 8)
            self.assertEqual(addendum["stable_patient_count"], 0)
            self.assertEqual(addendum["durable_replay"]["verified_hit_count"], 8)
            self.assertEqual(addendum["group_comparison"]["exact_assignment_count"], 6)
            corrupted_hit = root / "executed" / "results" / "P1-S1" / "hit.json"
            corrupted_hit.write_text("{}\n", encoding="utf-8")
            with self.assertRaisesRegex(module.BundleError, "replay bytes differ"):
                module.patient_witness_bottleneck_addendum(root)

    def test_categorical_pair_addendum_revalidates_patient_replay_and_nonpromotion(self):
        module = load_module()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "prepared").mkdir()
            (root / "execution" / "results").mkdir(parents=True)
            (root / "execution" / "replay").mkdir()
            (root / "execution" / "projects").mkdir()
            (root / "summary").mkdir()
            manifest_rows = []
            patient_rows = []
            miss_records = []
            hit_records = []
            pairs = (
                ("Connective", "Neoplastic"),
                ("Inflammatory", "Neoplastic"),
                ("Neoplastic", "Connective"),
                ("Neoplastic", "Inflammatory"),
            )
            for patient_index in range(8):
                patient = f"P{patient_index + 1}"
                group = "MSI" if patient_index < 4 else "MSS"
                patient_rows.append(
                    {
                        "cohort": "CPTAC_COAD_CellViT",
                        "patient_id": patient,
                        "group": group,
                        "feature": "pair.r20um",
                        "value": float(patient_index),
                    }
                )
                for slide_index in range(2):
                    pattern = f"{patient}-S{slide_index + 1}"
                    pattern_root = root / "prepared" / "patterns" / pattern
                    pattern_root.mkdir(parents=True)
                    cells = pattern_root / "cells.csv"
                    window = pattern_root / "window.geojson"
                    cells.write_text(
                        "x_um,y_um,histologic_compartment\n0,0,Neoplastic\n",
                        encoding="utf-8",
                    )
                    window.write_text("{}\n", encoding="utf-8")
                    manifest_rows.append(
                        {
                            "pattern_id": pattern,
                            "patient_id": patient,
                            "group": group,
                            "cell_count": 512,
                            "cells": f"patterns/{pattern}/cells.csv",
                            "window": f"patterns/{pattern}/window.geojson",
                            "source_payload_sha256": "0" * 64,
                            "prepared_cells_sha256": module.sha256(cells),
                            "prepared_window_sha256": module.sha256(window),
                        }
                    )
                    for source, target in pairs:
                        pair = f"{source.lower()}-to-{target.lower()}"
                        miss = (
                            root
                            / "execution"
                            / "results"
                            / pair
                            / f"{pattern}.json"
                        )
                        hit = (
                            root
                            / "execution"
                            / "replay"
                            / pair
                            / f"{pattern}.json"
                        )
                        miss.parent.mkdir(parents=True, exist_ok=True)
                        hit.parent.mkdir(parents=True, exist_ok=True)
                        payload = (
                            json.dumps(
                                {
                                    "format": "marklab.categorical-pair/1",
                                    "pattern_id": pattern,
                                    "pair": pair,
                                    "source_level": source,
                                    "target_level": target,
                                },
                                sort_keys=True,
                            )
                            + "\n"
                        ).encode()
                        miss.write_bytes(payload)
                        hit.write_bytes(payload)
                        ledger = root / "execution" / "projects" / pattern / pair
                        ledger.mkdir(parents=True)
                        (ledger / "executions.jsonl").write_text(
                            "{}\n", encoding="utf-8"
                        )
                        digest = hashlib.sha256(payload).hexdigest()
                        common = {
                            "pattern_id": pattern,
                            "source_level": source,
                            "target_level": target,
                            "result_sha256": digest,
                            "replay_bytes_equal": True,
                            "ledger_execution_count": 1,
                        }
                        miss_records.append(
                            {
                                **common,
                                "cache_status": "miss",
                                "result": f"results/{pair}/{pattern}.json",
                            }
                        )
                        hit_records.append(
                            {
                                **common,
                                "cache_status": "hit",
                                "result": f"replay/{pair}/{pattern}.json",
                            }
                        )
            with (root / "prepared" / "manifest.csv").open(
                "w", newline="", encoding="utf-8"
            ) as target:
                writer = csv.DictWriter(target, fieldnames=list(manifest_rows[0]))
                writer.writeheader()
                writer.writerows(manifest_rows)
            with (root / "summary" / "patient_fingerprints.csv").open(
                "w", newline="", encoding="utf-8"
            ) as target:
                writer = csv.DictWriter(target, fieldnames=list(patient_rows[0]))
                writer.writeheader()
                writer.writerows(patient_rows)
            (root / "prepared" / "design.json").write_text(
                json.dumps(
                    {
                        "population_unit": "patient",
                        "pattern_unit": "slide_nested_within_patient",
                        "patient_count": 8,
                        "pattern_count": 16,
                        "patterns_per_patient": 2,
                        "selection_uses_molecular_label": False,
                    }
                ),
                encoding="utf-8",
            )
            summary = {
                "schema_name": "marklab_crc_categorical_pair_patient_summary",
                "schema_version": "1.0",
                "population_unit": "patient",
                "pattern_unit": "slide_nested_within_patient",
                "patient_count": 8,
                "pattern_count": 16,
                "declared_endpoint_count": 32,
                "admitted_endpoint_count": 21,
                "nested_slide_stability": {"median": 0.76, "q10": 0.43},
                "models": {
                    "categorical_pair_only": {
                        "balanced_accuracy": 0.25,
                        "whole_patient_permutation": {
                            "assignment_count": 70,
                            "p_value_inclusive_exact": 0.88,
                        },
                    },
                    "m0_m3_nonspatial": {"balanced_accuracy": 0.625},
                    "m0_m3_categorical_pair": {"balanced_accuracy": 0.375},
                },
                "incremental_information": {
                    "balanced_accuracy_increment": -0.25,
                    "whole_patient_bootstrap_interval_95": [-0.625, 0.25],
                },
                "population_max_t": {
                    "endpoints": [
                        {"adjusted_p_value": 0.44 + 0.01 * index}
                        for index in range(21)
                    ]
                },
                "durable_replay": {
                    "miss_count": 64,
                    "backend_disabled_hit_count": 64,
                    "all_result_bytes_equal": True,
                    "all_ledgers_one_execution": True,
                },
                "leakage_checks": {
                    "patient_held_out": True,
                    "preprocessing_inside_each_training_fold": True,
                    "slides_nested_inside_patients": True,
                    "site_held_out": "unavailable_exact_blocker_no_acquisition_site_field_in_admitted_CPTAC_manifest",
                },
                "claim_limitations": ["eight-patient resource-feasible exploratory subset"],
            }
            (root / "summary" / "summary.json").write_text(
                json.dumps(summary), encoding="utf-8"
            )
            (root / "RESULTS.md").write_text(
                "Replay used `MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION=1`.\n",
                encoding="utf-8",
            )
            for filename, replay, records, status in (
                ("execution_manifest.json", False, miss_records, "miss"),
                ("replay_manifest.json", True, hit_records, "hit"),
            ):
                (root / "execution" / filename).write_text(
                    json.dumps(
                        {
                            "schema_name": "marklab_crc_categorical_pair_patient_execution",
                            "schema_version": "1.0",
                            "population_unit": "patient",
                            "replay": replay,
                            "job_count": 64,
                            "maximum_processes": 6,
                            "cache_status_counts": {status: 64},
                            "all_replay_bytes_equal": True,
                            "all_ledgers_one_execution": True,
                            "binary_sha256": "1" * 64,
                            "records": records,
                        }
                    ),
                    encoding="utf-8",
                )

            addendum = module.categorical_pair_addendum(root)

            self.assertEqual(addendum["patient_count"], 8)
            self.assertEqual(addendum["durable_replay"]["verified_hit_count"], 64)
            self.assertEqual(
                addendum["incremental_information"]["balanced_accuracy_increment"],
                -0.25,
            )
            self.assertEqual(
                addendum["fusion_status"], "unstable_nonincremental_not_added"
            )
            corrupted_hit = (
                root
                / "execution"
                / "replay"
                / "connective-to-neoplastic"
                / "P1-S1.json"
            )
            corrupted_hit.write_text("{}\n", encoding="utf-8")
            with self.assertRaisesRegex(module.BundleError, "digest or ledger proof differs"):
                module.categorical_pair_addendum(root)


if __name__ == "__main__":
    unittest.main()
