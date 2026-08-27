import importlib.util
from pathlib import Path
import unittest


MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_crc_fingerprint_bundle.py"
)


class CrcFingerprintBundleTest(unittest.TestCase):
    def test_interval_status_preserves_positive_negative_and_uncertain_results(self):
        spec = importlib.util.spec_from_file_location("bundle", MODULE_PATH)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)

        self.assertEqual(module.interval_status(0.2, 0.1, 0.3), "positive")
        self.assertEqual(module.interval_status(-0.2, -0.3, -0.1), "negative")
        self.assertEqual(module.interval_status(0.01, -0.1, 0.2), "uncertain")


if __name__ == "__main__":
    unittest.main()
