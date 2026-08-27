import importlib.util
from pathlib import Path
import unittest


MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_schurch_codex_fingerprint_adapter.py"
)


class SchurchCodexFingerprintAdapterTest(unittest.TestCase):
    def test_declared_neighborhood_vocabulary_is_harmonized_without_protein_vectors(self):
        spec = importlib.util.spec_from_file_location("codex_adapter", MODULE_PATH)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)

        self.assertEqual(
            module.NEIGHBORHOODS["1"],
            (
                "T cell enriched",
                "embedding_microenvironment_t_cell_enriched_fraction",
            ),
        )
        self.assertEqual(
            module.NEIGHBORHOODS["6"],
            (
                "Tumor boundary",
                "embedding_microenvironment_tumor_boundary_fraction",
            ),
        )
        self.assertEqual(len(module.NEIGHBORHOODS), 9)
        self.assertNotIn("protein", " ".join(module.FEATURES).lower())
        self.assertFalse(
            module.admit_cell("dirt", "1", {"dirt", "undefined"})
        )
        self.assertFalse(
            module.admit_cell("tumor cells", "N/A", {"dirt", "undefined"})
        )
        self.assertTrue(
            module.admit_cell("tumor cells", "2", {"dirt", "undefined"})
        )
        self.assertEqual(
            module.cell_subsample_admitted("core-a:cell-1"),
            module.cell_subsample_admitted("core-a:cell-1"),
        )
        self.assertTrue(
            any(
                module.cell_subsample_admitted(f"core-a:cell-{index}")
                for index in range(20)
            )
        )
        self.assertTrue(
            any(
                not module.cell_subsample_admitted(f"core-a:cell-{index}")
                for index in range(20)
            )
        )
        self.assertEqual(
            module.canonical_patient_order(["2", "10", "1"]),
            ["1", "10", "2"],
        )


if __name__ == "__main__":
    unittest.main()
