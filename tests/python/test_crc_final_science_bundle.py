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


if __name__ == "__main__":
    unittest.main()
