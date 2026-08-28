import importlib.util
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


if __name__ == "__main__":
    unittest.main()
