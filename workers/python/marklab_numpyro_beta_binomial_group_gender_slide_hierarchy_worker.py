#!/usr/bin/env python3
"""NumPyro agreement worker for the repeated-slide group/gender count hierarchy."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import jax

jax.config.update("jax_enable_x64", True)

import jax.numpy as jnp
import numpyro
import numpyro.distributions as dist

sys.path.append(str(Path(__file__).resolve().parent))

import marklab_pymc_beta_binomial_group_gender_slide_hierarchy_worker as pymc_contract
from marklab_worker_support.count_summaries import slide_hierarchy_report
from marklab_worker_support.numpyro_nuts import (
    fit_with_predictive,
    nuts_diagnostics,
    posterior_is_finite,
    prior_is_finite,
    result_header,
    validate_source_request,
)
from marklab_worker_support.nuts_contract import fit_summary
from marklab_worker_support.protocol import (
    MIB,
    derive_seed,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    worker_digests,
    write_result,
)

REQUEST_FORMAT = "marklab.numpyro_beta_binomial_group_gender_slide_hierarchy_worker_request"
RESULT_FORMAT = "marklab.numpyro_beta_binomial_group_gender_slide_hierarchy_worker_result"
MONITORED = [
    "intercept_log_odds",
    "group_log_odds_effect",
    "gender_log_odds_effect",
    "patient_log_odds_sd",
    "slide_concentration",
    "patient_log_odds_effect",
]


def seed_for(seed: int, purpose: str) -> int:
    return derive_seed(
        "marklab-numpyro-beta-binomial-group-gender-slide-hierarchy-v1", seed, purpose
    )


def model(
    observed_successes: jnp.ndarray | None,
    trials: jnp.ndarray,
    patient_index: jnp.ndarray,
    patient_group_indicator: jnp.ndarray,
    patient_gender_indicator: jnp.ndarray,
    patient_count: int,
    intercept_mean: float,
    intercept_sd: float,
    group_sd: float,
    gender_sd: float,
    patient_sd_prior: float,
    concentration_sd: float,
) -> None:
    """Slide beta-binomial counts nested in patients with group and gender effects.

    Likelihood: slide successes ``y_j ~ BetaBinomial(n_j, p_i * kappa, (1 - p_i) * kappa)`` for
    the slide's patient ``i``, with ``p_i = sigmoid(alpha + g_i * beta_group + s_i * beta_gender
    + u_i)``. Patient effects are non-centered, ``u_i = sigma * z_i`` with ``z_i ~ Normal(0, 1)``.
    Priors: ``alpha ~ Normal(intercept_mean, intercept_sd)``, ``beta_group ~ Normal(0,
    group_sd)``, ``beta_gender ~ Normal(0, gender_sd)``, ``sigma ~ HalfNormal(patient_sd_prior)``,
    and slide concentration ``kappa ~ HalfNormal(concentration_sd)``.
    Normal scales are SDs; half-normal scales are the SDs of their underlying normal distributions.
    """
    intercept = numpyro.sample("intercept_log_odds", dist.Normal(intercept_mean, intercept_sd))
    group_effect = numpyro.sample("group_log_odds_effect", dist.Normal(0.0, group_sd))
    gender_effect = numpyro.sample("gender_log_odds_effect", dist.Normal(0.0, gender_sd))
    patient_sd = numpyro.sample("patient_log_odds_sd", dist.HalfNormal(patient_sd_prior))
    patient_z = numpyro.sample(
        "patient_standardized_effect", dist.Normal(0.0, 1.0).expand((patient_count,))
    )
    patient_effect = numpyro.deterministic("patient_log_odds_effect", patient_sd * patient_z)
    patient_probability = numpyro.deterministic(
        "patient_probability",
        jax.nn.sigmoid(
            intercept
            + patient_group_indicator * group_effect
            + patient_gender_indicator * gender_effect
            + patient_effect
        ),
    )
    concentration = numpyro.sample("slide_concentration", dist.HalfNormal(concentration_sd))
    probability = patient_probability[patient_index]
    numpyro.sample(
        "successes",
        dist.BetaBinomial(
            probability * concentration,
            (1.0 - probability) * concentration,
            total_count=trials,
        ),
        obs=observed_successes,
    )


def model_arguments(config: dict[str, Any]) -> dict[str, Any]:
    """Keyword arguments of ``model`` other than the observations."""
    return {
        "trials": jnp.asarray(config["trials"]),
        "patient_index": jnp.asarray(config["patient_index"]),
        "patient_group_indicator": jnp.asarray(config["patient_group_indicator"]),
        "patient_gender_indicator": jnp.asarray(config["patient_gender_indicator"]),
        "patient_count": len(config["patient_ids"]),
        "intercept_mean": config["intercept_mean"],
        "intercept_sd": config["intercept_sd"],
        "group_sd": config["group_sd"],
        "gender_sd": config["gender_sd"],
        "patient_sd_prior": config["patient_sd_prior"],
        "concentration_sd": config["concentration_sd"],
    }


def fit(
    config: dict[str, Any], request_sha: str, source_sha: str, lock_sha: str, worker_sha: str
) -> dict[str, Any]:
    prior, sampler, samples, replicated = fit_with_predictive(
        model,
        config,
        seed_for,
        "observed_successes",
        jnp.asarray(config["successes"]),
        model_arguments(config),
        "successes",
    )
    extra = sampler.get_extra_fields(group_by_chain=True)
    diagnostics = nuts_diagnostics(
        samples, MONITORED, extra, config["maximum_tree_depth"], mcse=True
    )
    report, constraints = slide_hierarchy_report(config, samples, replicated)
    return {
        **result_header(RESULT_FORMAT, lock_sha, worker_sha, request_sha, source_sha),
        **fit_summary(
            diagnostics,
            config,
            prior_is_finite(prior),
            posterior_is_finite(samples, replicated),
            constraints,
        ),
        **report,
    }


def main() -> None:
    require_versions(numpyro=numpyro.__version__, jax=jax.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(2 * MIB)
    _, config, source_sha = validate_source_request(
        json.loads(raw), REQUEST_FORMAT, lock_sha, worker_sha, pymc_contract
    )
    result = fit(config, sha256_hex(raw), source_sha, lock_sha, worker_sha)
    write_result(result, max_bytes=config["maximum_output_bytes"], newline=False)


if __name__ == "__main__":
    run_worker(main, "Marklab NumPyro slide hierarchy worker failed")
