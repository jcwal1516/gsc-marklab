import importlib.util
from pathlib import Path
import unittest

import numpy as np


MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_scipy_pcca_em_worker.py"
)


def load_module():
    spec = importlib.util.spec_from_file_location("scipy_pcca_worker", MODULE_PATH)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


class ProbabilisticCcaCorrelationTest(unittest.TestCase):
    def test_canonical_correlations_are_invariant_to_latent_axis_rotation(self):
        worker = load_module()
        left = np.asarray(
            [
                [-2.0, 1.0],
                [-1.0, -1.0],
                [0.0, 0.0],
                [1.0, -1.0],
                [2.0, 1.0],
            ]
        )
        right = left[:, ::-1]

        actual = worker.canonical_correlations(left, right)

        np.testing.assert_allclose(actual, [1.0, 1.0], rtol=0.0, atol=2e-15)

        scaled = worker.canonical_correlations(left * 1e-100, right * 1e100)
        np.testing.assert_allclose(scaled, actual, rtol=2e-15, atol=2e-15)

    def test_rank_deficient_score_spaces_report_only_identified_correlation(self):
        worker = load_module()
        coordinate = np.asarray([-2.0, -1.0, 0.0, 1.0, 2.0])
        left = np.column_stack([coordinate, 2.0 * coordinate])
        right = np.column_stack([-3.0 * coordinate, coordinate])

        actual = worker.canonical_correlations(left, right)

        np.testing.assert_allclose(actual, [1.0, 0.0], rtol=0.0, atol=2e-15)


if __name__ == "__main__":
    unittest.main()
