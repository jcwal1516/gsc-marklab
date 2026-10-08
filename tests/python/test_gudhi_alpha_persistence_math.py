import importlib.util
from pathlib import Path
import unittest


MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_gudhi_alpha_persistence_worker.py"
)


def load_module():
    spec = importlib.util.spec_from_file_location("gudhi_alpha_persistence_worker", MODULE_PATH)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


class AlphaEulerCurveTest(unittest.TestCase):
    def test_threshold_does_not_include_a_later_nearby_filtration_event(self):
        worker = load_module()
        raw = [
            ((0,), 0.0),
            ((1,), 0.0),
            ((0, 1), 5.0e-15),
        ]
        request = {
            "maximum_dimension": 1,
            "euler_thresholds_alpha_squared": [0.0, 5.0e-15],
        }

        result = worker.euler_curve(raw, request)

        self.assertEqual(result["values"], [2, 1])
        self.assertEqual(result["simplex_counts_by_threshold"], [[2, 0], [2, 1]])


if __name__ == "__main__":
    unittest.main()
