#!/usr/bin/env python3
"""Simulation-based calibration of the NumPyro patient Dirichlet-multinomial two-group model."""

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

import marklab_numpyro_dirichlet_multinomial_group_worker as agreement
import marklab_pymc_dirichlet_multinomial_group_worker as pymc_contract
from marklab_worker_support import numpyro_sbc
from marklab_worker_support.count_summaries import class_probabilities
from marklab_worker_support.posterior import softmax
from marklab_worker_support.protocol import (
    MIB,
    derive_seed,
    read_request,
    require_versions,
    run_worker,
    worker_digests,
    write_result,
)
from marklab_worker_support.sbc import rank_and_coverage, rank_summary

REQUEST_FORMAT = "marklab.numpyro_dirichlet_multinomial_group_sbc_worker_request"
RESULT_FORMAT = "marklab.numpyro_dirichlet_multinomial_group_sbc_worker_result"


def seed_for(seed: int, purpose: str, replicate: int) -> int:
    return derive_seed("marklab-dirichlet-multinomial-group-sbc-v1", seed, purpose, replicate)


def total_cells(config: dict[str, Any]) -> int:
    """Cells simulated per replicate."""
    return int(np.asarray(config["totals"]).sum())


def column_ranks(draws: np.ndarray, truth: np.ndarray) -> tuple[list[int], list[bool]]:
    """Rank and 90%-interval coverage of each column's true value among its draws."""
    pairs = [rank_and_coverage(draws[:, index], truth[index]) for index in range(draws.shape[1])]
    return [rank for rank, _ in pairs], [covered for _, covered in pairs]


def replicate(config: dict[str, Any], kernel: NUTS, index: int) -> dict[str, Any] | str:
    """Simulate one data set from the prior, refit it, and rank the true values."""
    totals = np.asarray(config["totals"], dtype=np.int64)
    indicator = np.asarray(config["indicator"], dtype=np.int64)
    classes = len(config["class_ids"])
    rng = np.random.default_rng(seed_for(config["seed"], "simulate", index))
    true_baseline = rng.normal(0.0, config["logit_sd"], classes - 1)
    true_effect = rng.normal(0.0, config["group_sd"], classes - 1)
    true_concentration = float(abs(rng.normal(0.0, config["concentration_sd"])))
    true_reference = softmax(np.concatenate((true_baseline, [0.0]))[None, :])[0]
    true_comparison = softmax(np.concatenate((true_baseline + true_effect, [0.0]))[None, :])[0]
    true_difference = true_comparison - true_reference
    group_probabilities = np.stack((true_reference, true_comparison))[indicator]
    patient_probabilities = np.asarray(
        [rng.dirichlet(probability * true_concentration) for probability in group_probabilities]
    )
    observations = np.asarray(
        [
            rng.multinomial(int(total), probability)
            for total, probability in zip(totals, patient_probabilities, strict=True)
        ],
        dtype=np.int64,
    )
    samples, diagnostics = numpyro_sbc.sample_replicate(
        kernel,
        config,
        jax.random.PRNGKey(seed_for(config["seed"], "fit", index)),
        agreement.MONITORED,
        {
            "observed_counts": jnp.asarray(observations),
            "totals": jnp.asarray(totals),
            "total_count_max": int(totals.max()),
            "indicator": jnp.asarray(indicator),
            "classes": classes,
            "logit_sd": config["logit_sd"],
            "group_sd": config["group_sd"],
            "concentration_sd": config["concentration_sd"],
        },
    )
    failure = numpyro_sbc.diagnostics_failure(diagnostics, config)
    if failure is not None:
        return failure
    baseline_draws = samples["baseline_logits"].reshape(-1, classes - 1)
    effect_draws = samples["group_log_ratio_effects"].reshape(-1, classes - 1)
    concentration_draws = samples["concentration"].reshape(-1)
    reference_draws, comparison_draws = class_probabilities(baseline_draws, effect_draws)
    difference_draws = comparison_draws - reference_draws
    baseline_ranks, baseline_covered = column_ranks(baseline_draws, true_baseline)
    effect_ranks, effect_covered = column_ranks(effect_draws, true_effect)
    difference_ranks, difference_covered = column_ranks(difference_draws, true_difference)
    concentration_rank, concentration_covered = rank_and_coverage(
        concentration_draws, true_concentration
    )
    return {
        "replicate": index,
        "true_baseline_logits": true_baseline.tolist(),
        "true_group_log_ratio_effects": true_effect.tolist(),
        "true_concentration": true_concentration,
        "true_class_probability_differences": true_difference.tolist(),
        "baseline_logit_ranks": baseline_ranks,
        "group_log_ratio_effect_ranks": effect_ranks,
        "concentration_rank": concentration_rank,
        "class_probability_difference_ranks": difference_ranks,
        "baseline_logit_covered": baseline_covered,
        "group_log_ratio_effect_covered": effect_covered,
        "concentration_covered": concentration_covered,
        "class_probability_difference_covered": difference_covered,
        **diagnostics,
    }


def vector_summaries(
    rows: list[dict[str, Any]], prefix: str, width: int, config: dict[str, Any]
) -> list[dict[str, Any]]:
    """Rank summaries of each component of the vector quantity ``prefix``."""
    return [
        rank_summary(
            [row[f"{prefix}_ranks"][index] for row in rows],
            [row[f"{prefix}_covered"][index] for row in rows],
            config["chains"] * config["draws"],
            config["rank_bins"],
        )
        for index in range(width)
    ]


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
        simulated_resource="maximum_simulated_cells",
        simulated_per_replicate=total_cells,
    )
    kernel = numpyro_sbc.nuts_kernel(agreement.model, config)
    completed, failures = numpyro_sbc.run_replicates(
        config["replicates"], partial(replicate, config, kernel)
    )
    classes = len(config["class_ids"])
    diagnostics = {
        "baseline_logits": vector_summaries(completed, "baseline_logit", classes - 1, config),
        "group_log_ratio_effects": vector_summaries(
            completed, "group_log_ratio_effect", classes - 1, config
        ),
        "concentration": rank_summary(
            [row["concentration_rank"] for row in completed],
            [row["concentration_covered"] for row in completed],
            config["chains"] * config["draws"],
            config["rank_bins"],
        ),
        "class_probability_differences": vector_summaries(
            completed, "class_probability_difference", classes, config
        ),
    }
    calibrated = [
        *diagnostics["baseline_logits"],
        *diagnostics["group_log_ratio_effects"],
        diagnostics["concentration"],
        *diagnostics["class_probability_differences"],
    ]
    result = numpyro_sbc.result(
        RESULT_FORMAT,
        raw,
        lock_sha,
        worker_sha,
        config,
        completed,
        failures,
        diagnostics,
        calibrated,
    )
    write_result(result, max_bytes=config["maximum_output_bytes"], newline=False)


if __name__ == "__main__":
    run_worker(main, "Marklab Dirichlet-multinomial group SBC worker failed")
