import importlib.util
import unittest
from pathlib import Path
from unittest import mock

import jax
import numpy as np

MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_ccazoo_bayesian_pcca_worker.py"
)


def load_worker():
    specification = importlib.util.spec_from_file_location(
        MODULE_PATH.stem, MODULE_PATH
    )
    module = importlib.util.module_from_spec(specification)
    assert specification.loader is not None
    specification.loader.exec_module(module)
    return module


class BayesianPccaSeedTest(unittest.TestCase):
    def test_full_unsigned_request_seed_range_maps_to_a_jax_key(self):
        worker = load_worker()

        self.assertEqual(worker.jax_seed(17), 17)
        self.assertNotEqual(worker.jax_seed(0), worker.jax_seed(2**32))
        self.assertNotEqual(
            np.asarray(jax.random.PRNGKey(worker.jax_seed(0))).tolist(),
            np.asarray(jax.random.PRNGKey(worker.jax_seed(2**32))).tolist(),
        )
        high = worker.jax_seed(2**64 - 1)
        self.assertEqual(high, worker.jax_seed(2**64 - 1))
        self.assertGreaterEqual(high, 0)
        self.assertLessEqual(high, 2**32 - 1)
        self.assertEqual(np.asarray(jax.random.PRNGKey(high)).shape, (2,))

    def test_sampler_passes_the_mapped_seed_to_cca_zoo_and_jax(self):
        worker = load_worker()
        request = {
            "seed": 2**64 - 1,
            "warmup": 1,
            "samples": 1,
            "target_accept": 0.9,
        }
        expected = worker.jax_seed(request["seed"])
        with (
            mock.patch.object(worker, "ProbabilisticCCA") as model_type,
            mock.patch.object(worker, "NUTS"),
            mock.patch.object(worker, "MCMC") as mcmc_type,
            mock.patch.object(
                worker.jax.random, "PRNGKey", wraps=jax.random.PRNGKey
            ) as prng_key,
        ):
            model_type.return_value._setup_fit.return_value = [np.zeros((2, 1))] * 2
            mcmc_type.return_value.get_samples.return_value = {}
            mcmc_type.return_value.get_extra_fields.return_value = {}

            worker.sample_bayesian_pcca([np.zeros((2, 1))] * 2, request)

        self.assertEqual(model_type.call_args.kwargs["random_state"], expected)
        prng_key.assert_called_once_with(expected)


if __name__ == "__main__":
    unittest.main()
