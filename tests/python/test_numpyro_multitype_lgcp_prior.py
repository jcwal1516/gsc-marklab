import importlib.util
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

import numpy as np


WORKER_DIRECTORY = Path(__file__).resolve().parents[2] / "workers" / "python"
sys.path.insert(0, str(WORKER_DIRECTORY))
WORKERS = [
    "marklab_numpyro_replicated_arbitrary_window_multitype_lgcp_worker",
    "marklab_numpyro_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_worker",
    "marklab_numpyro_correlated_replicated_arbitrary_window_multitype_lgcp_worker",
]


def load_worker(name):
    spec = importlib.util.spec_from_file_location(name, WORKER_DIRECTORY / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def configuration(intercept_mean):
    return {
        "types": ["a", "b"],
        "patient_ids": ["p0", "p1"],
        "pattern_ids": ["s0", "s1"],
        "pattern_patients": np.array([0, 1]),
        "node_patterns": np.array([0, 0, 1, 1]),
        "node_patients": np.array([0, 0, 1, 1]),
        "patient_groups": np.array([0.0, 1.0]),
        "covariate": np.array([-1.0, 1.0, -1.0, 1.0]),
        "weights": np.ones(4),
        "cholesky": np.eye(4) * 0.1,
        "node_index": np.tile(np.arange(4), 2),
        "type_index": np.repeat(np.arange(2), 4),
        "observed": np.ones(8, dtype=int),
        "priors": {
            "intercept_mean": intercept_mean,
            "intercept_sd": 0.1,
            "group_effect_sd": 0.1,
            "covariate_effect_sd": 0.1,
            "patient_sd_scale": 0.1,
            "pattern_sd_scale": 0.1,
        },
        "request": {"nodes": [
            {"x_um": x, "y_um": y} for y in (0.0, 1.0) for x in (0.0, 1.0)
        ]},
        "amplitude_scale": 0.1,
        "length_scale": 1.0,
        "kernel_jitter": 1e-6,
        "correlation_prior": {"lkj_concentration": 2.0, "marginal_field_scale": 0.1},
        "policy": {"prior_predictive_draws": 8},
        "seed": 13,
        "target_accept": 0.9,
        "numpyro_maximum_tree_depth": 10,
        "tune": 100,
        "draws": 100,
        "chains": 2,
    }


class MultitypeLgcpPriorTest(unittest.TestCase):
    def test_ordinary_priors_generate_finite_counts_for_each_model(self):
        base = load_worker(WORKERS[0])
        for name in WORKERS:
            worker = load_worker(name)
            with self.subTest(worker=name):
                self.assertTrue(base.prior_is_finite(
                    worker.make_model(configuration(1.0)), 8, worker.seed_for(13, "prior")
                ))

    def test_invalid_prior_counts_are_rejected_before_posterior_sampling(self):
        for name in WORKERS:
            worker = load_worker(name)
            # exp(100) is finite but exceeds the Poisson integer range;
            # exp(1000) also overflows the floating-point intensity.
            for intercept_mean in (100.0, 1000.0):
                with self.subTest(worker=name, intercept=intercept_mean):
                    args = [configuration(intercept_mean), "request", "lock", "worker"]
                    if name == WORKERS[0]:
                        args.insert(2, "source")
                    with patch.object(worker, "MCMC", side_effect=AssertionError("invalid prior reached NUTS")):
                        with self.assertRaisesRegex(ValueError, "prior predictive"):
                            worker.fit(*args)


if __name__ == "__main__":
    unittest.main()
