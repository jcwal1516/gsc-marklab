import importlib.util
import math
import unittest
from pathlib import Path

import numpy as np

WORKERS = Path(__file__).resolve().parents[2] / "workers/python"


def load_worker(filename):
    path = WORKERS / filename
    specification = importlib.util.spec_from_file_location(path.stem, path)
    module = importlib.util.module_from_spec(specification)
    assert specification.loader is not None
    specification.loader.exec_module(module)
    return module


class PoissonExposureConstantTest(unittest.TestCase):
    def test_scalar_laplace_log_joint_is_invariant_to_rate_coordinate_change(self):
        worker = load_worker("marklab_scipy_pytensor_laplace_worker.py")
        base = {
            "prior_mean": -0.2,
            "prior_sd": 1.1,
            "counts": np.asarray([2, 0, 5], dtype=np.int64),
            "exposures": np.asarray([0.5, 2.0, 1.25]),
            "initial": 0.0,
            "max_iterations": 100,
            "tolerance": 1e-10,
            "seed": 7,
        }
        scale = 4.0
        shifted = {
            **base,
            "prior_mean": base["prior_mean"] - math.log(scale),
            "exposures": base["exposures"] * scale,
            "initial": base["initial"] - math.log(scale),
        }

        original = worker.fit(base, "request", "lock", "worker")
        reparameterized = worker.fit(shifted, "request", "lock", "worker")

        self.assertAlmostEqual(
            reparameterized["mode"]["log_rate"],
            original["mode"]["log_rate"] - math.log(scale),
            places=11,
        )
        self.assertAlmostEqual(
            reparameterized["mode"]["log_joint"],
            original["mode"]["log_joint"],
            places=11,
        )

    def test_nested_laplace_log_normalizer_is_invariant_to_rate_coordinate_change(self):
        worker = load_worker("marklab_scipy_pytensor_pymc_inla_worker.py")
        base = {
            "region_ids": ["a", "b", "c"],
            "counts": np.asarray([2, 0, 5], dtype=np.int64),
            "exposures": np.asarray([0.5, 2.0, 1.25]),
            "latent_mean": -0.2,
            "tau_shape": 2.0,
            "tau_rate": 1.0,
            "log_tau_min": -3.0,
            "log_tau_max": 3.0,
            "grid_points": 21,
            "mode_tolerance": 1e-10,
            "maximum_mode_iterations": 100,
        }
        scale = 4.0
        shifted = {
            **base,
            "latent_mean": base["latent_mean"] - math.log(scale),
            "exposures": base["exposures"] * scale,
        }

        original = worker.nested_laplace(base)
        reparameterized = worker.nested_laplace(shifted)

        self.assertAlmostEqual(
            reparameterized["log_normalizer"], original["log_normalizer"], places=10
        )
        np.testing.assert_allclose(
            reparameterized["weights"], original["weights"], rtol=1e-11, atol=1e-12
        )
        np.testing.assert_allclose(
            reparameterized["latent_means"],
            original["latent_means"] - math.log(scale),
            rtol=1e-11,
            atol=1e-11,
        )


if __name__ == "__main__":
    unittest.main()
