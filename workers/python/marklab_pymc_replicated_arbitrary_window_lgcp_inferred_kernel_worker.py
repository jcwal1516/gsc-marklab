#!/usr/bin/env python3
"""Pinned PyMC replicated exact-window LGCP with shared inferred Matérn scales."""

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
    file_sha256,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    worker_digests,
    write_result,
)
from marklab_worker_support.replicated_lgcp import (
    FIXED_KERNEL_WORKER,
    HIERARCHY_PARAMETERS,
    INFERRED_KERNEL_SEED_DOMAIN,
    effect_summaries,
    inferred_kernel_prior_is_finite,
    matern32,
    pattern_distances,
    pattern_posterior_predictive,
    validate_inferred_kernel_request,
)

RESULT_FORMAT = "marklab.bayesian_replicated_arbitrary_window_lgcp_inferred_kernel_fit"
SUMMARIZED_PARAMETERS = (*HIERARCHY_PARAMETERS, "field_amplitude", "field_length_scale_um")


def seed_for(seed: int, purpose: str) -> int:
    return derive_seed(INFERRED_KERNEL_SEED_DOMAIN, seed, purpose)


def fit(
    config: dict[str, Any], request_sha256: str, lock_digest: str, worker_digest: str
) -> dict[str, Any]:
    """Fit the inferred-kernel replicated patient LGCP by NUTS.

    Likelihood: node counts ``y ~ Poisson(weight * exp(eta))`` with
    ``eta = intercept + group_effect * group + covariate_effect * covariate
    + patient_effect + pattern_effect + latent``.
    Priors: as in the fixed-kernel model (normal intercept and effects, half-normal
    ``patient_sd`` and ``pattern_sd``), plus ``field_amplitude ~ HalfNormal(amplitude_scale)``
    and ``field_length_scale_um ~ HalfNormal(length_scale_scale)`` shared by all patterns.
    Parameterization: non-centred hierarchy (pattern effects centred within patient); each
    pattern's latent block is ``chol(K + jitter I) z_field`` centred within the pattern, where
    ``K`` is the Matérn-3/2 covariance of the pattern's node distances.
    """
    priors = config["priors"]
    patient_count = len(config["patient_ids"])
    with pm.Model():
        intercept = pm.Normal("intercept", priors["intercept_mean"], priors["intercept_sd"])
        group_effect = pm.Normal("group_effect", 0.0, priors["group_effect_sd"])
        covariate_effect = pm.Normal("covariate_effect", 0.0, priors["covariate_effect_sd"])
        patient_sd = pm.HalfNormal("patient_sd", priors["patient_sd_scale"])
        pattern_sd = pm.HalfNormal("pattern_sd", priors["pattern_sd_scale"])
        field_amplitude = pm.HalfNormal("field_amplitude", config["field_amplitude_scale"])
        field_length_scale_um = pm.HalfNormal(
            "field_length_scale_um", config["field_length_scale_scale_um"]
        )
        patient_raw = pm.Normal("patient_raw", 0.0, 1.0, shape=patient_count)
        pattern_raw = pm.Normal("pattern_raw", 0.0, 1.0, shape=len(config["pattern_ids"]))
        field_raw = pm.Normal("field_raw", 0.0, 1.0, shape=len(config["counts"]))
        patient_effect = pm.Deterministic("patient_effect", patient_sd * patient_raw)
        pattern_patient_mean = pt.stack(
            [
                pt.mean(pattern_raw[np.flatnonzero(config["pattern_patient_index"] == patient)])
                for patient in range(patient_count)
            ]
        )
        pattern_effect = pm.Deterministic(
            "pattern_effect",
            pattern_sd * (pattern_raw - pattern_patient_mean[config["pattern_patient_index"]]),
        )
        latent_blocks = []
        for selected, distance in pattern_distances(config):
            covariance = matern32(distance, field_amplitude, field_length_scale_um, pt.exp)
            covariance += config["kernel_jitter"] * np.eye(len(selected))
            block = pt.linalg.cholesky(covariance) @ field_raw[selected]
            latent_blocks.append(block - pt.mean(block))
        latent_effect = pm.Deterministic("latent_effect", pt.concatenate(latent_blocks))
        expected_count = pm.Deterministic(
            "expected_count",
            config["weight"]
            * pt.exp(
                intercept
                + group_effect * config["group"][config["patient_index"]]
                + covariate_effect * config["covariate"]
                + patient_effect[config["patient_index"]]
                + pattern_effect[config["pattern_index"]]
                + latent_effect
            ),
        )
        pm.Poisson("observed_count", expected_count, observed=config["counts"])
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
            nuts={"max_treedepth": config["maximum_tree_depth"]},
        )
    names = [*SUMMARIZED_PARAMETERS, "patient_raw", "pattern_raw", "field_raw"]
    r_hat, bulk, tail = rank_convergence(trace, names)
    mcse_mean, mcse_sd = mcse_extremes(trace, names)
    ebfmi, divergences, depth_hits = pymc_sampler_diagnostics(trace.sample_stats)
    posterior = trace.posterior
    expected = np.asarray(posterior["expected_count"], dtype=np.float64).reshape(
        -1, len(config["counts"])
    )
    latent = np.asarray(posterior["latent_effect"], dtype=np.float64).reshape(
        -1, len(config["counts"])
    )
    replicated = np.random.default_rng(seed_for(config["seed"], "predictive")).poisson(expected)
    predictive = pattern_posterior_predictive(config, replicated)
    policy = config["policy"]
    prior_finite = inferred_kernel_prior_is_finite(config)
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
        "version": 1,
        "backend": backend_identity("pymc", pm.__version__, lock_digest, worker_digest),
        "source_backend": config["request"]["backend"],
        "input_sha256": config["request"]["input_sha256"],
        "request_sha256": request_sha256,
        "source_request_sha256": config["outer_request"]["source_request_sha256"],
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"],
            "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"],
            "completed_draws": config["chains"] * config["draws"],
        },
        "posterior": {
            name: summary(posterior[name], dtype=np.float64, chains=config["chains"])
            for name in SUMMARIZED_PARAMETERS
        },
        **effect_summaries(
            config,
            np.asarray(posterior["patient_effect"]),
            np.asarray(posterior["pattern_effect"]),
            latent,
            expected,
        ),
        "pattern_posterior_predictive": predictive,
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
        "kernel_cube_work": config["kernel_cube_work"],
    }


def main() -> None:
    require_versions(pymc=pm.__version__)
    lock_digest, worker_digest = worker_digests(__file__)
    source_worker_digest = file_sha256(Path(__file__).with_name(FIXED_KERNEL_WORKER))
    raw = read_request(16 * MIB)
    config = validate_inferred_kernel_request(
        json.loads(raw), lock_digest, worker_digest, source_worker_digest
    )
    write_result(fit(config, sha256_hex(raw), lock_digest, worker_digest))


if __name__ == "__main__":
    run_worker(main, "marklab inferred-kernel replicated LGCP failed")
