"""Contracts and NumPy post-processing shared by the replicated multitype LGCP workers.

Every backend fits the same patient-replicated log-Gaussian Cox process on exact quadrature
windows. For type ``t`` at quadrature node ``n`` of pattern ``s`` belonging to patient ``p``::

    log mu[t, n] = intercept[t] + group_effect[t] * group[p] + covariate_effect[t] * x[n]
                   + patient_effect[p, t] + pattern_effect[s, t] + latent[t, n] + log w[n]
    count[n, t] ~ Poisson(mu[t, n])

Pattern effects are centred within each patient and latent fields within each pattern, which
identifies the intercepts. The PyMC fixed-kernel request is the source contract
(``validate_fixed_kernel_request``); the PyMC inferred-kernel request wraps it
(``validate_inferred_kernel_request``). The NumPyro, calibration, and SBC workers validate their
embedded source request with these functions and the sibling worker digests.
"""

from __future__ import annotations

import math
from typing import Any

import numpy as np

from . import versions
from .posterior import summary, two_sided_tail
from .protocol import ContractError, exact, integer, number, obj, validate_backend

FIXED_KERNEL_REQUEST_FORMAT = "marklab.pymc_replicated_arbitrary_window_multitype_lgcp_request"
INFERRED_KERNEL_REQUEST_FORMAT = (
    "marklab.pymc_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_request"
)


def identities(value: Any, path: str, low: int, high: int) -> list[str]:
    """Require ``low..high`` distinct non-empty identity strings of at most 256 characters."""
    if (
        not isinstance(value, list)
        or not low <= len(value) <= high
        or any(not isinstance(item, str) or not item or len(item) > 256 for item in value)
        or len(set(value)) != len(value)
    ):
        raise ContractError(f"{path} identities are invalid")
    return value


def source_request_digest(value: Any) -> str:
    """Require a 64-character source request digest (the alphabet is not checked)."""
    if not isinstance(value, str) or len(value) != 64:
        raise ContractError("source request digest is invalid")
    return value


def validate_fixed_kernel_request(request: Any, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    """Validate the PyMC fixed-kernel request and return the fitting configuration."""
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "input_sha256",
            "reference_group",
            "comparison_group",
            "type_ids",
            "reference_type",
            "patients",
            "patterns",
            "nodes",
            "node_type_counts",
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
        lock_sha256=lock_sha,
        worker_sha256=worker_sha,
    )
    input_sha = request["input_sha256"]
    if not isinstance(input_sha, str) or len(input_sha) != 64:
        raise ContractError("input digest is invalid")
    types = identities(request["type_ids"], "type_ids", 3, 8)
    exact(request["reference_type"], types[0], "reference_type")
    patients = request["patients"]
    patterns = request["patterns"]
    nodes = request["nodes"]
    counts = request["node_type_counts"]
    if not all(isinstance(rows, list) for rows in (patients, patterns, nodes, counts)):
        raise ContractError("hierarchy rows are invalid")
    patient_ids = [row["patient_id"] for row in patients]
    pattern_ids = [row["pattern_id"] for row in patterns]
    if patient_ids != sorted(set(patient_ids)) or pattern_ids != sorted(set(pattern_ids)):
        raise ContractError("patient or pattern identities are not canonical")
    last_patient = len(patients) - 1
    last_pattern = len(patterns) - 1
    patient_groups = np.asarray(
        [integer(row["group_index"], "patient.group_index", 0, 1) for row in patients],
        dtype=np.float64,
    )
    if set(patient_groups.tolist()) != {0.0, 1.0}:
        raise ContractError("both groups are required")
    pattern_patients = np.asarray(
        [
            integer(row["patient_index"], "pattern.patient_index", 0, last_patient)
            for row in patterns
        ],
        dtype=np.int64,
    )
    node_patterns = np.asarray(
        [integer(row["pattern_index"], "node.pattern_index", 0, last_pattern) for row in nodes],
        dtype=np.int64,
    )
    node_patients = np.asarray(
        [integer(row["patient_index"], "node.patient_index", 0, last_patient) for row in nodes],
        dtype=np.int64,
    )
    if np.any(node_patients != pattern_patients[node_patterns]):
        raise ContractError("node hierarchy differs")
    weights = np.asarray([number(row["weight_um2"], "node.weight") for row in nodes])
    covariate = np.asarray([number(row["covariate"], "node.covariate") for row in nodes])
    if np.any(weights <= 0.0):
        raise ContractError("node weights must be positive")
    if len(counts) != len(nodes) * len(types):
        raise ContractError("node-type table is incomplete")
    node_index = np.empty(len(counts), dtype=np.int64)
    type_index = np.empty(len(counts), dtype=np.int64)
    observed = np.empty(len(counts), dtype=np.int64)
    for index, row in enumerate(counts):
        row = obj(row, {"node_index", "type_index", "count"}, f"counts[{index}]")
        node_index[index] = integer(row["node_index"], "count.node_index", 0, len(nodes) - 1)
        type_index[index] = integer(row["type_index"], "count.type_index", 0, len(types) - 1)
        observed[index] = integer(row["count"], "count", 0, 2**63 - 1)
        exact(node_index[index], index // len(types), "count node order")
        exact(type_index[index], index % len(types), "count type order")
    for pattern_index, pattern in enumerate(patterns):
        selected_nodes = node_patterns == pattern_index
        if int(selected_nodes.sum()) != int(pattern["node_count"]):
            raise ContractError("pattern node count differs")
        selected_rows = selected_nodes[node_index]
        if int(observed[selected_rows].sum()) != int(pattern["event_count"]):
            raise ContractError("pattern event count differs")
        weight_total = float(weights[selected_nodes].sum())
        area = number(pattern["window_area_um2"], "pattern.area")
        tolerance = 1e-10 * max(1.0, abs(float(pattern["window_area_um2"])))
        if abs(weight_total - area) > tolerance:
            raise ContractError("pattern weights do not conserve area")
    dimension = len(nodes)
    cholesky = np.asarray(
        [number(value, "cholesky[]") for value in request["cholesky"]], dtype=np.float64
    )
    if cholesky.size != dimension * dimension:
        raise ContractError("Cholesky dimensions differ")
    cholesky = cholesky.reshape(dimension, dimension)
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
    prior = {name: number(value, f"priors.{name}") for name, value in priors.items()}
    if min(value for name, value in prior.items() if name != "intercept_mean") <= 0.0:
        raise ContractError("prior scales must be positive")
    sampling = obj(
        request["sampling"],
        {"chains", "tune_per_chain", "draws_per_chain", "target_accept", "seed"},
        "sampling",
    )
    chains = integer(sampling["chains"], "sampling.chains", 2, 8)
    tune = integer(sampling["tune_per_chain"], "sampling.tune", 100, 100_000)
    draws = integer(sampling["draws_per_chain"], "sampling.draws", 100, 100_000)
    target_accept = number(sampling["target_accept"], "sampling.target_accept")
    if not 0.5 <= target_accept < 1.0:
        raise ContractError("target acceptance is invalid")
    seed = integer(sampling["seed"], "sampling.seed", 0, 2**64 - 1)
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
            "maximum_types",
            "maximum_nodes_per_pattern",
            "maximum_total_nodes",
            "maximum_total_node_type_rows",
            "maximum_total_events",
            "maximum_draw_node_type_work",
            "draw_node_type_work",
            "timeout_seconds",
        },
        "resources",
    )
    work = chains * draws * len(counts)
    exact(resources["draw_node_type_work"], work, "resources.draw_work")
    maximum_nodes_per_pattern = integer(
        resources["maximum_nodes_per_pattern"], "resources.nodes_per_pattern", 4, 36
    )
    # The ceilings are validated lazily, in this order, as the original contract did.
    if (
        len(patients) > integer(resources["maximum_patients"], "resources.patients", 8, 32)
        or len(patterns) > integer(resources["maximum_patterns"], "resources.patterns", 16, 64)
        or len(types) > integer(resources["maximum_types"], "resources.types", 3, 8)
        or any(int(pattern["node_count"]) > maximum_nodes_per_pattern for pattern in patterns)
        or len(nodes) > integer(resources["maximum_total_nodes"], "resources.nodes", 64, 512)
        or len(counts)
        > integer(resources["maximum_total_node_type_rows"], "resources.rows", 1, 4096)
        or int(observed.sum())
        > integer(resources["maximum_total_events"], "resources.events", 1, 2**63 - 1)
        or work > integer(resources["maximum_draw_node_type_work"], "resources.work", 1, 2**63 - 1)
    ):
        raise ContractError("resource ceiling exceeded")
    return {
        "request": request,
        "input_sha": input_sha,
        "types": types,
        "patient_ids": patient_ids,
        "pattern_ids": pattern_ids,
        "patient_groups": patient_groups,
        "pattern_patients": pattern_patients,
        "node_patterns": node_patterns,
        "node_patients": node_patients,
        "weights": weights,
        "covariate": covariate,
        "node_index": node_index,
        "type_index": type_index,
        "observed": observed,
        "cholesky": cholesky,
        "priors": prior,
        "chains": chains,
        "tune": tune,
        "draws": draws,
        "target_accept": target_accept,
        "seed": seed,
        "policy": policy,
    }


def positive(value: Any, path: str) -> float:
    """Require a finite, strictly positive JSON number (not a boolean)."""
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ContractError(f"{path} must be numeric")
    result = float(value)
    if not math.isfinite(result) or result <= 0.0:
        raise ContractError(f"{path} must be finite and positive")
    return result


def validate_inferred_kernel_request(
    request: Any, lock_sha: str, worker_sha: str, source_worker_sha: str
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Validate the PyMC inferred-kernel request and its embedded fixed-kernel source request.

    ``worker_sha`` identifies the PyMC inferred-kernel worker and ``source_worker_sha`` the PyMC
    fixed-kernel worker. The fixed-kernel field amplitude and length scale become the half-normal
    prior scales of the inferred kernel.
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
        lock_sha256=lock_sha,
        worker_sha256=worker_sha,
    )
    source_backend = obj(
        request["source_backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "source_backend",
    )
    exact(source_backend, request["source_request"]["backend"], "source backend")
    source_sha = source_request_digest(request["source_request_sha256"])
    config = validate_fixed_kernel_request(request["source_request"], lock_sha, source_worker_sha)
    priors = obj(
        request["kernel_priors"],
        {"field_amplitude_scale", "field_length_scale_scale_um", "jitter"},
        "kernel_priors",
    )
    amplitude = positive(priors["field_amplitude_scale"], "amplitude scale")
    length = positive(priors["field_length_scale_scale_um"], "length scale")
    jitter = positive(priors["jitter"], "jitter")
    exact(config["priors"]["field_amplitude"], amplitude, "source amplitude reference")
    exact(config["priors"]["field_length_scale_um"], length, "source length reference")
    exact(config["priors"]["jitter"], jitter, "source jitter")
    resources = obj(
        request["resources"],
        {"maximum_kernel_cube_work", "kernel_cube_work", "maximum_tree_depth", "timeout_seconds"},
        "resources",
    )
    sizes = np.bincount(config["node_patterns"], minlength=len(config["pattern_ids"]))
    work = int(np.sum(sizes.astype(object) ** 3))
    exact(resources["kernel_cube_work"], work, "kernel cube work")
    if work > int(resources["maximum_kernel_cube_work"]):
        raise ContractError("kernel cube work exceeds maximum")
    depth = resources["maximum_tree_depth"]
    if isinstance(depth, bool) or not isinstance(depth, int) or not 10 <= depth <= 14:
        raise ContractError("maximum tree depth is outside [10, 14]")
    config.update(
        {
            "outer_request": request,
            "source_request_sha256": source_sha,
            "amplitude_scale": amplitude,
            "length_scale": length,
            "kernel_jitter": jitter,
            "kernel_cube_work": work,
            "maximum_tree_depth": depth,
        }
    )
    return request, config


def pattern_distances(config: dict[str, Any]) -> list[tuple[np.ndarray, np.ndarray]]:
    """Return, per pattern, its node indices and their Euclidean distance matrix (um)."""
    blocks = []
    for pattern in range(len(config["pattern_ids"])):
        selected = np.flatnonzero(config["node_patterns"] == pattern)
        rows = [config["request"]["nodes"][index] for index in selected]
        coordinates = np.asarray([[row["x_um"], row["y_um"]] for row in rows])
        distance = np.linalg.norm(coordinates[:, None, :] - coordinates[None, :, :], axis=2)
        blocks.append((selected, distance))
    return blocks


def hierarchy_effect_draws(
    config: dict[str, Any],
    patient_sd: np.ndarray,
    patient_raw: np.ndarray,
    pattern_sd: np.ndarray,
    pattern_raw: np.ndarray,
) -> tuple[np.ndarray, np.ndarray]:
    """Patient and pattern effects for draw-major arrays, centring patterns within patients."""
    patient_effect = patient_sd[:, None, :] * patient_raw
    pattern_centered = pattern_raw.copy()
    for patient in range(len(config["patient_ids"])):
        selected = config["pattern_patients"] == patient
        pattern_centered[:, selected, :] -= pattern_centered[:, selected, :].mean(
            axis=1, keepdims=True
        )
    pattern_effect = pattern_sd[:, None, :] * pattern_centered
    return patient_effect, pattern_effect


def fixed_kernel_latent_draws(config: dict[str, Any], field_raw: np.ndarray) -> np.ndarray:
    """Latent fields ``field_raw @ L.T`` for draw-major arrays, centred within each pattern."""
    latent = np.einsum("dkn,mn->dkm", field_raw, config["cholesky"])
    for pattern in range(len(config["pattern_ids"])):
        selected = config["node_patterns"] == pattern
        latent[:, :, selected] -= latent[:, :, selected].mean(axis=2, keepdims=True)
    return latent


def expected_count_draws(
    config: dict[str, Any],
    intercept: np.ndarray,
    group_effect: np.ndarray,
    covariate_effect: np.ndarray,
    patient_effect: np.ndarray,
    pattern_effect: np.ndarray,
    latent: np.ndarray,
) -> np.ndarray:
    """Expected counts in node-type row order for draw-major parameter arrays."""
    node_patients = config["node_patients"]
    node_patterns = config["node_patterns"]
    log_expected = (
        intercept[:, :, None]
        + group_effect[:, :, None] * config["patient_groups"][node_patients][None, None, :]
        + covariate_effect[:, :, None] * config["covariate"][None, None, :]
        + np.transpose(patient_effect[:, node_patients, :], (0, 2, 1))
        + np.transpose(pattern_effect[:, node_patterns, :], (0, 2, 1))
        + latent
        + np.log(config["weights"])[None, None, :]
    )
    expected_matrix = np.exp(log_expected)
    return expected_matrix[:, config["type_index"], config["node_index"]]


def summarize(values: Any, chains: int) -> dict[str, float]:
    """Float64 posterior summary with MCSE of chain-major draws from ``chains`` chains."""
    return summary(values, dtype=np.float64, chains=chains)


def type_rows(config: dict[str, Any], flat: dict[str, np.ndarray]) -> list[dict[str, Any]]:
    """Per-type posterior summaries of the fixed effects and hierarchy scales."""
    return [
        {
            "type_id": identity,
            "intercept": summarize(flat["intercept"][:, index], config["chains"]),
            "group_effect": summarize(flat["group_effect"][:, index], config["chains"]),
            "covariate_effect": summarize(flat["covariate_effect"][:, index], config["chains"]),
            "patient_sd": summarize(flat["patient_sd"][:, index], config["chains"]),
            "pattern_sd": summarize(flat["pattern_sd"][:, index], config["chains"]),
        }
        for index, identity in enumerate(config["types"])
    ]


def group_effect_differences(
    config: dict[str, Any], group_effect: np.ndarray
) -> list[dict[str, Any]]:
    """Posterior summaries of pairwise group-effect differences between types."""
    types = config["types"]
    return [
        {
            "type_a": types[left],
            "type_b": types[right],
            "difference": summarize(
                group_effect[:, left] - group_effect[:, right], config["chains"]
            ),
        }
        for left in range(len(types))
        for right in range(left + 1, len(types))
    ]


def owner_type_effects(
    owner_ids: list[str], types: list[str], effect: np.ndarray, chains: int
) -> list[dict[str, Any]]:
    """Posterior summaries of a ``(draw, owner, type)`` effect array."""
    return [
        {
            "owner_id": owner,
            "type_id": identity,
            "effect": summarize(effect[:, owner_index, type_index], chains),
        }
        for owner_index, owner in enumerate(owner_ids)
        for type_index, identity in enumerate(types)
    ]


def node_type_rows(
    config: dict[str, Any], latent: np.ndarray, expected: np.ndarray
) -> list[dict[str, Any]]:
    """Posterior summaries of the latent effect and expected count for each node-type row."""
    nodes = config["request"]["nodes"]
    rows = []
    pairs = zip(config["node_index"], config["type_index"], strict=True)
    for row, (node, type_index) in enumerate(pairs):
        rows.append(
            {
                "pattern_id": nodes[int(node)]["pattern_id"],
                "node_id": nodes[int(node)]["node_id"],
                "type_id": config["types"][int(type_index)],
                "latent_effect": summarize(latent[:, int(type_index), int(node)], config["chains"]),
                "expected_count": summarize(expected[:, row], config["chains"]),
            }
        )
    return rows


def pattern_type_predictive(config: dict[str, Any], replicated: np.ndarray) -> list[dict[str, Any]]:
    """Posterior predictive checks of per-pattern, per-type totals and node-count variances."""
    predictive = []
    row_patterns = config["node_patterns"][config["node_index"]]
    for pattern_index, pattern_id in enumerate(config["pattern_ids"]):
        for type_index, type_id in enumerate(config["types"]):
            selected = (row_patterns == pattern_index) & (config["type_index"] == type_index)
            observed_nodes = config["observed"][selected]
            replicated_nodes = replicated[:, selected]
            observed_total = int(observed_nodes.sum())
            replicated_totals = replicated_nodes.sum(axis=1)
            observed_variance = float(np.var(observed_nodes))
            replicated_variances = np.var(replicated_nodes, axis=1)
            predictive.append(
                {
                    "pattern_id": pattern_id,
                    "type_id": type_id,
                    "observed_total_count": observed_total,
                    "replicate_count": int(len(replicated_totals)),
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
    return predictive


def centering_constraints_hold(
    config: dict[str, Any], pattern_effect: np.ndarray, latent: np.ndarray
) -> bool:
    """Whether pattern effects sum to zero within patients and latent fields within patterns."""
    patterns_centered = all(
        np.max(np.abs(pattern_effect[:, config["pattern_patients"] == patient, :].sum(axis=1)))
        < 1e-10
        for patient in range(len(config["patient_ids"]))
    )
    return patterns_centered and all(
        np.max(np.abs(latent[:, :, config["node_patterns"] == pattern].sum(axis=2))) < 1e-10
        for pattern in range(len(config["pattern_ids"]))
    )


def moment_check(
    component: str,
    values: np.ndarray,
    expected_mean: float,
    expected_sd: float,
    maximum_mean_error_sd: float,
    maximum_relative_sd_error: float,
) -> dict[str, Any]:
    """Compare the sample mean and SD of prior draws with their analytic values."""
    flat = np.asarray(values, dtype=np.float64).reshape(-1)
    observed_mean = float(flat.mean())
    observed_sd = float(flat.std(ddof=1))
    mean_error_sd = abs(observed_mean - expected_mean) / expected_sd
    relative_sd_error = abs(observed_sd - expected_sd) / expected_sd
    return {
        "component": component,
        "expected_mean": expected_mean,
        "observed_mean": observed_mean,
        "mean_error_sd": mean_error_sd,
        "expected_sd": expected_sd,
        "observed_sd": observed_sd,
        "relative_sd_error": relative_sd_error,
        "passes": bool(
            mean_error_sd <= maximum_mean_error_sd
            and relative_sd_error <= maximum_relative_sd_error
        ),
    }
