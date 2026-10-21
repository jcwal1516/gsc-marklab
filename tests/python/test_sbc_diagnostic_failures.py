import importlib.util
import unittest
from contextlib import ExitStack
from pathlib import Path
from unittest import mock

import numpy as np

WORKERS = Path(__file__).resolve().parents[2] / "workers" / "python"


class SbcDiagnosticFailuresTest(unittest.TestCase):
    def test_nonconverged_replicates_are_failures_without_ranks(self):
        config = {
            "seed": 17,
            "replicates": 1,
            "patients": 3,
            "observations_per_patient": 2,
            "global_mean": 0.0,
            "global_sd": 1.0,
            "between_sd": 0.5,
            "known_sigma": 1.0,
            "intercept_mean": 0.0,
            "intercept_sd": 1.0,
            "coefficient_mean": 0.0,
            "coefficient_sd": 1.0,
            "covariate": np.array([-0.5, 0.5]),
            "offset": np.zeros(2),
            "cholesky": np.eye(2),
            "cell_area": 1.0,
            "latent_cell_index": 0,
            "chains": 2,
            "tune": 100,
            "draws": 100,
            "target_accept": 0.95,
            "maximum_tree_depth": 10,
            "maximum_r_hat": 1.01,
            "minimum_bulk_ess": 400.0,
            "minimum_tail_ess": 400.0,
            "minimum_ebfmi": 0.3,
            "maximum_divergences": 0,
            "maximum_tree_depth_hits": 0,
        }
        for model in ("hierarchical", "gridded_lgcp"):
            path = WORKERS / f"marklab_numpyro_{model}_sbc_worker.py"
            spec = importlib.util.spec_from_file_location(path.stem, path)
            worker = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(worker)
            samples = {
                name: np.zeros((2, 100))
                for name in (
                    "global_mean",
                    "between_patient_sd",
                    "intercept",
                    "coefficient",
                )
            }
            samples["field_raw"] = np.zeros((2, 100, 2))
            for diagnostic, value in (
                (None, None),
                ("r_hat", 1.02),
                ("ess_bulk", 399.0),
                ("ess_tail", 399.0),
                ("minimum_ebfmi", 0.2),
                ("divergences", 1),
                ("max_tree_depth_hits", 1),
            ):
                with (
                    self.subTest(model=model, diagnostic=diagnostic),
                    ExitStack() as stack,
                ):
                    metrics = {
                        "r_hat": 1.0,
                        "ess_bulk": 500.0,
                        "ess_tail": 500.0,
                        "minimum_ebfmi": 0.8,
                        "divergences": 0,
                        "max_tree_depth_hits": 0,
                    }
                    if diagnostic is not None:
                        metrics[diagnostic] = value
                    sampler = stack.enter_context(
                        mock.patch.object(worker, "MCMC")
                    ).return_value
                    sampler.get_samples.return_value = samples
                    sampler.get_extra_fields.return_value = {
                        "diverging": np.zeros((2, 100), dtype=bool),
                        "num_steps": np.ones((2, 100)),
                        "energy": np.ones((2, 100)),
                    }
                    stack.enter_context(
                        mock.patch.object(
                            worker,
                            "rank_convergence",
                            return_value=(
                                metrics["r_hat"],
                                metrics["ess_bulk"],
                                metrics["ess_tail"],
                            ),
                        )
                    )
                    stack.enter_context(
                        mock.patch.object(
                            worker,
                            "divergence_count",
                            return_value=metrics["divergences"],
                        )
                    )
                    stack.enter_context(
                        mock.patch.object(
                            worker,
                            "tree_depth_hits",
                            return_value=metrics["max_tree_depth_hits"],
                        )
                    )
                    stack.enter_context(
                        mock.patch.object(
                            worker,
                            "minimum_ebfmi",
                            return_value=metrics["minimum_ebfmi"],
                        )
                    )
                    if model == "gridded_lgcp":
                        stack.enter_context(
                            mock.patch.object(
                                worker.lgcp, "model_arguments", return_value={}
                            )
                        )
                    ranks = stack.enter_context(
                        mock.patch.object(
                            worker, "rank_and_coverage", return_value=(50, True)
                        )
                    )
                    completed, failures = worker.run_sbc(config)
                    if diagnostic is None:
                        self.assertEqual(len(completed), 1)
                        self.assertEqual(failures, [])
                        self.assertGreater(ranks.call_count, 0)
                    else:
                        self.assertEqual(completed, [])
                        self.assertEqual(len(failures), 1)
                        self.assertEqual(failures[0]["replicate"], 0)
                        self.assertTrue(
                            failures[0]["reason"].startswith("diagnostics_failed:")
                        )
                        ranks.assert_not_called()


if __name__ == "__main__":
    unittest.main()
