"""Behavioural tests for the shared worker support package."""

import contextlib
import hashlib
import io
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import numpy as np

WORKER_DIRECTORY = Path(__file__).resolve().parent
sys.path.append(str(WORKER_DIRECTORY))

import arviz as az  # noqa: E402

from marklab_worker_support import (  # noqa: E402
    diagnostics,
    posterior,
    protocol,
    sbc,
    versions,
)
from marklab_worker_support.protocol import ContractError  # noqa: E402


@contextlib.contextmanager
def captured_stdout():
    buffer = io.BytesIO()
    stream = io.TextIOWrapper(buffer, encoding="utf-8")
    try:
        with mock.patch.object(sys, "stdout", stream):
            yield buffer
    finally:
        stream.detach()


def stdin_of(data: bytes):
    return mock.patch.object(sys, "stdin", io.TextIOWrapper(io.BytesIO(data)))


class ValidatorTests(unittest.TestCase):
    def test_contract_error_is_a_value_error(self):
        self.assertTrue(issubclass(ContractError, ValueError))

    def test_exact(self):
        protocol.exact("pymc", "pymc", "backend.name")
        with self.assertRaisesRegex(ContractError, r"backend\.name must equal 'pymc'"):
            protocol.exact("numpyro", "pymc", "backend.name")

    def test_obj_requires_exact_fields(self):
        value = {"a": 1, "b": 2}
        self.assertIs(protocol.obj(value, {"a", "b"}, "request"), value)
        with self.assertRaisesRegex(ContractError, r"missing=\['b'\], unknown=\['c'\]"):
            protocol.obj({"a": 1, "c": 3}, {"a", "b"}, "request")
        for not_object in ([], "a", None):
            with self.assertRaises(ContractError):
                protocol.obj(not_object, {"a"}, "request")

    def test_number_boundaries(self):
        self.assertEqual(protocol.number(3, "x"), 3.0)
        self.assertIsInstance(protocol.number(3, "x"), float)
        self.assertEqual(protocol.number(0.5, "x", 0.5, 1.0), 0.5)
        self.assertEqual(protocol.number(1.0, "x", 0.5, 1.0), 1.0)
        for rejected in (True, "1", None, float("nan"), float("inf"), -float("inf")):
            with self.subTest(value=rejected), self.assertRaises(ContractError):
                protocol.number(rejected, "x")
        for outside in (0.4999, 1.0001):
            with self.subTest(value=outside), self.assertRaises(ContractError):
                protocol.number(outside, "x", 0.5, 1.0)

    def test_integer_boundaries(self):
        self.assertEqual(protocol.integer(2, "chains", 2, 8), 2)
        self.assertEqual(protocol.integer(8, "chains", 2, 8), 8)
        for rejected in (1, 9, True, 2.0, "2", None):
            with self.subTest(value=rejected), self.assertRaises(ContractError):
                protocol.integer(rejected, "chains", 2, 8)

    def test_sha256_digest(self):
        valid = "0123456789abcdef" * 4
        self.assertEqual(protocol.sha256_digest(valid, "d"), valid)
        for rejected in (valid.upper(), valid[:-1], valid + "0", "g" * 64, None):
            with self.subTest(value=rejected), self.assertRaises(ContractError):
                protocol.sha256_digest(rejected, "d")


class BackendIdentityTests(unittest.TestCase):
    def test_backend_identity_is_accepted(self):
        identity = protocol.backend_identity("pymc", versions.PYMC, "1" * 64, "2" * 64)
        self.assertEqual(identity["python_version"], versions.PYTHON)
        accepted = protocol.validate_backend(
            identity,
            "request.backend",
            name="pymc",
            version=versions.PYMC,
            lock_sha256="1" * 64,
            worker_sha256="2" * 64,
        )
        self.assertIs(accepted, identity)
        with self.assertRaisesRegex(ContractError, r"request\.backend\.worker_sha256"):
            protocol.validate_backend(
                identity,
                "request.backend",
                name="pymc",
                version=versions.PYMC,
                lock_sha256="1" * 64,
                worker_sha256="3" * 64,
            )

    def test_version_drift_is_rejected(self):
        protocol.require_versions(numpy=versions.NUMPY)
        with self.assertRaisesRegex(ContractError, "backend version drift: numpy"):
            protocol.require_versions(numpy="1.0.0")

    def test_worker_digests_hash_the_lock_and_the_worker(self):
        with tempfile.TemporaryDirectory() as directory:
            worker = Path(directory) / "worker.py"
            worker.write_bytes(b"print(1)\n")
            (Path(directory) / "uv.lock").write_bytes(b"lock")
            self.assertEqual(
                protocol.worker_digests(worker),
                (hashlib.sha256(b"lock").hexdigest(), hashlib.sha256(b"print(1)\n").hexdigest()),
            )

    def test_derive_seed_matches_the_framed_sha256(self):
        expected = hashlib.sha256(b"marklab-test-v1\x007\x00chain\x000").digest()
        self.assertEqual(
            protocol.derive_seed("marklab-test-v1", 7, "chain", 0),
            int.from_bytes(expected[:4], "little"),
        )
        self.assertEqual(
            protocol.derive_seed("marklab-test-v1", 7, "chain", 0, width=8),
            int.from_bytes(expected[:8], "little"),
        )


class RequestTests(unittest.TestCase):
    def test_empty_request_is_rejected(self):
        with stdin_of(b""), self.assertRaisesRegex(ContractError, "empty"):
            protocol.read_request(8)

    def test_request_at_the_cap_is_accepted(self):
        with stdin_of(b"x" * 8):
            self.assertEqual(protocol.read_request(8), b"x" * 8)

    def test_oversize_request_is_rejected_after_reading_one_extra_byte(self):
        source = io.BytesIO(b"x" * 100)
        with mock.patch.object(sys, "stdin", io.TextIOWrapper(source)):
            with self.assertRaisesRegex(ContractError, "8-byte input limit"):
                protocol.read_request(8)
            self.assertEqual(source.tell(), 9)


class WriterTests(unittest.TestCase):
    def test_canonical_json_with_newline(self):
        with captured_stdout() as buffer:
            protocol.write_result({"b": 1, "a": [1.5, None]})
        self.assertEqual(buffer.getvalue(), b'{"a":[1.5,null],"b":1}\n')

    def test_insertion_order_without_newline(self):
        with captured_stdout() as buffer:
            protocol.write_result({"b": 1, "a": 2}, sort_keys=False, newline=False)
        self.assertEqual(buffer.getvalue(), b'{"b":1,"a":2}')

    def test_non_finite_numbers_are_rejected(self):
        for value in (float("nan"), float("inf")):
            with self.subTest(value=value), captured_stdout() as buffer:
                with self.assertRaises(ValueError):
                    protocol.write_result({"x": value})
                self.assertEqual(buffer.getvalue(), b"")

    def test_output_limit_counts_every_written_byte(self):
        payload = b'{"x":1}\n'
        with captured_stdout() as buffer:
            protocol.write_result({"x": 1}, max_bytes=len(payload))
        self.assertEqual(buffer.getvalue(), payload)
        with captured_stdout() as buffer:
            with self.assertRaisesRegex(ContractError, "output limit"):
                protocol.write_result({"x": 1}, max_bytes=len(payload) - 1)
            self.assertEqual(buffer.getvalue(), b"")


class RunWorkerTests(unittest.TestCase):
    def test_reported_failure_exits_with_status_two(self):
        def main():
            raise ContractError("bad request")

        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr), self.assertRaises(SystemExit) as exit_:
            protocol.run_worker(main, "test worker failed")
        self.assertEqual(exit_.exception.code, 2)
        self.assertEqual(stderr.getvalue(), "test worker failed: ContractError: bad request\n")

    def test_unlisted_errors_propagate(self):
        def main():
            raise RuntimeError("bug")

        with self.assertRaises(RuntimeError):
            protocol.run_worker(main, "label", errors=protocol.CONTRACT_ERRORS)

    def test_success_exits_with_the_main_status(self):
        for status in (None, 0):
            with self.subTest(status=status), self.assertRaises(SystemExit) as exit_:
                protocol.run_worker(lambda status=status: status, "label")
            self.assertEqual(exit_.exception.code, status)

    def test_worker_process_reports_contract_failure(self):
        script = (
            "import sys\n"
            f"sys.path.append({str(WORKER_DIRECTORY)!r})\n"
            "from marklab_worker_support.protocol import read_request, run_worker\n"
            "run_worker(lambda: read_request(4), 'probe worker failed')\n"
        )
        completed = subprocess.run(
            [sys.executable, "-P", "-s", "-B", "-c", script],
            input=b"",
            capture_output=True,
            check=False,
        )
        self.assertEqual(completed.returncode, 2)
        self.assertEqual(completed.stdout, b"")
        self.assertEqual(
            completed.stderr.decode(), "probe worker failed: ContractError: request is empty\n"
        )


class DiagnosticsTests(unittest.TestCase):
    def test_minimum_ebfmi_on_known_energy(self):
        # Chain 0: mean squared step 2.5, variance 14/9; chain 1: 1 and 2/9.
        energy = np.asarray([[1.0, 2.0, 4.0], [0.0, 1.0, 0.0]])
        self.assertAlmostEqual(diagnostics.minimum_ebfmi(energy), 45.0 / 28.0, places=12)

    def test_tree_depth_hits_count_saturated_trajectories(self):
        self.assertEqual(diagnostics.tree_depth_hits([1, 1022, 1023, 2047], 10), 2)

    def test_rank_convergence_reports_the_worst_parameter(self):
        rng = np.random.default_rng(7)
        mixed = rng.normal(size=(2, 400))
        stuck = rng.normal(size=(2, 400)) * 0.1 + np.asarray([[0.0], [5.0]])
        idata = az.from_dict({"posterior": {"mixed": mixed, "stuck": stuck}})
        r_hat, bulk, tail = diagnostics.rank_convergence(idata, ["mixed", "stuck"])
        mixed_r_hat, _, _ = diagnostics.rank_convergence(idata, ["mixed"])
        self.assertLess(mixed_r_hat, 1.02)
        self.assertGreater(r_hat, 1.5)
        self.assertLess(bulk, 50.0)
        self.assertLess(tail, 50.0)

    def test_mean_mcse_is_each_quantitys_own_error(self):
        rng = np.random.default_rng(11)
        independent = rng.normal(size=(4, 1000))
        # A random walk within each chain mixes slowly, so its mean is far less certain than
        # the same number of independent draws would make it.
        sticky = np.cumsum(rng.normal(size=(4, 1000)), axis=1)
        independent_mcse = diagnostics.mean_mcse(independent, 4)
        naive = independent.std(ddof=1) / np.sqrt(independent.size)
        self.assertAlmostEqual(independent_mcse, naive, delta=0.2 * naive)
        sticky_naive = sticky.std(ddof=1) / np.sqrt(sticky.size)
        self.assertGreater(diagnostics.mean_mcse(sticky, 4), 5.0 * sticky_naive)
        self.assertEqual(diagnostics.mean_mcse(independent.reshape(-1), 4), independent_mcse)
        self.assertEqual(diagnostics.mean_mcse(np.full((2, 50), 3.5), 2), 0.0)


class SbcTests(unittest.TestCase):
    def test_rank_and_coverage(self):
        draws = np.arange(10.0)
        self.assertEqual(sbc.rank_and_coverage(draws, 3.5), (4, True))
        self.assertEqual(sbc.rank_and_coverage(draws, 9.5), (10, False))

    def test_rank_summary(self):
        summary = sbc.rank_summary([0, 5, 10, 7], [True, True, False, True], draws=10, bins=2)
        self.assertEqual(summary["rank_histogram"], [2, 2])
        # Eleven possible ranks split 6:5 between the bins. The exact multinomial tail includes
        # every four-replicate histogram at least as far from that null as counts [2, 2].
        self.assertEqual(summary["rank_uniformity_p_value"], 1.0)
        self.assertEqual(summary["coverage_90"], 0.75)
        self.assertEqual(summary["mean_normalized_rank"], 0.55)
        empty = sbc.rank_summary([], [], draws=10, bins=2)
        self.assertEqual(empty["rank_histogram"], [0, 0])
        self.assertEqual(empty["coverage_90"], 0.0)

        ranks = np.tile(np.arange(21), 100)
        uniform = sbc.rank_summary(ranks, np.ones(len(ranks), dtype=bool), draws=20, bins=20)
        self.assertEqual(uniform["rank_uniformity_p_value"], 1.0)
        self.assertEqual(sbc.maximum_rank_ecdf_deviation(ranks, 20), 0.0)
        biased = sbc.rank_summary([0] * 100, [True] * 100, draws=20, bins=20)
        self.assertLess(biased["rank_uniformity_p_value"], 0.01)
        self.assertGreater(sbc.maximum_rank_ecdf_deviation([0] * 100, 20), 0.9)

        divisible = sbc.rank_summary([0, 5, 9], [True, False, True], draws=9, bins=2)
        self.assertEqual(divisible["rank_uniformity_p_value"], 1.0)

        finite_sample = sbc.rank_uniformity(np.asarray([8, 1, 0, 2, 1, 0, 2, 2, 2, 2]), draws=200)
        self.assertAlmostEqual(finite_sample[1], 0.010168225602421036, places=12)

        larger_sparse = np.asarray([10, 3, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 4])
        first = sbc.rank_uniformity(larger_sparse, draws=200)
        replay = sbc.rank_uniformity(larger_sparse, draws=200)
        self.assertEqual(first, replay)
        self.assertAlmostEqual(first[1], 0.30583470826458675, places=15)

        wide_sparse = np.zeros(128, dtype=np.int64)
        wide_sparse[0] = 20
        wide_p_value = sbc.rank_uniformity(wide_sparse, draws=127)[1]
        self.assertEqual(wide_p_value, 1.0 / (sbc.MONTE_CARLO_REPLICATES + 1))

    def test_calibration_passes(self):
        good = {"rank_uniformity_p_value": 0.5, "coverage_90": 0.9}
        self.assertTrue(sbc.calibration_passes([good], [], 0.01, 0.8, 0.98))
        self.assertFalse(sbc.calibration_passes([good], [{"replicate": 1}], 0.01, 0.8, 0.98))
        self.assertFalse(sbc.calibration_passes([good], [], 0.6, 0.8, 0.98))


class PosteriorTests(unittest.TestCase):
    def test_summary(self):
        result = posterior.summary(np.asarray([[1.0, 2.0], [3.0, 4.0]]))
        self.assertEqual(result["mean"], 2.5)
        self.assertAlmostEqual(result["sd"], np.sqrt(5.0 / 3.0), places=12)
        self.assertAlmostEqual(result["interval_lower"], 1.075, places=12)
        self.assertAlmostEqual(result["interval_upper"], 3.925, places=12)
        json.dumps(result, allow_nan=False)

    def test_summary_with_chains_adds_the_quantitys_mcse(self):
        draws = np.random.default_rng(3).normal(size=(2, 200))
        plain = posterior.summary(draws)
        result = posterior.summary(draws, chains=2)
        self.assertEqual({name: result[name] for name in plain}, plain)
        self.assertEqual(result["mcse_mean"], diagnostics.mean_mcse(draws, 2))
        json.dumps(result, allow_nan=False)

    def test_two_sided_tail(self):
        replicated = np.arange(10.0)
        self.assertEqual(posterior.two_sided_tail(replicated, 0.0), 0.2)
        self.assertEqual(posterior.two_sided_tail(replicated, 4.5), 1.0)


class FitSummaryTests(unittest.TestCase):
    CONFIG = {
        "chains": 2,
        "tune": 100,
        "draws": 200,
        "maximum_r_hat": 1.01,
        "minimum_bulk_ess": 400.0,
        "minimum_tail_ess": 400.0,
        "minimum_ebfmi": 0.3,
        "maximum_divergences": 0,
        "maximum_tree_depth_hits": 0,
    }
    CONVERGED = {
        "r_hat": 1.0,
        "ess_bulk": 500.0,
        "ess_tail": 450.0,
        "minimum_ebfmi": 0.9,
        "divergences": 0,
        "max_tree_depth_hits": 0,
    }

    def test_complete_only_when_finite_valid_and_converged(self):
        from marklab_worker_support.nuts_contract import fit_summary

        summary = fit_summary(self.CONVERGED, self.CONFIG, True, True, True)
        self.assertEqual(summary["fit_state"], "complete")
        self.assertEqual(summary["sampling"]["completed_draws"], 400)
        self.assertEqual(summary["diagnostics"]["r_hat"], 1.0)
        self.assertIs(summary["diagnostics"]["identifiability_checks_passed"], True)
        for flags in [(False, True, True), (True, False, True), (True, True, False)]:
            with self.subTest(flags=flags):
                failed = fit_summary(self.CONVERGED, self.CONFIG, *flags)
                self.assertEqual(failed["fit_state"], "nonconverged")
        diverged = {**self.CONVERGED, "divergences": 1}
        self.assertEqual(
            fit_summary(diverged, self.CONFIG, True, True, True)["fit_state"], "nonconverged"
        )


class ReplicateHarnessTests(unittest.TestCase):
    def test_failures_keep_their_replicate_index_and_reason(self):
        from marklab_worker_support.numpyro_sbc import run_replicates

        def replicate(index):
            if index == 1:
                raise ValueError("bad draw")
            if index == 2:
                return "diagnostics_failed:r_hat=2"
            return {"replicate": index}

        completed, failures = run_replicates(4, replicate)
        self.assertEqual(completed, [{"replicate": 0}, {"replicate": 3}])
        self.assertEqual(
            failures,
            [
                {"replicate": 1, "reason": "ValueError:bad draw"},
                {"replicate": 2, "reason": "diagnostics_failed:r_hat=2"},
            ],
        )

    def test_diagnostics_failure_names_every_statistic(self):
        from marklab_worker_support.numpyro_sbc import diagnostics_failure

        config = FitSummaryTests.CONFIG
        self.assertIsNone(diagnostics_failure(FitSummaryTests.CONVERGED, config))
        reason = diagnostics_failure({**FitSummaryTests.CONVERGED, "ess_tail": 10.0}, config)
        self.assertEqual(
            reason,
            "diagnostics_failed:r_hat=1,bulk=500,tail=10,ebfmi=0.90000000000000002,"
            "divergences=0,depth_hits=0",
        )


class CountSummaryTests(unittest.TestCase):
    def test_predictive_class_report_accepts_chain_grouped_or_flat_draws(self):
        from marklab_worker_support.count_summaries import predictive_class_report

        rng = np.random.default_rng(3)
        config = {
            "class_ids": ["a", "b", "c"],
            "indicator": np.asarray([0, 0, 1, 1]),
            "counts": rng.integers(0, 20, size=(4, 3)),
        }
        config["totals"] = config["counts"].sum(axis=1)
        grouped = rng.integers(0, 20, size=(2, 5, 4, 3))
        flat = grouped.reshape(-1, 4, 3)
        self.assertEqual(
            predictive_class_report(config, grouped), predictive_class_report(config, flat)
        )
        report = predictive_class_report(config, flat)
        reference = (config["counts"][:2] / config["totals"][:2, None]).mean(axis=0)
        self.assertEqual([row["class_id"] for row in report], ["a", "b", "c"])
        self.assertAlmostEqual(report[1]["observed_reference_mean_proportion"], reference[1])


class NameTests(unittest.TestCase):
    def test_valid_name(self):
        from marklab_worker_support.pymc_request import valid_name

        self.assertTrue(valid_name("patient-1"))
        for rejected in ["", " padded", "x" * 129, "tab\tname", 7, None]:
            with self.subTest(rejected=rejected):
                self.assertFalse(valid_name(rejected))


if __name__ == "__main__":
    unittest.main()
