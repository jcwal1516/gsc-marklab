import importlib.util
from pathlib import Path
import unittest


MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_schurch_codex_retrieval_summary.py"
)


class SchurchCodexRetrievalSummaryTest(unittest.TestCase):
    def test_top_five_majority_is_label_deterministic(self):
        spec = importlib.util.spec_from_file_location("codex_summary", MODULE_PATH)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        matches = [
            ("a", "MSS", 0.1),
            ("b", "MSI", 0.2),
            ("c", "MSI", 0.3),
            ("d", "MSS", 0.4),
        ]

        self.assertEqual(module.majority_label(matches, 5), "MSI")
        self.assertEqual(module.majority_label(matches[:3], 1), "MSS")


if __name__ == "__main__":
    unittest.main()
