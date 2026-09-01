import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_cellvit_patient_local_fields.py"
)


def load_module():
    spec = importlib.util.spec_from_file_location("cellvit_patient_local_fields", MODULE_PATH)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class CellvitPatientLocalFieldsTest(unittest.TestCase):
    def test_coordinate_only_eligibility_removes_radius_isolates(self):
        module = load_module()
        rows = [
            {"cell_id": "a", "x_um": "0", "y_um": "0"},
            {"cell_id": "b", "x_um": "1", "y_um": "0"},
            {"cell_id": "c", "x_um": "100", "y_um": "100"},
        ]
        retained, removed = module.drop_radius_isolates(rows, 2.0)
        self.assertEqual([row["cell_id"] for row in retained], ["a", "b"])
        self.assertEqual(removed, ["c"])

    def test_admission_keeps_every_labeled_patient_with_repeated_slides(self):
        module = load_module()
        rows = [
            {"biological_unit": "p1", "permutation_stratum": "s1"},
            {"biological_unit": "p1", "permutation_stratum": "s2"},
            {"biological_unit": "p2", "permutation_stratum": "s3"},
            {"biological_unit": "p2", "permutation_stratum": "s4"},
            {"biological_unit": "p3", "permutation_stratum": "s5"},
        ]
        admitted = module.admit_repeated_slides(rows, {"p1": "MSI", "p2": "MSS", "p3": "MSS"})
        self.assertEqual(
            admitted,
            [
                {"patient_id": "p1", "group": "MSI", "slide_id": "s1"},
                {"patient_id": "p1", "group": "MSI", "slide_id": "s2"},
                {"patient_id": "p2", "group": "MSS", "slide_id": "s3"},
                {"patient_id": "p2", "group": "MSS", "slide_id": "s4"},
            ],
        )

    def test_local_result_features_are_fixed_and_sign_invariant(self):
        module = load_module()
        document = {
            "format": "marklab.local_multivariate_moran",
            "version": 1,
            "population_claim": "within_specimen_field_diagnostic_only",
            "point_count": 4,
            "dimension": 2,
            "radius_um": 200.0,
            "global_max_abs_statistic": 2.0,
            "global_max_abs_p_value": 0.25,
            "locations": [
                {"statistic": -2.0, "adjusted_p_value": 0.04},
                {"statistic": 1.0, "adjusted_p_value": 0.20},
                {"statistic": -0.5, "adjusted_p_value": 0.02},
                {"statistic": 0.5, "adjusted_p_value": 1.0},
            ],
        }
        self.assertEqual(
            module.local_result_features(document, expected_points=4, expected_radius_um=200.0),
            {
                "local_global_max_abs": 2.0,
                "local_mean_abs": 1.0,
                "local_adjusted_significant_fraction": 0.5,
            },
        )

    def test_variant_stability_keeps_slide_identity_and_reports_magnitude(self):
        module = load_module()
        baseline = {
            "s1": {"a": 1.0, "b": 2.0, "c": 3.0},
            "s2": {"a": 2.0, "b": 1.0, "c": 3.0},
        }
        variants = {
            "cell_subsample_80": {
                "s1": {"a": 1.0, "b": 2.0, "c": 3.0},
                "s2": {"a": 2.0, "b": 1.0, "c": 3.0},
            },
            "nearby_radius_low": {
                "s1": {"a": 1.0, "b": 3.0, "c": 2.0},
                "s2": {"a": 2.0, "b": 1.0, "c": 3.0},
            },
        }
        result = module.variant_stability(baseline, variants)
        self.assertEqual(result["cell_subsample_80"]["comparison_count"], 2)
        self.assertEqual(result["cell_subsample_80"]["median_rank_spearman"], 1.0)
        self.assertEqual(result["cell_subsample_80"]["median_relative_rms_difference"], 0.0)
        self.assertLess(result["nearby_radius_low"]["median_rank_spearman"], 1.0)
        self.assertGreater(result["nearby_radius_low"]["median_relative_rms_difference"], 0.0)

    def test_patient_endpoint_table_requires_every_manifest_result(self):
        module = load_module()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "results").mkdir()
            manifest = [
                {"patient_id": "p1", "group": "MSI", "slide_id": "s1", "cell_count": "2"},
                {"patient_id": "p1", "group": "MSI", "slide_id": "s2", "cell_count": "2"},
            ]
            document = {
                "format": "marklab.local_multivariate_moran",
                "version": 1,
                "population_claim": "within_specimen_field_diagnostic_only",
                "point_count": 2,
                "dimension": 2,
                "radius_um": 200.0,
                "global_max_abs_statistic": 1.0,
                "global_max_abs_p_value": 1.0,
                "locations": [
                    {"statistic": 1.0, "adjusted_p_value": 1.0},
                    {"statistic": -1.0, "adjusted_p_value": 1.0},
                ],
            }
            (root / "results/s1.json").write_text(json.dumps(document), encoding="utf-8")
            with self.assertRaisesRegex(module.LocalFieldError, "missing local result"):
                module.patient_endpoint_rows(manifest, root / "results", 200.0)

    def test_incremental_blocks_average_slides_inside_patients(self):
        module = load_module()
        slide_rows = [
            {"patient_id": "p1", "embedding": [1.0, 3.0], "composition": [0.2, 0.8]},
            {"patient_id": "p1", "embedding": [3.0, 5.0], "composition": [0.4, 0.6]},
            {"patient_id": "p2", "embedding": [10.0, 20.0], "composition": [0.7, 0.3]},
            {"patient_id": "p2", "embedding": [14.0, 24.0], "composition": [0.5, 0.5]},
        ]
        blocks = module.patient_model_blocks(
            slide_rows,
            {"p1": [50.0, 0.0], "p2": [60.0, 1.0]},
            {"p1": [0.1, 0.2, 0.3], "p2": [0.4, 0.5, 0.6]},
        )
        self.assertEqual(blocks["nonspatial_embedding"]["p1"], [2.0, 4.0])
        self.assertAlmostEqual(blocks["composition"]["p1"][0], 0.3)
        self.assertAlmostEqual(blocks["composition"]["p1"][1], 0.7)
        self.assertEqual(blocks["technical"]["p2"], [60.0, 1.0])
        self.assertEqual(blocks["local_field"]["p2"], [0.4, 0.5, 0.6])

    def test_adaptive_spde_features_are_sign_invariant(self):
        module = load_module()
        document = {
            "format": "marklab.adaptive_window_spde",
            "version": 1,
            "claim_status": "fitted_arbitrary_window_spde_diagnostic",
            "window": {"canonical_sha256": "a" * 64},
            "mesh": {"relative_area_error": 0.001},
            "spatial_factor": {
                "factor_count": 1,
                "projected_region_factor": [-2.0, 0.0, 2.0],
                "loadings": [-3.0, 4.0],
                "reconstruction_rmse": 0.25,
                "gradient_maximum": 0.01,
                "iterations": 50,
            },
        }
        self.assertEqual(
            module.adaptive_result_features(document, "a" * 64, 3),
            {
                "spde_factor_sd": (8.0 / 3.0) ** 0.5,
                "spde_loading_l2": 5.0,
                "spde_reconstruction_rmse": 0.25,
            },
        )

    def test_adaptive_spde_execution_resumes_verified_slide_outputs(self):
        module = load_module()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            prepared = root / "prepared"
            output = root / "execution"
            prepared.mkdir()
            row = {"slide_id": "s1", "request": "inputs/s1.json"}
            payload = b'{"format":"marklab.adaptive_window_spde"}'
            for relative in ("miss/s1.json", "hit/s1.json", "results/s1.json"):
                path = output / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(payload)
            ledger = output / "projects/s1/executions.jsonl"
            ledger.parent.mkdir(parents=True)
            ledger.write_text("{}\n", encoding="utf-8")

            result = module._run_adaptive_slide(
                root / "unused-marklab", prepared, output, row
            )
            self.assertTrue(result["resumed"])
            self.assertEqual(result["ledger_rows"], 1)


if __name__ == "__main__":
    unittest.main()
