import importlib.util
from pathlib import Path
import unittest


MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_tcga_crc_m4_stability_summary.py"
)


class M4StabilitySummaryTest(unittest.TestCase):
    def test_semivariance_requires_two_pairs_and_a_finite_value(self):
        spec = importlib.util.spec_from_file_location("m4_stability", MODULE_PATH)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        document = {
            "format": "marklab.vector_semivariogram",
            "version": 1,
            "embedding_dimension": 1280,
            "curve": [
                {"bin_id": "near", "pair_count": 2, "semivariance": 3.5},
                {"bin_id": "far", "pair_count": 1, "semivariance": 9.0},
            ],
        }

        self.assertEqual(module.semivariance(document, "near"), 3.5)
        self.assertIsNone(module.semivariance(document, "far"))
        self.assertIsNone(module.semivariance(document, "missing"))


if __name__ == "__main__":
    unittest.main()
