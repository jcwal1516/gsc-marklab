import importlib.util
from pathlib import Path
import unittest
from unittest import mock

import numpy as np


WORKERS = Path(__file__).resolve().parents[2] / "workers/python"


def load_worker(filename, module_name):
    spec = importlib.util.spec_from_file_location(module_name, WORKERS / filename)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


FIXED = load_worker(
    "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_worker.py",
    "fixed_multitype_lgcp_worker",
)
INFERRED = load_worker(
    "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_worker.py",
    "inferred_multitype_lgcp_worker",
)


def base_config(intercept_mean=0.0):
    node_patterns = np.asarray([0, 0, 1, 1], dtype=np.int64)
    node_patients = np.asarray([0, 0, 1, 1], dtype=np.int64)
    return {
        "types": ["a", "b", "c"],
        "patient_ids": ["p0", "p1"],
        "pattern_ids": ["s0", "s1"],
        "patient_groups": np.asarray([0.0, 1.0]),
        "pattern_patients": np.asarray([0, 1], dtype=np.int64),
        "node_patterns": node_patterns,
        "node_patients": node_patients,
        "weights": np.ones(4),
        "covariate": np.zeros(4),
        "node_index": np.repeat(np.arange(4, dtype=np.int64), 3),
        "type_index": np.tile(np.arange(3, dtype=np.int64), 4),
        "observed": np.zeros(12, dtype=np.int64),
        "cholesky": np.eye(4),
        "priors": {
            "intercept_mean": intercept_mean,
            "intercept_sd": 1.0e-9,
            "group_effect_sd": 1.0e-9,
            "covariate_effect_sd": 1.0e-9,
            "patient_sd_scale": 1.0e-9,
            "pattern_sd_scale": 1.0e-9,
            "field_amplitude": 1.0e-9,
            "field_length_scale_um": 1.0,
            "jitter": 1.0e-6,
        },
        "seed": 17,
        "policy": {
            "prior_predictive_draws": 2,
            "maximum_tree_depth": 10,
        },
        "request": {
            "nodes": [
                {"x_um": 0.0, "y_um": 0.0},
                {"x_um": 1.0, "y_um": 0.0},
                {"x_um": 0.0, "y_um": 1.0},
                {"x_um": 1.0, "y_um": 1.0},
            ]
        },
    }


def inferred_config(intercept_mean=0.0):
    config = base_config(intercept_mean)
    config.update(
        {
            "source_module": FIXED,
            "amplitude_scale": 1.0e-9,
            "length_scale": 1.0,
            "kernel_jitter": 1.0e-6,
            "maximum_tree_depth": 10,
        }
    )
    return config


class ReplicatedMultitypeLgcpPriorPredictiveTest(unittest.TestCase):
    def test_full_models_check_conditional_poisson_count_domain_before_nuts(self):
        for worker, config_factory in [
            (FIXED, base_config),
            (INFERRED, inferred_config),
        ]:
            with self.subTest(worker=worker.__name__, case="ordinary"):
                config = config_factory()
                model = worker.build_model(config)
                self.assertTrue(worker.prior_is_finite(config, model))

            invalid = config_factory(100.0)
            with self.subTest(worker=worker.__name__, case="finite_rate_outside_count_domain"):
                with mock.patch.object(
                    worker.pm,
                    "sample",
                    side_effect=AssertionError("posterior sampler must not run"),
                ):
                    with self.assertRaisesRegex(
                        worker.ContractError,
                        "prior predictive contains invalid rates or conditional Poisson counts",
                    ):
                        worker.fit(invalid, "request", "lock", "worker")


if __name__ == "__main__":
    unittest.main()
