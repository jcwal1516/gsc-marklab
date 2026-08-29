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


if __name__ == "__main__":
    unittest.main()
