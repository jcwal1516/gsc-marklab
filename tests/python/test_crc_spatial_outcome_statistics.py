import importlib.util
import math
from pathlib import Path
import unittest


MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/crc_spatial_outcome_statistics.py"
)


def load_module():
    spec = importlib.util.spec_from_file_location("crc_spatial_outcome_statistics", MODULE_PATH)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class CrcSpatialOutcomeStatisticsTest(unittest.TestCase):
    def test_repeatability_resamples_patients_and_retrieves_coherent_specimens(self):
        module = load_module()
        records = [
            {"patient_id": "A", "specimen_id": "A1", "x": 0.0, "y": 0.0},
            {"patient_id": "A", "specimen_id": "A2", "x": 0.1, "y": 0.0},
            {"patient_id": "B", "specimen_id": "B1", "x": 10.0, "y": 10.0},
            {"patient_id": "B", "specimen_id": "B2", "x": 10.1, "y": 10.0},
            {"patient_id": "C", "specimen_id": "C1", "x": 20.0, "y": 20.0},
            {"patient_id": "C", "specimen_id": "C2", "x": 20.1, "y": 20.0},
        ]

        result = module.repeatability_analysis(
            records,
            ["x", "y"],
            bootstrap_replicates=100,
            seed=17,
        )

        self.assertEqual(result["population_unit"], "patient")
        self.assertEqual(result["patient_count"], 3)
        self.assertEqual(result["repeated_patient_count"], 3)
        self.assertEqual(result["specimen_count"], 6)
        self.assertLess(
            result["median_within_patient_distance"],
            result["median_between_patient_distance"],
        )
        self.assertEqual(result["leave_one_specimen_out_patient_retrieval"], 1.0)
        self.assertEqual(result["bootstrap"]["resampling_unit"], "whole_patient")
        self.assertEqual(result["bootstrap"]["replicates"], 100)

    def test_patient_label_permutation_is_deterministic_and_rejects_pseudoreplication(self):
        module = load_module()
        records = [
            {"patient_id": "A", "group": "MSI", "value": 4.0},
            {"patient_id": "B", "group": "MSI", "value": 3.0},
            {"patient_id": "C", "group": "MSS", "value": 0.0},
            {"patient_id": "D", "group": "MSS", "value": 1.0},
        ]

        first = module.patient_label_permutation(
            records, "MSI", "MSS", permutations=199, seed=9
        )
        second = module.patient_label_permutation(
            records, "MSI", "MSS", permutations=199, seed=9
        )

        self.assertEqual(first, second)
        self.assertEqual(first["population_unit"], "patient")
        self.assertEqual(first["group_counts"], {"MSI": 2, "MSS": 2})
        self.assertGreater(first["difference_in_means"], 0.0)
        with self.assertRaisesRegex(ValueError, "duplicate patient"):
            module.patient_label_permutation(
                records + [{"patient_id": "A", "group": "MSI", "value": 9.0}],
                "MSI",
                "MSS",
                permutations=99,
                seed=9,
            )

    def test_breslow_cox_reports_patient_level_hazard_direction(self):
        module = load_module()
        times = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]
        events = [1, 0, 1, 0, 1, 0, 1, 0, 1, 0]
        phenotype = [2.0, 0.0, -0.5, 1.5, 1.0, -2.0, -1.0, -1.5, 0.5, 0.0]

        result = module.fit_cox_breslow(
            times,
            events,
            [[value] for value in phenotype],
            ["phenotype"],
        )

        effect = result["effects"][0]
        self.assertEqual(result["population_unit"], "patient")
        self.assertEqual(result["patient_count"], 10)
        self.assertEqual(result["event_count"], 5)
        self.assertTrue(result["converged"])
        self.assertGreater(effect["coefficient"], 0.0)
        self.assertGreater(effect["hazard_ratio"], 1.0)
        self.assertTrue(math.isfinite(effect["standard_error"]))
        self.assertLess(effect["hazard_ratio_ci_95"][0], effect["hazard_ratio_ci_95"][1])

    def test_benjamini_hochberg_preserves_input_order(self):
        module = load_module()
        self.assertEqual(
            module.benjamini_hochberg([0.01, 0.04, 0.03]),
            [0.03, 0.04, 0.04],
        )


if __name__ == "__main__":
    unittest.main()
