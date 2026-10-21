import json
import subprocess
import sys
import unittest
from pathlib import Path

WORKER = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_topology_stability_worker.py"
)


class TopologyStabilityMathTest(unittest.TestCase):
    def test_maximum_sensitivity_includes_negative_radius_shifts(self):
        mask = [[False] * 7 for _ in range(7)]
        mask[3][1] = mask[3][3] = True
        for candidates, shifts in [([[3, 3]], {-2.0}), ([[3, 3], [3, 6]], {-2.0, 1.0})]:
            with self.subTest(candidates=candidates):
                request = {
                    "format": "marklab.topology_stability_request",
                    "version": 1,
                    "backend": {"test": True},
                    "base_mask": mask,
                    "pixel_size_um": 1.0,
                    "scales_um": [0.0, 1.0, 2.0],
                    "perturbation_generator": {
                        "kind": "toggle_declared_pixels",
                        "candidate_pixels": candidates,
                        "toggles_per_repetition": 1,
                    },
                    "repetitions": 6,
                    "seed": 99,
                }
                completed = subprocess.run(
                    [sys.executable, str(WORKER)],
                    input=json.dumps(request),
                    capture_output=True,
                    text=True,
                    check=True,
                )
                result = json.loads(completed.stdout)
                self.assertEqual(
                    {row["critical_radius_shift_um"] for row in result["results"]},
                    shifts,
                )
                self.assertEqual(
                    result["maximum_observed_sensitivity"]["critical_radius_shift_um"],
                    2.0,
                )


if __name__ == "__main__":
    unittest.main()
