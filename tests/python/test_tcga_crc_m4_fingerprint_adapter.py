import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_tcga_crc_m4_fingerprint_adapter.py"
)


class M4FingerprintAdapterTest(unittest.TestCase):
    def test_variogram_features_preserve_prespecified_distance_order(self):
        spec = importlib.util.spec_from_file_location("m4_fingerprint", MODULE_PATH)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "result.json"
            path.write_text(
                json.dumps(
                    {
                        "format": "marklab.vector_semivariogram",
                        "version": 1,
                        "embedding_dimension": 1280,
                        "curve": [
                            {"bin_id": "near", "pair_count": 2, "semivariance": 1.0},
                            {"bin_id": "intermediate", "pair_count": 3, "semivariance": 2.0},
                            {"bin_id": "far", "pair_count": 4, "semivariance": 4.0},
                        ],
                    }
                )
            )
            self.assertEqual(module.variogram_features(path), [1.0, 2.0, 4.0])


if __name__ == "__main__":
    unittest.main()
