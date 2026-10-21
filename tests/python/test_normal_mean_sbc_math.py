import importlib.util
import unittest
from pathlib import Path

MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_numpy_scipy_sbc_worker.py"
)
spec = importlib.util.spec_from_file_location("normal_mean_sbc_worker", MODULE_PATH)
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)


class NormalMeanSbcMathTest(unittest.TestCase):
    def test_exact_sampler_passes_with_twenty_posterior_draws(self):
        config = {
            "prior_mean": 0.0,
            "prior_sd": 1.0,
            "known_sigma": 1.0,
            "observations": 1,
            "replicates": 5000,
            "posterior_draws": 20,
            "histogram_bins": 20,
            "p_minimum": 0.01,
            "envelope_alpha": 0.01,
            "seed": 0,
        }
        result = worker.fit(config, "request", "lock", "worker")
        self.assertEqual(result["failures"], [])
        self.assertEqual(result["fit_state"], "complete")
        diagnostics = result["diagnostics"]
        self.assertAlmostEqual(
            diagnostics["rank_uniformity_p_value"], 0.6977752642296075
        )
        self.assertAlmostEqual(
            diagnostics["maximum_ecdf_deviation"], 0.008961904761904768
        )
        self.assertGreater(diagnostics["coverage_95"], 0.94)
        self.assertLess(diagnostics["coverage_95"], 0.96)


if __name__ == "__main__":
    unittest.main()
