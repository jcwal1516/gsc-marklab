import json
from pathlib import Path
import subprocess
import sys
import unittest

import numpy as np


WORKER = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_scipy_serial_stack_worker.py"
)


class SerialStackPosteriorTest(unittest.TestCase):
    def test_shared_reference_landmark_noise_correlates_section_translations(self):
        base = [[0.0, 0.0], [10.0, 0.0], [0.0, 10.0], [10.0, 10.0]]
        request = {
            "format": "marklab.scipy_serial_stack_request",
            "version": 1,
            "backend": {"test": True},
            "stack_id": "shared-reference-oracle",
            "coordinate_unit": "um",
            "sections": [
                {"section_id": "s0", "z_um": 0.0, "observed_landmarks": base},
                {"section_id": "ref", "z_um": 1.0, "observed_landmarks": base},
                {"section_id": "s2", "z_um": 2.0, "observed_landmarks": base},
            ],
            "reference_section_id": "ref",
            "landmark_noise_standard_deviation_um": 1.0,
            "posterior_draws": 2_000,
            "seed": 418,
        }
        completed = subprocess.run(
            [sys.executable, str(WORKER)],
            input=json.dumps(request).encode(),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=True,
        )
        result = json.loads(completed.stdout)
        by_section = {
            row["section_id"]: np.asarray(row["draws"], dtype=float)
            for row in result["stack_posterior"]["section_translations_xy_um"]
        }

        correlation = float(
            np.corrcoef(by_section["s0"][:, 0], by_section["s2"][:, 0])[0, 1]
        )
        self.assertGreater(correlation, 0.4)
        self.assertLess(correlation, 0.6)
        np.testing.assert_array_equal(by_section["ref"], np.zeros((2_000, 2)))
        centroid_sd = result["downstream_uncertainty"][
            "stack_centroid_standard_deviation_um"
        ][0]
        self.assertGreater(centroid_sd, 0.38)
        self.assertLess(centroid_sd, 0.44)


if __name__ == "__main__":
    unittest.main()
