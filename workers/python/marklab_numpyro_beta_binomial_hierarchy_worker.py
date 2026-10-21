#!/usr/bin/env python3
"""NumPyro agreement worker for the patient beta-binomial hierarchy."""

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

import marklab_pymc_beta_binomial_hierarchy_worker as pymc_contract
from marklab_worker_support.count_summaries import hierarchy_report
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

REQUEST_FORMAT = "marklab.numpyro_beta_binomial_hierarchy_worker_request"
RESULT_FORMAT = "marklab.numpyro_beta_binomial_hierarchy_worker_result"
MONITORED = ["population_probability", "concentration", "patient_probability"]


def seed_for(seed: int, purpose: str) -> int:
    return derive_seed("marklab-numpyro-beta-binomial-v1", seed, purpose)


def model(
    observed_successes: jnp.ndarray | None,
    trials: jnp.ndarray,
    patient_count: int,
    population_alpha: float,
    population_beta: float,
    concentration_sd: float,
) -> None:
    """Patient beta-binomial hierarchy with explicit patient probabilities.

    Likelihood: ``y_i ~ Binomial(n_i, p_i)``. Patient probabilities are
    ``p_i ~ Beta(mu * phi, (1 - mu) * phi)`` around the population probability
    ``mu ~ Beta(population_alpha, population_beta)`` with concentration
    ``phi ~ HalfNormal(concentration_sd)``.
    Normal scales are SDs; half-normal scales are the SDs of their underlying normal distributions.
    """
    population = numpyro.sample(
        "population_probability", dist.Beta(population_alpha, population_beta)
    )
    concentration = numpyro.sample("concentration", dist.HalfNormal(concentration_sd))
    patient_probability = numpyro.sample(
        "patient_probability",
        dist.Beta(population * concentration, (1.0 - population) * concentration).expand(
            [patient_count]
        ),
    )
    numpyro.sample(
        "successes",
        dist.Binomial(total_count=trials, probs=patient_probability),
        obs=observed_successes,
    )


def fit(
    config: dict[str, Any], request_sha: str, source_sha: str, lock_sha: str, worker_sha: str
) -> dict[str, Any]:
    arguments = {
        "trials": jnp.asarray(config["trials"]),
        "patient_count": len(config["patient_ids"]),
        "population_alpha": config["population_alpha"],
        "population_beta": config["population_beta"],
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
    report, constraints = hierarchy_report(
        config,
        samples["population_probability"],
        samples["concentration"],
        samples["patient_probability"],
        replicated,
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
        json.loads(raw), REQUEST_FORMAT, lock_sha, worker_sha, pymc_contract
    )
    result = fit(config, sha256_hex(raw), source_sha, lock_sha, worker_sha)
    write_result(result, max_bytes=2 * MIB)


if __name__ == "__main__":
    run_worker(main, "Marklab NumPyro beta-binomial hierarchy worker failed")
