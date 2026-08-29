import importlib.util
from pathlib import Path
import unittest


MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_crc_graph_topology_summary.py"
)


def load_module():
    spec = importlib.util.spec_from_file_location("crc_graph_topology_summary", MODULE_PATH)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class CrcGraphTopologySummaryTest(unittest.TestCase):
    def test_graph_features_normalize_extensive_energy_by_cells(self):
        module = load_module()
        document = {
            "format": "marklab.graph_sparse_radius_scattering",
            "node_count": 10,
            "edge_count": 5,
            "isolated_node_count": 2,
            "coarse_mean_absolute": 0.4,
            "coarse_energy": 20.0,
            "first_order": [
                {"level": level, "mean_absolute": 0.2, "energy": 5.0}
                for level in range(4)
            ],
            "second_order": [
                {
                    "first_level": first,
                    "second_level": second,
                    "mean_absolute": 0.1,
                    "energy": 2.0,
                }
                for first in range(4)
                for second in range(first + 1, 4)
            ],
        }

        features = module.graph_features(document)

        self.assertEqual(features["edge_density"], 5 / 45)
        self.assertEqual(features["isolated_fraction"], 0.2)
        self.assertEqual(features["coarse_energy_per_cell"], 2.0)
        self.assertEqual(features["first_0_energy_per_cell"], 0.5)
        self.assertEqual(features["second_0_1_energy_per_cell"], 0.2)

    def test_topology_features_separate_essential_classes_and_finite_persistence(self):
        module = load_module()
        document = {
            "format": "marklab.witness_persistence",
            "approximation": {"landmark_count": 4, "coverage_radius_um": 3.0},
            "filtration": {"simplex_counts_by_dimension": [4, 3, 1]},
            "persistence": {
                "by_dimension": [
                    {
                        "dimension": 0,
                        "finite_pairs": [{"birth": 0.0, "death": 2.0}],
                        "essential_births": [0.0],
                    },
                    {
                        "dimension": 1,
                        "finite_pairs": [{"birth": 1.0, "death": 5.0}],
                        "essential_births": [],
                    },
                    {
                        "dimension": 2,
                        "finite_pairs": [],
                        "essential_births": [3.0, 4.0],
                    },
                ]
            },
        }

        features = module.topology_features(document)

        self.assertEqual(features["coverage_radius_um"], 3.0)
        self.assertEqual(features["dimension_0_finite_count_per_landmark"], 0.25)
        self.assertEqual(features["dimension_1_total_persistence_um_squared"], 4.0)
        self.assertEqual(features["dimension_2_essential_count_per_landmark"], 0.5)


if __name__ == "__main__":
    unittest.main()
