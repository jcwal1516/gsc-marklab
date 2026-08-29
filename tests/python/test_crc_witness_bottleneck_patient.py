import csv
import json
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / "workers/python/marklab_crc_witness_bottleneck_patient.py"


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


FAKE_MARKLAB = r'''#!/usr/bin/env python3
import json
import os
from pathlib import Path
import sys

arguments = sys.argv[1:]
project = Path(arguments[arguments.index("--project") + 1])
output = Path(arguments[arguments.index("--out") + 1])
stored = project / "stored.json"
ledger = project / "executions.jsonl"
if os.environ.get("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION") == "1":
    if not stored.is_file():
        raise SystemExit("backend-disabled replay missed")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(stored.read_bytes())
    print("cache_status=hit", file=sys.stderr)
else:
    project.mkdir(parents=True, exist_ok=True)
    result = {
        "format": "marklab.witness_persistence_bottleneck_stability",
        "version": 1,
        "maximum_bottleneck_distance_um_squared_allowed": 600.0,
        "maximum_finite_bottleneck_distance_um_squared": 700.0,
        "has_infinite_essential_mismatch": False,
        "bottleneck_comparisons": 12,
        "bottleneck_interval_count": 120,
        "total_backend_executions": 6,
        "stable_under_bottleneck_threshold": False,
        "stable_under_all_declared_thresholds": False,
    }
    encoded = (json.dumps(result, sort_keys=True) + "\n").encode()
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(encoded)
    stored.write_bytes(encoded)
    ledger.write_text("{}\n", encoding="utf-8")
    print("cache_status=miss", file=sys.stderr)
'''


class CrcWitnessBottleneckPatientTest(unittest.TestCase):
    def test_cli_runs_patient_nested_miss_hit_and_exact_group_summary(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "marks.csv"
            with source.open("w", newline="", encoding="utf-8") as target:
                writer = csv.DictWriter(target, fieldnames=list(marks()[0]))
                writer.writeheader()
                writer.writerows(marks())
            prepared = root / "prepared"
            executed = root / "executed"
            summary = root / "summary"
            fake = root / "marklab"
            fake.write_text(FAKE_MARKLAB, encoding="utf-8")
            fake.chmod(fake.stat().st_mode | stat.S_IXUSR)

            subprocess.run(
                [sys.executable, str(WORKFLOW), "prepare", "--marks", str(source), "--out", str(prepared)],
                check=True,
            )
            subprocess.run(
                [
                    sys.executable,
                    str(WORKFLOW),
                    "execute",
                    "--prepared",
                    str(prepared),
                    "--binary",
                    str(fake),
                    "--out",
                    str(executed),
                    "--maximum-processes",
                    "2",
                ],
                check=True,
            )
            subprocess.run(
                [
                    sys.executable,
                    str(WORKFLOW),
                    "summarize",
                    "--prepared",
                    str(prepared),
                    "--executed",
                    str(executed),
                    "--out",
                    str(summary),
                ],
                check=True,
            )

            design = json.loads((prepared / "design.json").read_text())
            request = json.loads(
                next((prepared / "requests").glob("*/*.json")).read_text()
            )
            result = json.loads((summary / "summary.json").read_text())
            execution = json.loads((executed / "execution_manifest.json").read_text())
            self.assertEqual(design["population_unit"], "patient")
            self.assertEqual(design["patient_count"], 4)
            self.assertEqual(request["maximum_bottleneck_comparisons"], 12)
            self.assertEqual(request["maximum_bottleneck_interval_budget"], 12_000_000)
            self.assertEqual(request["maximum_total_backend_executions"], 6)
            self.assertEqual(result["patient_count"], 4)
            self.assertEqual(result["pattern_count"], 8)
            self.assertEqual(result["stable_patient_fraction"], 0.0)
            self.assertEqual(result["group_comparison"]["mean_difference_msi_minus_mss"], 0.0)
            self.assertEqual(result["group_comparison"]["exact_two_sided_p_value"], 1.0)
            self.assertEqual(execution["miss_count"], 8)
            self.assertEqual(execution["backend_disabled_hit_count"], 8)
            self.assertTrue(execution["all_result_bytes_equal"])
            self.assertTrue(execution["all_ledgers_one_execution"])


if __name__ == "__main__":
    unittest.main()
