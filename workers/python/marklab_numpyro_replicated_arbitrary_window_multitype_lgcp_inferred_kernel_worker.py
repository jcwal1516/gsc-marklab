#!/usr/bin/env python3
"""Independent NumPyro replicated multitype LGCP with inferred shared kernel."""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path
from typing import Any

sys.path.append(str(Path(__file__).resolve().parent))

import jax

jax.config.update("jax_enable_x64", True)

import jax.numpy as jnp
import numpy as np
import numpyro
import numpyro.distributions as dist
from numpyro.infer import MCMC, NUTS

from marklab_worker_support import multitype_lgcp as lgcp
from marklab_worker_support import versions
from marklab_worker_support.multitype_lgcp_numpyro import (
    hierarchical_model,
    posterior_draws,
    prior_is_finite,
)
from marklab_worker_support.numpyro_nuts import nuts_diagnostics
from marklab_worker_support.nuts_contract import convergence_passes
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
    backend_identity,
    derive_seed,
    exact,
    file_sha256,
    integer,
    obj,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    validate_backend,
    worker_digests,
    write_result,
)

REQUEST_FORMAT = (
    "marklab.numpyro_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_request"
)
RESULT_FORMAT = "marklab.numpyro_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_result"
PYMC_INFERRED_WORKER = (
    "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_worker.py"
)
PYMC_FIXED_WORKER = "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_worker.py"
MONITORED = [
    "intercept",
    "group_effect",
    "covariate_effect",
    "patient_sd",
    "pattern_sd",
    "field_amplitude",
    "field_length_scale_um",
    "patient_raw",
    "pattern_raw",
    "field_raw",
]


def validate(
    request: Any,
    lock_sha: str,
    worker_sha: str,
    pymc_inferred_sha: str,
    pymc_fixed_sha: str,
) -> tuple[dict[str, Any], dict[str, Any]]:
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "jax_version",
            "source_request_sha256",
            "source_request",
            "maximum_tree_depth",
        },
        "request",
    )
    exact(request["format"], REQUEST_FORMAT, "request.format")
    exact(request["version"], 1, "request.version")
    validate_backend(
        request["backend"],
        "backend",
        name="numpyro",
        version=versions.NUMPYRO,
        lock_sha256=lock_sha,
        worker_sha256=worker_sha,
    )
    exact(request["jax_version"], versions.JAX, "request.jax_version")
    source_sha = lgcp.source_request_digest(request["source_request_sha256"])
    _, config = lgcp.validate_inferred_kernel_request(
        request["source_request"], lock_sha, pymc_inferred_sha, pymc_fixed_sha
    )
    config["numpyro_maximum_tree_depth"] = integer(
        request["maximum_tree_depth"], "maximum tree depth", 10, 14
    )
    config["source_request_sha256"] = source_sha
    return request, config


def seed_for(seed: int, purpose: str) -> int:
    return derive_seed("marklab-numpyro-replicated-multitype-inferred-kernel-v1", seed, purpose)


def make_model(config: dict[str, Any]) -> Any:
    """Inferred-kernel NumPyro model; see ``multitype_lgcp_numpyro.hierarchical_model``.

    ``field_amplitude ~ HalfNormal(amplitude_scale)`` and ``field_length_scale_um ~
    HalfNormal(length_scale)`` parameterize a Matern-3/2 covariance shared by all types; each
    pattern's latent block is ``field_raw @ chol(K + jitter I).T``, centred within the pattern.
    """
    blocks = [
        (selected, jnp.asarray(distance)) for selected, distance in lgcp.pattern_distances(config)
    ]

    def kernel_priors() -> tuple[Any, Any]:
        amplitude = numpyro.sample("field_amplitude", dist.HalfNormal(config["amplitude_scale"]))
        length = numpyro.sample("field_length_scale_um", dist.HalfNormal(config["length_scale"]))
        return amplitude, length

    def latent_field(field_raw: Any, kernel: tuple[Any, Any]) -> Any:
        amplitude, length = kernel
        latent_blocks = []
        for selected, distance in blocks:
            scaled = math.sqrt(3.0) * distance / length
            covariance = amplitude**2 * (1.0 + scaled) * jnp.exp(-scaled)
            covariance += config["kernel_jitter"] * jnp.eye(len(selected))
            block = field_raw[:, selected] @ jnp.linalg.cholesky(covariance).T
            latent_blocks.append(block - block.mean(axis=1, keepdims=True))
        return jnp.concatenate(latent_blocks, axis=1)

    return hierarchical_model(config, kernel_priors, latent_field)


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    model = make_model(config)
    prior_finite = prior_is_finite(
        model, int(config["policy"]["prior_predictive_draws"]), seed_for(config["seed"], "prior")
    )
    if not prior_finite:
        raise ContractError("prior predictive counts are non-finite or outside the Poisson range")
    maximum_tree_depth = config["numpyro_maximum_tree_depth"]
    sampler = MCMC(
        NUTS(model, target_accept_prob=config["target_accept"], max_tree_depth=maximum_tree_depth),
        num_warmup=config["tune"],
        num_samples=config["draws"],
        num_chains=config["chains"],
        chain_method="sequential",
        progress_bar=False,
    )
    sampler.run(
        jax.random.PRNGKey(seed_for(config["seed"], "nuts")),
        extra_fields=("diverging", "num_steps", "energy"),
    )
    samples, flat = posterior_draws(sampler)
    patient_effect, pattern_effect = lgcp.hierarchy_effect_draws(
        config, flat["patient_sd"], flat["patient_raw"], flat["pattern_sd"], flat["pattern_raw"]
    )
    latent = np.asarray(samples["latent_effect"]).reshape(
        (-1, len(config["types"]), len(config["weights"]))
    )
    expected = np.asarray(samples["expected_count"]).reshape((-1, len(config["observed"])))
    diagnostics = nuts_diagnostics(
        samples,
        MONITORED,
        sampler.get_extra_fields(group_by_chain=True),
        maximum_tree_depth,
        mcse=True,
    )
    constraints = bool(lgcp.centering_constraints_hold(config, pattern_effect, latent))
    finite = bool(
        all(
            np.isfinite(value).all() for value in (patient_effect, pattern_effect, latent, expected)
        )
    )
    complete = bool(finite and constraints and convergence_passes(config["policy"], diagnostics))
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": backend_identity("numpyro", numpyro.__version__, lock_sha, worker_sha),
        "jax_version": jax.__version__,
        "input_sha256": config["input_sha"],
        "request_sha256": request_sha,
        "source_request_sha256": config["source_request_sha256"],
        "maximum_tree_depth": maximum_tree_depth,
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"],
            "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"],
            "completed_draws": config["chains"] * config["draws"],
        },
        "kernel_posterior": {
            "field_amplitude": lgcp.summarize(flat["field_amplitude"], config["chains"]),
            "field_length_scale_um": lgcp.summarize(
                flat["field_length_scale_um"], config["chains"]
            ),
        },
        "type_posteriors": lgcp.type_rows(config, flat),
        "group_effect_differences": lgcp.group_effect_differences(config, flat["group_effect"]),
        "patient_type_effects": lgcp.owner_type_effects(
            config["patient_ids"], config["types"], patient_effect, config["chains"]
        ),
        "pattern_type_effects": lgcp.owner_type_effects(
            config["pattern_ids"], config["types"], pattern_effect, config["chains"]
        ),
        "node_type_posteriors": lgcp.node_type_rows(config, latent, expected),
        "diagnostics": {
            "prior_predictive_finite": prior_finite,
            "posterior_finite": finite,
            **diagnostics,
            "constraints_valid": constraints,
            "identifiability_checks_passed": constraints,
        },
    }


def main() -> None:
    require_versions(numpyro=numpyro.__version__, jax=jax.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    worker_directory = Path(__file__).parent
    pymc_inferred_sha = file_sha256(worker_directory / PYMC_INFERRED_WORKER)
    pymc_fixed_sha = file_sha256(worker_directory / PYMC_FIXED_WORKER)
    raw = read_request(64 * MIB)
    _, config = validate(json.loads(raw), lock_sha, worker_sha, pymc_inferred_sha, pymc_fixed_sha)
    write_result(fit(config, sha256_hex(raw), lock_sha, worker_sha))


if __name__ == "__main__":
    run_worker(main, "marklab NumPyro inferred multitype LGCP failed")
