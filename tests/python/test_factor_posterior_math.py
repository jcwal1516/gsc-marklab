"""Known posterior curvature and identified prediction variances for factor workers."""

import sys
import unittest
from pathlib import Path

import numpy as np
from scipy.optimize import OptimizeResult

sys.path.append(str(Path(__file__).resolve().parents[2] / "workers/python"))

from marklab_worker_support.factor_models import (
    delta_method_variance,
    inverse_hessian_covariance,
    minimize_lbfgs,
)
from marklab_worker_support.protocol import ContractError


class FactorPosteriorMathTests(unittest.TestCase):
    def fitted_covariance(self, precision):
        def objective(parameters):
            return float(
                0.5 * parameters @ precision @ parameters
            ), precision @ parameters

        _, fit = minimize_lbfgs(
            objective,
            np.zeros(len(precision)),
            maximum_iterations=100,
            ftol=1e-10,
            gtol=1e-6,
            failure="fit failed",
        )
        return inverse_hessian_covariance(fit, "invalid posterior curvature", objective)

    def test_gaussian_prediction_variance_uses_curvature_even_when_started_at_mode(
        self,
    ):
        for scale in [1.0, 1e-12]:
            covariance = self.fitted_covariance(scale * np.diag([100.0, 0.01]))
            for gradient, variance in [([1.0, 0.0], 0.01), ([0.0, 1.0], 100.0)]:
                with self.subTest(scale=scale, gradient=gradient):
                    np.testing.assert_allclose(
                        delta_method_variance(np.asarray(gradient), covariance, 0.25),
                        variance / scale + 0.25,
                        rtol=1e-10,
                    )

    def test_null_directions_allow_only_identified_predictions(self):
        # Rotating the parameter coordinates must not change an identified prediction.
        rotation = np.asarray([[0.6, -0.8], [0.8, 0.6]])
        covariance = self.fitted_covariance(rotation @ np.diag([0.0, 2.0]) @ rotation.T)
        self.assertAlmostEqual(
            delta_method_variance(rotation[:, 1], covariance, 0.25), 0.75, places=9
        )
        with self.assertRaises(ContractError):
            delta_method_variance(rotation[:, 0], covariance, 0.25)

    def test_stationary_saddle_does_not_produce_posterior_uncertainty(self):
        with self.assertRaises(ContractError):
            self.fitted_covariance(np.diag([1.0, -1.0]))

    def test_factor_rotations_are_removed_at_a_near_stationary_fit(self):
        def objective(parameters):
            left, right = parameters[:2], parameters[2:]
            residual = left @ right - 1.1
            value = 0.5 * residual**2 + 0.05 * (parameters @ parameters)
            gradient = residual * np.concatenate([right, left]) + 0.1 * parameters
            return float(value), gradient

        # Simultaneous rotation of both factors preserves their dot product and prior.
        # Just short of the mode, this rotation has small negative Hessian curvature.
        value = 1.0 - 1e-4
        parameters = np.asarray([value, 0.0, value, 0.0])
        rotation = np.asarray([[0.0], [value], [0.0], [value]])
        covariance = inverse_hessian_covariance(
            OptimizeResult(x=parameters), "invalid curvature", objective, rotation
        )
        gradient = np.asarray([value, 0.0, value, 0.0])
        expected = 2.0 * value**2 / (3.0 * value**2 - 1.0) + 0.25
        self.assertAlmostEqual(
            delta_method_variance(gradient, covariance, 0.25), expected, places=8
        )


if __name__ == "__main__":
    unittest.main()
