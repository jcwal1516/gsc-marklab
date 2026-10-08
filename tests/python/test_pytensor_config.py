#!/usr/bin/env python3

from pathlib import Path
import os
import subprocess
import sys
import tempfile
import unittest


WORKER_DIRECTORY = Path(__file__).resolve().parents[2] / "workers" / "python"

# Run the way marklab launches workers: -P keeps the worker directory off sys.path.
SEEDED_FIT = f"""
import sys
sys.path.append({str(WORKER_DIRECTORY)!r})
import marklab_pytensor_config
import numpy as np
import pymc as pm

x = np.linspace(-1.0, 1.0, 8)
y = np.array([-0.09, -0.26, 0.52, 0.90, 1.49, 1.76, 2.35, 2.19])
with pm.Model():
    intercept = pm.Normal("intercept", 0.0, 3.0)
    slope = pm.Normal("slope", 0.0, 2.0)
    sigma = pm.HalfNormal("sigma", 1.0)
    pm.Normal("y", intercept + slope * x, sigma, observed=y)
    idata = pm.sample(
        draws=300, tune=300, chains=1, cores=1, random_seed=7302,
        progressbar=False, compute_convergence_checks=False,
    )
sys.stdout.write(idata.posterior["slope"].values.tobytes().hex())
"""


class PytensorConfigTest(unittest.TestCase):
    def test_compile_flags_request_ld64_only_when_the_toolchain_links_with_it(self):
        sys.path.insert(0, str(WORKER_DIRECTORY))
        try:
            import marklab_pytensor_config  # noqa: F401
        finally:
            sys.path.pop(0)
        from pytensor.link.c.cmodule import GCC_compiler

        links_with_ld64 = bool(GCC_compiler.try_flags(["-ld64"], comp_args=False))
        self.assertEqual("-ld64" in GCC_compiler.compile_args(), links_with_ld64)

    def test_seeded_fit_is_identical_with_cold_and_warm_compile_caches(self):
        with tempfile.TemporaryDirectory() as cache:
            environment = dict(os.environ, PYTENSOR_FLAGS=f"base_compiledir={cache}")
            draws = [
                subprocess.run(
                    [sys.executable, "-P", "-s", "-B", "-c", SEEDED_FIT],
                    env=environment, capture_output=True, text=True, check=True,
                ).stdout
                for _ in ("cold", "warm")
            ]
        self.assertTrue(draws[0])
        self.assertEqual(draws[0], draws[1])


if __name__ == "__main__":
    unittest.main()
