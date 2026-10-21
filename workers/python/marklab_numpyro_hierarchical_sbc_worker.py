#!/usr/bin/env python3
"""Bounded simulation-based calibration for Marklab's NumPyro Gaussian patient hierarchy."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import jax

jax.config.update("jax_enable_x64", True)

import arviz as az
import jax.numpy as jnp
import numpy as np
import numpyro
from numpyro.infer import MCMC, NUTS

sys.path.append(str(Path(__file__).resolve().parent))

import marklab_numpyro_hierarchical_worker as hierarchy
from marklab_worker_support import versions
from marklab_worker_support.diagnostics import (
    divergence_count,
    minimum_ebfmi,
    rank_convergence,
    tree_depth_hits,
)
from marklab_worker_support.numpyro_sbc import diagnostics_failure
from marklab_worker_support.nuts_contract import nuts_sampling
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
    backend_identity,
    derive_seed,
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
from marklab_worker_support.sbc import calibration_passes, rank_and_coverage, rank_summary

REQUEST_FORMAT = "marklab.numpyro_hierarchical_sbc_worker_request"
RESULT_FORMAT = "marklab.numpyro_hierarchical_sbc_worker_result"
PARAMETERS = ["global_mean", "between_patient_sd"]


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "jax_version",
            "model",
            "sampling",
            "calibration",
            "diagnostic_policy",
            "resources",
        },
        "request",
    )
    exact(request["format"], REQUEST_FORMAT, "request.format")
    exact(request["version"], 1, "request.version")
    validate_backend(
        request["backend"],
        "request.backend",
        name="numpyro",
        version=versions.NUMPYRO,
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )
    exact(request["jax_version"], versions.JAX, "request.jax_version")

    model = obj(
        request["model"],
        {
            "format",
            "version",
            "family",
            "global_mean_prior",
            "between_patient_sd_prior",
            "patient_effect_parameterization",
            "likelihood",
            "observation_unit",
            "biological_unit",
            "hierarchy",
            "spatial_component",
            "generated_quantities",
            "posterior_predictive_statistics",
            "backend_capability",
            "maturity",
        },
        "request.model",
    )
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    exact(model["version"], 1, "model.version")
    exact(model["family"], "gaussian_patient_varying_intercept", "model.family")
    global_prior = obj(
        model["global_mean_prior"], {"family", "mean", "sd", "rationale"}, "global prior"
    )
    exact(global_prior["family"], "normal", "global prior family")
    global_mean = number(global_prior["mean"], "global prior mean")
    global_sd = number(global_prior["sd"], "global prior sd")
    between_prior = obj(
        model["between_patient_sd_prior"],
        {"family", "sd", "support", "rationale"},
        "between prior",
    )
    exact(between_prior["family"], "half_normal", "between prior family")
    between_sd = number(between_prior["sd"], "between prior sd")
    likelihood = obj(model["likelihood"], {"family", "known_sigma", "link"}, "likelihood")
    exact(likelihood["family"], "normal_known_sigma", "likelihood.family")
    known_sigma = number(likelihood["known_sigma"], "known sigma")
    if min(global_sd, between_sd, known_sigma) <= 0.0:
        raise ContractError("hierarchy scales must be positive")

    sampling = nuts_sampling(request["sampling"], "sampling")

    calibration = obj(
        request["calibration"],
        {
            "replicates",
            "patient_count",
            "observations_per_patient",
            "rank_bins",
            "interval_probability",
            "minimum_rank_uniformity_p_value",
            "minimum_coverage",
            "maximum_coverage",
        },
        "calibration",
    )
    replicates = integer(calibration["replicates"], "calibration.replicates", 20, 100)
    patients = integer(calibration["patient_count"], "calibration.patient_count", 3, 32)
    observations_per_patient = integer(
        calibration["observations_per_patient"], "calibration.observations_per_patient", 2, 32
    )
    rank_bins = integer(calibration["rank_bins"], "calibration.rank_bins", 2, 100)
    interval = number(calibration["interval_probability"], "calibration.interval_probability")
    exact(interval, 0.9, "calibration.interval_probability")
    minimum_rank_p = number(
        calibration["minimum_rank_uniformity_p_value"],
        "calibration.minimum_rank_uniformity_p_value",
    )
    minimum_coverage = number(calibration["minimum_coverage"], "calibration.minimum_coverage")
    maximum_coverage = number(calibration["maximum_coverage"], "calibration.maximum_coverage")

    policy = obj(
        request["diagnostic_policy"],
        {
            "prior_predictive_draws",
            "maximum_r_hat",
            "minimum_bulk_ess",
            "minimum_tail_ess",
            "minimum_ebfmi",
            "maximum_divergences",
            "maximum_tree_depth_hits",
            "maximum_tree_depth",
        },
        "diagnostic_policy",
    )
    resources = obj(
        request["resources"],
        {
            "maximum_replicates",
            "maximum_simulated_observations",
            "maximum_total_iterations",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    if replicates > integer(resources["maximum_replicates"], "resources.replicates", 1, 100):
        raise ContractError("replicates exceed resources")
    total_iterations = replicates * sampling["chains"] * (sampling["tune"] + sampling["draws"])
    maximum_iterations = integer(
        resources["maximum_total_iterations"], "resources.iterations", 1, 1_000_000
    )
    if total_iterations > maximum_iterations:
        raise ContractError("iterations exceed resources")
    maximum_observations = integer(
        resources["maximum_simulated_observations"], "resources.observations", 1, 100_000
    )
    if replicates * patients * observations_per_patient > maximum_observations:
        raise ContractError("simulated observations exceed resources")
    return {
        "global_mean": global_mean,
        "global_sd": global_sd,
        "between_sd": between_sd,
        "known_sigma": known_sigma,
        **sampling,
        "replicates": replicates,
        "patients": patients,
        "observations_per_patient": observations_per_patient,
        "rank_bins": rank_bins,
        "minimum_rank_p": minimum_rank_p,
        "minimum_coverage": minimum_coverage,
        "maximum_coverage": maximum_coverage,
        "maximum_r_hat": number(policy["maximum_r_hat"], "policy.maximum_r_hat"),
        "minimum_bulk_ess": number(policy["minimum_bulk_ess"], "policy.minimum_bulk_ess"),
        "minimum_tail_ess": number(policy["minimum_tail_ess"], "policy.minimum_tail_ess"),
        "minimum_ebfmi": number(policy["minimum_ebfmi"], "policy.minimum_ebfmi"),
        "maximum_divergences": integer(
            policy["maximum_divergences"], "policy.maximum_divergences", 0, 2**63 - 1
        ),
        "maximum_tree_depth_hits": integer(
            policy["maximum_tree_depth_hits"], "policy.maximum_tree_depth_hits", 0, 2**63 - 1
        ),
        "maximum_tree_depth": integer(
            policy["maximum_tree_depth"], "policy.maximum_tree_depth", 1, 32
        ),
        "maximum_output_bytes": integer(
            resources["maximum_output_bytes"], "resources.output", 1, 2 * MIB
        ),
    }


def seed_for(seed: int, purpose: str, replicate: int) -> int:
    return derive_seed("marklab-hierarchical-sbc-v1", seed, purpose, replicate)


def fit_replicate(
    config: dict[str, Any], kernel: NUTS, patient_index: np.ndarray, replicate: int
) -> dict[str, Any]:
    """Simulate one data set from the prior, refit it, and return its ranks or failure reason.

    The truth is drawn from the same hierarchy that ``hierarchy.model`` defines: a normal
    global mean, a half-normal between-patient SD, non-centred patient means, and normal
    observations with the known SD.
    """
    rng = np.random.default_rng(seed_for(config["seed"], "simulate", replicate))
    true_global = float(rng.normal(config["global_mean"], config["global_sd"]))
    true_between = float(abs(rng.normal(0.0, config["between_sd"])))
    patient_z = rng.normal(0.0, 1.0, config["patients"])
    patient_means = true_global + true_between * patient_z
    observations = rng.normal(patient_means[patient_index], config["known_sigma"])
    sampler = MCMC(
        kernel,
        num_warmup=config["tune"],
        num_samples=config["draws"],
        num_chains=config["chains"],
        chain_method="sequential",
        progress_bar=False,
    )
    sampler.run(
        jax.random.PRNGKey(seed_for(config["seed"], "fit", replicate)),
        patient_index=jnp.asarray(patient_index),
        observations=jnp.asarray(observations),
        patient_count=config["patients"],
        global_prior_mean=config["global_mean"],
        global_prior_sd=config["global_sd"],
        between_prior_sd=config["between_sd"],
        known_sigma=config["known_sigma"],
        extra_fields=("diverging", "num_steps", "energy"),
    )
    samples = {
        name: np.asarray(value, dtype=np.float64)
        for name, value in sampler.get_samples(group_by_chain=True).items()
    }
    posterior = az.from_dict({"posterior": {name: samples[name] for name in PARAMETERS}})
    r_hat, ess_bulk, ess_tail = rank_convergence(posterior, PARAMETERS)
    extra = sampler.get_extra_fields(group_by_chain=True)
    divergences = divergence_count(extra["diverging"])
    depth_hits = tree_depth_hits(extra["num_steps"], config["maximum_tree_depth"])
    ebfmi = minimum_ebfmi(np.asarray(extra["energy"], dtype=np.float64))
    failure = diagnostics_failure(
        {
            "r_hat": r_hat,
            "ess_bulk": ess_bulk,
            "ess_tail": ess_tail,
            "minimum_ebfmi": ebfmi,
            "divergences": divergences,
            "max_tree_depth_hits": depth_hits,
        },
        config,
    )
    if failure is not None:
        return {"replicate": replicate, "reason": failure}
    global_rank, global_covered = rank_and_coverage(samples["global_mean"].reshape(-1), true_global)
    between_rank, between_covered = rank_and_coverage(
        samples["between_patient_sd"].reshape(-1), true_between
    )
    return {
        "replicate": replicate,
        "true_global_mean": true_global,
        "true_between_patient_sd": true_between,
        "global_mean_rank": global_rank,
        "between_patient_sd_rank": between_rank,
        "global_mean_covered": global_covered,
        "between_patient_sd_covered": between_covered,
        "r_hat": r_hat,
        "ess_bulk": ess_bulk,
        "ess_tail": ess_tail,
        "divergences": divergences,
        "max_tree_depth_hits": depth_hits,
    }


def run_sbc(config: dict[str, Any]) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    patient_index = np.repeat(
        np.arange(config["patients"], dtype=np.int32), config["observations_per_patient"]
    )
    kernel = NUTS(
        hierarchy.model,
        target_accept_prob=config["target_accept"],
        max_tree_depth=config["maximum_tree_depth"],
    )
    completed = []
    failures = []
    for replicate in range(config["replicates"]):
        try:
            row = fit_replicate(config, kernel, patient_index, replicate)
        except Exception as error:
            failures.append(
                {"replicate": replicate, "reason": f"{type(error).__name__}:{str(error)[:400]}"}
            )
            continue
        if "reason" in row:
            failures.append(row)
        else:
            completed.append(row)
    return completed, failures


def main() -> None:
    require_versions(numpyro=numpyro.__version__, jax=jax.__version__)
    lock_digest, worker_digest = worker_digests(__file__)
    raw = read_request(2 * MIB)
    config = validate(json.loads(raw), lock_digest, worker_digest)
    completed, failures = run_sbc(config)
    draws = config["chains"] * config["draws"]
    diagnostics = {
        name: rank_summary(
            [row[f"{name}_rank"] for row in completed],
            [row[f"{name}_covered"] for row in completed],
            draws,
            config["rank_bins"],
        )
        for name in PARAMETERS
    }
    passes = calibration_passes(
        diagnostics.values(),
        failures,
        config["minimum_rank_p"],
        config["minimum_coverage"],
        config["maximum_coverage"],
    )
    result = {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": backend_identity("numpyro", numpyro.__version__, lock_digest, worker_digest),
        "jax_version": jax.__version__,
        "request_sha256": sha256_hex(raw),
        "fit_state": "complete" if passes else "nonconverged",
        "replicates": completed,
        "failures": failures,
        "diagnostics": diagnostics,
    }
    write_result(result, max_bytes=config["maximum_output_bytes"], newline=False)


if __name__ == "__main__":
    run_worker(main, "Marklab hierarchy SBC worker failed")
