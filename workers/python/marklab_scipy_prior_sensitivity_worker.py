#!/usr/bin/env python3
"""SciPy worker for exact conjugate normal-mean prior sensitivity."""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path
from typing import Any

import numpy as np
import scipy
from scipy import stats

sys.path.append(str(Path(__file__).resolve().parent))

from marklab_worker_support import versions
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
    backend_identity,
    exact,
    integer,
    number,
    obj,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    validate_backend,
    worker_digests,
    write_result,
)

BACKEND_VERSION = f"scipy-{versions.SCIPY}"
MODEL_KEYS = {
    "format",
    "version",
    "family",
    "likelihood",
    "known_sigma",
    "posterior_method",
    "predictive_method",
    "decision_quantity",
    "decision_threshold",
    "decision_probability_threshold",
    "material_mean_shift",
    "backend_capability",
    "maturity",
}


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "model",
            "observations",
            "priors",
            "base_prior",
            "resources",
        },
        "request",
    )
    exact(request["format"], "marklab.pymc_worker_request", "request.format")
    integer(request["version"], "request.version", 1, 1)
    validate_backend(
        request["backend"],
        "backend",
        name="scipy",
        version=BACKEND_VERSION,
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )
    model = obj(request["model"], MODEL_KEYS, "model")
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
    if known_sigma <= 0.0 or not 0.5 < probability_threshold < 1.0 or material_shift <= 0.0:
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
        {
            "maximum_observations",
            "maximum_priors",
            "maximum_work_units",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    maximum_observations = integer(
        resources["maximum_observations"], "resources.observations", 1, 100_000
    )
    if len(observations) > maximum_observations:
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
    """Exact conjugate posterior and leave-one-out predictive score under one prior.

    Model: ``y_i ~ Normal(mu, known_sigma)`` with ``mu ~ Normal(prior_mean, prior_sd)``. Reports
    the posterior mean and SD, ``P(mu > decision_threshold)`` and the resulting decision, and the
    exact leave-one-out log predictive density.
    """
    observations = config["observations"]
    prior_variance = prior["prior_sd"] ** 2
    observation_variance = config["known_sigma"] ** 2
    posterior_variance = 1.0 / (1.0 / prior_variance + len(observations) / observation_variance)
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
        "backend": backend_identity("scipy", BACKEND_VERSION, lock_sha, worker_sha),
        "request_sha256": request_sha,
        "observation_count": len(config["observations"]),
        "observed_mean": float(config["observations"].mean()),
        "priors": summaries,
        "conclusion_changed_priors": changed,
        "material_mean_shift_priors": shifted,
    }


def main() -> None:
    require_versions(scipy=scipy.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(16 * MIB)
    config = validate(json.loads(raw), lock_sha, worker_sha)
    write_result(fit(config, sha256_hex(raw), lock_sha, worker_sha))


if __name__ == "__main__":
    run_worker(main, "marklab prior-sensitivity worker failed")
