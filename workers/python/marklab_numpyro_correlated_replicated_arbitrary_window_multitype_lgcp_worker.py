#!/usr/bin/env python3
"""NumPyro coregionalized replicated multitype LGCP on exact quadrature windows."""

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

from marklab_worker_support import multitype_lgcp as lgcp
from marklab_worker_support import versions
from marklab_worker_support.multitype_lgcp_numpyro import (
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
    sha256_hex,
    validate_backend,
    worker_digests,
    write_result,
)

REQUEST_FORMAT = "marklab.numpyro_correlated_replicated_arbitrary_window_multitype_lgcp_request"
RESULT_FORMAT = "marklab.numpyro_correlated_replicated_arbitrary_window_multitype_lgcp_result"
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
    "field_scale",
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
            "correlation_prior",
            "resources",
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
    prior = obj(
        request["correlation_prior"],
        {"lkj_concentration", "marginal_field_scale"},
        "correlation_prior",
    )
    # As in version one of this contract, booleans pass the numeric type test here.
    concentration = prior["lkj_concentration"]
    field_scale = prior["marginal_field_scale"]
    if (
        not isinstance(concentration, (int, float))
        or not math.isfinite(concentration)
        or concentration < 1.0
        or not isinstance(field_scale, (int, float))
        or not math.isfinite(field_scale)
        or field_scale <= 0.0
    ):
        raise ContractError("correlation prior is invalid")
    resources = obj(
        request["resources"],
        {
            "coregionalization_work",
            "maximum_coregionalization_work",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    for name, value in resources.items():
        if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
            raise ContractError(f"resources.{name} must be a positive integer")
    if resources["coregionalization_work"] > resources["maximum_coregionalization_work"]:
        raise ContractError("coregionalization work exceeds its maximum")
    depth = integer(request["maximum_tree_depth"], "maximum tree depth", 10, 14)
    if not 2 <= len(config["types"]) <= 8:
        raise ContractError("coregionalization requires two through eight types")
    config["correlation_prior"] = prior
    config["correlation_resources"] = resources
    config["numpyro_maximum_tree_depth"] = depth
    config["source_request_sha256"] = source_sha
    return request, config


def seed_for(seed: int, purpose: str) -> int:
    return derive_seed("marklab-numpyro-correlated-multitype-lgcp-v1", seed, purpose)


def make_model(config: dict[str, Any]) -> Any:
    """Coregionalized NumPyro model; see ``multitype_lgcp_numpyro.hierarchical_model``.

    Kernel priors: per-type ``field_scale ~ HalfNormal(marginal_field_scale)``, a cross-type
    correlation Cholesky factor ``C ~ LKJCholesky(types, lkj_concentration)``, and a shared
    ``field_length_scale_um ~ HalfNormal(length_scale)``. Each pattern's latent block is
    ``field_scale * (C @ Z)``, where ``Z = field_raw @ chol(K + jitter I).T`` is centred within
    the pattern and ``K`` is a unit-variance Matern-3/2 kernel.
    """
    types = len(config["types"])
    correlation_prior = config["correlation_prior"]
    blocks = [
        (selected, jnp.asarray(distance)) for selected, distance in lgcp.pattern_distances(config)
    ]

    def kernel_priors() -> tuple[Any, Any, Any]:
        field_scale = numpyro.sample(
            "field_scale",
            dist.HalfNormal(correlation_prior["marginal_field_scale"]).expand([types]).to_event(1),
        )
        correlation_cholesky = numpyro.sample(
            "field_cholesky_correlation",
            dist.LKJCholesky(types, concentration=correlation_prior["lkj_concentration"]),
        )
        numpyro.deterministic("correlation_matrix", correlation_cholesky @ correlation_cholesky.T)
        length = numpyro.sample("field_length_scale_um", dist.HalfNormal(config["length_scale"]))
        return field_scale, correlation_cholesky, length

    def latent_field(field_raw: Any, kernel: tuple[Any, Any, Any]) -> Any:
        field_scale, correlation_cholesky, length = kernel
        latent_blocks = []
        for selected, distance in blocks:
            scaled = math.sqrt(3.0) * distance / length
            covariance = (1.0 + scaled) * jnp.exp(-scaled)
            covariance += config["kernel_jitter"] * jnp.eye(len(selected))
            independent = field_raw[:, selected] @ jnp.linalg.cholesky(covariance).T
            independent -= independent.mean(axis=1, keepdims=True)
            latent_blocks.append(field_scale[:, None] * (correlation_cholesky @ independent))
        return jnp.concatenate(latent_blocks, axis=1)

    return hierarchical_model(config, kernel_priors, latent_field)


def coregionalization_oracle() -> dict[str, Any]:
    """Check that a known lower-triangular factor reconstructs its correlation matrix."""
    target = np.asarray(
        [[1.0, 0.5, -0.25], [0.5, 1.0, 0.0], [-0.25, 0.0, 1.0]],
        dtype=np.float64,
    )
    lower = np.asarray(
        [
            [1.0, 0.0, 0.0],
            [0.5, math.sqrt(0.75), 0.0],
            [-0.25, 0.125 / math.sqrt(0.75), math.sqrt(11.0 / 12.0)],
        ],
        dtype=np.float64,
    )
    reconstructed = lower @ lower.T
    return {
        "target_correlation_matrix": target.reshape(-1).tolist(),
        "reconstructed_correlation_matrix": reconstructed.reshape(-1).tolist(),
        "maximum_absolute_error": float(np.max(np.abs(target - reconstructed))),
    }


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    types = config["types"]
    model = make_model(config)
    prior_finite = prior_is_finite(
        model, int(config["policy"]["prior_predictive_draws"]), seed_for(config["seed"], "prior")
    )
    if not prior_finite:
        raise ContractError("prior predictive counts are non-finite or outside the Poisson range")
    maximum_tree_depth = config["numpyro_maximum_tree_depth"]
    samples, flat, extra = sample_posterior(model, config, seed_for(config["seed"], "nuts"))
    patient_effect, pattern_effect = lgcp.hierarchy_effect_draws(
        config, flat["patient_sd"], flat["patient_raw"], flat["pattern_sd"], flat["pattern_raw"]
    )
    latent = np.asarray(samples["latent_effect"]).reshape((-1, len(types), len(config["weights"])))
    expected = np.asarray(samples["expected_count"]).reshape((-1, len(config["observed"])))
    correlations = np.asarray(samples["correlation_matrix"]).reshape((-1, len(types), len(types)))
    type_pairs = [
        (left, right) for left in range(len(types)) for right in range(left + 1, len(types))
    ]
    correlation_rows = [
        {
            "type_a": types[left],
            "type_b": types[right],
            "correlation": lgcp.summarize(correlations[:, left, right], config["chains"]),
        }
        for left, right in type_pairs
    ]
    predictive_rng = np.random.default_rng(seed_for(config["seed"], "posterior_predictive"))
    replicated = predictive_rng.poisson(expected)
    off_diagonal = np.asarray([correlations[:, left, right] for left, right in type_pairs])
    diagnostic_samples = {name: samples[name] for name in MONITORED}
    diagnostic_samples["correlation_off_diagonal"] = off_diagonal.T.reshape(
        (config["chains"], config["draws"], -1)
    )
    diagnostics = nuts_diagnostics(
        diagnostic_samples,
        list(diagnostic_samples),
        extra,
        maximum_tree_depth,
        mcse=True,
    )
    correlations_valid = np.all(
        np.diagonal(correlations, axis1=1, axis2=2) > 0.999999999
    ) and np.all(np.linalg.eigvalsh(correlations) > 0.0)
    constraints = bool(
        lgcp.centering_constraints_hold(config, pattern_effect, latent) and correlations_valid
    )
    finite, complete = fit_disposition(
        config,
        diagnostics,
        constraints,
        (patient_effect, pattern_effect, latent, expected, correlations, replicated),
    )
    return {
        **result_header(
            RESULT_FORMAT,
            config,
            request_sha,
            config["source_request_sha256"],
            lock_sha,
            worker_sha,
            complete,
            report_maximum_tree_depth=False,
        ),
        "field_length_scale_um": lgcp.summarize(flat["field_length_scale_um"], config["chains"]),
        "type_field_scales": [
            {
                "type_id": identity,
                "scale": lgcp.summarize(flat["field_scale"][:, index], config["chains"]),
            }
            for index, identity in enumerate(types)
        ],
        "cross_type_correlations": correlation_rows,
        "posterior_mean_correlation_matrix": correlations.mean(axis=0).reshape(-1).tolist(),
        "type_posteriors": lgcp.type_rows(config, flat),
        "group_effect_differences": lgcp.group_effect_differences(config, flat["group_effect"]),
        "pattern_type_posterior_predictive": lgcp.pattern_type_predictive(config, replicated),
        "diagnostics": diagnostic_report(prior_finite, finite, diagnostics, constraints),
        "coregionalization_oracle": coregionalization_oracle(),
        "coregionalization_work": config["correlation_resources"]["coregionalization_work"],
    }


def main() -> None:
    require_versions(numpyro=numpyro.__version__, jax=jax.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    worker_directory = Path(__file__).parent
    pymc_inferred_sha = file_sha256(worker_directory / PYMC_INFERRED_WORKER)
    pymc_fixed_sha = file_sha256(worker_directory / PYMC_FIXED_WORKER)
    raw = read_request(64 * MIB)
    _, config = validate(json.loads(raw), lock_sha, worker_sha, pymc_inferred_sha, pymc_fixed_sha)
    result = fit(config, sha256_hex(raw), lock_sha, worker_sha)
    write_result(result, max_bytes=config["correlation_resources"]["maximum_output_bytes"])


if __name__ == "__main__":
    run_worker(main, "marklab correlated NumPyro multitype LGCP failed")
