import importlib.util
import unittest
from pathlib import Path

import numpy as np
from scipy.optimize import minimize
from scipy.stats import norm

MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_scipy_clone_models_worker.py"
)
spec = importlib.util.spec_from_file_location("clone_models_worker", MODULE_PATH)
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)


class CloneNicheMathTest(unittest.TestCase):
    def test_hard_assignments_match_patient_intercept_regression(self):
        patients = np.repeat(np.arange(4), 10)
        offsets = np.asarray([-1.0, 0.5, 2.0, -0.3])[patients]
        for count in (5, 8, 9):
            with self.subTest(clone_a_count=count):
                clone_a = np.tile(np.arange(10) < count, 4)
                values = offsets + np.where(clone_a, 1.0, -1.0)
                probabilities = np.column_stack([clone_a, ~clone_a]).astype(float)
                contrast, se, means, _ = worker.fit_niche(
                    values, probabilities, patients, 4
                )
                self.assertAlmostEqual(contrast, 2.0, places=5)
                self.assertGreater(se, 0.0)
                np.testing.assert_allclose(means, values.reshape(4, 10).mean(axis=1))

        rng = np.random.default_rng(18)
        clone_a = np.concatenate([np.arange(10) < count for count in (5, 8, 9, 6)])
        values = offsets + np.where(clone_a, 1.0, -1.0) + rng.normal(0.0, 0.4, 40)
        probabilities = np.column_stack([clone_a, ~clone_a]).astype(float)
        design = np.column_stack([np.where(clone_a, 0.5, -0.5), np.eye(4)[patients]])
        coefficients = np.linalg.lstsq(design, values, rcond=None)[0]
        residual_variance = np.mean((values - design @ coefficients) ** 2)
        expected_se = np.sqrt(
            residual_variance * np.linalg.inv(design.T @ design)[0, 0]
        )

        contrast, se, _, _ = worker.fit_niche(values, probabilities, patients, 4)
        self.assertAlmostEqual(contrast, coefficients[0], places=5)
        self.assertAlmostEqual(se, expected_se, places=5)

    def test_uncertain_assignments_use_profile_likelihood_uncertainty(self):
        rng = np.random.default_rng(4)
        probability_a = rng.uniform(0.1, 0.9, 40)
        clone_a = rng.uniform(size=40) < probability_a
        values = np.where(clone_a, 1.0, -1.0) + rng.normal(size=40)
        probabilities = np.column_stack([probability_a, 1.0 - probability_a])
        patients = np.repeat(np.arange(4), 10)
        contrast, se, _, objective = worker.fit_niche(
            values, probabilities, patients, 4
        )

        # Independently refit every nuisance parameter at fixed contrasts. The resulting
        # curvature includes the patient intercepts and residual scale without a Hessian.
        def profile(delta):
            def negative_log_likelihood(parameters):
                sigma = np.exp(parameters[0])
                intercept = parameters[1:][patients]
                density = probability_a * norm.pdf(
                    values, intercept + delta / 2.0, sigma
                )
                density += (1.0 - probability_a) * norm.pdf(
                    values, intercept - delta / 2.0, sigma
                )
                return -np.log(density).sum()

            initial = np.r_[0.0, values.reshape(4, 10).mean(axis=1)]
            fit = minimize(
                negative_log_likelihood,
                initial,
                method="BFGS",
                options={"gtol": 1e-6},
            )
            self.assertLess(np.linalg.norm(fit.jac), 1e-4)
            return fit.fun

        step = 0.002
        center = profile(contrast)
        lower, upper = profile(contrast - step), profile(contrast + step)
        self.assertAlmostEqual(objective, center, places=6)
        self.assertLess(abs((upper - lower) / (2.0 * step)), 1e-4)
        profile_se = 1.0 / np.sqrt((upper + lower - 2.0 * center) / step**2)
        self.assertAlmostEqual(se / profile_se, 1.0, places=4)

    def test_confounded_patient_and_clone_effects_do_not_report_an_se(self):
        patients = np.repeat(np.arange(4), 10)
        clone_a = patients < 2
        values = np.where(clone_a, 1.0, -1.0) + np.tile(np.linspace(-0.2, 0.2, 10), 4)
        probabilities = np.column_stack([clone_a, ~clone_a]).astype(float)
        with self.assertRaises(worker.ContractError):
            worker.fit_niche(values, probabilities, patients, 4)


if __name__ == "__main__":
    unittest.main()
