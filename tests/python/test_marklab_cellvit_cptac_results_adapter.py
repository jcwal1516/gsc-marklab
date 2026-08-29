import importlib.util
from collections import Counter
from pathlib import Path
import struct
import unittest


MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_cellvit_cptac_results_adapter.py"
)


class CellvitCptacResultsAdapterTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        spec = importlib.util.spec_from_file_location("cellvit_cptac_adapter", MODULE_PATH)
        cls.module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cls.module)

    def test_source_cell_id_is_exact_and_shared_across_bounded_lanes(self):
        selected = self.module.bounded_indices(10_000, 32, "slide-uuid")
        identities = [
            self.module.source_cell_id("slide-uuid", row) for row in selected
        ]

        self.assertEqual(identities, sorted(identities))
        self.assertEqual(identities[0], f"slide-uuid:{selected[0]:09d}")
        self.assertEqual(len(set(identities)), len(selected))

    def test_binary_threshold_uses_the_exact_exported_f32_probability(self):
        for source in (0.7499999, 0.7499999999, 0.75, 0.7500001):
            encoded, marked = self.module.canonical_f32_probability(source, 0.75)
            imported = struct.unpack("!f", struct.pack("!f", float(encoded)))[0]
            self.assertEqual(marked, int(imported >= 0.75))

    def test_multiclass_group_counts_retain_zero_classes_per_patient(self):
        rows = self.module.dirichlet_multinomial_group_rows(
            {
                "patient-b": Counter({0: 3, 1: 1}),
                "patient-a": Counter({1: 2, 2: 2}),
                "unlabeled": Counter({0: 9}),
            },
            ((0, "Neoplastic"), (1, "Inflammatory"), (2, "Connective")),
            {"patient-a": "MSI", "patient-b": "MSS"},
        )

        self.assertEqual(
            rows,
            [
                {
                    "patient_id": "patient-a",
                    "group": "MSI",
                    "class_id": "Neoplastic",
                    "count": 0,
                },
                {
                    "patient_id": "patient-a",
                    "group": "MSI",
                    "class_id": "Inflammatory",
                    "count": 2,
                },
                {
                    "patient_id": "patient-a",
                    "group": "MSI",
                    "class_id": "Connective",
                    "count": 2,
                },
                {
                    "patient_id": "patient-b",
                    "group": "MSS",
                    "class_id": "Neoplastic",
                    "count": 3,
                },
                {
                    "patient_id": "patient-b",
                    "group": "MSS",
                    "class_id": "Inflammatory",
                    "count": 1,
                },
                {
                    "patient_id": "patient-b",
                    "group": "MSS",
                    "class_id": "Connective",
                    "count": 0,
                },
            ],
        )

    def test_sparse_graph_heat_input_retains_cell_identity_and_neoplastic_signal(self):
        result = self.module.sparse_radius_heat_input(
            [
                {
                    "cell_id": "slide:000000001",
                    "x_um": "10.5",
                    "y_um": "20.25",
                    "histologic_compartment": "Inflammatory",
                },
                {
                    "cell_id": "slide:000000002",
                    "x_um": "11.5",
                    "y_um": "20.25",
                    "histologic_compartment": "Neoplastic",
                },
            ]
        )

        self.assertEqual(result["nodes"][0]["id"], "slide:000000001")
        self.assertEqual(result["nodes"][0]["signal"], 0.0)
        self.assertEqual(result["nodes"][1]["signal"], 1.0)
        self.assertEqual(result["maximum_nodes"], 2)
        self.assertEqual(result["radius_um"], 50.0)
        self.assertGreaterEqual(result["maximum_edges"], 1)

    def test_witness_persistence_input_retains_exact_cell_identity_and_coordinates(self):
        result = self.module.witness_persistence_input(
            [
                {
                    "cell_id": "slide:000000001",
                    "x_um": "10.5",
                    "y_um": "20.25",
                },
                {
                    "cell_id": "slide:000000002",
                    "x_um": "11.5",
                    "y_um": "20.25",
                },
                {
                    "cell_id": "slide:000000003",
                    "x_um": "11.5",
                    "y_um": "21.25",
                },
            ]
        )

        self.assertEqual(
            result["points"],
            [
                {"id": "slide:000000001", "coordinates_um": [10.5, 20.25]},
                {"id": "slide:000000002", "coordinates_um": [11.5, 20.25]},
                {"id": "slide:000000003", "coordinates_um": [11.5, 21.25]},
            ],
        )
        self.assertEqual(result["landmark_method"], "farthest_point")
        self.assertEqual(result["landmark_count"], 2)
        self.assertEqual(result["maximum_dimension"], 2)
        self.assertEqual(result["nu"], 0)
        self.assertEqual(result["max_scale_um"], 200.0)
        self.assertEqual(result["maximum_simplices"], 500_000)
        self.assertEqual(result["timeout_seconds"], 180)

    def test_arbitrary_window_ipp_events_retain_identity_and_fixed_x_covariate(self):
        result = self.module.arbitrary_window_ipp_event_rows(
            [
                {"cell_id": "slide:000000001", "x_um": "10", "y_um": "20"},
                {"cell_id": "slide:000000002", "x_um": "11", "y_um": "21"},
            ],
            (10.0, 20.0, 12.0, 22.0),
        )

        self.assertEqual(
            result,
            [
                {
                    "event_id": "slide:000000001",
                    "x_um": "10",
                    "y_um": "20",
                    "covariate": "-1",
                    "offset": 0,
                },
                {
                    "event_id": "slide:000000002",
                    "x_um": "11",
                    "y_um": "21",
                    "covariate": "0",
                    "offset": 0,
                },
            ],
        )

    def test_arbitrary_window_ipp_events_retain_quadrature_membership_when_requested(self):
        result = self.module.arbitrary_window_ipp_event_membership_rows(
            [
                {"cell_id": "slide:000000001", "x_um": "10", "y_um": "20"},
                {"cell_id": "slide:000000002", "x_um": "12", "y_um": "22"},
            ],
            (10.0, 20.0, 12.0, 22.0),
            8,
        )

        self.assertEqual(result[0]["quadrature_node_id"], "q-000-000")
        self.assertEqual(result[1]["quadrature_node_id"], "q-007-007")

        coarse = self.module.arbitrary_window_ipp_event_membership_rows(
            [
                {"cell_id": "slide:000000001", "x_um": "10", "y_um": "20"},
                {"cell_id": "slide:000000002", "x_um": "12", "y_um": "22"},
            ],
            (10.0, 20.0, 12.0, 22.0),
            4,
        )
        self.assertEqual(coarse[0]["quadrature_node_id"], "q-000-000")
        self.assertEqual(coarse[1]["quadrature_node_id"], "q-003-003")


if __name__ == "__main__":
    unittest.main()
