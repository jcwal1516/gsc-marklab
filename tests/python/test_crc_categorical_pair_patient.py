import csv
import importlib.util
import json
import math
from pathlib import Path
import stat
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = ROOT / "workers/python/marklab_crc_categorical_pair_patient.py"


def load_module():
    spec = importlib.util.spec_from_file_location("crc_categorical_pair_patient", MODULE_PATH)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class FakeWindow:
    def covers(self, point):
        x, y = point
        return 0.0 <= x <= 100.0 and 0.0 <= y <= 100.0


class FakeAdapter:
    @staticmethod
    def one_file(root, pattern):
        matches = list(root.glob(pattern))
        if len(matches) != 1:
            raise ValueError("source file count differs")
        return matches[0]

    @staticmethod
    def read_cells(path):
        return json.loads(path.read_text(encoding="utf-8"))

    @staticmethod
    def patch_window(metadata):
        geometry = {
            "type": "MultiPolygon",
            "coordinates": [[[[0, 0], [100, 0], [100, 100], [0, 100], [0, 0]]]],
        }
        return geometry, FakeWindow()

    @staticmethod
    def projected_row_index(cell_id, roi_id):
        return int(cell_id.removeprefix(f"{roi_id}:"))


FAKE_MARKLAB = r'''#!/usr/bin/env python3
import csv
import json
import math
import os
from pathlib import Path
import sys

args = sys.argv[1:]
if args[:2] in (
    ["project", "categorical-pair"],
    ["project", "categorical-cross-pair-correlation"],
):
    def value(name):
        return args[args.index(name) + 1]
    project = Path(value("--project"))
    output = Path(value("--out"))
    stored = project / "stored.json"
    if os.environ.get("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION") == "1":
        if not stored.is_file():
            raise SystemExit("backend-disabled replay missed")
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(stored.read_bytes())
        print(f"project {args[1]} cache_status=hit", file=sys.stderr)
        raise SystemExit(0)
    source = value("--source-level")
    target = value("--target-level")
    pattern = project.parts[-2]
    radii = [float(raw) for raw in value("--radii-um").split(",")]
    expected = 0.25
    curve = []
    for index, radius in enumerate(radii):
        delta = ((sum(map(ord, pattern + source + target)) + index) % 7 - 3) / 100.0
        theoretical = math.pi * radius * radius
        eligible = not (
            pattern == "P8-S2"
            and source == "Connective"
            and target == "Neoplastic"
            and index == len(radii) - 1
        )
        if args[1] == "categorical-pair":
            curve.append(
                {
                    "radius_um": radius,
                    "connection_probability": expected + delta if eligible else None,
                    "cross_k": theoretical * (1.0 + delta) if eligible else None,
                    "theoretical_cross_k": theoretical,
                    "connection_inference_eligible": eligible,
                    "cross_k_inference_eligible": eligible,
                }
            )
        else:
            curve.append(
                {
                    "radius_um": radius,
                    "cross_g": 1.0 + delta if eligible else None,
                    "theoretical_cross_g": 1.0,
                    "inference_eligible": eligible,
                }
            )
    if args[1] == "categorical-pair":
        document = {
            "format": "marklab.categorical-pair/1",
            "source_level": source,
            "target_level": target,
            "source_count": 2,
            "target_count": 2,
            "expected_random_label_connection": expected,
            "curve": curve,
            "inference": {"permutations_requested": 99, "permutations_completed": 99},
        }
    else:
        document = {
            "source_level": source,
            "target_level": target,
            "source_count": 2,
            "target_count": 2,
            "kernel": "epanechnikov",
            "bandwidth_um": 10.0,
            "curve": curve,
            "inference": {
                "null_model": "random_labeling",
                "permutation_unit": "complete_categorical_row",
                "permutations_completed": 99,
            },
        }
    encoded = (json.dumps(document, sort_keys=True) + "\n").encode()
    project.mkdir(parents=True, exist_ok=True)
    stored.write_bytes(encoded)
    (project / "executions.jsonl").write_text("{}\n", encoding="utf-8")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(encoded)
    print(f"project {args[1]} cache_status=miss", file=sys.stderr)
elif args[:2] == ["cohort", "max-t"]:
    def value(name):
        return args[args.index(name) + 1]
    rows = list(csv.DictReader(Path(value("--input")).open(encoding="utf-8")))
    endpoints = []
    for endpoint in sorted({row["endpoint"] for row in rows}):
        msi = [float(row["value"]) for row in rows if row["endpoint"] == endpoint and row["group"] == "MSI"]
        mss = [float(row["value"]) for row in rows if row["endpoint"] == endpoint and row["group"] == "MSS"]
        endpoints.append({
            "endpoint": endpoint,
            "effect_group_a_minus_group_b": sum(msi) / len(msi) - sum(mss) / len(mss),
            "studentized_statistic": 0.0,
            "adjusted_p_value": 1.0,
        })
    document = {
        "format": "marklab.cohort_max_t",
        "version": 1,
        "design": {"randomization_unit": "patient", "correction": "step_down_max_t"},
        "groups": {"group_a_patients": 4, "group_b_patients": 4},
        "endpoints": endpoints,
        "alpha": 0.05,
        "permutations": {"requested": 999, "completed": 999},
    }
    Path(value("--out")).write_text(json.dumps(document, sort_keys=True) + "\n", encoding="utf-8")
else:
    raise SystemExit(f"unsupported fake command: {args}")
'''


class CrcCategoricalPairPatientTest(unittest.TestCase):
    def test_patient_workflow_prepares_executes_replays_and_summarizes(self):
        module = load_module()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            inference = root / "inference"
            marks_path = root / "marks.csv"
            rows = []
            baseline_rows = []
            type_names = ["Neoplastic", "Inflammatory", "Connective", "Epithelial"]
            for patient_index in range(8):
                patient = f"P{patient_index + 1}"
                group = "MSI" if patient_index < 4 else "MSS"
                baseline_rows.extend(
                    {
                        "patient_id": patient,
                        "group": group,
                        "feature": feature,
                        "value": value,
                    }
                    for feature, value in (
                        ("composition", patient_index / 10.0),
                        ("nonspatial", (patient_index % 3) / 5.0),
                    )
                )
                for slide_index in range(2):
                    pattern = f"{patient}-S{slide_index + 1}"
                    cells = []
                    for cell_index in range(8):
                        type_id = cell_index % len(type_names) + 1
                        x = float(5 + cell_index * 5)
                        y = float(10 + slide_index * 10)
                        cells.append({"centroid": [x, y], "type": type_id})
                        rows.append(
                            {
                                "pattern_id": pattern,
                                "patient_id": patient,
                                "group": group,
                                "point_id": f"{pattern}:{cell_index:09d}",
                                "x_um": x,
                                "y_um": y,
                                "type_id": type_names[type_id - 1],
                            }
                        )
                    slide_root = inference / pattern
                    slide_root.mkdir(parents=True)
                    (slide_root / f"{pattern}_cells.json.snappy").write_text(
                        json.dumps(
                            {
                                "cells": cells,
                                "type_map": {
                                    str(index + 1): name
                                    for index, name in enumerate(type_names)
                                },
                                "wsi_metadata": {"base_mpp": 1.0},
                            }
                        ),
                        encoding="utf-8",
                    )
            with marks_path.open("w", newline="", encoding="utf-8") as target:
                writer = csv.DictWriter(target, fieldnames=list(rows[0]))
                writer.writeheader()
                writer.writerows(rows)
            baseline = root / "baseline.csv"
            with baseline.open("w", newline="", encoding="utf-8") as target:
                writer = csv.DictWriter(target, fieldnames=list(baseline_rows[0]))
                writer.writeheader()
                writer.writerows(baseline_rows)
            fake = root / "marklab"
            fake.write_text(FAKE_MARKLAB, encoding="utf-8")
            fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            prepared = root / "prepared"
            executed = root / "executed"
            summary_root = root / "summary"

            manifest = module.prepare(
                marks_path,
                inference,
                prepared,
                adapter=FakeAdapter(),
                expected_cells_per_pattern=8,
            )
            module.execute(prepared, fake, executed, 2, 30, replay=False)
            replay = module.execute(prepared, fake, executed, 2, 30, replay=True)
            summary = module.summarize(
                prepared, executed, baseline, fake, summary_root, 20260829
            )

            self.assertEqual(len(manifest), 16)
            self.assertEqual(replay["cache_status_counts"], {"hit": 64})
            self.assertTrue(replay["all_replay_bytes_equal"])
            self.assertTrue(replay["all_ledgers_one_execution"])
            self.assertEqual(summary["patient_count"], 8)
            self.assertEqual(summary["pattern_count"], 16)
            self.assertEqual(summary["pair_family_count"], 4)
            self.assertEqual(summary["population_max_t"]["format"], "marklab.cohort_max_t")
            self.assertTrue(summary["leakage_checks"]["patient_held_out"])
            self.assertIn("balanced_accuracy_increment", summary["incremental_information"])
            self.assertEqual(
                len(list((executed / "projects").glob("*/*/executions.jsonl"))), 64
            )

            cross_executed = root / "cross_executed"
            cross_summary_root = root / "cross_summary"
            module.execute(
                prepared,
                fake,
                cross_executed,
                2,
                30,
                replay=False,
                analysis="categorical-cross-pair-correlation",
            )
            cross_replay = module.execute(
                prepared,
                fake,
                cross_executed,
                2,
                30,
                replay=True,
                analysis="categorical-cross-pair-correlation",
            )
            cross_summary = module.summarize(
                prepared,
                cross_executed,
                baseline,
                fake,
                cross_summary_root,
                20260829,
                analysis="categorical-cross-pair-correlation",
            )

            self.assertEqual(cross_replay["cache_status_counts"], {"hit": 64})
            self.assertTrue(cross_replay["all_replay_bytes_equal"])
            self.assertEqual(
                cross_summary["schema_name"],
                "marklab_crc_categorical_cross_g_patient_summary",
            )
            self.assertEqual(cross_summary["declared_endpoint_count"], 16)
            self.assertIn(
                "balanced_accuracy_increment",
                cross_summary["incremental_information"],
            )
            self.assertEqual(
                cross_summary["promotion_status"],
                "nonincremental_not_promoted",
            )
            self.assertEqual(
                cross_summary["fusion_status"],
                "not_added_without_positive_incremental_information",
            )
            self.assertEqual(
                len(list((cross_executed / "projects").glob("*/*/executions.jsonl"))),
                64,
            )


if __name__ == "__main__":
    unittest.main()
