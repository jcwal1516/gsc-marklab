from __future__ import annotations

import csv
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("marklab_cellvit_lgcp_adapter.py")


class CellvitLgcpAdapterTest(unittest.TestCase):
    def test_exact_patch_becomes_complete_physical_grid(self) -> None:
        specification = importlib.util.spec_from_file_location("cellvit_lgcp", SCRIPT)
        assert specification is not None and specification.loader is not None
        module = importlib.util.module_from_spec(specification)
        specification.loader.exec_module(module)
        payload = {
            "wsi_metadata": {
                "patch_size": 100,
                "downsampling": 1,
                "patch_overlap": 0,
                "target_patch_mpp": 1.0,
                "marklab_patch_selection": {
                    "selected_grid_coordinates": [[1, 2, 0.0]],
                },
            },
            "type_map": {"1": "Neoplastic"},
            "cells": [
                {
                    "centroid": [210.0, 110.0],
                    "patch_coordinates": [1, 2],
                    "type": 1,
                    "type_prob": 0.9,
                },
                {
                    "centroid": [225.0, 125.0],
                    "patch_coordinates": [1, 2],
                    "type": 1,
                    "type_prob": 0.8,
                },
                {
                    "centroid": [249.0, 149.0],
                    "patch_coordinates": [1, 2],
                    "type": 1,
                    "type_prob": 0.7,
                },
                {
                    "centroid": [260.0, 160.0],
                    "patch_coordinates": [1, 2],
                    "type": 1,
                    "type_prob": 0.6,
                },
                {
                    "centroid": [290.0, 190.0],
                    "patch_coordinates": [1, 2],
                    "type": 1,
                    "type_prob": 0.5,
                },
            ],
        }
        with tempfile.TemporaryDirectory() as raw_directory:
            directory = Path(raw_directory)
            events = directory / "events.csv"
            grid = directory / "grid.csv"
            provenance = directory / "provenance.json"
            result = module.prepare_payload(
                payload,
                source_path=Path("cells.json.snappy"),
                source_sha256="a" * 64,
                patch_row=1,
                patch_column=2,
                grid_x=2,
                grid_y=2,
                events_path=events,
                grid_path=grid,
                provenance_path=provenance,
            )

            with events.open(newline="") as stream:
                event_rows = list(csv.DictReader(stream))
            with grid.open(newline="") as stream:
                grid_rows = list(csv.DictReader(stream))
            recorded = json.loads(provenance.read_text())

        self.assertEqual(result["events"], 5)
        self.assertEqual(len(event_rows), 5)
        self.assertEqual(len(grid_rows), 4)
        self.assertEqual({row["covariate"] for row in grid_rows}, {"-1", "1"})
        self.assertEqual(event_rows[0]["event_id"], "cell:000000000")
        self.assertEqual(event_rows[-1]["event_id"], "cell:000000004")
        self.assertEqual(recorded["window_um"], [200.0, 100.0, 300.0, 200.0])
        self.assertEqual(recorded["source_sha256"], "a" * 64)
        self.assertEqual(recorded["cell_selection"], "all_cells_with_exact_patch_coordinates")


if __name__ == "__main__":
    unittest.main()
