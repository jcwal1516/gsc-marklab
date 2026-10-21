#!/usr/bin/env python3
"""Prior-generative SBC for the replicated exact-window patient LGCP."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import jax

jax.config.update("jax_enable_x64", True)
sys.path.append(str(Path(__file__).resolve().parent))

import numpy as np
import numpyro
from numpyro.infer import MCMC, NUTS

import marklab_numpyro_replicated_arbitrary_window_lgcp_worker as fixed_kernel_fit
from marklab_worker_support import replicated_lgcp_sbc
from marklab_worker_support.protocol import (
    MIB,
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
    center_within_groups,
    fixed_kernel_latent,
    validate_request,
)

REQUEST_FORMAT = "marklab.numpyro_replicated_arbitrary_window_lgcp_sbc_request"
RESULT_FORMAT = "marklab.numpyro_replicated_arbitrary_window_lgcp_sbc_result"
CALIBRATED_PARAMETERS = ("group_effect", "patient_sd", "pattern_sd")


def seed_for(seed: int, purpose: str, replicate: int) -> int:
    return derive_seed("marklab-replicated-lgcp-sbc-v1", seed, purpose, replicate)


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    def validate_source(source_request: Any) -> dict[str, Any]:
        pymc_worker_digest = file_sha256(Path(__file__).with_name(FIXED_KERNEL_WORKER))
        return validate_request(source_request, lock_digest, pymc_worker_digest)

    return replicated_lgcp_sbc.validate_sbc_request(
        request, REQUEST_FORMAT, lock_digest, worker_digest, validate_source
    )


def simulate(config: dict[str, Any], replicate: int) -> tuple[dict[str, float], np.ndarray]:
    """Draw one prior-generative data set from the fixed-kernel model."""
    rng = np.random.default_rng(seed_for(config["seed"], "simulate", replicate))
    truth = replicated_lgcp_sbc.draw_hierarchy_truth(config, rng)
    patient_effect, pattern_effect = replicated_lgcp_sbc.draw_random_effects(config, rng, truth)
    latent = config["cholesky"] @ rng.normal(0.0, 1.0, len(config["counts"]))
    center_within_groups(latent, config["pattern_index"], len(config["pattern_ids"]))
    counts = replicated_lgcp_sbc.simulate_counts(
        config, rng, truth, patient_effect, pattern_effect, latent
    )
    return truth, counts


def run_sbc(config: dict[str, Any]) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    maximum_tree_depth = int(config["policy"]["maximum_tree_depth"])
    kernel = NUTS(
        fixed_kernel_fit.model,
        target_accept_prob=config["target_accept"],
        max_tree_depth=maximum_tree_depth,
    )

    def sample_posterior(replicate: int, counts: np.ndarray) -> MCMC:
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
            **fixed_kernel_fit.model_data(config, counts),
            extra_fields=("diverging", "num_steps", "energy"),
        )
        return sampler

    return replicated_lgcp_sbc.run_sbc(
        config,
        simulate=lambda replicate: simulate(config, replicate),
        sample_posterior=sample_posterior,
        monitored=fixed_kernel_fit.MONITORED,
        calibrated=CALIBRATED_PARAMETERS,
        latent_draws=lambda samples: fixed_kernel_latent(config, samples["field_raw"]),
        maximum_tree_depth=maximum_tree_depth,
    )


def main() -> None:
    require_versions(numpyro=numpyro.__version__, jax=jax.__version__)
    lock_digest, worker_digest = worker_digests(__file__)
    raw = read_request(16 * MIB)
    config = validate(json.loads(raw), lock_digest, worker_digest)
    completed, failures = run_sbc(config)
    result = replicated_lgcp_sbc.sbc_result(
        config,
        completed,
        failures,
        (*CALIBRATED_PARAMETERS, "latent_node"),
        RESULT_FORMAT,
        sha256_hex(raw),
        lock_digest,
        worker_digest,
    )
    write_result(result, max_bytes=config["maximum_output_bytes"])


if __name__ == "__main__":
    run_worker(main, "marklab replicated LGCP SBC failed")
