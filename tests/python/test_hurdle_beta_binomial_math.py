import importlib.util
from pathlib import Path
import sys
import unittest

import numpy as np
from scipy.stats import betabinom


WORKER_DIRECTORY = Path(__file__).resolve().parents[2] / "workers" / "python"
MODULE_PATH = WORKER_DIRECTORY / "marklab_pymc_hurdle_beta_binomial_group_worker.py"


def load_module():
    sys.path.insert(0, str(WORKER_DIRECTORY))
    try:
        spec = importlib.util.spec_from_file_location("hurdle_beta_binomial_worker", MODULE_PATH)
        module = importlib.util.module_from_spec(spec)
        assert spec.loader is not None
        spec.loader.exec_module(module)
        return module
    finally:
        sys.path.pop(0)


class HurdleBetaBinomialExpectationTest(unittest.TestCase):
    def test_positive_mean_conditions_on_the_excluded_zero_count(self):
        worker = load_module()
        probabilities = np.asarray([0.2, 0.65])
        concentrations = np.asarray([5.0, 12.0])
        trials = np.asarray([2, 7, 19])

        actual = worker.zero_truncated_beta_binomial_mean_proportion(
            probabilities, concentrations, trials
        )

        expected = np.empty_like(actual)
        for draw, (probability, concentration) in enumerate(
            zip(probabilities, concentrations, strict=True)
        ):
            alpha = probability * concentration
            beta = (1.0 - probability) * concentration
            for patient, count in enumerate(trials):
                support = np.arange(1, count + 1)
                positive_mass = 1.0 - betabinom.pmf(0, count, alpha, beta)
                expected[draw, patient] = np.sum(
                    support * betabinom.pmf(support, count, alpha, beta)
                ) / (count * positive_mass)

        np.testing.assert_allclose(actual, expected, rtol=2e-14, atol=2e-14)
        self.assertTrue(np.all(actual > probabilities[:, None]))

    def test_chain_draw_shape_and_extreme_admitted_parameters_remain_finite(self):
        worker = load_module()
        probabilities = np.asarray([[1e-12, 0.5], [1.0 - 1e-12, 0.9]])
        concentrations = np.asarray([[1e-3, 1e6], [1e-2, 1e4]])

        actual = worker.zero_truncated_beta_binomial_mean_proportion(
            probabilities, concentrations, np.asarray([1, 10, 10_000_000])
        )

        self.assertEqual(actual.shape, (2, 2, 3))
        self.assertTrue(np.isfinite(actual).all())
        self.assertTrue(np.all(actual >= probabilities[..., None]))
        self.assertTrue(np.all(actual <= 1.0))


if __name__ == "__main__":
    unittest.main()
