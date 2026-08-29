import hashlib
import json
import subprocess
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
WORKER = ROOT / "workers/python/marklab_gudhi_witness_bottleneck_worker.py"
PYTHON = ROOT / "target/pymc-venv/bin/python"


class GudhiWitnessBottleneckWorkerTests(unittest.TestCase):
    def test_known_finite_distance_and_essential_mismatch(self):
        request = {
            "format": "marklab.gudhi_witness_bottleneck_request",
            "version": 1,
            "backend": {
                "name": "gudhi",
                "version": "3.13.0",
                "python_version": "3.12",
                "license": "test",
                "environment_lock_sha256": "0" * 64,
                "worker_sha256": "1" * 64,
            },
            "metric": "bottleneck_linf",
            "coefficient": 0.0,
            "comparisons": [
                {
                    "replicate": 0,
                    "dimension": 0,
                    "baseline": {
                        "finite_pairs": [{"birth": 0.0, "death": 2.0}],
                        "essential_births": [],
                    },
                    "perturbed": {
                        "finite_pairs": [{"birth": 0.0, "death": 3.0}],
                        "essential_births": [],
                    },
                },
                {
                    "replicate": 0,
                    "dimension": 1,
                    "baseline": {"finite_pairs": [], "essential_births": [0.0]},
                    "perturbed": {"finite_pairs": [], "essential_births": []},
                },
            ],
            "maximum_comparisons": 2,
            "maximum_interval_budget": 5,
        }
        request_bytes = json.dumps(request, sort_keys=True, separators=(",", ":")).encode()
        completed = subprocess.run(
            [str(PYTHON), "-I", str(WORKER)],
            input=request_bytes,
            capture_output=True,
            check=True,
        )
        result = json.loads(completed.stdout)
        self.assertEqual(result["request_sha256"], hashlib.sha256(request_bytes).hexdigest())
        self.assertEqual(result["maximum_finite_bottleneck_distance_um_squared"], 1.0)
        self.assertTrue(result["has_infinite_essential_mismatch"])
        self.assertEqual(result["comparisons"][0]["status"], "finite")
        self.assertEqual(result["comparisons"][0]["bottleneck_distance_um_squared"], 1.0)
        self.assertEqual(
            result["comparisons"][1]["status"], "infinite_essential_count_mismatch"
        )
        self.assertIsNone(result["comparisons"][1]["bottleneck_distance_um_squared"])


if __name__ == "__main__":
    unittest.main()
