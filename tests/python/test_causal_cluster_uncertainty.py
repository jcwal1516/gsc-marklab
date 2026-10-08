import importlib.util
from pathlib import Path
import unittest

import numpy as np
from scipy.optimize import minimize
from scipy.special import expit


MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_scipy_causal_active_worker.py"
)


def load_module():
    spec = importlib.util.spec_from_file_location("causal_active_worker", MODULE_PATH)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


class ClusterUncertaintyTest(unittest.TestCase):
    def test_validation_does_not_report_unexecuted_controls_as_passed(self):
        worker = load_module()
        result = worker.validation(229)
        entries = {entry["validation_id"]: entry for entry in result["entries"]}
        for name in (
            "exposure_probability_normalization",
            "aipw_one_correct_nuisance",
            "cluster_dml_coverage",
            "positivity_failure_detection",
            "negative_control_null",
            "active_information_beats_random",
            "biological_replication_preference",
            "power_monotonicity",
        ):
            with self.subTest(control=name):
                self.assertEqual(entries[name]["status"], "not_verified_unexecuted_control")
                self.assertIsNone(entries[name]["value"])
        self.assertEqual(
            result["overall_status"],
            "partial_synthetic_and_prospective_evidence_required",
        )

    def test_observational_standard_error_uses_independent_clusters(self):
        worker = load_module()
        rows = []
        for cluster in range(8):
            for unit in range(12):
                x = ((unit * 7 + cluster * 3) % 23) / 11.0 - 1.0
                treatment = int((unit + 2 * cluster) % 5 < 2)
                dose = 0.25 + 0.5 * treatment + 0.2 * x
                exposure = ((unit + cluster) % 4) / 3.0
                noise = ((unit * 13 + cluster * 5) % 17) / 80.0 - 0.1
                rows.append(
                    {
                        "unit_id": f"c{cluster:02}u{unit:02}",
                        "cluster_id": f"c{cluster:02}",
                        "baseline_covariates": [x],
                        "treatment": treatment,
                        "dose": dose,
                        "neighbor_exposure": exposure,
                        "outcome": 0.4 + 2.0 * dose + 0.75 * exposure + 0.6 * x + noise,
                        "negative_control_outcome": -0.2 + 0.3 * x + noise,
                    }
                )
        spec = {
            "rows": rows,
            "cluster_folds": 4,
            "propensity_clip": 0.01,
            "dose_basis_degree": 2,
        }
        original = worker.observational(spec)
        self.assertEqual(original["version"], 2)
        self.assertNotIn("exposure_aipw", original)
        self.assertIn("adjusted_exposure_regression", original)
        self.assertEqual(
            [term["power"] for term in original["dose_response"]["polynomial_terms"]],
            [1, 2],
        )
        quadratic = worker.observational({
            **spec,
            "rows": [
                {**row, "outcome": row["outcome"] + 3.0 * row["dose"] ** 2}
                for row in rows
            ],
        })
        for index, expected_change in [(0, 0.0), (1, 3.0)]:
            self.assertAlmostEqual(
                quadratic["dose_response"]["polynomial_terms"][index]["coefficient"]
                - original["dose_response"]["polynomial_terms"][index]["coefficient"],
                expected_change,
                places=10,
            )
        duplicated_rows = []
        for row in rows:
            for copy in range(2):
                duplicated_rows.append({**row, "unit_id": f"{row['unit_id']}-{copy}"})
        duplicated = worker.observational({**spec, "rows": duplicated_rows})

        self.assertAlmostEqual(
            duplicated["dose_response"]["standard_error"],
            original["dose_response"]["standard_error"],
            places=13,
        )

        x = np.asarray([row["baseline_covariates"] for row in rows], dtype=float)
        treatment = np.asarray([row["treatment"] for row in rows], dtype=float)
        outcome = np.asarray([row["outcome"] for row in rows], dtype=float)
        design = np.column_stack([np.ones(len(rows)), x])
        cluster_ids = [row["cluster_id"] for row in rows]
        fold_for = {
            cluster_id: index % spec["cluster_folds"]
            for index, cluster_id in enumerate(sorted(set(cluster_ids)))
        }
        propensity = np.empty(len(rows))
        mean_control = np.empty(len(rows))
        mean_treated = np.empty(len(rows))
        for fold in range(spec["cluster_folds"]):
            heldout = np.asarray([fold_for[value] == fold for value in cluster_ids])
            training = ~heldout
            fit = minimize(
                lambda coefficients: float(
                    np.sum(
                        np.logaddexp(0.0, design[training] @ coefficients)
                        - treatment[training] * (design[training] @ coefficients)
                    )
                    + 1e-6 * (coefficients @ coefficients)
                ),
                np.zeros(design.shape[1]),
                method="BFGS",
            )
            self.assertTrue(fit.success)
            propensity[heldout] = expit(design[heldout] @ fit.x)
            control = training & (treatment == 0.0)
            treated = training & (treatment == 1.0)
            mean_control[heldout] = design[heldout] @ np.linalg.lstsq(
                design[control], outcome[control], rcond=None
            )[0]
            mean_treated[heldout] = design[heldout] @ np.linalg.lstsq(
                design[treated], outcome[treated], rcond=None
            )[0]
        expected_scores = (
            mean_treated
            - mean_control
            + treatment * (outcome - mean_treated) / propensity
            - (1.0 - treatment) * (outcome - mean_control) / (1.0 - propensity)
        )
        self.assertAlmostEqual(
            original["cross_fitted_aipw"]["effect"],
            float(expected_scores.mean()),
            places=13,
        )

    def test_cluster_mean_standard_error_uses_cluster_sums(self):
        worker = load_module()
        values = np.asarray([1.0, 1.0, -1.0, -1.0, 2.0, 2.0, -2.0, -2.0])
        clusters = np.repeat(["a", "b", "c", "d"], 2)

        actual = worker.cluster_mean_standard_error(values, clusters)
        repeated = worker.cluster_mean_standard_error(
            np.repeat(values, 3), np.repeat(clusters, 3)
        )
        centered_cluster_sums = np.asarray([2.0, -2.0, 4.0, -4.0])
        expected = np.sqrt(
            4.0 / 3.0 * np.sum(centered_cluster_sums**2) / len(values) ** 2
        )

        self.assertAlmostEqual(actual, expected, places=15)
        self.assertAlmostEqual(repeated, actual, places=15)

    def test_cluster_randomized_regression_does_not_count_rows_as_replicates(self):
        worker = load_module()
        rows = []
        for cluster in range(8):
            treatment = float(cluster % 2)
            for unit in range(5):
                exposure = ((unit + cluster) % 4) / 3.0
                mediator_noise = ((unit * 3 + cluster) % 7) / 50.0 - 0.06
                outcome_noise = ((unit * 5 + cluster * 2) % 11) / 40.0 - 0.125
                mediator = 0.3 + 1.5 * treatment + 0.5 * exposure + mediator_noise
                rows.append(
                    {
                        "unit_id": f"c{cluster:02}u{unit:02}",
                        "cluster_id": f"c{cluster:02}",
                        "treatment": treatment,
                        "neighbor_exposure": exposure,
                        "mediator": mediator,
                        "outcome": -0.2
                        + treatment
                        + 0.8 * mediator
                        + 0.4 * exposure
                        + outcome_noise,
                        "negative_control_outcome": outcome_noise,
                    }
                )
        spec = {
            "rows": rows,
            "assignment": "cluster_randomized",
            "temporal_order": ["treatment", "mediator", "outcome"],
            "mediator_outcome_no_unmeasured_confounding_declared": True,
            "sensitivity_shifts": [-0.2, 0.0, 0.2],
        }
        original = worker.perturbation(spec)
        duplicated_rows = [
            {**row, "unit_id": f"{row['unit_id']}-{copy}"}
            for row in rows
            for copy in range(3)
        ]
        duplicated = worker.perturbation({**spec, "rows": duplicated_rows})

        self.assertAlmostEqual(
            duplicated["primary"]["direct_standard_error"],
            original["primary"]["direct_standard_error"],
            places=13,
        )


if __name__ == "__main__":
    unittest.main()
