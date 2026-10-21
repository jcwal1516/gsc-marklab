#!/usr/bin/env python3
"""Simulation-based calibration of the NumPyro patient beta-binomial hierarchy."""

from __future__ import annotations

import json
import sys
from functools import partial
from pathlib import Path
from typing import Any

import jax

jax.config.update("jax_enable_x64", True)

import jax.numpy as jnp
import numpy as np
import numpyro
import numpyro.distributions as dist
from numpyro.infer import NUTS

sys.path.append(str(Path(__file__).resolve().parent))

import marklab_pymc_beta_binomial_hierarchy_worker as pymc_contract
from marklab_worker_support import numpyro_sbc
from marklab_worker_support.protocol import (
    MIB,
    derive_seed,
    read_request,
    require_versions,
    run_worker,
    worker_digests,
    write_result,
)

REQUEST_FORMAT = "marklab.numpyro_beta_binomial_hierarchy_sbc_worker_request"
RESULT_FORMAT = "marklab.numpyro_beta_binomial_hierarchy_sbc_worker_result"
MONITORED = ["population_probability", "concentration"]
CALIBRATED = ["population_probability", "concentration", "patient_probability_0"]


def seed_for(seed: int, purpose: str, replicate: int) -> int:
    return derive_seed("marklab-beta-binomial-sbc-v1", seed, purpose, replicate)


def collapsed_model(
    observed_successes: jnp.ndarray,
    trials: jnp.ndarray,
    population_alpha: float,
    population_beta: float,
    concentration_sd: float,
) -> None:
    """The hierarchy with patient probabilities integrated out.

    Likelihood: ``y_i ~ BetaBinomial(n_i, mu * phi, (1 - mu) * phi)``. Priors: population
    probability ``mu ~ Beta(population_alpha, population_beta)`` and concentration
    ``phi ~ HalfNormal(concentration_sd)``.
    """
    population = numpyro.sample(
        "population_probability", dist.Beta(population_alpha, population_beta)
    )
    concentration = numpyro.sample("concentration", dist.HalfNormal(concentration_sd))
    numpyro.sample(
        "successes",
        dist.BetaBinomial(
            population * concentration,
            (1.0 - population) * concentration,
            total_count=trials,
        ),
        obs=observed_successes,
    )


def replicate(config: dict[str, Any], kernel: NUTS, index: int) -> dict[str, Any] | str:
    """Simulate one data set from the prior, refit it, and rank the true values."""
    trials = np.asarray(config["trials"], dtype=np.int64)
    rng = np.random.default_rng(seed_for(config["seed"], "simulate", index))
    true_population = float(rng.beta(config["population_alpha"], config["population_beta"]))
    true_concentration = float(abs(rng.normal(0.0, config["concentration_sd"])))
    true_patients = rng.beta(
        true_population * true_concentration,
        (1.0 - true_population) * true_concentration,
        len(config["patient_ids"]),
    )
    observations = rng.binomial(trials, true_patients)
    samples, diagnostics = numpyro_sbc.sample_replicate(
        kernel,
        config,
        jax.random.PRNGKey(seed_for(config["seed"], "fit", index)),
        MONITORED,
        {
            "observed_successes": jnp.asarray(observations),
            "trials": jnp.asarray(trials),
            "population_alpha": config["population_alpha"],
            "population_beta": config["population_beta"],
            "concentration_sd": config["concentration_sd"],
        },
    )
    failure = numpyro_sbc.diagnostics_failure(diagnostics, config)
    if failure is not None:
        return failure
    population_draws = samples["population_probability"].reshape(-1)
    concentration_draws = samples["concentration"].reshape(-1)
    # The first patient's probability is drawn from its conditional beta posterior.
    conditional_rng = np.random.default_rng(seed_for(config["seed"], "conditional", index))
    patient_draws = conditional_rng.beta(
        observations[0] + population_draws * concentration_draws,
        trials[0] - observations[0] + (1.0 - population_draws) * concentration_draws,
    )
    row: dict[str, Any] = {
        "replicate": index,
        "true_population_probability": true_population,
        "true_concentration": true_concentration,
        "true_patient_probability_0": float(true_patients[0]),
        **diagnostics,
    }
    numpyro_sbc.record_ranks(
        row,
        [
            ("population_probability", population_draws, true_population),
            ("concentration", concentration_draws, true_concentration),
            ("patient_probability_0", patient_draws, float(true_patients[0])),
        ],
    )
    return row


def main() -> None:
    require_versions(numpyro=numpyro.__version__, jax=jax.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(2 * MIB)
    config = numpyro_sbc.validate_request(
        json.loads(raw), REQUEST_FORMAT, lock_sha, worker_sha, pymc_contract
    )
    kernel = numpyro_sbc.nuts_kernel(collapsed_model, config)
    completed, failures = numpyro_sbc.run_replicates(
        config["replicates"], partial(replicate, config, kernel)
    )
    diagnostics = numpyro_sbc.summarize(completed, CALIBRATED, config)
    result = numpyro_sbc.result(
        RESULT_FORMAT,
        raw,
        lock_sha,
        worker_sha,
        config,
        completed,
        failures,
        diagnostics,
        diagnostics.values(),
    )
    write_result(result, max_bytes=config["maximum_output_bytes"], newline=False)


if __name__ == "__main__":
    run_worker(main, "Marklab beta-binomial SBC worker failed")
