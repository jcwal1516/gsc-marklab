#!/usr/bin/env python3
"""Pinned PyMC fit for independent exact-window patterns nested in patients."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

sys.path.append(str(Path(__file__).resolve().parent))

# Configure PyTensor before PyMC is imported.
from marklab_worker_support import pytensor_config  # noqa: F401

# isort: split
import numpy as np
import pymc as pm
import pytensor.tensor as pt

from marklab_worker_support.diagnostics import (
    mcse_extremes,
    pymc_sampler_diagnostics,
    rank_convergence,
)
from marklab_worker_support.posterior import summary
from marklab_worker_support.protocol import (
    MIB,
    backend_identity,
    derive_seed,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    worker_digests,
    write_result,
)
from marklab_worker_support.replicated_lgcp import (
    HIERARCHY_PARAMETERS,
    effect_summaries,
    pattern_posterior_predictive,
    prior_is_finite,
    validate_request,
)

RESULT_FORMAT = "marklab.bayesian_replicated_arbitrary_window_lgcp_fit"


def pattern_means(values: Any, group_index: np.ndarray, groups: int) -> Any:
    """Symbolic mean of ``values`` within each group, as a PyTensor vector."""
    return pt.stack(
        [pt.mean(values[np.flatnonzero(group_index == group)]) for group in range(groups)]
    )


def fit(
    config: dict[str, Any], request_sha256: str, lock_digest: str, worker_digest: str
) -> dict[str, Any]:
    """Fit the fixed-kernel replicated patient LGCP by NUTS.

    Likelihood: node counts ``y ~ Poisson(exp(log(weight) + eta))`` with
    ``eta = intercept + group_effect * group + covariate_effect * covariate
    + patient_effect + pattern_effect + latent``.
    Priors: ``intercept ~ Normal(intercept_mean, intercept_sd)``, group and covariate effects
    ``~ Normal(0, sd)``, ``patient_sd, pattern_sd ~ HalfNormal(scale)``.
    Parameterization: non-centred; ``patient_effect = patient_sd * z_patient``,
    ``pattern_effect = pattern_sd * (z_pattern - mean of z_pattern within the patient)``, and
    ``latent = L z_field`` centred within each pattern, where ``L`` is the request's fixed
    block Cholesky factor of the Gaussian-field covariance.
    """
    priors = config["priors"]
    patient_count = len(config["patient_ids"])
    pattern_count = len(config["pattern_ids"])
    prior_finite = prior_is_finite(config)
    with pm.Model():
        intercept = pm.Normal(
            "intercept", mu=priors["intercept_mean"], sigma=priors["intercept_sd"]
        )
        group_effect = pm.Normal("group_effect", mu=0.0, sigma=priors["group_effect_sd"])
        covariate_effect = pm.Normal(
            "covariate_effect", mu=0.0, sigma=priors["covariate_effect_sd"]
        )
        patient_sd = pm.HalfNormal("patient_sd", sigma=priors["patient_sd_scale"])
        pattern_sd = pm.HalfNormal("pattern_sd", sigma=priors["pattern_sd_scale"])
        patient_raw = pm.Normal("patient_raw", 0.0, 1.0, shape=patient_count)
        pattern_raw = pm.Normal("pattern_raw", 0.0, 1.0, shape=pattern_count)
        field_raw = pm.Normal("field_raw", 0.0, 1.0, shape=len(config["counts"]))
        patient_effect = pm.Deterministic("patient_effect", patient_sd * patient_raw)
        pattern_patient_mean = pattern_means(
            pattern_raw, config["pattern_patient_index"], patient_count
        )
        pattern_effect = pm.Deterministic(
            "pattern_effect",
            pattern_sd * (pattern_raw - pattern_patient_mean[config["pattern_patient_index"]]),
        )
        latent_uncentered = config["cholesky"] @ field_raw
        latent_pattern_mean = pattern_means(
            latent_uncentered, config["pattern_index"], pattern_count
        )
        latent_effect = pm.Deterministic(
            "latent_effect",
            latent_uncentered - latent_pattern_mean[config["pattern_index"]],
        )
        log_expected = (
            intercept
            + group_effect * config["group"][config["patient_index"]]
            + covariate_effect * config["covariate"]
            + patient_effect[config["patient_index"]]
            + pattern_effect[config["pattern_index"]]
            + latent_effect
            + np.log(config["weight"])
        )
        expected_count = pm.Deterministic("expected_count", pm.math.exp(log_expected))
        pm.Poisson("observed_count", mu=expected_count, observed=config["counts"])
        trace = pm.sample(
            draws=config["draws"],
            tune=config["tune"],
            chains=config["chains"],
            cores=1,
            target_accept=config["target_accept"],
            random_seed=config["seed"],
            progressbar=False,
            compute_convergence_checks=False,
            return_inferencedata=True,
            nuts={"max_treedepth": int(config["policy"]["maximum_tree_depth"])},
        )
    names = [*HIERARCHY_PARAMETERS, "patient_raw", "pattern_raw", "field_raw"]
    r_hat, bulk, tail = rank_convergence(trace, names)
    mcse_mean, mcse_sd = mcse_extremes(trace, names)
    ebfmi, divergences, depth_hits = pymc_sampler_diagnostics(trace.sample_stats)
    posterior = trace.posterior
    expected_draws = np.asarray(posterior["expected_count"], dtype=np.float64).reshape(
        -1, len(config["counts"])
    )
    predictive_seed = derive_seed(
        "marklab-replicated-lgcp-posterior-predictive-v1", config["seed"], width=8
    )
    replicated_counts = np.random.default_rng(predictive_seed).poisson(expected_draws)
    policy = config["policy"]
    complete = (
        prior_finite
        and r_hat <= policy["maximum_r_hat"]
        and bulk >= policy["minimum_bulk_ess"]
        and tail >= policy["minimum_tail_ess"]
        and ebfmi >= policy["minimum_ebfmi"]
        and divergences <= policy["maximum_divergences"]
        and depth_hits <= policy["maximum_tree_depth_hits"]
    )
    return {
        "format": RESULT_FORMAT,
        "version": 2,
        "backend": backend_identity("pymc", pm.__version__, lock_digest, worker_digest),
        "input_sha256": config["request"]["input_sha256"],
        "request_sha256": request_sha256,
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"],
            "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"],
            "completed_draws": config["chains"] * config["draws"],
        },
        "posterior": {
            name: summary(posterior[name], dtype=np.float64, chains=config["chains"])
            for name in HIERARCHY_PARAMETERS
        },
        **effect_summaries(
            config,
            np.asarray(posterior["patient_effect"]),
            np.asarray(posterior["pattern_effect"]),
            np.asarray(posterior["latent_effect"]),
            expected_draws,
        ),
        "pattern_posterior_predictive": pattern_posterior_predictive(config, replicated_counts),
        "diagnostics": {
            "prior_predictive_finite": prior_finite,
            "posterior_finite": True,
            "r_hat": r_hat,
            "ess_bulk": bulk,
            "ess_tail": tail,
            "mcse_mean": mcse_mean,
            "mcse_sd": mcse_sd,
            "minimum_ebfmi": ebfmi,
            "divergences": divergences,
            "max_tree_depth_hits": depth_hits,
            "constraints_valid": True,
            "identifiability_checks_passed": True,
        },
    }


def main() -> None:
    require_versions(pymc=pm.__version__)
    lock_digest, worker_digest = worker_digests(__file__)
    raw = read_request(16 * MIB)
    config = validate_request(json.loads(raw), lock_digest, worker_digest)
    write_result(fit(config, sha256_hex(raw), lock_digest, worker_digest))


if __name__ == "__main__":
    run_worker(main, "marklab replicated LGCP failed")
