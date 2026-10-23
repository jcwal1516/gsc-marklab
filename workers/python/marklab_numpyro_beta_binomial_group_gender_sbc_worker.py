#!/usr/bin/env python3
"""Simulation-based calibration of the NumPyro patient group/gender beta-binomial regression."""

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
from numpyro.infer import NUTS

sys.path.append(str(Path(__file__).resolve().parent))

from marklab_worker_support import beta_binomial_group_gender as source_contract
from marklab_numpyro_beta_binomial_group_gender_worker import model
from marklab_worker_support import numpyro_sbc
from marklab_worker_support.count_summaries import group_gender_probabilities
from marklab_worker_support.posterior import sigmoid
from marklab_worker_support.protocol import (
    MIB,
    derive_seed,
    read_request,
    require_versions,
    run_worker,
    worker_digests,
    write_result,
)

REQUEST_FORMAT = "marklab.numpyro_beta_binomial_group_gender_sbc_worker_request"
RESULT_FORMAT = "marklab.numpyro_beta_binomial_group_gender_sbc_worker_result"
MONITORED = [
    "intercept_log_odds",
    "group_log_odds_effect",
    "gender_log_odds_effect",
    "concentration",
]
CALIBRATED = [
    "intercept_log_odds",
    "group_log_odds_effect",
    "gender_log_odds_effect",
    "concentration",
    "marginal_probability_difference",
    "patient_probability_0",
]


def seed_for(seed: int, purpose: str, replicate: int) -> int:
    return derive_seed("marklab-beta-binomial-group-gender-sbc-v1", seed, purpose, replicate)


def replicate(config: dict[str, Any], kernel: NUTS, index: int) -> dict[str, Any] | str:
    """Simulate one data set from the prior, refit it, and rank the true values."""
    trials = np.asarray(config["trials"], dtype=np.int64)
    group_indicator = np.asarray(config["group_indicator"], dtype=np.float64)
    gender_indicator = np.asarray(config["gender_indicator"], dtype=np.float64)
    gender_weight = float(gender_indicator.mean())
    rng = np.random.default_rng(seed_for(config["seed"], "simulate", index))
    true_intercept = float(rng.normal(config["intercept_mean"], config["intercept_sd"]))
    true_group_effect = float(rng.normal(0.0, config["group_sd"]))
    true_gender_effect = float(rng.normal(0.0, config["gender_sd"]))
    true_concentration = float(abs(rng.normal(0.0, config["concentration_sd"])))
    true_difference = float(
        group_gender_probabilities(
            true_intercept, true_group_effect, true_gender_effect, gender_weight
        )["marginal_difference"]
    )
    linear_probabilities = sigmoid(
        true_intercept + group_indicator * true_group_effect + gender_indicator * true_gender_effect
    )
    true_patients = rng.beta(
        linear_probabilities * true_concentration,
        (1.0 - linear_probabilities) * true_concentration,
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
            "group_indicator": jnp.asarray(group_indicator),
            "gender_indicator": jnp.asarray(gender_indicator),
            "intercept_mean": config["intercept_mean"],
            "intercept_sd": config["intercept_sd"],
            "group_sd": config["group_sd"],
            "gender_sd": config["gender_sd"],
            "concentration_sd": config["concentration_sd"],
        },
    )
    failure = numpyro_sbc.diagnostics_failure(diagnostics, config)
    if failure is not None:
        return failure
    intercept_draws = samples["intercept_log_odds"].reshape(-1)
    group_effect_draws = samples["group_log_odds_effect"].reshape(-1)
    gender_effect_draws = samples["gender_log_odds_effect"].reshape(-1)
    concentration_draws = samples["concentration"].reshape(-1)
    difference_draws = group_gender_probabilities(
        intercept_draws, group_effect_draws, gender_effect_draws, gender_weight
    )["marginal_difference"]
    # The first patient's probability is drawn from its conditional beta posterior.
    patient_linear_draws = sigmoid(
        intercept_draws
        + group_indicator[0] * group_effect_draws
        + gender_indicator[0] * gender_effect_draws
    )
    conditional_rng = np.random.default_rng(seed_for(config["seed"], "conditional", index))
    patient_draws = conditional_rng.beta(
        observations[0] + patient_linear_draws * concentration_draws,
        trials[0] - observations[0] + (1.0 - patient_linear_draws) * concentration_draws,
    )
    row: dict[str, Any] = {
        "replicate": index,
        "true_intercept_log_odds": true_intercept,
        "true_group_log_odds_effect": true_group_effect,
        "true_gender_log_odds_effect": true_gender_effect,
        "true_concentration": true_concentration,
        "true_marginal_probability_difference": true_difference,
        "true_patient_probability_0": float(true_patients[0]),
        **diagnostics,
    }
    numpyro_sbc.record_ranks(
        row,
        [
            ("intercept_log_odds", intercept_draws, true_intercept),
            ("group_log_odds_effect", group_effect_draws, true_group_effect),
            ("gender_log_odds_effect", gender_effect_draws, true_gender_effect),
            ("concentration", concentration_draws, true_concentration),
            ("marginal_probability_difference", difference_draws, true_difference),
            ("patient_probability_0", patient_draws, float(true_patients[0])),
        ],
    )
    return row


def main() -> None:
    require_versions(numpyro=numpyro.__version__, jax=jax.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(2 * MIB)
    config = numpyro_sbc.validate_request(
        json.loads(raw), REQUEST_FORMAT, lock_sha, worker_sha, source_contract
    )
    kernel = numpyro_sbc.nuts_kernel(model, config)
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
    run_worker(main, "Marklab beta-binomial group/gender SBC worker failed")
