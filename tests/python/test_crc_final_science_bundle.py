import importlib.util
from pathlib import Path
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


if __name__ == "__main__":
    unittest.main()
