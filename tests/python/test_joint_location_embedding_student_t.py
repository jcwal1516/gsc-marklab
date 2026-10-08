#!/usr/bin/env python3

import importlib.util
import math
from pathlib import Path
import unittest

import numpy as np


MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers"
    / "python"
    / "marklab_numpyro_joint_replicated_location_embedding_worker.py"
)


def load_module():
    spec = importlib.util.spec_from_file_location("joint_embedding_worker", MODULE_PATH)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


class StudentTEmbeddingLikelihoodTest(unittest.TestCase):
    def test_density_matches_closed_form_oracle(self):
        worker = load_module()
        degrees = 5.0
        values = np.asarray([[1.25, -0.5]])
        mean = np.asarray([[[0.25, 0.5]]])
        scale = np.asarray([[2.0, 0.5]])
        actual = worker.embedding_log_density(
            {
                "embedding_residual_family": "student_t",
                "student_t_degrees_of_freedom": degrees,
            },
            values,
            mean,
            scale,
        )[0, 0]
        expected = []
        for value, center, width in zip(values[0], mean[0, 0], scale[0], strict=True):
            residual = (value - center) / width
            expected.append(
                math.lgamma((degrees + 1.0) / 2.0)
                - math.lgamma(degrees / 2.0)
                - 0.5 * math.log(degrees * math.pi)
                - math.log(width)
                - 0.5 * (degrees + 1.0) * math.log1p(residual**2 / degrees)
            )
        np.testing.assert_allclose(actual, expected, rtol=0.0, atol=1e-15)


class JointVectorPredictiveScoreTest(unittest.TestCase):
    def test_integrates_complete_vector_within_each_posterior_draw(self):
        worker = load_module()
        log_density = np.asarray(
            [
                [[0.0, -10.0], [-2.0, -2.0]],
                [[-10.0, 0.0], [-2.0, -2.0]],
            ]
        )

        actual = worker.joint_vector_log_predictive_density(log_density)

        np.testing.assert_allclose(actual, [-10.0, -4.0], rtol=0.0, atol=1e-12)
        self.assertLess(actual[0], actual[1])

    def test_handles_one_feature_identical_draws_permutations_and_extremes(self):
        worker = load_module()
        one_feature = np.asarray([[[-3.0]], [[-7.0]]])
        expected_one = -3.0 + math.log((1.0 + math.exp(-4.0)) / 2.0)
        np.testing.assert_allclose(
            worker.joint_vector_log_predictive_density(one_feature),
            [expected_one],
            rtol=0.0,
            atol=1e-15,
        )

        identical_draws = np.asarray(
            [[[-2.0, -3.0], [-1000.0, -1000.0]]] * 3
        )
        actual = worker.joint_vector_log_predictive_density(identical_draws)
        permuted = worker.joint_vector_log_predictive_density(
            identical_draws[:, :, ::-1]
        )
        np.testing.assert_allclose(actual, [-5.0, -2000.0], rtol=0.0, atol=1e-12)
        np.testing.assert_array_equal(actual, permuted)


class PairedPosteriorPredictiveTest(unittest.TestCase):
    def test_compares_each_replicated_discrepancy_with_matching_observed_draw(self):
        worker = load_module()

        actual = worker.paired_two_sided_tail(
            np.asarray([1.0, 11.0]), np.asarray([0.0, 10.0])
        )

        self.assertEqual(actual, 0.0)

    def test_preserves_draw_pairing(self):
        worker = load_module()
        replicated = np.asarray([1.0, 11.0, 7.0, 3.0])
        observed = np.asarray([0.0, 10.0, 8.0, 4.0])

        self.assertEqual(worker.paired_two_sided_tail(replicated, observed), 1.0)
        self.assertEqual(
            worker.paired_two_sided_tail(replicated, observed[[1, 2, 3, 0]]),
            0.5,
        )

    def test_evaluate_uses_paired_rmse_for_gaussian_and_student_t_residuals(self):
        worker = load_module()
        draws = 4
        config = {
            "patients": ["p0", "p1"],
            "seed": 17,
            "student_t_degrees_of_freedom": 5.0,
        }
        data = {
            "embedding_heldout": np.asarray([True, True, True, True]),
            "embedding_patient": np.asarray([0, 0, 1, 1]),
            "embedding_nearest": np.asarray([0, 1, 0, 1]),
            "embedding_values": np.asarray(
                [[0.0, 0.5], [1.0, -0.5], [0.25, 0.75], [1.25, -0.25]]
            ),
            "node_count": np.asarray([2, 3]),
        }
        joint = {
            "patient_factor": np.zeros((draws, 2, 1)),
            "spatial_factor": np.zeros((draws, 1, 2)),
            "embedding_intercept": np.zeros((draws, 2)),
            "embedding_loading": np.ones((draws, 2, 1)),
            "embedding_noise": np.ones((draws, 2)),
            "location_expected": np.full((draws, 2), 2.5),
        }
        baseline = {
            "embedding_intercept": np.zeros((draws, 2)),
            "embedding_patient_sd": np.ones((draws, 2)),
            "embedding_patient_raw": np.zeros((draws, 2, 2)),
            "embedding_noise": np.ones((draws, 2)),
        }

        for family in ("gaussian", "student_t"):
            with self.subTest(family=family):
                config["embedding_residual_family"] = family
                comparison, ppc = worker.evaluate(config, data, joint, baseline)
                self.assertEqual(comparison["score_version"], 2)
                self.assertEqual(
                    comparison["score_unit"], "heldout_embedding_point_complete_vector"
                )
                self.assertEqual(comparison["heldout_embedding_point_count"], 4)
                self.assertEqual(
                    comparison["heldout_embedding_point_counts_by_patient"], [2, 2]
                )
                self.assertIn("patient_score_spread", comparison["joint_spatial"])
                self.assertNotIn("uncertainty", comparison["joint_spatial"])
                self.assertGreaterEqual(
                    ppc["heldout_embedding_rmse"]["two_sided_tail_probability"], 0.0
                )
                self.assertLessEqual(
                    ppc["heldout_embedding_rmse"]["two_sided_tail_probability"], 1.0
                )


if __name__ == "__main__":
    unittest.main()
