#!/usr/bin/env python3
"""Static SciPy worker for exact conjugate Normal prior sensitivity."""

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

BACKEND_VERSION = "scipy-1.18.1"


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
        request,
        {"format", "version", "backend", "model", "observations", "priors", "base_prior", "resources"},
        "request",
    )
    exact(request["format"], "marklab.pymc_worker_request", "request.format")
    integer(request["version"], "request.version", 1, 1)
    backend = obj(
        request["backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "backend",
    )
    exact(backend["name"], "scipy", "backend.name")
    exact(backend["version"], BACKEND_VERSION, "backend.version")
    exact(backend["python_version"], "3.12", "backend.python")
    exact(backend["environment_lock_sha256"], lock_digest, "backend.lock")
    exact(backend["worker_sha256"], worker_digest, "backend.worker")
    model = obj(
        request["model"],
        {
            "format", "version", "family", "likelihood", "known_sigma", "posterior_method",
            "predictive_method", "decision_quantity", "decision_threshold",
            "decision_probability_threshold", "material_mean_shift", "backend_capability", "maturity",
        },
        "model",
    )
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    integer(model["version"], "model.version", 1, 1)
    exact(model["family"], "normal_mean_known_sigma", "model.family")
    exact(model["likelihood"], "normal_known_sigma", "model.likelihood")
    known_sigma = number(model["known_sigma"], "model.known_sigma")
    exact(model["posterior_method"], "exact_conjugate", "model.posterior")
    exact(model["predictive_method"], "exact_leave_one_out_normal", "model.predictive")
    exact(
        model["decision_quantity"],
        "posterior_probability_mu_above_threshold",
        "model.decision_quantity",
    )
    decision_threshold = number(model["decision_threshold"], "model.decision_threshold")
    probability_threshold = number(
        model["decision_probability_threshold"], "model.probability_threshold"
    )
    material_shift = number(model["material_mean_shift"], "model.material_shift")
    exact(model["backend_capability"], "prior_sensitivity", "model.capability")
    exact(model["maturity"], "experimental_sensitivity", "model.maturity")
    if (
        known_sigma <= 0.0
        or not 0.5 < probability_threshold < 1.0
        or material_shift <= 0.0
    ):
        raise ContractError("model controls are invalid")
    observations_raw = request["observations"]
    if not isinstance(observations_raw, list) or not 2 <= len(observations_raw) <= 100_000:
        raise ContractError("observations require 2-100000 entries")
    observations = np.asarray(
        [number(value, "observations[]") for value in observations_raw], dtype=np.float64
    )
    priors_raw = request["priors"]
    if not isinstance(priors_raw, list) or not 2 <= len(priors_raw) <= 32:
        raise ContractError("priors require 2-32 entries")
    priors = []
    for index, raw in enumerate(priors_raw):
        prior = obj(raw, {"prior_name", "prior_mean", "prior_sd"}, f"priors[{index}]")
        name = prior["prior_name"]
        if (
            not isinstance(name, str)
            or not name
            or len(name) > 128
            or name.strip() != name
            or (priors and priors[-1]["prior_name"] >= name)
        ):
            raise ContractError("prior names must be exact and increasing")
        mean = number(prior["prior_mean"], "prior.mean")
        sd = number(prior["prior_sd"], "prior.sd")
        if sd <= 0.0:
            raise ContractError("prior SDs must be positive")
        priors.append({"prior_name": name, "prior_mean": mean, "prior_sd": sd})
    base_prior = request["base_prior"]
    if not isinstance(base_prior, str) or sum(p["prior_name"] == base_prior for p in priors) != 1:
        raise ContractError("base prior must name one prior")
    resources = obj(
        request["resources"],
        {"maximum_observations", "maximum_priors", "maximum_work_units", "maximum_output_bytes", "timeout_seconds"},
        "resources",
    )
    if len(observations) > integer(resources["maximum_observations"], "resources.observations", 1, 100_000):
        raise ContractError("observation limit exceeded")
    if len(priors) > integer(resources["maximum_priors"], "resources.priors", 1, 32):
        raise ContractError("prior limit exceeded")
    if len(observations) * len(priors) > integer(
        resources["maximum_work_units"], "resources.work", 1, 3_200_000
    ):
        raise ContractError("work limit exceeded")
    integer(resources["maximum_output_bytes"], "resources.output", 1, 1_048_576)
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3_600)
    return {
        "known_sigma": known_sigma,
        "decision_threshold": decision_threshold,
        "probability_threshold": probability_threshold,
        "material_shift": material_shift,
        "observations": observations,
        "priors": priors,
        "base_prior": base_prior,
    }


def summarize(config: dict[str, Any], prior: dict[str, Any]) -> dict[str, Any]:
    observations = config["observations"]
    prior_variance = prior["prior_sd"] ** 2
    observation_variance = config["known_sigma"] ** 2
    posterior_variance = 1.0 / (
        1.0 / prior_variance + len(observations) / observation_variance
    )
    posterior_mean = posterior_variance * (
        prior["prior_mean"] / prior_variance + observations.sum() / observation_variance
    )
    posterior_sd = math.sqrt(posterior_variance)
    probability = float(
        stats.norm.sf((config["decision_threshold"] - posterior_mean) / posterior_sd)
    )
    loo_elpd = 0.0
    total = float(observations.sum())
    for observation in observations:
        leave_variance = 1.0 / (
            1.0 / prior_variance + (len(observations) - 1) / observation_variance
        )
        leave_mean = leave_variance * (
            prior["prior_mean"] / prior_variance
            + (total - float(observation)) / observation_variance
        )
        loo_elpd += float(
            stats.norm.logpdf(
                observation,
                loc=leave_mean,
                scale=math.sqrt(observation_variance + leave_variance),
            )
        )
    return {
        **prior,
        "posterior_mean": float(posterior_mean),
        "posterior_sd": posterior_sd,
        "probability_above_threshold": probability,
        "decision": probability >= config["probability_threshold"],
        "loo_elpd": loo_elpd,
    }


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    summaries = [summarize(config, prior) for prior in config["priors"]]
    base = next(summary for summary in summaries if summary["prior_name"] == config["base_prior"])
    changed = []
    shifted = []
    for summary in summaries:
        summary["mean_difference_from_base"] = summary["posterior_mean"] - base["posterior_mean"]
        summary["sd_difference_from_base"] = summary["posterior_sd"] - base["posterior_sd"]
        summary["probability_difference_from_base"] = (
            summary["probability_above_threshold"] - base["probability_above_threshold"]
        )
        summary["loo_elpd_difference_from_base"] = summary["loo_elpd"] - base["loo_elpd"]
        summary["conclusion_changed"] = summary["decision"] != base["decision"]
        summary["material_mean_shift"] = (
            abs(summary["mean_difference_from_base"]) >= config["material_shift"]
        )
        if summary["conclusion_changed"]:
            changed.append(summary["prior_name"])
        if summary["material_mean_shift"]:
            shifted.append(summary["prior_name"])
    return {
        "format": "marklab.scipy_prior_sensitivity_worker_result",
        "version": 1,
        "backend": {
            "name": "scipy",
            "version": BACKEND_VERSION,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha,
            "worker_sha256": worker_sha,
        },
        "request_sha256": request_sha,
        "observation_count": len(config["observations"]),
        "observed_mean": float(config["observations"].mean()),
        "priors": summaries,
        "conclusion_changed_priors": changed,
        "material_mean_shift_priors": shifted,
    }


def main() -> None:
    if scipy.__version__ != "1.18.1" or sys.version_info[:2] != (3, 12):
        raise ContractError("SciPy or Python version drift")
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
        print(f"marklab prior-sensitivity worker failed: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from error
