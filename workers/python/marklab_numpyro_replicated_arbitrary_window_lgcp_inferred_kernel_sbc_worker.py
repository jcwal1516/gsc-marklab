#!/usr/bin/env python3
"""Prior-generative SBC for the inferred-kernel replicated patient LGCP."""

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

import marklab_numpyro_replicated_arbitrary_window_lgcp_inferred_kernel_worker as inferred_fit
from marklab_worker_support import replicated_lgcp_sbc
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
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
    INFERRED_KERNEL_WORKER,
    matern32_latent_draw,
    pattern_distances,
    validate_inferred_kernel_request,
)

REQUEST_FORMAT = "marklab.numpyro_replicated_arbitrary_window_lgcp_inferred_kernel_sbc_request"
RESULT_FORMAT = "marklab.numpyro_replicated_arbitrary_window_lgcp_inferred_kernel_sbc_result"
CALIBRATED_PARAMETERS = (
    "group_effect",
    "patient_sd",
    "pattern_sd",
    "field_amplitude",
    "field_length_scale_um",
)


def seed_for(seed: int, purpose: str, replicate: int) -> int:
    return derive_seed("marklab-replicated-inferred-kernel-lgcp-sbc-v1", seed, purpose, replicate)


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    def validate_source(source_request: Any) -> dict[str, Any]:
        directory = Path(__file__).parent
        inferred_digest = file_sha256(directory / INFERRED_KERNEL_WORKER)
        fixed_digest = file_sha256(directory / FIXED_KERNEL_WORKER)
        return validate_inferred_kernel_request(
            source_request, lock_digest, inferred_digest, fixed_digest
        )

    return replicated_lgcp_sbc.validate_sbc_request(
        request, REQUEST_FORMAT, lock_digest, worker_digest, validate_source
    )


def simulate(
    config: dict[str, Any], blocks: list[tuple[np.ndarray, np.ndarray]], replicate: int
) -> tuple[dict[str, float], np.ndarray]:
    """Draw one prior-generative data set, including the Matérn kernel scales."""
    rng = np.random.default_rng(seed_for(config["seed"], "simulate", replicate))
    truth = replicated_lgcp_sbc.draw_hierarchy_truth(config, rng)
    truth["field_amplitude"] = float(abs(rng.normal(0.0, config["field_amplitude_scale"])))
    truth["field_length_scale_um"] = float(
        abs(rng.normal(0.0, config["field_length_scale_scale_um"]))
    )
    if truth["field_amplitude"] <= 0.0 or truth["field_length_scale_um"] <= 0.0:
        raise ContractError("degenerate kernel prior draw")
    patient_effect, pattern_effect = replicated_lgcp_sbc.draw_random_effects(config, rng, truth)
    latent = matern32_latent_draw(
        config, blocks, truth["field_amplitude"], truth["field_length_scale_um"], rng
    )
    counts = replicated_lgcp_sbc.simulate_counts(
        config, rng, truth, patient_effect, pattern_effect, latent
    )
    return truth, counts


def run_sbc(config: dict[str, Any]) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    maximum_tree_depth = int(config["maximum_tree_depth"])
    blocks = pattern_distances(config)

    def sample_posterior(replicate: int, counts: np.ndarray) -> MCMC:
        fit_config = dict(config)
        fit_config["counts"] = counts
        kernel = NUTS(
            inferred_fit.make_model(fit_config),
            target_accept_prob=config["target_accept"],
            max_tree_depth=maximum_tree_depth,
        )
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
            extra_fields=("diverging", "num_steps", "energy"),
        )
        return sampler

    return replicated_lgcp_sbc.run_sbc(
        config,
        simulate=lambda replicate: simulate(config, blocks, replicate),
        sample_posterior=sample_posterior,
        monitored=inferred_fit.MONITORED,
        calibrated=CALIBRATED_PARAMETERS,
        latent_draws=lambda samples: samples["latent_effect"],
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
    run_worker(main, "marklab inferred-kernel replicated LGCP SBC failed")
