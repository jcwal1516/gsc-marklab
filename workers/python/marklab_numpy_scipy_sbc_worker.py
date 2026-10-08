#!/usr/bin/env python3
"""Static NumPy/SciPy SBC worker for the conjugate normal-mean model."""

from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path
import sys
from typing import Any

import numpy as np
import scipy
from scipy import stats

BACKEND_VERSION = "numpy-2.4.6+scipy-1.18.1"


class ContractError(ValueError):
    pass


def obj(value: Any, keys: set[str], path: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        actual = set(value) if isinstance(value, dict) else set()
        raise ContractError(
            f"{path} fields differ: missing={sorted(keys - actual)}, unknown={sorted(actual - keys)}"
        )
    return value


def number(value: Any, path: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ContractError(f"{path} must be numeric")
    result = float(value)
    if not math.isfinite(result):
        raise ContractError(f"{path} must be finite")
    return result


def integer(value: Any, path: str, low: int, high: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not low <= value <= high:
        raise ContractError(f"{path} must be an integer in [{low}, {high}]")
    return value


def exact(value: Any, expected: Any, path: str) -> None:
    if value != expected:
        raise ContractError(f"{path} must equal {expected!r}")


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request, {"format", "version", "backend", "model", "sbc", "resources"}, "request"
    )
    exact(request["format"], "marklab.pymc_worker_request", "request.format")
    integer(request["version"], "request.version", 1, 1)
    backend = obj(
        request["backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "backend",
    )
    exact(backend["name"], "numpy_scipy", "backend.name")
    exact(backend["version"], BACKEND_VERSION, "backend.version")
    exact(backend["python_version"], "3.12", "backend.python")
    exact(backend["environment_lock_sha256"], lock_digest, "backend.lock")
    exact(backend["worker_sha256"], worker_digest, "backend.worker")

    model = obj(
        request["model"],
        {
            "format",
            "version",
            "family",
            "parameter",
            "prior",
            "prior_mean",
            "prior_sd",
            "likelihood",
            "known_sigma",
            "inference_algorithm",
            "backend_capability",
            "maturity",
        },
        "model",
    )
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    integer(model["version"], "model.version", 1, 1)
    exact(model["family"], "normal_mean_known_sigma", "model.family")
    exact(model["parameter"], "mu", "model.parameter")
    exact(model["prior"], "normal", "model.prior")
    prior_mean = number(model["prior_mean"], "model.prior_mean")
    prior_sd = number(model["prior_sd"], "model.prior_sd")
    exact(model["likelihood"], "normal_known_sigma", "model.likelihood")
    known_sigma = number(model["known_sigma"], "model.known_sigma")
    exact(
        model["inference_algorithm"],
        "exact_conjugate_independent_posterior_sampler",
        "model.inference",
    )
    exact(model["backend_capability"], "simulation_based_calibration", "model.capability")
    exact(model["maturity"], "experimental_calibration", "model.maturity")
    if min(prior_sd, known_sigma) <= 0.0:
        raise ContractError("normal-mean scales must be positive")

    sbc = obj(
        request["sbc"],
        {
            "observations_per_replicate",
            "replicates",
            "posterior_draws",
            "interval_probability",
            "rank_histogram_bins",
            "uniformity_p_value_minimum",
            "envelope_alpha",
            "seed",
        },
        "sbc",
    )
    observations = integer(sbc["observations_per_replicate"], "sbc.observations", 1, 1_000)
    replicates = integer(sbc["replicates"], "sbc.replicates", 20, 5_000)
    posterior_draws = integer(sbc["posterior_draws"], "sbc.draws", 20, 5_000)
    interval_probability = number(sbc["interval_probability"], "sbc.interval")
    histogram_bins = integer(
        sbc["rank_histogram_bins"], "sbc.histogram_bins", 1, posterior_draws + 1
    )
    p_minimum = number(sbc["uniformity_p_value_minimum"], "sbc.p_minimum")
    envelope_alpha = number(sbc["envelope_alpha"], "sbc.envelope_alpha")
    seed = integer(sbc["seed"], "sbc.seed", 0, 2**64 - 1)
    if (
        interval_probability != 0.95
        or not 0.0 < p_minimum < 1.0
        or not 0.0 < envelope_alpha < 1.0
    ):
        raise ContractError("SBC diagnostic controls are invalid")

    resources = obj(
        request["resources"],
        {"maximum_simulated_values", "maximum_output_bytes", "timeout_seconds"},
        "resources",
    )
    maximum_values = integer(
        resources["maximum_simulated_values"], "resources.values", 1, 2_000_000
    )
    if replicates * (observations + posterior_draws) > maximum_values:
        raise ContractError("SBC work exceeds resource limit")
    integer(resources["maximum_output_bytes"], "resources.output", 1, 4 * 1_048_576)
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3_600)
    return {
        "prior_mean": prior_mean,
        "prior_sd": prior_sd,
        "known_sigma": known_sigma,
        "observations": observations,
        "replicates": replicates,
        "posterior_draws": posterior_draws,
        "histogram_bins": histogram_bins,
        "p_minimum": p_minimum,
        "envelope_alpha": envelope_alpha,
        "seed": seed,
    }


def seed_for(seed: int, replicate: int, purpose: str) -> int:
    digest = hashlib.sha256(
        f"marklab-normal-mean-sbc-v1\0{seed}\0{replicate}\0{purpose}".encode()
    ).digest()
    return int.from_bytes(digest[:8], "little")


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    results: list[dict[str, Any]] = []
    failures: list[dict[str, Any]] = []
    prior_variance = config["prior_sd"] ** 2
    observation_variance = config["known_sigma"] ** 2
    posterior_variance = 1.0 / (
        1.0 / prior_variance + config["observations"] / observation_variance
    )
    posterior_sd = math.sqrt(posterior_variance)
    shrinkage = 1.0 - posterior_variance / prior_variance
    for replicate in range(config["replicates"]):
        try:
            simulation_rng = np.random.default_rng(
                seed_for(config["seed"], replicate, "simulation")
            )
            true_value = float(simulation_rng.normal(config["prior_mean"], config["prior_sd"]))
            observations = simulation_rng.normal(
                true_value, config["known_sigma"], size=config["observations"]
            )
            posterior_mean = posterior_variance * (
                config["prior_mean"] / prior_variance
                + float(observations.sum()) / observation_variance
            )
            posterior_rng = np.random.default_rng(
                seed_for(config["seed"], replicate, "posterior")
            )
            posterior = posterior_rng.normal(
                posterior_mean, posterior_sd, size=config["posterior_draws"]
            )
            less = int(np.count_nonzero(posterior < true_value))
            ties = int(np.count_nonzero(posterior == true_value))
            rank = less + int(posterior_rng.integers(0, ties + 1))
            interval_lower, interval_upper = np.quantile(posterior, [0.025, 0.975])
            values = np.asarray(
                [
                    true_value,
                    posterior_mean,
                    posterior_sd,
                    interval_lower,
                    interval_upper,
                    shrinkage,
                ]
            )
            if not np.isfinite(values).all():
                raise ArithmeticError("non-finite replicate state")
            results.append(
                {
                    "replicate": replicate,
                    "true_value": true_value,
                    "posterior_mean": float(posterior_mean),
                    "posterior_sd": posterior_sd,
                    "rank": rank,
                    "interval_lower": float(interval_lower),
                    "interval_upper": float(interval_upper),
                    "covered": bool(interval_lower <= true_value <= interval_upper),
                    "z_score": float((true_value - posterior_mean) / posterior_sd),
                    "shrinkage": shrinkage,
                }
            )
        except Exception as error:
            failures.append(
                {"replicate": replicate, "reason": f"{type(error).__name__}: {error}"}
            )
    if len(results) < 2:
        raise ContractError("SBC produced fewer than two valid replicates")

    ranks = np.asarray([result["rank"] for result in results], dtype=np.int64)
    histogram = np.zeros(config["histogram_bins"], dtype=np.int64)
    for rank in ranks:
        bin_index = int(rank) * config["histogram_bins"] // (config["posterior_draws"] + 1)
        histogram[min(bin_index, config["histogram_bins"] - 1)] += 1
    expected = len(results) / config["histogram_bins"]
    chi_square = float(np.sum((histogram - expected) ** 2 / expected))
    p_value = float(stats.chi2.sf(chi_square, config["histogram_bins"] - 1))
    sorted_ranks = np.sort(ranks)
    empirical_cdf = np.arange(1, len(results) + 1, dtype=np.float64) / len(results)
    expected_cdf = (sorted_ranks + 1.0) / (config["posterior_draws"] + 1.0)
    maximum_ecdf_deviation = float(np.max(np.abs(empirical_cdf - expected_cdf)))
    ecdf_envelope = math.sqrt(
        math.log(2.0 / config["envelope_alpha"]) / (2.0 * len(results))
    )
    covered = np.asarray([result["covered"] for result in results], dtype=np.float64)
    coverage = float(covered.mean())
    coverage_error = float(
        (coverage - 0.95) / math.sqrt(0.95 * 0.05 / len(results))
    )
    z_scores = np.asarray([result["z_score"] for result in results], dtype=np.float64)
    shrinkages = np.asarray([result["shrinkage"] for result in results], dtype=np.float64)
    failure_rate = len(failures) / config["replicates"]
    complete = (
        not failures
        and p_value >= config["p_minimum"]
        and maximum_ecdf_deviation <= ecdf_envelope
        and abs(coverage_error) <= 3.0
        and abs(float(z_scores.mean())) <= 3.0 / math.sqrt(config["replicates"])
        and 0.8 <= float(z_scores.std(ddof=1)) <= 1.2
    )
    return {
        "format": "marklab.numpy_scipy_normal_mean_sbc_worker_result",
        "version": 1,
        "backend": {
            "name": "numpy_scipy",
            "version": BACKEND_VERSION,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha,
            "worker_sha256": worker_sha,
        },
        "request_sha256": request_sha,
        "fit_state": "complete" if complete else "nonconverged",
        "replicates": results,
        "failures": failures,
        "diagnostics": {
            "rank_histogram": [int(value) for value in histogram],
            "rank_uniformity_chi_square": chi_square,
            "rank_uniformity_p_value": p_value,
            "maximum_ecdf_deviation": maximum_ecdf_deviation,
            "ecdf_envelope": ecdf_envelope,
            "coverage_95": coverage,
            "coverage_standardized_error": coverage_error,
            "z_score_mean": float(z_scores.mean()),
            "z_score_sd": float(z_scores.std(ddof=1)),
            "mean_shrinkage": float(shrinkages.mean()),
            "failure_rate": failure_rate,
            "posterior_draws_exchangeable": True,
            "autocorrelation_correction": "not_required_independent_conjugate_draws",
        },
    }


def main() -> None:
    if (
        np.__version__ != "2.4.6"
        or scipy.__version__ != "1.18.1"
        or sys.version_info[:2] != (3, 12)
    ):
        raise ContractError("NumPy, SciPy, or Python version drift")
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    raw = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if len(raw) > 16 * 1024 * 1024:
        raise ContractError("request exceeds 16 MiB")
    request_sha = hashlib.sha256(raw).hexdigest()
    result = fit(validate(json.loads(raw), lock_sha, worker_sha), request_sha, lock_sha, worker_sha)
    sys.stdout.write(json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")))
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"marklab SBC worker failed: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from error
