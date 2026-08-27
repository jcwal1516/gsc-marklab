import importlib.util
from pathlib import Path
import unittest


MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_tcga_crc_m3_stability_summary.py"
)


class M3StabilitySummaryTest(unittest.TestCase):
    def module(self):
        spec = importlib.util.spec_from_file_location("m3_stability", MODULE_PATH)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def test_spearman_uses_midrank_ties(self):
        module = self.module()

        self.assertEqual(module.spearman([1, 2, 3], [3, 2, 1]), -1.0)
        self.assertAlmostEqual(
            module.spearman([1, 1, 3], [2, 2, 4]),
            1.0,
        )

    def test_cell_subsample_is_field_bounded_and_order_independent(self):
        module = self.module()
        object_ids = [
            f"field-{field}:cell-{index}"
            for field in range(4)
            for index in range(8)
        ]

        selected = module.cell_subsample_indices(object_ids, 0.8)
        reversed_selected = module.cell_subsample_indices(
            list(reversed(object_ids)), 0.8
        )

        self.assertEqual(len(selected), 24)
        self.assertEqual(
            {object_ids[index] for index in selected},
            {
                list(reversed(object_ids))[index]
                for index in reversed_selected
            },
        )

        sparse = ["field-a:only-cell", "field-b:cell-1", "field-b:cell-2"]
        sparse_selected = module.cell_subsample_indices(sparse, 0.8)
        self.assertEqual(len(sparse_selected), 2)
        self.assertIn(0, sparse_selected)


if __name__ == "__main__":
    unittest.main()
