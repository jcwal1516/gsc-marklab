import sys
import unittest
from pathlib import Path

import numpy as np

sys.path.append(str(Path(__file__).resolve().parents[2] / "workers/python"))

from marklab_worker_support.ordinal import level_reports


class OrdinalPosteriorPredictiveTest(unittest.TestCase):
    def test_observed_and_replicated_discrepancies_use_the_same_posterior_draw(self):
        outcomes = np.asarray([0] * 5 + [1] * 5 + [0] * 5 + [1] * 5)
        comparison = np.asarray([False] * 10 + [True] * 10)
        probabilities = np.asarray([[0.1, 0.9], [0.9, 0.1]])
        replicated = np.asarray(
            [([0] + [1] * 9) * 2, ([0] * 9 + [1]) * 2], dtype=np.int64
        )

        _, checks = level_reports(
            ["low", "high"],
            outcomes,
            comparison,
            probabilities,
            probabilities,
            replicated,
        )

        self.assertEqual(
            checks[0]["probability_absolute_replication_error_at_least_observed"],
            0.0,
        )


if __name__ == "__main__":
    unittest.main()
