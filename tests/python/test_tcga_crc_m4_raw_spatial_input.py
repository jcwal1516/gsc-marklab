import importlib.util
import math
from pathlib import Path
import unittest


MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_tcga_crc_m4_raw_spatial_input.py"
)


class M4RawSpatialInputTest(unittest.TestCase):
    def test_cell_selection_is_bounded_deterministic_and_order_independent(self):
        spec = importlib.util.spec_from_file_location("m4_input", MODULE_PATH)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        rows = [{"cell_id": f"cell-{index}"} for index in range(20)]

        selected = module.select_rows(rows, 8)
        reversed_selected = module.select_rows(list(reversed(rows)), 8)

        self.assertEqual(len(selected), 8)
        self.assertEqual(
            {row["cell_id"] for row in selected},
            {row["cell_id"] for row in reversed_selected},
        )

    def test_field_frames_preserve_within_field_distance_and_exclude_cross_field_pairs(self):
        spec = importlib.util.spec_from_file_location("m4_input", MODULE_PATH)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        left = [
            {"cell_id": "left-a", "x_um": "1000", "y_um": "2000"},
            {"cell_id": "left-b", "x_um": "1003", "y_um": "2004"},
        ]
        right = [
            {"cell_id": "right-a", "x_um": "1000", "y_um": "2000"},
            {"cell_id": "right-b", "x_um": "1003", "y_um": "2004"},
        ]

        left_framed = module.frame_field_coordinates(left, 0)
        right_framed = module.frame_field_coordinates(right, 1)

        self.assertEqual(
            math.dist(left_framed[0][1:], left_framed[1][1:]),
            5.0,
        )
        self.assertEqual(
            math.dist(right_framed[0][1:], right_framed[1][1:]),
            5.0,
        )
        self.assertGreater(
            min(
                math.dist(left_point[1:], right_point[1:])
                for left_point in left_framed
                for right_point in right_framed
            ),
            100.0,
        )


if __name__ == "__main__":
    unittest.main()
