import importlib.util
from pathlib import Path
import unittest

import numpy as np


MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_tcga_crc_raw_embedding_summary.py"
)


class RawEmbeddingSummaryTest(unittest.TestCase):
    def test_summary_uses_all_rows_and_channels_without_fitted_projection(self):
        spec = importlib.util.spec_from_file_location("raw_summary", MODULE_PATH)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        values = np.asarray([[1.0, 2.0, 3.0, 4.0], [3.0, 4.0, 5.0, 6.0]])

        names, summary = module.summarize_raw_embeddings(values, probabilities=(0.0, 0.5, 1.0))

        self.assertEqual(len(names), 15)
        self.assertEqual(len(summary), 15)
        self.assertEqual(summary[0:3], [2.0, 3.5, 5.0])
        self.assertEqual(summary[3:6], [1.0, 1.0, 1.0])
        self.assertTrue(all(np.isfinite(summary)))
        self.assertTrue(all(name.startswith("embedding_raw_") for name in names))


if __name__ == "__main__":
    unittest.main()
