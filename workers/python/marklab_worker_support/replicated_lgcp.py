"""Replicated exact-window patient LGCP shared by the PyMC, NumPyro, and SBC workers.

Each pattern is one exact observation window of one patient, discretized into quadrature nodes
with area weights. The node count is Poisson with mean ``weight * exp(eta)``, where

    eta = intercept + group_effect * group[patient] + covariate_effect * covariate
          + patient_effect[patient] + pattern_effect[pattern] + latent[node].

Patient effects are ``patient_sd * N(0, 1)``; pattern effects are ``pattern_sd`` times standard
normals centred within each patient; the latent field is a Gaussian field over each pattern's
nodes, centred within the pattern. The fixed-kernel model takes the field's block Cholesky
factor from the request; the inferred-kernel model gives each pattern an independent Matérn-3/2
field whose amplitude and length scale have half-normal priors.

This module is NumPy-only so that the NumPyro workers validate requests and run prior checks
without importing PyMC.
"""

from __future__ import annotations

import math
from collections.abc import Callable
from typing import Any

import numpy as np

from . import versions
from .nuts_contract import convergence_passes
from .posterior import summary, two_sided_tail
from .protocol import (
    BACKEND_KEYS,
    ContractError,
    derive_seed,
    exact,
    number,
    obj,
    sha256_digest,
    validate_backend,
)

FIXED_KERNEL_REQUEST_FORMAT = "marklab.pymc_replicated_arbitrary_window_lgcp_request"
INFERRED_KERNEL_REQUEST_FORMAT = (
    "marklab.pymc_replicated_arbitrary_window_lgcp_inferred_kernel_request"
)
FIXED_KERNEL_WORKER = "marklab_pymc_replicated_arbitrary_window_lgcp_worker.py"
INFERRED_KERNEL_WORKER = "marklab_pymc_replicated_arbitrary_window_lgcp_inferred_kernel_worker.py"
INFERRED_KERNEL_SEED_DOMAIN = "marklab-replicated-lgcp-inferred-kernel-v1"
HIERARCHY_PARAMETERS = ("intercept", "group_effect", "covariate_effect", "patient_sd", "pattern_sd")


def _validate_numpyro_envelope(
    request: Any,
    request_format: str,
    lock_digest: str,
    worker_digest: str,
    validate_source: Callable[[Any], dict[str, Any]],
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Validate the common NumPyro agreement envelope and delegate its source request."""
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
    exact(request["format"], request_format, "request.format")
    exact(request["version"], 1, "request.version")
    validate_backend(
        request["backend"],
        "backend",
        name="numpyro",
        version=versions.NUMPYRO,
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )
    exact(request["jax_version"], versions.JAX, "request.jax_version")
    sha256_digest(request["source_request_sha256"], "source request digest")
    maximum_tree_depth = request["maximum_tree_depth"]
    if (
        isinstance(maximum_tree_depth, bool)
        or not isinstance(maximum_tree_depth, int)
        or not 10 <= maximum_tree_depth <= 14
    ):
        raise ContractError("maximum_tree_depth is outside [10, 14]")
    config = validate_source(request["source_request"])
    config["numpyro_maximum_tree_depth"] = maximum_tree_depth
    return request, config


def _fit_is_complete(
    prior_finite: bool,
    posterior_finite: bool,
    policy: dict[str, Any],
    diagnostics: dict[str, Any],
) -> bool:
    """Apply the source request's convergence policy after the shared finiteness gates."""
    return prior_finite and posterior_finite and convergence_passes(policy, diagnostics)


def validate_request(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    """Validate the fixed-kernel PyMC request and return the fit configuration."""
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "input_sha256",
            "reference_group",
            "comparison_group",
            "patients",
            "patterns",
            "nodes",
            "cholesky",
            "priors",
            "sampling",
            "diagnostic_policy",
            "resources",
        },
        "request",
    )
    exact(request["format"], FIXED_KERNEL_REQUEST_FORMAT, "request.format")
    exact(request["version"], 1, "request.version")
    validate_backend(
        request["backend"],
        "backend",
        name="pymc",
        version=versions.PYMC,
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )
    patients = request["patients"]
    patterns = request["patterns"]
    nodes = request["nodes"]
    if not all(isinstance(rows, list) for rows in (patients, patterns, nodes)):
        raise ContractError("patient, pattern, or node rows are invalid")
    patient_ids = [row["patient_id"] for row in patients]
    pattern_ids = [row["pattern_id"] for row in patterns]
    if patient_ids != sorted(set(patient_ids)) or pattern_ids != sorted(set(pattern_ids)):
        raise ContractError("patient or pattern identities are not canonical")
    patient_index = np.asarray([int(row["patient_index"]) for row in nodes], dtype=np.int64)
    pattern_index = np.asarray([int(row["pattern_index"]) for row in nodes], dtype=np.int64)
    covariate = np.asarray([number(row["covariate"], "node.covariate") for row in nodes])
    weight = np.asarray([number(row["weight_um2"], "node.weight") for row in nodes])
    counts = np.asarray([int(row["count"]) for row in nodes], dtype=np.int64)
    if (
        np.any(patient_index < 0)
        or np.any(patient_index >= len(patients))
        or np.any(pattern_index < 0)
        or np.any(pattern_index >= len(patterns))
        or np.any(weight <= 0.0)
        or np.any(counts < 0)
    ):
        raise ContractError("node indices, weights, or counts are invalid")
    group = np.asarray([int(row["group_index"]) for row in patients], dtype=np.float64)
    if set(group.tolist()) != {0.0, 1.0}:
        raise ContractError("both exact groups are required")
    dimension = len(nodes)
    cholesky_values = [number(value, "cholesky[]") for value in request["cholesky"]]
    cholesky = np.asarray(cholesky_values).reshape(dimension, dimension)
    if np.max(np.abs(np.triu(cholesky, 1))) > 1e-14 or np.any(np.diag(cholesky) <= 0.0):
        raise ContractError("physical block Cholesky is invalid")
    priors = obj(
        request["priors"],
        {
            "intercept_mean",
            "intercept_sd",
            "group_effect_sd",
            "covariate_effect_sd",
            "patient_sd_scale",
            "pattern_sd_scale",
            "field_amplitude",
            "field_length_scale_um",
            "jitter",
        },
        "priors",
    )
    sampling = obj(
        request["sampling"],
        {"chains", "tune_per_chain", "draws_per_chain", "target_accept", "seed"},
        "sampling",
    )
    policy = obj(
        request["diagnostic_policy"],
        {
            "prior_predictive_draws",
            "maximum_r_hat",
            "minimum_bulk_ess",
            "minimum_tail_ess",
            "minimum_ebfmi",
            "maximum_divergences",
            "maximum_tree_depth_hits",
            "maximum_tree_depth",
        },
        "diagnostic_policy",
    )
    resources = obj(
        request["resources"],
        {
            "maximum_patients",
            "maximum_patterns",
            "maximum_nodes_per_pattern",
            "maximum_total_nodes",
            "maximum_total_events",
            "maximum_draw_node_work",
            "draw_node_work",
            "timeout_seconds",
        },
        "resources",
    )
    chains = int(sampling["chains"])
    tune = int(sampling["tune_per_chain"])
    draws = int(sampling["draws_per_chain"])
    actual_work = chains * draws * dimension
    if (
        len(patients) > int(resources["maximum_patients"])
        or len(patterns) > int(resources["maximum_patterns"])
        or dimension > int(resources["maximum_total_nodes"])
        or int(counts.sum()) > int(resources["maximum_total_events"])
        or actual_work != int(resources["draw_node_work"])
        or actual_work > int(resources["maximum_draw_node_work"])
    ):
        raise ContractError("replicated LGCP resource identity or ceiling differs")
    return {
        "request": request,
        "patient_ids": patient_ids,
        "pattern_ids": pattern_ids,
        "pattern_patient_index": np.asarray(
            [int(row["patient_index"]) for row in patterns], dtype=np.int64
        ),
        "patient_index": patient_index,
        "pattern_index": pattern_index,
        "group": group,
        "covariate": covariate,
        "weight": weight,
        "counts": counts,
        "cholesky": cholesky,
        "priors": {name: number(value, f"priors.{name}") for name, value in priors.items()},
        "chains": chains,
        "tune": tune,
        "draws": draws,
        "target_accept": number(sampling["target_accept"], "sampling.target_accept"),
        "seed": int(sampling["seed"]),
        "policy": policy,
    }


def validate_inferred_kernel_request(
    request: Any, lock_digest: str, worker_digest: str, source_worker_digest: str
) -> dict[str, Any]:
    """Validate the inferred-kernel PyMC request, which wraps a fixed-kernel source request.

    ``source_worker_digest`` is the digest of the fixed-kernel PyMC worker that the source
    request names; the kernel prior scales must equal the source request's fixed kernel.
    """
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "source_backend",
            "source_request_sha256",
            "source_request",
            "kernel_priors",
            "resources",
        },
        "request",
    )
    exact(request["format"], INFERRED_KERNEL_REQUEST_FORMAT, "request.format")
    exact(request["version"], 1, "request.version")
    validate_backend(
        request["backend"],
        "backend",
        name="pymc",
        version=versions.PYMC,
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )
    source_backend = obj(request["source_backend"], BACKEND_KEYS, "source_backend")
    exact(source_backend, request["source_request"]["backend"], "source backend")
    source_sha = request["source_request_sha256"]
    if (
        not isinstance(source_sha, str)
        or len(source_sha) != 64
        or any(character not in "0123456789abcdef" for character in source_sha)
    ):
        raise ContractError("source request digest is invalid")
    config = validate_request(request["source_request"], lock_digest, source_worker_digest)
    priors = obj(
        request["kernel_priors"],
        {"field_amplitude_scale", "field_length_scale_scale_um", "jitter"},
        "kernel_priors",
    )
    amplitude_scale = number(priors["field_amplitude_scale"], "kernel amplitude scale")
    length_scale = number(priors["field_length_scale_scale_um"], "kernel length scale")
    jitter = number(priors["jitter"], "kernel jitter")
    if min(amplitude_scale, length_scale, jitter) <= 0.0:
        raise ContractError("kernel priors must be positive")
    exact(config["priors"]["field_amplitude"], amplitude_scale, "source amplitude reference")
    exact(
        config["priors"]["field_length_scale_um"],
        length_scale,
        "source length-scale reference",
    )
    exact(config["priors"]["jitter"], jitter, "source jitter")
    resources = obj(
        request["resources"],
        {"maximum_kernel_cube_work", "kernel_cube_work", "maximum_tree_depth", "timeout_seconds"},
        "resources",
    )
    pattern_sizes = np.bincount(config["pattern_index"], minlength=len(config["pattern_ids"]))
    cube_work = int(np.sum(pattern_sizes.astype(object) ** 3))
    exact(int(resources["kernel_cube_work"]), cube_work, "kernel cube work")
    if cube_work > int(resources["maximum_kernel_cube_work"]):
        raise ContractError("kernel cube work exceeds maximum")
    maximum_tree_depth = resources["maximum_tree_depth"]
    if (
        isinstance(maximum_tree_depth, bool)
        or not isinstance(maximum_tree_depth, int)
        or not 10 <= maximum_tree_depth <= 14
    ):
        raise ContractError("maximum tree depth is outside [10, 14]")
    config.update(
        {
            "outer_request": request,
            "field_amplitude_scale": amplitude_scale,
            "field_length_scale_scale_um": length_scale,
            "kernel_jitter": jitter,
            "kernel_cube_work": cube_work,
            "maximum_tree_depth": maximum_tree_depth,
        }
    )
    return config


def center_within_groups(values: np.ndarray, group_index: np.ndarray, groups: int) -> np.ndarray:
    """Subtract each group's mean over the last axis from ``values`` in place; return it."""
    for group in range(groups):
        selected = group_index == group
        values[..., selected] -= values[..., selected].mean(axis=-1, keepdims=True)
    return values


def expected_counts(
    config: dict[str, Any],
    intercept: Any,
    group_effect: Any,
    covariate_effect: Any,
    patient_effect: np.ndarray,
    pattern_effect: np.ndarray,
    latent: np.ndarray,
) -> np.ndarray:
    """Node Poisson means ``weight * exp(eta)``; leading axes of the arguments broadcast."""
    return config["weight"] * np.exp(
        intercept
        + group_effect * config["group"][config["patient_index"]]
        + covariate_effect * config["covariate"]
        + patient_effect[..., config["patient_index"]]
        + pattern_effect[..., config["pattern_index"]]
        + latent
    )


def fixed_kernel_latent(config: dict[str, Any], field_raw: np.ndarray) -> np.ndarray:
    """Pattern-centred latent field ``L @ z`` for ``(chain, draw, node)`` standard normals."""
    latent = np.einsum("ij,cdj->cdi", config["cholesky"], field_raw)
    return center_within_groups(latent, config["pattern_index"], len(config["pattern_ids"]))


def pattern_distances(config: dict[str, Any]) -> list[tuple[np.ndarray, np.ndarray]]:
    """Return each pattern's node indices and pairwise node distances in micrometres."""
    blocks = []
    for pattern in range(len(config["pattern_ids"])):
        selected = np.flatnonzero(config["pattern_index"] == pattern)
        nodes = [config["request"]["nodes"][index] for index in selected]
        coordinates = np.asarray([[row["x_um"], row["y_um"]] for row in nodes])
        distance = np.linalg.norm(coordinates[:, None, :] - coordinates[None, :, :], axis=2)
        blocks.append((selected, distance))
    return blocks


def matern32(distance: Any, amplitude: Any, length: Any, exp: Callable[[Any], Any]) -> Any:
    """Matérn-3/2 covariance ``a^2 (1 + sqrt(3) d / l) exp(-sqrt(3) d / l)``.

    ``exp`` is the array library's exponential so that NumPy, PyTensor, and JAX share one
    expression.
    """
    scaled = math.sqrt(3.0) * distance / length
    return amplitude**2 * (1.0 + scaled) * exp(-scaled)


def matern32_latent_draw(
    config: dict[str, Any],
    blocks: list[tuple[np.ndarray, np.ndarray]],
    amplitude: float,
    length: float,
    rng: np.random.Generator,
) -> np.ndarray:
    """Draw one pattern-centred Matérn-3/2 latent field, one pattern block at a time."""
    latent = np.empty(len(config["counts"]), dtype=np.float64)
    for selected, distance in blocks:
        covariance = matern32(distance, amplitude, length, np.exp)
        covariance += config["kernel_jitter"] * np.eye(len(selected))
        block = np.linalg.cholesky(covariance) @ rng.normal(0.0, 1.0, len(selected))
        latent[selected] = block - block.mean()
    return latent


def prior_is_finite(config: dict[str, Any]) -> bool:
    """Prior predictive check of the fixed-kernel model: every expected count finite and > 0."""
    rng = np.random.default_rng(
        derive_seed("marklab-replicated-lgcp-prior-v1", config["seed"], width=8)
    )
    draws = int(config["policy"]["prior_predictive_draws"])
    priors = config["priors"]
    intercept = rng.normal(priors["intercept_mean"], priors["intercept_sd"], draws)
    group_effect = rng.normal(0.0, priors["group_effect_sd"], draws)
    covariate_effect = rng.normal(0.0, priors["covariate_effect_sd"], draws)
    patient_sd = np.abs(rng.normal(0.0, priors["patient_sd_scale"], draws))
    pattern_sd = np.abs(rng.normal(0.0, priors["pattern_sd_scale"], draws))
    patient_raw = rng.normal(0.0, 1.0, (draws, len(config["patient_ids"])))
    patient_effect = patient_sd[:, None] * patient_raw
    pattern_raw = rng.normal(0.0, 1.0, (draws, len(config["pattern_ids"])))
    center_within_groups(pattern_raw, config["pattern_patient_index"], len(config["patient_ids"]))
    pattern_effect = pattern_sd[:, None] * pattern_raw
    latent = rng.normal(0.0, 1.0, (draws, len(config["counts"]))) @ config["cholesky"].T
    center_within_groups(latent, config["pattern_index"], len(config["pattern_ids"]))
    with np.errstate(over="ignore", invalid="ignore"):
        expected = expected_counts(
            config,
            intercept[:, None],
            group_effect[:, None],
            covariate_effect[:, None],
            patient_effect,
            pattern_effect,
            latent,
        )
    return bool(np.isfinite(expected).all() and np.all(expected > 0.0))


def inferred_kernel_prior_is_finite(config: dict[str, Any]) -> bool:
    """Prior check of the inferred-kernel model: positive kernel draws and a finite field."""
    rng = np.random.default_rng(derive_seed(INFERRED_KERNEL_SEED_DOMAIN, config["seed"], "prior"))
    blocks = pattern_distances(config)
    for _ in range(int(config["policy"]["prior_predictive_draws"])):
        amplitude = abs(rng.normal(0.0, config["field_amplitude_scale"]))
        length = abs(rng.normal(0.0, config["field_length_scale_scale_um"]))
        if amplitude <= 0.0 or length <= 0.0:
            return False
        latent = matern32_latent_draw(config, blocks, amplitude, length, rng)
        if not np.isfinite(latent).all():
            return False
    return True


def pattern_posterior_predictive(
    config: dict[str, Any], replicated_counts: np.ndarray
) -> list[dict[str, Any]]:
    """Per-pattern posterior predictive checks of the total count and the node-count variance.

    ``replicated_counts`` has one row of replicated node counts per posterior draw.
    """
    checks = []
    for index, pattern_id in enumerate(config["pattern_ids"]):
        selected = config["pattern_index"] == index
        observed_nodes = config["counts"][selected]
        replicated_nodes = replicated_counts[:, selected]
        observed_total = int(observed_nodes.sum())
        replicated_totals = replicated_nodes.sum(axis=1)
        observed_variance = float(np.var(observed_nodes))
        replicated_variances = np.var(replicated_nodes, axis=1)
        checks.append(
            {
                "pattern_id": pattern_id,
                "observed_total_count": observed_total,
                "replicate_count": int(replicated_totals.size),
                "replicated_total_count_mean": float(replicated_totals.mean()),
                "replicated_total_count_sd": float(replicated_totals.std(ddof=1)),
                "replicated_total_count_interval_lower": float(
                    np.quantile(replicated_totals, 0.025)
                ),
                "replicated_total_count_interval_upper": float(
                    np.quantile(replicated_totals, 0.975)
                ),
                "total_count_two_sided_tail_probability": two_sided_tail(
                    replicated_totals, observed_total
                ),
                "observed_node_count_variance": observed_variance,
                "replicated_node_count_variance_mean": float(replicated_variances.mean()),
                "node_variance_two_sided_tail_probability": two_sided_tail(
                    replicated_variances, observed_variance
                ),
            }
        )
    return checks


def effect_summaries(
    config: dict[str, Any],
    patient_effect: np.ndarray,
    pattern_effect: np.ndarray,
    latent: np.ndarray,
    expected: np.ndarray,
) -> dict[str, list[dict[str, Any]]]:
    """Summaries of patient, pattern, and node quantities; the last axis indexes the unit."""
    nodes = config["request"]["nodes"]
    chains = config["chains"]
    return {
        "patient_effects": [
            {
                "patient_id": patient_id,
                "effect": summary(patient_effect[..., index], dtype=np.float64, chains=chains),
            }
            for index, patient_id in enumerate(config["patient_ids"])
        ],
        "pattern_effects": [
            {
                "pattern_id": pattern_id,
                "effect": summary(pattern_effect[..., index], dtype=np.float64, chains=chains),
            }
            for index, pattern_id in enumerate(config["pattern_ids"])
        ],
        "nodes": [
            {
                "pattern_id": nodes[index]["pattern_id"],
                "node_id": nodes[index]["node_id"],
                "latent_effect": summary(latent[..., index], dtype=np.float64, chains=chains),
                "expected_count": summary(expected[..., index], dtype=np.float64, chains=chains),
            }
            for index in range(len(config["counts"]))
        ],
    }
