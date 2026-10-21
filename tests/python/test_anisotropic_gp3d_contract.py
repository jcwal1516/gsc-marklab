import importlib.util
import unittest
from pathlib import Path

WORKER = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_pymc_anisotropic_gp3d_worker.py"
)
spec = importlib.util.spec_from_file_location("anisotropic_gp3d_worker", WORKER)
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)


def request(coordinates, predictions=1):
    digest = "0" * 64
    return {
        "format": "marklab.pymc_worker_request",
        "version": 1,
        "backend": {
            "name": "pymc", "version": worker.versions.PYMC,
            "python_version": worker.versions.PYTHON,
            "environment_lock_sha256": digest, "worker_sha256": digest,
        },
        "model": {
            "format": "marklab.bayesian_model_ir", "version": 1,
            "family": "exact_axis_aligned_anisotropic_matern32_gp3d",
            "coordinate_dimension": 3, "coordinate_unit": "micrometre",
            "kernel": "matern_3_2", "anisotropy": "axis_aligned_positive_length_scales",
            "mean_prior_mean": 0.0, "mean_prior_sd": 1.0, "amplitude_prior_sd": 1.0,
            "length_scale_prior_sd_um": [1.0] * 3, "noise_prior_sd": 1.0, "jitter": 1e-6,
            "observation_unit": "scalar_field_value",
            "backend_capability": "nuts_exact_dense_anisotropic_gp3d", "maturity": "experimental",
        },
        "observations": [
            {"observation_id": f"o-{i:02}", "coordinates_um": point, "value": float(i)}
            for i, point in enumerate(coordinates)
        ],
        "predictions": [
            {"prediction_id": f"p-{i:03}", "coordinates_um": [0.5] * 3}
            for i in range(predictions)
        ],
        "sampling": {
            "chains": 2, "tune_per_chain": 100, "draws_per_chain": 100,
            "target_accept": 0.9, "seed": 1,
        },
        "resources": {
            "maximum_observations": 64, "maximum_predictions": 512,
            "maximum_total_iterations": 800_000, "maximum_conditioning_work": 2_000_000_000,
            "maximum_output_bytes": 1_048_576, "timeout_seconds": 60,
        },
        "diagnostic_policy": {
            "prior_predictive_draws": 100, "maximum_r_hat": 1.05,
            "minimum_bulk_ess": 100.0, "minimum_tail_ess": 100.0, "minimum_ebfmi": 0.3,
            "maximum_divergences": 0, "maximum_tree_depth_hits": 0, "maximum_tree_depth": 10,
        },
    }


class AnisotropicGp3dContractTest(unittest.TestCase):
    def test_collinear_axes_are_rejected_but_identifiable_planar_points_are_admitted(self):
        with self.assertRaisesRegex(worker.ContractError, "distinguish"):
            worker.validate(request([[float(i)] * 3 for i in range(8)]), "0" * 64, "0" * 64)

        planar = [[float(i % 4), float(i // 4), float(i % 4 + i // 4)] for i in range(8)]
        for scales in ([1.0, 1.0, 1.0], [1e-6, 1e6, 1e-3]):
            with self.subTest(scales=scales):
                points = [[value * scale for value, scale in zip(point, scales)] for point in planar]
                config = worker.validate(request(points), "0" * 64, "0" * 64)
                self.assertEqual(config["x"].shape, (8, 3))

    def test_joint_prediction_factorization_is_included_in_admission(self):
        points = [[float(i % 2), float((i // 2) % 2), float(i // 4)] for i in range(8)]
        for count in (2, 200):
            with self.subTest(predictions=count):
                config = worker.validate(request(points, count), "0" * 64, "0" * 64)
                self.assertEqual(config["x_new"].shape, (count, 3))
        for count in (256, 512):
            with self.subTest(predictions=count):
                with self.assertRaisesRegex(worker.ContractError, "resource"):
                    worker.validate(request(points, count), "0" * 64, "0" * 64)


if __name__ == "__main__":
    unittest.main()
