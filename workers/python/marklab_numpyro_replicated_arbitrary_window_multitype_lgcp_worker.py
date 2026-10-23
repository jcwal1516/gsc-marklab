#!/usr/bin/env python3
"""Independent NumPyro fit for replicated exact-window multitype LGCPs."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

sys.path.append(str(Path(__file__).resolve().parent))

import jax

jax.config.update("jax_enable_x64", True)

import jax.numpy as jnp
import numpyro

from marklab_worker_support import multitype_lgcp as lgcp
from marklab_worker_support import versions
from marklab_worker_support.multitype_lgcp_numpyro import (
    FIXED_KERNEL_SEED_DOMAIN,
    diagnostic_report,
    fit_disposition,
    hierarchical_model,
    prior_is_finite,
    result_header,
    sample_posterior,
)
from marklab_worker_support.numpyro_nuts import nuts_diagnostics
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
    derive_seed,
    exact,
    file_sha256,
    integer,
    obj,
    read_request,
    require_versions,
    run_worker,
    sha256_digest,
    sha256_hex,
    validate_backend,
    worker_digests,
    write_result,
)

REQUEST_FORMAT = "marklab.numpyro_replicated_arbitrary_window_multitype_lgcp_request"
RESULT_FORMAT = "marklab.numpyro_replicated_arbitrary_window_multitype_lgcp_result"
PYMC_SOURCE_WORKER = "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_worker.py"
MONITORED = [
    "intercept",
    "group_effect",
    "covariate_effect",
    "patient_sd",
    "pattern_sd",
    "patient_raw",
    "pattern_raw",
    "field_raw",
]


def validate(
    request: Any, lock_sha: str, worker_sha: str, pymc_worker_sha: str
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
    sha256_digest(request["source_request_sha256"], "source request digest")
    depth = integer(request["maximum_tree_depth"], "maximum tree depth", 10, 14)
    config = lgcp.validate_fixed_kernel_request(
        request["source_request"], lock_sha, pymc_worker_sha
    )
    config["numpyro_maximum_tree_depth"] = depth
    return request, config


def seed_for(seed: int, purpose: str) -> int:
    return derive_seed(FIXED_KERNEL_SEED_DOMAIN, seed, purpose)


def make_model(config: dict[str, Any]) -> Any:
    """Fixed-kernel NumPyro model; see ``multitype_lgcp_numpyro.hierarchical_model``.

    The latent field is ``field_raw @ L.T`` for the request's fixed block Cholesky factor ``L``,
    centred within each pattern.
    """
    patterns = len(config["pattern_ids"])
    node_patterns = jnp.asarray(config["node_patterns"])
    cholesky = jnp.asarray(config["cholesky"])

    def pattern_sums(values: Any) -> Any:
        return jnp.zeros(patterns).at[node_patterns].add(values)

    def latent_field(field_raw: Any, _kernel: Any) -> Any:
        latent_uncentered = field_raw @ cholesky.T
        latent_sums = jax.vmap(pattern_sums)(latent_uncentered)
        node_numbers = jnp.zeros(patterns).at[node_patterns].add(1.0)
        pattern_means = latent_sums[:, node_patterns] / node_numbers[node_patterns][None, :]
        return latent_uncentered - pattern_means

    return hierarchical_model(config, lambda: None, latent_field)


def fit(
    config: dict[str, Any], request_sha: str, source_sha: str, lock_sha: str, worker_sha: str
) -> dict[str, Any]:
    model = make_model(config)
    prior_finite = prior_is_finite(
        model, int(config["policy"]["prior_predictive_draws"]), seed_for(config["seed"], "prior")
    )
    if not prior_finite:
        raise ContractError("prior predictive counts are non-finite or outside the Poisson range")
    maximum_tree_depth = config["numpyro_maximum_tree_depth"]
    samples, flat, extra = sample_posterior(
        model, config, seed_for(config["seed"], "nuts")
    )
    patient_effect, pattern_effect = lgcp.hierarchy_effect_draws(
        config, flat["patient_sd"], flat["patient_raw"], flat["pattern_sd"], flat["pattern_raw"]
    )
    latent = lgcp.fixed_kernel_latent_draws(config, flat["field_raw"])
    expected = lgcp.expected_count_draws(
        config,
        flat["intercept"],
        flat["group_effect"],
        flat["covariate_effect"],
        patient_effect,
        pattern_effect,
        latent,
    )
    diagnostics = nuts_diagnostics(
        samples,
        MONITORED,
        extra,
        maximum_tree_depth,
        mcse=True,
    )
    constraints = bool(lgcp.centering_constraints_hold(config, pattern_effect, latent))
    finite, complete = fit_disposition(
        config, diagnostics, constraints, (patient_effect, pattern_effect, latent, expected)
    )
    return {
        **result_header(
            RESULT_FORMAT,
            config,
            request_sha,
            source_sha,
            lock_sha,
            worker_sha,
            complete,
            report_maximum_tree_depth=True,
        ),
        "type_posteriors": lgcp.type_rows(config, flat),
        "group_effect_differences": lgcp.group_effect_differences(config, flat["group_effect"]),
        "patient_type_effects": lgcp.owner_type_effects(
            config["patient_ids"], config["types"], patient_effect, config["chains"]
        ),
        "pattern_type_effects": lgcp.owner_type_effects(
            config["pattern_ids"], config["types"], pattern_effect, config["chains"]
        ),
        "node_type_posteriors": lgcp.node_type_rows(config, latent, expected),
        "diagnostics": diagnostic_report(prior_finite, finite, diagnostics, constraints),
    }


def main() -> None:
    require_versions(numpyro=numpyro.__version__, jax=jax.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    pymc_worker_sha = file_sha256(Path(__file__).with_name(PYMC_SOURCE_WORKER))
    raw = read_request(64 * MIB)
    request, config = validate(json.loads(raw), lock_sha, worker_sha, pymc_worker_sha)
    result = fit(config, sha256_hex(raw), request["source_request_sha256"], lock_sha, worker_sha)
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "marklab NumPyro replicated multitype LGCP failed")
