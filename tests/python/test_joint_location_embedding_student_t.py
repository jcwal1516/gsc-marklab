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


if __name__ == "__main__":
    unittest.main()
