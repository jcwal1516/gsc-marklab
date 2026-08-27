import importlib.util
from pathlib import Path
import unittest


MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_tcga_crc_m2_stability_summary.py"
)


class M2StabilitySummaryTest(unittest.TestCase):
    def test_patient_rank_stability_and_roi_bootstrap_are_deterministic(self):
        spec = importlib.util.spec_from_file_location("m2_stability", MODULE_PATH)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)

        self.assertEqual(module.spearman([1.0, 2.0, 3.0], [10.0, 20.0, 30.0]), 1.0)
        self.assertEqual(module.spearman([1.0, 2.0, 3.0], [30.0, 20.0, 10.0]), -1.0)
        first = module.roi_bootstrap_absolute_deviations([0.0, 1.0, 2.0, 3.0], 100, 17)
        second = module.roi_bootstrap_absolute_deviations([0.0, 1.0, 2.0, 3.0], 100, 17)
        self.assertEqual(first, second)
        self.assertEqual(len(first), 100)
        self.assertTrue(all(value >= 0.0 for value in first))


if __name__ == "__main__":
    unittest.main()
