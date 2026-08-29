import importlib.util
import csv
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_crc_graph_topology_final.py"
)


def load_module():
    spec = importlib.util.spec_from_file_location("crc_graph_topology_final", MODULE_PATH)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def marks():
    rows = []
    for patient_index, patient in enumerate(("P1", "P2", "P3", "P4")):
        group = "MSI" if patient_index < 2 else "MSS"
        for specimen_index in range(2):
            pattern = f"{patient}-S{specimen_index + 1}"
            for point_index in range(10):
                rows.append(
                    {
                        "pattern_id": pattern,
                        "patient_id": patient,
                        "group": group,
                        "point_id": f"{pattern}:{point_index:09d}",
                        "x_um": str(20 * patient_index + point_index),
                        "y_um": str(10 * specimen_index + point_index % 3),
                        "type_id": ("Neoplastic", "Inflammatory", "Connective")[
                            point_index % 3
                        ],
                    }
                )
    return rows


class CrcGraphTopologyFinalTest(unittest.TestCase):
    def test_prepare_cli_writes_the_typed_analysis_design(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "marks.csv"
            with source.open("w", newline="", encoding="utf-8") as target:
                writer = csv.DictWriter(target, fieldnames=list(marks()[0]))
                writer.writeheader()
                writer.writerows(marks())
            output = root / "prepared"

            subprocess.run(
                [
                    sys.executable,
                    str(MODULE_PATH),
                    "prepare",
                    "--marks",
                    str(source),
                    "--out",
                    str(output),
                ],
                check=True,
                capture_output=True,
                text=True,
            )

            design = load_module().read_json(output / "design.json")
            self.assertEqual(design["population_unit"], "patient")
            self.assertEqual(design["patient_count"], 4)
            self.assertFalse(design["selection_uses_molecular_label"])

    def test_prepare_preserves_patient_nesting_and_deterministic_bounded_requests(self):
        module = load_module()
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "prepared"
            manifest = module.prepare_requests(marks(), output, seed=20260829)

            self.assertEqual(len(manifest), 8)
            self.assertEqual({row["patient_id"] for row in manifest}, {"P1", "P2", "P3", "P4"})
            self.assertTrue(all(row["pattern_count_for_patient"] == 2 for row in manifest))
            baseline = module.read_json(output / manifest[0]["graph_baseline_request"])
            subsample = module.read_json(output / manifest[0]["graph_subsample_request"])
            topology_subsample = module.read_json(
                output / manifest[0]["topology_subsample_request"]
            )
            witness = module.read_json(output / manifest[0]["topology_stability_request"])
            self.assertEqual(baseline["radius_um"], 50.0)
            self.assertEqual(baseline["times"], [0.025, 0.05, 0.1, 0.2])
            self.assertEqual(baseline["scattering_order"], 2)
            self.assertEqual(len(baseline["nodes"]), 10)
            self.assertEqual(len(subsample["nodes"]), 8)
            self.assertEqual(len(topology_subsample["points"]), 8)
            self.assertEqual(witness["perturbation_replicates"], 4)
            self.assertEqual(witness["maximum_coordinate_jitter_um"], 1.0)
            self.assertEqual(witness["maximum_backend_executions"], 5)

            second = output / "second"
            replay = module.prepare_requests(marks(), second, seed=20260829)
            self.assertEqual(
                (output / manifest[0]["graph_subsample_request"]).read_bytes(),
                (second / replay[0]["graph_subsample_request"]).read_bytes(),
            )

    def test_fusion_gate_rejects_an_unstable_or_nonincremental_block(self):
        module = load_module()
        stable = {
            "cell_subsample": {"median": 0.96, "q10": 0.82},
            "specimen_leave_one_out": {"median": 0.90, "q10": 0.70},
            "coordinate_perturbation": {"median": 0.95, "q10": 0.80},
            "nearby_scale": {"median": 0.88, "q10": 0.65},
        }
        self.assertTrue(module.fusion_gate(stable, 0.125)["eligible"])

        unstable = dict(stable)
        unstable["coordinate_perturbation"] = {"median": 0.89, "q10": 0.80}
        rejected = module.fusion_gate(unstable, 0.125)
        self.assertFalse(rejected["eligible"])
        self.assertIn("coordinate_perturbation_median", rejected["failed_checks"])

        nonincremental = module.fusion_gate(stable, 0.0)
        self.assertFalse(nonincremental["eligible"])
        self.assertIn("heldout_balanced_accuracy_increment", nonincremental["failed_checks"])

    def test_nonspatial_embedding_summary_is_cell_order_invariant(self):
        module = load_module()
        first = module.embedding_summary([[1.0, 4.0], [3.0, 2.0], [2.0, 3.0]])
        second = module.embedding_summary([[2.0, 3.0], [1.0, 4.0], [3.0, 2.0]])

        self.assertEqual(first, second)
        self.assertEqual(first["cell_count"], 3)
        self.assertEqual(first["embedding_width"], 2)
        self.assertEqual(first["mean"], [2.0, 3.0])
        self.assertAlmostEqual(first["standard_deviation"][0], (2.0 / 3.0) ** 0.5)

    def test_heldout_model_fits_every_transform_inside_the_patient_fold(self):
        module = load_module()
        labels = {"A": "MSI", "B": "MSI", "C": "MSS", "D": "MSS"}
        blocks = {
            "composition_technical": {
                "A": [2.0, 0.0],
                "B": [1.5, 0.1],
                "C": [-2.0, 0.0],
                "D": [-1.5, -0.1],
            },
            "nonspatial_embedding": {
                "A": [4.0, 3.0, 2.0, 1.0],
                "B": [3.5, 2.5, 1.5, 0.5],
                "C": [-4.0, -3.0, -2.0, -1.0],
                "D": [-3.5, -2.5, -1.5, -0.5],
            },
        }

        result = module.heldout_model(labels, blocks, pca_components=2)

        self.assertEqual(result["population_unit"], "patient")
        self.assertEqual(result["patient_count"], 4)
        self.assertEqual(len(result["predictions"]), 4)
        self.assertEqual(result["balanced_accuracy"], 1.0)
        self.assertEqual(result["retrieval_group_accuracy"], 1.0)
        self.assertEqual(
            result["preprocessing"],
            "standardization_and_pca_fit_inside_each_patient_held_out_training_fold",
        )

    def test_feature_stability_uses_patients_not_specimens_as_rank_units(self):
        module = load_module()
        reference = {
            "P1": {"a": 1.0, "b": 4.0},
            "P2": {"a": 2.0, "b": 3.0},
            "P3": {"a": 3.0, "b": 2.0},
            "P4": {"a": 4.0, "b": 1.0},
        }
        same_ranks = {
            "P1": {"a": 10.0, "b": 40.0},
            "P2": {"a": 20.0, "b": 30.0},
            "P3": {"a": 30.0, "b": 20.0},
            "P4": {"a": 40.0, "b": 10.0},
        }

        result = module.feature_stability(reference, [same_ranks])

        self.assertEqual(result["population_unit"], "patient")
        self.assertEqual(result["patient_count"], 4)
        self.assertEqual(result["feature_count"], 2)
        self.assertEqual(result["median"], 1.0)
        self.assertEqual(result["q10"], 1.0)


if __name__ == "__main__":
    unittest.main()
