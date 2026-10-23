#!/usr/bin/env python3
"""Simulation-based calibration of the NumPyro repeated-slide group/gender count hierarchy."""

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

import marklab_numpyro_beta_binomial_group_gender_slide_hierarchy_worker as agreement
from marklab_worker_support import beta_binomial_slide_hierarchy as source_contract
from marklab_worker_support import numpyro_sbc
from marklab_worker_support.count_summaries import group_gender_probabilities
from marklab_worker_support.posterior import sigmoid
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

REQUEST_FORMAT = "marklab.numpyro_beta_binomial_group_gender_slide_hierarchy_sbc_worker_request"
RESULT_FORMAT = "marklab.numpyro_beta_binomial_group_gender_slide_hierarchy_sbc_worker_result"
WORKER_IDENTITY_PREFIX = (
    b"marklab.numpyro_beta_binomial_group_gender_slide_hierarchy_sbc_worker_identity.v1\0"
)
CALIBRATED = [
    "intercept_log_odds",
    "group_log_odds_effect",
    "gender_log_odds_effect",
    "patient_log_odds_sd",
    "slide_concentration",
    "marginal_probability_difference",
    "patient_probability_0",
    "patient_random_effect_0",
]


def worker_identity(worker_file: str) -> str:
    """Digest of this worker and the agreement worker whose model it calibrates.

    The digest covers ``WORKER_IDENTITY_PREFIX``, this file, one NUL byte, and the agreement
    worker file, matching the identity the Rust caller places in the request.
    """
    script = Path(worker_file)
    model_file = script.with_name(Path(agreement.__file__).name)
    return sha256_hex(
        WORKER_IDENTITY_PREFIX + script.read_bytes() + b"\0" + model_file.read_bytes()
    )


def seed_for(seed: int, purpose: str, replicate: int) -> int:
    return derive_seed(
        "marklab-beta-binomial-group-gender-slide-hierarchy-sbc-v1", seed, purpose, replicate
    )


def replicate(config: dict[str, Any], kernel: NUTS, index: int) -> dict[str, Any] | str:
    """Simulate one data set from the prior, refit it, and rank the true values."""
    trials = np.asarray(config["trials"], dtype=np.int64)
    patient_index = np.asarray(config["patient_index"], dtype=np.int64)
    group_indicator = np.asarray(config["patient_group_indicator"], dtype=np.float64)
    gender_indicator = np.asarray(config["patient_gender_indicator"], dtype=np.float64)
    patient_count = len(config["patient_ids"])
    gender_weight = float(gender_indicator.mean())
    rng = np.random.default_rng(seed_for(config["seed"], "simulate", index))
    true_intercept = float(rng.normal(config["intercept_mean"], config["intercept_sd"]))
    true_group = float(rng.normal(0.0, config["group_sd"]))
    true_gender = float(rng.normal(0.0, config["gender_sd"]))
    true_patient_sd = float(abs(rng.normal(0.0, config["patient_sd_prior"])))
    true_effects = rng.normal(0.0, true_patient_sd, patient_count)
    true_concentration = float(abs(rng.normal(0.0, config["concentration_sd"])))
    true_patient_probabilities = sigmoid(
        true_intercept
        + group_indicator * true_group
        + gender_indicator * true_gender
        + true_effects
    )
    slide_means = true_patient_probabilities[patient_index]
    slide_probabilities = rng.beta(
        slide_means * true_concentration,
        (1.0 - slide_means) * true_concentration,
    )
    observations = rng.binomial(trials, slide_probabilities)
    true_difference = marginal_difference(true_intercept, true_group, true_gender, gender_weight)
    samples, diagnostics = numpyro_sbc.sample_replicate(
        kernel,
        config,
        jax.random.PRNGKey(seed_for(config["seed"], "fit", index)),
        agreement.MONITORED,
        {"observed_successes": jnp.asarray(observations), **agreement.model_arguments(config)},
    )
    failure = numpyro_sbc.diagnostics_failure(diagnostics, config)
    if failure is not None:
        return failure
    intercept = samples["intercept_log_odds"].reshape(-1)
    group = samples["group_log_odds_effect"].reshape(-1)
    gender = samples["gender_log_odds_effect"].reshape(-1)
    patient_sd = samples["patient_log_odds_sd"].reshape(-1)
    concentration = samples["slide_concentration"].reshape(-1)
    effects = samples["patient_log_odds_effect"].reshape(-1, patient_count)
    probabilities = samples["patient_probability"].reshape(-1, patient_count)
    difference = marginal_difference(intercept, group, gender, gender_weight)
    row: dict[str, Any] = {
        "replicate": index,
        "true_intercept_log_odds": true_intercept,
        "true_group_log_odds_effect": true_group,
        "true_gender_log_odds_effect": true_gender,
        "true_patient_log_odds_sd": true_patient_sd,
        "true_slide_concentration": true_concentration,
        "true_marginal_probability_difference": true_difference,
        "true_patient_probability_0": float(true_patient_probabilities[0]),
        "true_patient_random_effect_0": float(true_effects[0]),
        **diagnostics,
    }
    numpyro_sbc.record_ranks(
        row,
        [
            ("intercept_log_odds", intercept, true_intercept),
            ("group_log_odds_effect", group, true_group),
            ("gender_log_odds_effect", gender, true_gender),
            ("patient_log_odds_sd", patient_sd, true_patient_sd),
            ("slide_concentration", concentration, true_concentration),
            ("marginal_probability_difference", difference, true_difference),
            ("patient_probability_0", probabilities[:, 0], float(true_patient_probabilities[0])),
            ("patient_random_effect_0", effects[:, 0], float(true_effects[0])),
        ],
    )
    return row


def marginal_difference(intercept: Any, group: Any, gender: Any, gender_weight: float) -> Any:
    """Gender-standardized comparison-minus-reference group probability.

    Evaluated as ``w1 * p10 + w * p11 - w1 * p00 - w * p01`` with ``w1 = 1 - w``, the order
    this calibration has always used.
    """
    cells = group_gender_probabilities(intercept, group, gender, gender_weight)
    return (
        (1.0 - gender_weight) * cells["p10"]
        + gender_weight * cells["p11"]
        - (1.0 - gender_weight) * cells["p00"]
        - gender_weight * cells["p01"]
    )


def main() -> None:
    require_versions(numpyro=numpyro.__version__, jax=jax.__version__)
    lock_sha, _ = worker_digests(__file__)
    worker_sha = worker_identity(__file__)
    raw = read_request(2 * MIB)
    config = numpyro_sbc.validate_request(
        json.loads(raw), REQUEST_FORMAT, lock_sha, worker_sha, source_contract
    )
    kernel = numpyro_sbc.nuts_kernel(agreement.model, config)
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
    run_worker(main, "Marklab repeated slide SBC worker failed")
