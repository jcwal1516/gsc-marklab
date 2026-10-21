import importlib.util
import unittest
from pathlib import Path

import numpy as np
from numpyro.infer.util import log_density
from scipy.stats import halfnorm, nbinom, norm

MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_numpyro_negative_binomial_hierarchy_worker.py"
)


def load_worker():
    specification = importlib.util.spec_from_file_location(
        "negative_binomial_hierarchy_worker", MODULE_PATH
    )
    module = importlib.util.module_from_spec(specification)
    assert specification.loader is not None
    specification.loader.exec_module(module)
    return module


class NegativeBinomialHierarchyMathTest(unittest.TestCase):
    def test_model_nb2_likelihood_and_complete_joint_density_match_scipy(self):
        worker = load_worker()
        counts = np.asarray([0, 3, 2, 8, 1, 5], dtype=np.int64)
        areas = np.asarray([0.5, 1.2, 0.8, 2.0, 1.5, 0.7])
        within = np.asarray([-1.1, 0.4, 0.7, -0.8, 0.1, 0.7])
        patient_index = np.asarray([0, 0, 0, 1, 1, 1], dtype=np.int32)
        slide_index = np.asarray([0, 0, 1, 2, 2, 3], dtype=np.int32)
        priors = {
            "intercept_mean": -0.2,
            "intercept_sd": 1.3,
            "slope_sd": 0.9,
            "log_dispersion_mean": 1.0,
            "log_dispersion_sd": 0.6,
            "patient_intercept_sd_scale": 0.7,
            "patient_slope_sd_scale": 0.5,
            "slide_sd_scale": 0.4,
        }
        state = {
            "alpha": 0.15,
            "beta": 0.55,
            "log_dispersion": np.log(4.2),
            "patient_intercept_sd": 0.35,
            "patient_slope_sd": 0.22,
            "slide_sd": 0.18,
            "patient_intercept_z": np.asarray([-0.4, 0.8]),
            "patient_slope_z": np.asarray([0.3, -0.6]),
            "slide_z": np.asarray([-0.5, 0.2, 0.7, -0.1]),
        }

        eta = (
            state["alpha"]
            + state["beta"] * within
            + state["patient_intercept_sd"]
            * state["patient_intercept_z"][patient_index]
            + state["patient_slope_sd"]
            * state["patient_slope_z"][patient_index]
            * within
            + state["slide_sd"] * state["slide_z"][slide_index]
        )
        means = areas * np.exp(eta)
        dispersion = np.exp(state["log_dispersion"])
        probability = dispersion / (dispersion + means)
        expected = float(np.sum(nbinom.logpmf(counts, dispersion, probability)))
        expected += float(
            norm.logpdf(
                state["alpha"], priors["intercept_mean"], priors["intercept_sd"]
            )
        )
        expected += float(norm.logpdf(state["beta"], 0.0, priors["slope_sd"]))
        expected += float(
            norm.logpdf(
                state["log_dispersion"],
                priors["log_dispersion_mean"],
                priors["log_dispersion_sd"],
            )
        )
        expected += float(
            halfnorm.logpdf(
                state["patient_intercept_sd"],
                scale=priors["patient_intercept_sd_scale"],
            )
        )
        expected += float(
            halfnorm.logpdf(
                state["patient_slope_sd"],
                scale=priors["patient_slope_sd_scale"],
            )
        )
        expected += float(
            halfnorm.logpdf(state["slide_sd"], scale=priors["slide_sd_scale"])
        )
        expected += float(np.sum(norm.logpdf(state["patient_intercept_z"], 0.0, 1.0)))
        expected += float(np.sum(norm.logpdf(state["patient_slope_z"], 0.0, 1.0)))
        expected += float(np.sum(norm.logpdf(state["slide_z"], 0.0, 1.0)))

        model_actual, trace = log_density(
            worker.model,
            (),
            {
                "counts": worker.jnp.asarray(counts),
                "areas": worker.jnp.asarray(areas),
                "within_predictor": worker.jnp.asarray(within),
                "patient_index": worker.jnp.asarray(patient_index),
                "slide_index": worker.jnp.asarray(slide_index),
                "patient_template": worker.jnp.zeros(2),
                "slide_template": worker.jnp.zeros(4),
                "priors": priors,
            },
            state,
        )
        likelihood_site = trace["count_log_likelihood"]
        likelihood_actual = float(
            np.asarray(likelihood_site["fn"].log_prob(likelihood_site["value"])).sum()
        )
        likelihood_expected = float(
            np.sum(nbinom.logpmf(counts, dispersion, probability))
        )
        self.assertAlmostEqual(likelihood_actual, likelihood_expected, delta=1e-10)
        self.assertAlmostEqual(float(model_actual), expected, delta=1e-8)

    def test_predictor_compilation_is_stable_to_large_coordinate_origin(self):
        worker = load_worker()

        def observations(origin):
            rows = []
            for patient in range(8):
                for slide in range(2):
                    for roi, predictor in enumerate((0.0, 0.0, 0.0, 1.0)):
                        rows.append(
                            {
                                "patient_id": f"patient-{patient:02d}",
                                "slide_id": f"slide-{patient:02d}-{slide}",
                                "roi_id": f"roi-{patient:02d}-{slide}-{roi}",
                                "count": 0,
                                "area": 1.0,
                                "predictor": origin + predictor,
                            }
                        )
            return rows

        reference = worker.compile_design(observations(0.0))
        shifted = worker.compile_design(observations(float(2**52)))

        np.testing.assert_array_equal(
            shifted["within_predictor"], reference["within_predictor"]
        )
        self.assertEqual(
            shifted["within_predictor_sd"], reference["within_predictor_sd"]
        )


if __name__ == "__main__":
    unittest.main()
