import importlib.util
from pathlib import Path
import unittest


MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_tcga_crc_m6_fingerprint_adapter.py"
)


class M6FingerprintAdapterTest(unittest.TestCase):
    def test_fusion_uses_only_stability_admitted_spatial_components(self):
        spec = importlib.util.spec_from_file_location("m6_adapter", MODULE_PATH)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        document = {
            "format": "marklab.vector_semivariogram",
            "version": 1,
            "embedding_dimension": 1280,
            "curve": [
                {"bin_id": "near", "pair_count": 1, "semivariance": 1.0},
                {"bin_id": "intermediate", "pair_count": 2, "semivariance": 2.0},
                {"bin_id": "far", "pair_count": 3, "semivariance": 3.0},
            ],
        }

        self.assertEqual(module.stable_m4_values(document), [2.0, 3.0])
        self.assertEqual(len(module.M6_FEATURES), 67)
        self.assertNotIn("embedding_raw_variogram_0_25um", module.M6_FEATURES)
        self.assertNotIn(
            "embedding_coordinate_graph_2x2_low_energy_fraction",
            module.M6_FEATURES,
        )


if __name__ == "__main__":
    unittest.main()
