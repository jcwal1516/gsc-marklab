#!/usr/bin/env python3

import csv
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


WORKER_DIRECTORY = Path(__file__).resolve().parents[2] / "workers" / "python"
MODULE_PATH = WORKER_DIRECTORY / "marklab_cellvit_joint_location_embedding_table.py"


def load_module():
    sys.path.insert(0, str(WORKER_DIRECTORY))
    try:
        spec = importlib.util.spec_from_file_location("joint_table", MODULE_PATH)
        module = importlib.util.module_from_spec(spec)
        assert spec.loader is not None
        spec.loader.exec_module(module)
        return module
    finally:
        sys.path.pop(0)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


class JointLocationEmbeddingTableTest(unittest.TestCase):
    def fixture(self, root: Path, outside: bool = False) -> Path:
        prepared = root / "prepared"
        (prepared / "inputs").mkdir(parents=True)
        (prepared / "windows").mkdir()
        rows = []
        for group in ("MSI", "MSS"):
            for patient_index in range(2):
                patient = f"{group.lower()}-{patient_index}"
                for slide_index in range(3):
                    slide = f"{patient}-slide-{slide_index}"
                    input_path = prepared / "inputs" / f"{slide}.csv"
                    window_path = prepared / "windows" / f"{slide}.geojson"
                    window_path.write_text(
                        json.dumps(
                            {
                                "type": "Polygon",
                                "coordinates": [
                                    [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0], [0.0, 0.0]]
                                ],
                            },
                            sort_keys=True,
                            separators=(",", ":"),
                        )
                        + "\n"
                    )
                    fields = ["cell_id", "x_um", "y_um"] + [
                        f"cellvit_pc_{index:03d}" for index in range(16)
                    ]
                    with input_path.open("w", newline="") as stream:
                        writer = csv.DictWriter(stream, fieldnames=fields, lineterminator="\n")
                        writer.writeheader()
                        for cell_index, (x_um, y_um) in enumerate(
                            [(0.5, 0.5), (2.5, 0.5), (2.5, 2.5)]
                        ):
                            if outside and group == "MSI" and patient_index == cell_index == 0:
                                x_um = 5.0
                            writer.writerow(
                                {
                                    "cell_id": f"{slide}:{cell_index:09d}",
                                    "x_um": format(x_um, ".17g"),
                                    "y_um": format(y_um, ".17g"),
                                    **{
                                        f"cellvit_pc_{index:03d}": format(
                                            float(group == "MSI") + index + cell_index / 10.0,
                                            ".17g",
                                        )
                                        for index in range(16)
                                    },
                                }
                            )
                    rows.append(
                        {
                            "patient_id": patient,
                            "group": group,
                            "slide_id": slide,
                            "cell_count": "3",
                            "dimension": "16",
                            "input": str(input_path.relative_to(prepared)),
                            "input_sha256": sha256(input_path),
                            "window": str(window_path.relative_to(prepared)),
                            "window_sha256": sha256(window_path),
                        }
                    )
        manifest = prepared / "manifest.csv"
        with manifest.open("w", newline="") as stream:
            writer = csv.DictWriter(stream, fieldnames=list(rows[0]), lineterminator="\n")
            writer.writeheader()
            writer.writerows(rows)
        (prepared / "design.json").write_text(
            json.dumps(
                {
                    "projected_index_sha256": "a" * 64,
                    "population_unit": "patient",
                    "specimen_unit": "slide_nested_within_patient",
                },
                sort_keys=True,
                separators=(",", ":"),
            )
            + "\n"
        )
        return prepared

    def test_materializes_balanced_exact_correspondence(self):
        module = load_module()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            prepared = self.fixture(root)
            output = root / "output"
            result = module.materialize(prepared, output, patients_per_group=2, grid_size=4)

            with (output / "location.csv").open(newline="") as stream:
                location = list(csv.DictReader(stream))
            with (output / "embedding.csv").open(newline="") as stream:
                embedding = list(csv.DictReader(stream))
            selected_patterns = {row["pattern_id"] for row in embedding}
            self.assertEqual(len(selected_patterns), 8)
            self.assertEqual({row["group"] for row in embedding}, {"MSI", "MSS"})
            self.assertEqual(len(embedding), 24)
            self.assertEqual(sum(int(row["count"]) for row in location), 24)
            self.assertEqual({row["pattern_id"] for row in location}, selected_patterns)
            self.assertEqual({row["type_id"] for row in location}, {"selected_cell"})
            self.assertTrue(all(row["point_id"].startswith(row["pattern_id"] + ":") for row in embedding))
            self.assertEqual(result["patient_count"], 4)
            self.assertEqual(result["pattern_count"], 8)
            self.assertEqual(result["embedding_projection_identity"], "a" * 64)
            self.assertEqual(result["population_unit"], "patient")
            self.assertEqual(sha256(output / "location.csv"), result["location_sha256"])
            self.assertEqual(sha256(output / "embedding.csv"), result["embedding_sha256"])

    def test_rejects_a_cell_outside_its_exact_window(self):
        module = load_module()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            prepared = self.fixture(root, outside=True)
            with self.assertRaisesRegex(module.JointTableError, "outside exact window"):
                module.materialize(prepared, root / "output", patients_per_group=2, grid_size=4)

    def test_cli_publishes_the_consumed_tables(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            prepared = self.fixture(root)
            output = root / "output"
            completed = subprocess.run(
                [
                    sys.executable,
                    str(MODULE_PATH),
                    "--prepared",
                    str(prepared),
                    "--out",
                    str(output),
                    "--patients-per-group",
                    "2",
                    "--grid-size",
                    "4",
                ],
                check=True,
                capture_output=True,
                text=True,
            )
            summary = json.loads(completed.stdout)
            self.assertEqual(summary["patient_count"], 4)
            self.assertEqual(summary["pattern_count"], 8)
            self.assertTrue((output / "location.csv").is_file())
            self.assertTrue((output / "embedding.csv").is_file())
            self.assertTrue((output / "SHA256SUMS").is_file())


if __name__ == "__main__":
    unittest.main()
