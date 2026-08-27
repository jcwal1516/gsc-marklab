import importlib.util
from pathlib import Path
import unittest


MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_tcga_crc_m4_stability_input.py"
)


class M4StabilityInputTest(unittest.TestCase):
    def test_subsample_is_deterministic_within_each_field(self):
        spec = importlib.util.spec_from_file_location("m4_stability_input", MODULE_PATH)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        rows = [
            {"object_id": f"field-{field}:cell-{index}"}
            for field in range(2)
            for index in range(8)
        ]

        selected = module.cell_subsample(rows)
        reversed_selected = module.cell_subsample(list(reversed(rows)))

        self.assertEqual(len(selected), 12)
        self.assertEqual(
            {row["object_id"] for row in selected},
            {row["object_id"] for row in reversed_selected},
        )


if __name__ == "__main__":
    unittest.main()
