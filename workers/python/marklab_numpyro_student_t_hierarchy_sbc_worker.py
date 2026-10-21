#!/usr/bin/env python3
"""Simulation-based calibration of the NumPyro robust Student-t patient hierarchy."""

from __future__ import annotations

import json
import sys
from functools import partial
from pathlib import Path
from typing import Any

import jax

jax.config.update("jax_enable_x64", True)

import numpy as np
import numpyro
from numpyro.infer import NUTS

sys.path.append(str(Path(__file__).resolve().parent))

import marklab_numpyro_student_t_hierarchy_worker as agreement
import marklab_pymc_student_t_hierarchy_worker as pymc_contract
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
from marklab_worker_support.student_t_hierarchy import student_t_scale_from_sd

REQUEST_FORMAT = "marklab.numpyro_student_t_hierarchy_sbc_worker_request"
RESULT_FORMAT = "marklab.numpyro_student_t_hierarchy_sbc_worker_result"
MONITORED = [
    "global_mean",
    "between_patient_sd",
    "observation_sd",
    "degrees_of_freedom",
    "patient_mean",
]
CALIBRATED = ["global_mean", "between_patient_sd", "observation_sd", "degrees_of_freedom"]


def seed_for(seed: int, purpose: str, replicate: int) -> int:
    return derive_seed("marklab-student-t-sbc-v1", seed, purpose, replicate)


def observation_count(config: dict[str, Any]) -> int:
    return sum(map(len, config["patient_values"]))


def degrees_of_freedom(samples: dict[str, np.ndarray]) -> dict[str, np.ndarray]:
    return {"degrees_of_freedom": 2.0 + samples["degrees_of_freedom_excess"]}


def replicate(config: dict[str, Any], kernel: NUTS, index: int) -> dict[str, Any] | str:
    """Simulate one data set from the prior, refit it, and rank the true values."""
    patient_index = agreement.observation_patients(config)
    rng = np.random.default_rng(seed_for(config["seed"], "simulate", index))
    true_global = float(rng.normal(config["global_mean"], config["global_sd"]))
    true_between = float(abs(rng.normal(0.0, config["between_sd"])))
    true_observation = float(abs(rng.normal(0.0, config["observation_sd"])))
    true_df = float(2.0 + rng.exponential(1.0 / config["df_rate"]))
    patient_means = true_global + true_between * rng.normal(0.0, 1.0, len(config["patient_ids"]))
    true_scale = student_t_scale_from_sd(true_observation, true_df)
    observations = patient_means[patient_index] + true_scale * rng.standard_t(
        true_df, patient_index.size
    )
    samples, diagnostics = numpyro_sbc.sample_replicate(
        kernel,
        config,
        jax.random.PRNGKey(seed_for(config["seed"], "fit", index)),
        MONITORED,
        agreement.model_arguments(config, patient_index, observations),
        derive=degrees_of_freedom,
    )
    failure = numpyro_sbc.diagnostics_failure(diagnostics, config)
    if failure is not None:
        return failure
    row: dict[str, Any] = {
        "replicate": index,
        "true_global_mean": true_global,
        "true_between_patient_sd": true_between,
        "true_observation_sd": true_observation,
        "true_degrees_of_freedom": true_df,
        **diagnostics,
    }
    numpyro_sbc.record_ranks(
        row,
        [
            ("global_mean", samples["global_mean"].reshape(-1), true_global),
            ("between_patient_sd", samples["between_patient_sd"].reshape(-1), true_between),
            ("observation_sd", samples["observation_sd"].reshape(-1), true_observation),
            ("degrees_of_freedom", samples["degrees_of_freedom"].reshape(-1), true_df),
        ],
    )
    return row


def main() -> None:
    require_versions(numpyro=numpyro.__version__, jax=jax.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(2 * MIB)
    config = numpyro_sbc.validate_request(
        json.loads(raw),
        REQUEST_FORMAT,
        lock_sha,
        worker_sha,
        pymc_contract,
        simulated_resource="maximum_simulated_observations",
        simulated_limit=10_000_000,
        simulated_per_replicate=observation_count,
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
    run_worker(main, "Marklab Student-t SBC worker failed")
