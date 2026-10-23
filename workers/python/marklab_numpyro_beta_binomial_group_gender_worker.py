#!/usr/bin/env python3
"""NumPyro agreement worker for the additive patient group/gender beta-binomial regression."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import jax

jax.config.update("jax_enable_x64", True)

import jax.numpy as jnp
import numpy as np
import numpyro
import numpyro.distributions as dist

sys.path.append(str(Path(__file__).resolve().parent))

from marklab_worker_support import beta_binomial_group_gender as source_contract
from marklab_worker_support.count_summaries import group_gender_report
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

REQUEST_FORMAT = "marklab.numpyro_beta_binomial_group_gender_worker_request"
RESULT_FORMAT = "marklab.numpyro_beta_binomial_group_gender_worker_result"
MONITORED = [
    "intercept_log_odds",
    "group_log_odds_effect",
    "gender_log_odds_effect",
    "concentration",
]


def seed_for(seed: int, purpose: str) -> int:
    return derive_seed("marklab-numpyro-beta-binomial-group-gender-v1", seed, purpose)


def model(
    observed_successes: jnp.ndarray | None,
    trials: jnp.ndarray,
    group_indicator: jnp.ndarray,
    gender_indicator: jnp.ndarray,
    intercept_mean: float,
    intercept_sd: float,
    group_sd: float,
    gender_sd: float,
    concentration_sd: float,
) -> None:
    """Collapsed patient beta-binomial regression with additive group and gender effects.

    Likelihood: ``y_i ~ BetaBinomial(n_i, p_i * phi, (1 - p_i) * phi)`` with
    ``p_i = sigmoid(alpha + g_i * beta_group + s_i * beta_gender)``. Priors: intercept
    ``alpha ~ Normal(intercept_mean, intercept_sd)``, ``beta_group ~ Normal(0, group_sd)``,
    ``beta_gender ~ Normal(0, gender_sd)``, and concentration
    ``phi ~ HalfNormal(concentration_sd)``.
    Normal scales are SDs; half-normal scales are the SDs of their underlying normal distributions.
    """
    intercept = numpyro.sample("intercept_log_odds", dist.Normal(intercept_mean, intercept_sd))
    group_effect = numpyro.sample("group_log_odds_effect", dist.Normal(0.0, group_sd))
    gender_effect = numpyro.sample("gender_log_odds_effect", dist.Normal(0.0, gender_sd))
    concentration = numpyro.sample("concentration", dist.HalfNormal(concentration_sd))
    probability = jax.nn.sigmoid(
        intercept + group_indicator * group_effect + gender_indicator * gender_effect
    )
    numpyro.sample(
        "successes",
        dist.BetaBinomial(
            probability * concentration,
            (1.0 - probability) * concentration,
            total_count=trials,
        ),
        obs=observed_successes,
    )


def fit(
    config: dict[str, Any], request_sha: str, source_sha: str, lock_sha: str, worker_sha: str
) -> dict[str, Any]:
    arguments = {
        "trials": jnp.asarray(config["trials"]),
        "group_indicator": jnp.asarray(config["group_indicator"]),
        "gender_indicator": jnp.asarray(config["gender_indicator"]),
        "intercept_mean": config["intercept_mean"],
        "intercept_sd": config["intercept_sd"],
        "group_sd": config["group_sd"],
        "gender_sd": config["gender_sd"],
        "concentration_sd": config["concentration_sd"],
    }
    prior, sampler, samples, replicated = fit_with_predictive(
        model,
        config,
        seed_for,
        "observed_successes",
        jnp.asarray(config["successes"]),
        arguments,
        "successes",
    )
    extra = sampler.get_extra_fields(group_by_chain=True)
    diagnostics = nuts_diagnostics(
        samples, MONITORED, extra, config["maximum_tree_depth"], mcse=True
    )
    report, constraints = group_gender_report(
        config,
        samples["intercept_log_odds"],
        samples["group_log_odds_effect"],
        samples["gender_log_odds_effect"],
        samples["concentration"],
        replicated,
        np.random.default_rng(seed_for(config["seed"], "conditional")),
    )
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
        json.loads(raw), REQUEST_FORMAT, lock_sha, worker_sha, source_contract
    )
    result = fit(config, sha256_hex(raw), source_sha, lock_sha, worker_sha)
    write_result(result, max_bytes=2 * MIB)


if __name__ == "__main__":
    run_worker(main, "Marklab NumPyro group/gender worker failed")
