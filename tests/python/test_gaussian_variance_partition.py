import importlib.util
import unittest
from pathlib import Path

import numpy as np

MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_pymc_gaussian_crossed_nested_hierarchy_worker.py"
)
spec = importlib.util.spec_from_file_location("crossed_nested_worker", MODULE_PATH)
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)


class GaussianVariancePartitionTest(unittest.TestCase):
    def test_variance_shares_match_the_observation_covariance(self):
        for patient_counts in ([4, 4, 4], [4, 4, 5]):
            with self.subTest(patient_counts=patient_counts):
                parent_cohort = np.repeat(np.arange(3), patient_counts)
                patients = len(parent_cohort)
                parent_patient = np.repeat(np.arange(patients), 2)
                parent_slide = np.repeat(np.arange(patients * 2), 2)
                patient_index = np.repeat(np.arange(patients), 8)
                observations = len(patient_index)
                exposure = np.linspace(-2.0, 3.0, observations) ** 2
                exposure = (exposure - exposure.mean()) / exposure.std()
                config = {
                    "exposure": exposure,
                    "cohort_index": parent_cohort[patient_index],
                    "patient_index": patient_index,
                    "slide_index": np.repeat(np.arange(patients * 2), 4),
                    "roi_index": np.repeat(np.arange(patients * 4), 2),
                    "batch_index": np.arange(observations) % 2,
                    "cohort_center": worker.center_global(3),
                    "patient_center": worker.center_within(parent_cohort),
                    "slide_center": worker.center_within(parent_patient),
                    "roi_center": worker.center_within(parent_slide),
                    "batch_center": worker.center_global(2),
                }
                arrays = {name: np.ones(2) for name in worker.COMPONENT_NAMES}
                arrays["residual_sd"] = np.asarray([1.0, 2.0])

                # Construct each random effect's actual observation covariance at unit
                # scale. Its average diagonal is an independent dense-matrix oracle.
                variances = []
                for name in worker.COMPONENT_NAMES:
                    if name == "residual_sd":
                        covariance = np.eye(observations)
                    else:
                        level = name.split("_")[0]
                        center = config[f"{level}_center"]
                        design = np.eye(len(center))[config[f"{level}_index"]] @ center
                        if "slope" in name:
                            design = exposure[:, None] * design
                        covariance = design @ design.T
                    variances.append(
                        np.trace(covariance) / observations * arrays[name] ** 2
                    )
                variances = np.asarray(variances).T
                expected = (variances / variances.sum(axis=1, keepdims=True)).mean(
                    axis=0
                )

                result = worker.variance_partition(config, arrays)
                actual = [
                    result[name.removesuffix("_sd")] for name in worker.COMPONENT_NAMES
                ]
                np.testing.assert_allclose(actual, expected, rtol=1e-12, atol=1e-12)
                if patient_counts == [4, 4, 4]:
                    # Unit scales yield residual share 3/16; residual SD 2 yields 12/25.
                    self.assertAlmostEqual(result["residual"], (3 / 16 + 12 / 25) / 2)


if __name__ == "__main__":
    unittest.main()
