"""Conditional pseudolikelihood models for hard multitype marks.

Each point's type is categorical given the counts of every type among its neighbours within a
fixed radius::

    logit P(type_i = k | neighbours) = intercept[k] + sum_l counts[i, l] * potential[k, l]

Type 0 is the reference (``intercept[0] = 0`` and ``potential[0, 0] = 0``) and ``potential`` is
symmetric, so the free parameters are ``types - 1`` intercepts and ``types * (types + 1) / 2 - 1``
pair potentials. The replicated model adds a two-group shift and non-centred patient and
pattern-within-patient random effects on both free vectors.

This module holds the request contracts (owned by the PyMC workers and reused by the NumPyro
agreement and SBC workers) and the numpy post-processing shared by all of them.
"""

from __future__ import annotations

from typing import Any, NamedTuple

import numpy as np

from .posterior import softmax, summary
from .protocol import (
    ContractError,
    exact,
    integer,
    number,
    obj,
    sha256_digest,
    validate_backend,
)
from .sbc import rank_summary
from .versions import PYMC

SINGLE_REQUEST_FORMAT = "marklab.pymc_conditional_multitype_mark_request"
REPLICATED_REQUEST_FORMAT = "marklab.pymc_replicated_conditional_multitype_mark_request"
HIERARCHY_PARAMETERS = [
    "baseline_intercept",
    "group_intercept",
    "baseline_potential",
    "group_potential",
    "patient_intercept_sd",
    "patient_potential_sd",
    "pattern_intercept_sd",
    "pattern_potential_sd",
    "patient_intercept_z",
    "patient_potential_z",
    "pattern_intercept_z",
    "pattern_potential_z",
]
SUMMARY_CHUNK_DRAWS = 128


def free_pairs(types: int) -> list[tuple[int, int]]:
    """Upper-triangle type pairs that carry a free potential (all except ``(0, 0)``)."""
    return [
        (left, right)
        for left in range(types)
        for right in range(left, types)
        if left != 0 or right != 0
    ]


def distinct_pairs(types: int) -> list[tuple[int, int]]:
    """Type pairs ``left < right`` that have an affinity contrast."""
    return [(left, right) for left in range(types) for right in range(left + 1, types)]


def potential_design(types: int) -> np.ndarray:
    """Map free potentials to symmetric matrices: ``potential = free @ design``."""
    pairs = free_pairs(types)
    result = np.zeros((len(pairs), types, types), dtype=np.float64)
    for index, (left, right) in enumerate(pairs):
        result[index, left, right] = 1.0
        result[index, right, left] = 1.0
    return result


def free_matrices(
    intercept_free: np.ndarray, potential_free: np.ndarray, types: int
) -> tuple[np.ndarray, np.ndarray]:
    """Expand free draws (any leading shape) into full intercept vectors and potentials."""
    intercept = np.zeros((*intercept_free.shape[:-1], types), dtype=np.float64)
    intercept[..., 1:] = intercept_free
    potential = np.zeros((*potential_free.shape[:-1], types, types), dtype=np.float64)
    for index, (left, right) in enumerate(free_pairs(types)):
        potential[..., left, right] = potential_free[..., index]
        potential[..., right, left] = potential_free[..., index]
    return intercept, potential


def affinity_contrast(potential: np.ndarray, left: int, right: int) -> np.ndarray:
    """Cross-type potential minus the mean of the two same-type potentials."""
    same_type = potential[..., left, left] + potential[..., right, right]
    return potential[..., left, right] - 0.5 * same_type


def affinity_rows(values: np.ndarray, type_ids: list[str], chains: int) -> list[dict[str, Any]]:
    """Summaries of every affinity contrast of chain-major ``(draw, type, type)`` potentials."""
    return [
        {
            "type_a": type_ids[left],
            "type_b": type_ids[right],
            "contrast": _summary(affinity_contrast(values, left, right), chains),
        }
        for left, right in distinct_pairs(len(type_ids))
    ]


def policy_met(config: dict[str, Any], diagnostics: dict[str, Any]) -> bool:
    """Whether NUTS diagnostics satisfy the request's diagnostic policy."""
    return (
        diagnostics["r_hat"] <= config["maximum_r_hat"]
        and diagnostics["ess_bulk"] >= config["minimum_bulk_ess"]
        and diagnostics["ess_tail"] >= config["minimum_tail_ess"]
        and diagnostics["minimum_ebfmi"] >= config["minimum_ebfmi"]
        and diagnostics["divergences"] <= config["maximum_divergences"]
        and diagnostics["max_tree_depth_hits"] <= config["maximum_tree_depth_hits"]
    )


def sbc_parameter_summaries(
    completed: list[dict[str, Any]], names: list[str], draws: int, bins: int
) -> list[dict[str, Any]]:
    """Per-parameter SBC rank summaries from replicates that list ``parameters`` rows."""
    summaries = []
    for name in names:
        rows = [
            next(value for value in row["parameters"] if value["parameter"] == name)
            for row in completed
        ]
        ranks = [row["rank"] for row in rows]
        covered = [row["covered"] for row in rows]
        summaries.append({"parameter": name, **rank_summary(ranks, covered, draws, bins)})
    return summaries


def _summary(values: np.ndarray, chains: int) -> dict[str, float]:
    return summary(values, dtype=np.float64, chains=chains)


def _identities(value: Any, path: str, low: int, high: int) -> list[str]:
    if (
        not isinstance(value, list)
        or not low <= len(value) <= high
        or any(not isinstance(item, str) or not item or len(item) > 256 for item in value)
        or len(set(value)) != len(value)
    ):
        raise ContractError(f"{path} identities are invalid")
    return value


def _sampling(request: dict[str, Any]) -> dict[str, Any]:
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
        raise ContractError("target_accept is invalid")
    seed = integer(sampling["seed"], "sampling.seed", 0, 2**64 - 1)
    return {
        "chains": chains,
        "tune": tune,
        "draws": draws,
        "target_accept": target_accept,
        "seed": seed,
    }


def _policy(request: dict[str, Any]) -> dict[str, Any]:
    return obj(
        request["diagnostic_policy"],
        {
            "maximum_r_hat",
            "minimum_bulk_ess",
            "minimum_tail_ess",
            "minimum_ebfmi",
            "maximum_divergences",
            "maximum_tree_depth_hits",
        },
        "diagnostic_policy",
    )


def _policy_thresholds(policy: dict[str, Any]) -> dict[str, Any]:
    return {
        "maximum_r_hat": number(policy["maximum_r_hat"], "policy.rhat"),
        "minimum_bulk_ess": number(policy["minimum_bulk_ess"], "policy.bulk"),
        "minimum_tail_ess": number(policy["minimum_tail_ess"], "policy.tail"),
        "minimum_ebfmi": number(policy["minimum_ebfmi"], "policy.ebfmi"),
        "maximum_divergences": integer(
            policy["maximum_divergences"], "policy.divergences", 0, 2**63 - 1
        ),
        "maximum_tree_depth_hits": integer(
            policy["maximum_tree_depth_hits"], "policy.depth_hits", 0, 2**63 - 1
        ),
    }


def validate_single_request(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    """Validate a single-pattern PyMC request; ``worker_digest`` is the PyMC worker's."""
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "input_sha256",
            "type_ids",
            "reference_type",
            "radius_um",
            "points",
            "edges",
            "priors",
            "sampling",
            "diagnostic_policy",
            "resources",
        },
        "request",
    )
    exact(request["format"], SINGLE_REQUEST_FORMAT, "request.format")
    exact(request["version"], 1, "request.version")
    validate_backend(
        request["backend"],
        "backend",
        name="pymc",
        version=PYMC,
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )
    input_sha = sha256_digest(request["input_sha256"], "input_sha256")
    type_ids = request["type_ids"]
    if (
        not isinstance(type_ids, list)
        or not 3 <= len(type_ids) <= 8
        or any(not isinstance(value, str) or not value or len(value) > 128 for value in type_ids)
        or len(set(type_ids)) != len(type_ids)
        or request["reference_type"] != type_ids[0]
    ):
        raise ContractError("type identities or reference are invalid")
    radius = number(request["radius_um"], "radius_um")
    if radius <= 0.0:
        raise ContractError("radius must be positive")

    points = request["points"]
    if not isinstance(points, list) or not 6 <= len(points) <= 10_000:
        raise ContractError("point count is invalid")
    point_ids: list[str] = []
    observed: list[int] = []
    neighbor_counts: list[list[int]] = []
    for index, raw in enumerate(points):
        row = obj(raw, {"point_id", "type_index", "neighbor_counts"}, f"points[{index}]")
        point_id = row["point_id"]
        if not isinstance(point_id, str) or not point_id or len(point_id) > 256:
            raise ContractError("point identity is invalid")
        vector = row["neighbor_counts"]
        if not isinstance(vector, list) or len(vector) != len(type_ids):
            raise ContractError("neighbor-count dimensions are invalid")
        point_ids.append(point_id)
        observed.append(integer(row["type_index"], "point.type_index", 0, len(type_ids) - 1))
        neighbor_counts.append(
            [integer(value, "point.neighbor_count", 0, len(points) - 1) for value in vector]
        )
    if point_ids != sorted(point_ids) or len(set(point_ids)) != len(point_ids):
        raise ContractError("point identities must be unique and sorted")
    if any(observed.count(index) < 2 for index in range(len(type_ids))):
        raise ContractError("every type requires at least two points")

    edges = request["edges"]
    if not isinstance(edges, list) or not edges:
        raise ContractError("edges are invalid")
    edge_rows: list[tuple[int, int]] = []
    previous = (-1, -1)
    recomputed = np.zeros((len(points), len(type_ids)), dtype=np.int64)
    for index, raw in enumerate(edges):
        row = obj(raw, {"left", "right"}, f"edges[{index}]")
        left = integer(row["left"], "edge.left", 0, len(points) - 1)
        right = integer(row["right"], "edge.right", 0, len(points) - 1)
        if left >= right or (left, right) <= previous:
            raise ContractError("edges must be unique and lexicographically sorted")
        previous = (left, right)
        edge_rows.append((left, right))
        recomputed[left, observed[right]] += 1
        recomputed[right, observed[left]] += 1
    counts = np.asarray(neighbor_counts, dtype=np.int64)
    if not np.array_equal(counts, recomputed):
        raise ContractError("neighbor counts differ from exact edges and observed types")

    priors = obj(request["priors"], {"intercept_sd", "interaction_sd"}, "priors")
    intercept_sd = number(priors["intercept_sd"], "priors.intercept_sd")
    interaction_sd = number(priors["interaction_sd"], "priors.interaction_sd")
    if min(intercept_sd, interaction_sd) <= 0.0:
        raise ContractError("prior scales must be positive")
    sampling = _sampling(request)
    policy = _policy(request)
    resources = obj(
        request["resources"],
        {
            "maximum_points",
            "maximum_types",
            "maximum_neighbor_visits",
            "neighbor_visits",
            "maximum_edges",
            "maximum_draw_parameter_work",
            "draw_parameter_work",
            "maximum_working_bytes",
            "estimated_working_bytes",
            "maximum_output_bytes",
            "memory_scope",
            "maximum_tree_depth",
            "timeout_seconds",
        },
        "resources",
    )
    if len(points) > integer(resources["maximum_points"], "resources.points", 6, 10_000):
        raise ContractError("point ceiling exceeded")
    if len(type_ids) > integer(resources["maximum_types"], "resources.types", 3, 8):
        raise ContractError("type ceiling exceeded")
    expected_visits = len(points) * (len(points) - 1) // 2
    exact(resources["neighbor_visits"], expected_visits, "resources.neighbor_visits")
    maximum_visits = integer(
        resources["maximum_neighbor_visits"], "resources.maximum_neighbor_visits", 1, 100_000_000
    )
    if expected_visits > maximum_visits:
        raise ContractError("neighbor-visit ceiling exceeded")
    if len(edges) > integer(resources["maximum_edges"], "resources.maximum_edges", 1, 10_000_000):
        raise ContractError("edge ceiling exceeded")
    parameter_count = len(type_ids) - 1 + len(type_ids) * (len(type_ids) + 1) // 2 - 1
    work = sampling["chains"] * sampling["draws"] * parameter_count
    exact(resources["draw_parameter_work"], work, "resources.draw_parameter_work")
    maximum_work = integer(
        resources["maximum_draw_parameter_work"],
        "resources.maximum_draw_parameter_work",
        1,
        100_000_000,
    )
    if work > maximum_work:
        raise ContractError("draw-parameter ceiling exceeded")
    working = integer(
        resources["estimated_working_bytes"], "resources.estimated_working_bytes", 1, 2**63 - 1
    )
    maximum_working = integer(
        resources["maximum_working_bytes"], "resources.maximum_working_bytes", 1, 8 * 1024**3
    )
    if working > maximum_working:
        raise ContractError("working-byte ceiling exceeded")
    exact(
        resources["memory_scope"],
        "retained_request_graph_and_posterior_summaries_excluding_pinned_runtime_rss",
        "resources.memory_scope",
    )
    maximum_output_bytes = integer(
        resources["maximum_output_bytes"], "resources.maximum_output_bytes", 1, 2 * 1_048_576
    )
    maximum_tree_depth = integer(
        resources["maximum_tree_depth"], "resources.maximum_tree_depth", 10, 14
    )
    return {
        "request": request,
        "input_sha": input_sha,
        "type_ids": type_ids,
        "observed": np.asarray(observed, dtype=np.int64),
        "neighbor_counts": counts.astype(np.float64),
        "edges": np.asarray(edge_rows, dtype=np.int64),
        "intercept_sd": intercept_sd,
        "interaction_sd": interaction_sd,
        **sampling,
        "maximum_tree_depth": maximum_tree_depth,
        **_policy_thresholds(policy),
        "maximum_output_bytes": maximum_output_bytes,
    }


def validate_replicated_request(
    request: Any, lock_digest: str, worker_digest: str
) -> dict[str, Any]:
    """Validate a replicated PyMC request; ``worker_digest`` is the PyMC worker's."""
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "input_sha256",
            "group_ids",
            "reference_group",
            "type_ids",
            "reference_type",
            "radius_um",
            "patients",
            "patterns",
            "points",
            "edges",
            "priors",
            "sampling",
            "diagnostic_policy",
            "resources",
        },
        "request",
    )
    exact(request["format"], REPLICATED_REQUEST_FORMAT, "request.format")
    exact(request["version"], 1, "request.version")
    validate_backend(
        request["backend"],
        "backend",
        name="pymc",
        version=PYMC,
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )
    input_sha = sha256_digest(request["input_sha256"], "input_sha256")
    groups = _identities(request["group_ids"], "group_ids", 2, 2)
    types = _identities(request["type_ids"], "type_ids", 3, 8)
    exact(request["reference_group"], groups[0], "reference_group")
    exact(request["reference_type"], types[0], "reference_type")
    radius = number(request["radius_um"], "radius_um")
    if radius <= 0.0:
        raise ContractError("radius must be positive")

    patients = request["patients"]
    if not isinstance(patients, list) or not 4 <= len(patients) <= 64:
        raise ContractError("patient count is invalid")
    patient_ids: list[str] = []
    patient_groups: list[int] = []
    for index, raw in enumerate(patients):
        row = obj(raw, {"patient_id", "group_index"}, f"patients[{index}]")
        identity = row["patient_id"]
        if not isinstance(identity, str) or not identity or len(identity) > 256:
            raise ContractError("patient identity is invalid")
        patient_ids.append(identity)
        patient_groups.append(integer(row["group_index"], "patient.group_index", 0, 1))
    if len(set(patient_ids)) != len(patient_ids) or any(
        patient_groups.count(group) < 2 for group in (0, 1)
    ):
        raise ContractError("patient identities or group support differ")

    patterns = request["patterns"]
    if not isinstance(patterns, list) or not 8 <= len(patterns) <= 256:
        raise ContractError("pattern count is invalid")
    pattern_ids: list[str] = []
    pattern_patients: list[int] = []
    pattern_groups: list[int] = []
    declared_pattern_points: list[int] = []
    declared_pattern_edges: list[int] = []
    for index, raw in enumerate(patterns):
        row = obj(
            raw,
            {"pattern_id", "patient_index", "group_index", "point_count", "edge_count"},
            f"patterns[{index}]",
        )
        identity = row["pattern_id"]
        if not isinstance(identity, str) or not identity or len(identity) > 256:
            raise ContractError("pattern identity is invalid")
        patient = integer(row["patient_index"], "pattern.patient_index", 0, len(patients) - 1)
        group = integer(row["group_index"], "pattern.group_index", 0, 1)
        if group != patient_groups[patient]:
            raise ContractError("pattern group differs from patient group")
        pattern_ids.append(identity)
        pattern_patients.append(patient)
        pattern_groups.append(group)
        declared_pattern_points.append(
            integer(row["point_count"], "pattern.point_count", 6, 100_000)
        )
        declared_pattern_edges.append(
            integer(row["edge_count"], "pattern.edge_count", 1, 10_000_000)
        )
    if pattern_ids != sorted(pattern_ids) or len(set(pattern_ids)) != len(pattern_ids):
        raise ContractError("pattern identities must be unique and sorted")
    if any(pattern_patients.count(patient) < 2 for patient in range(len(patients))):
        raise ContractError("every patient requires at least two patterns")

    points = request["points"]
    if not isinstance(points, list) or not 24 <= len(points) <= 100_000:
        raise ContractError("point count is invalid")
    point_ids: list[str] = []
    point_patterns: list[int] = []
    observed: list[int] = []
    neighbor_counts: list[list[int]] = []
    for index, raw in enumerate(points):
        row = obj(
            raw,
            {
                "point_id",
                "pattern_index",
                "patient_index",
                "group_index",
                "type_index",
                "neighbor_counts",
            },
            f"points[{index}]",
        )
        identity = row["point_id"]
        if not isinstance(identity, str) or not identity or len(identity) > 256:
            raise ContractError("point identity is invalid")
        pattern = integer(row["pattern_index"], "point.pattern_index", 0, len(patterns) - 1)
        patient = integer(row["patient_index"], "point.patient_index", 0, len(patients) - 1)
        group = integer(row["group_index"], "point.group_index", 0, 1)
        if patient != pattern_patients[pattern] or group != pattern_groups[pattern]:
            raise ContractError("point hierarchy differs from pattern")
        vector = row["neighbor_counts"]
        if not isinstance(vector, list) or len(vector) != len(types):
            raise ContractError("neighbor-count dimensions are invalid")
        point_ids.append(identity)
        point_patterns.append(pattern)
        observed.append(integer(row["type_index"], "point.type_index", 0, len(types) - 1))
        neighbor_counts.append(
            [integer(value, "point.neighbor_count", 0, len(points) - 1) for value in vector]
        )
    if len(set(point_ids)) != len(point_ids):
        raise ContractError("point identities must be globally unique")
    if [point_patterns.count(index) for index in range(len(patterns))] != declared_pattern_points:
        raise ContractError("declared pattern point counts differ")
    for pattern in range(len(patterns)):
        labels = [observed[index] for index, owner in enumerate(point_patterns) if owner == pattern]
        if any(labels.count(type_index) < 2 for type_index in range(len(types))):
            raise ContractError("every pattern requires every type twice")

    edges = request["edges"]
    if not isinstance(edges, list) or not edges:
        raise ContractError("edges are invalid")
    edge_rows: list[tuple[int, int]] = []
    edge_patterns: list[int] = []
    recomputed = np.zeros((len(points), len(types)), dtype=np.int64)
    previous = (-1, -1)
    for index, raw in enumerate(edges):
        row = obj(raw, {"left", "right", "pattern_index"}, f"edges[{index}]")
        left = integer(row["left"], "edge.left", 0, len(points) - 1)
        right = integer(row["right"], "edge.right", 0, len(points) - 1)
        pattern = integer(row["pattern_index"], "edge.pattern_index", 0, len(patterns) - 1)
        if left >= right or (left, right) <= previous:
            raise ContractError("edges must be unique and globally ordered")
        if point_patterns[left] != pattern or point_patterns[right] != pattern:
            raise ContractError("edge crosses patterns")
        previous = (left, right)
        edge_rows.append((left, right))
        edge_patterns.append(pattern)
        recomputed[left, observed[right]] += 1
        recomputed[right, observed[left]] += 1
    if [edge_patterns.count(index) for index in range(len(patterns))] != declared_pattern_edges:
        raise ContractError("declared pattern edge counts differ")
    counts = np.asarray(neighbor_counts, dtype=np.int64)
    if not np.array_equal(counts, recomputed):
        raise ContractError("neighbor counts differ from exact edges and labels")

    priors = obj(
        request["priors"],
        {
            "intercept_sd",
            "interaction_sd",
            "group_effect_sd",
            "patient_sd_scale",
            "pattern_sd_scale",
        },
        "priors",
    )
    prior_values = {name: number(value, f"priors.{name}") for name, value in priors.items()}
    if min(prior_values.values()) <= 0.0:
        raise ContractError("prior scales must be positive")
    sampling = _sampling(request)
    policy = _policy(request)
    resources = obj(
        request["resources"],
        {
            "maximum_patients",
            "maximum_patterns",
            "maximum_points",
            "maximum_types",
            "maximum_neighbor_visits",
            "neighbor_visits",
            "maximum_edges",
            "maximum_draw_parameter_work",
            "draw_parameter_work",
            "maximum_working_bytes",
            "estimated_working_bytes",
            "memory_scope",
            "maximum_output_bytes",
            "maximum_tree_depth",
            "timeout_seconds",
        },
        "resources",
    )
    if len(patients) > integer(resources["maximum_patients"], "resources.patients", 4, 64):
        raise ContractError("patient ceiling exceeded")
    if len(patterns) > integer(resources["maximum_patterns"], "resources.patterns", 8, 256):
        raise ContractError("pattern ceiling exceeded")
    if len(points) > integer(resources["maximum_points"], "resources.points", 24, 100_000):
        raise ContractError("point ceiling exceeded")
    if len(types) > integer(resources["maximum_types"], "resources.types", 3, 8):
        raise ContractError("type ceiling exceeded")
    visits = sum(count * (count - 1) // 2 for count in declared_pattern_points)
    exact(resources["neighbor_visits"], visits, "resources.neighbor_visits")
    maximum_visits = integer(
        resources["maximum_neighbor_visits"], "resources.maximum_neighbor_visits", 1, 2**63 - 1
    )
    if visits > maximum_visits:
        raise ContractError("neighbor-visit ceiling exceeded")
    if len(edges) > integer(resources["maximum_edges"], "resources.maximum_edges", 1, 10_000_000):
        raise ContractError("edge ceiling exceeded")
    free = len(types) - 1 + len(types) * (len(types) + 1) // 2 - 1
    parameter_count = 2 * free + (len(patients) + len(patterns)) * free + 4
    work = sampling["chains"] * sampling["draws"] * parameter_count
    exact(resources["draw_parameter_work"], work, "resources.draw_parameter_work")
    maximum_work = integer(
        resources["maximum_draw_parameter_work"],
        "resources.maximum_draw_parameter_work",
        1,
        2**63 - 1,
    )
    if work > maximum_work:
        raise ContractError("draw-parameter ceiling exceeded")
    estimated = integer(
        resources["estimated_working_bytes"], "resources.estimated_working_bytes", 1, 2**63 - 1
    )
    maximum_working = integer(
        resources["maximum_working_bytes"], "resources.maximum_working_bytes", 1, 16 * 1024**3
    )
    if estimated > maximum_working:
        raise ContractError("working-byte ceiling exceeded")
    exact(
        resources["memory_scope"],
        "retained_request_graph_and_posterior_parameters_excluding_pinned_runtime_rss",
        "resources.memory_scope",
    )
    maximum_tree_depth = integer(
        resources["maximum_tree_depth"], "resources.maximum_tree_depth", 10, 14
    )
    maximum_output_bytes = integer(
        resources["maximum_output_bytes"], "resources.maximum_output_bytes", 1, 8 * 1_048_576
    )
    return {
        "request": request,
        "input_sha": input_sha,
        "groups": groups,
        "types": types,
        "patient_ids": patient_ids,
        "patient_groups": np.asarray(patient_groups, dtype=np.int64),
        "pattern_ids": pattern_ids,
        "pattern_patients": np.asarray(pattern_patients, dtype=np.int64),
        "pattern_groups": np.asarray(pattern_groups, dtype=np.int64),
        "point_patterns": np.asarray(point_patterns, dtype=np.int64),
        "observed": np.asarray(observed, dtype=np.int64),
        "neighbor_counts": counts.astype(np.float64),
        "edges": np.asarray(edge_rows, dtype=np.int64),
        "edge_patterns": np.asarray(edge_patterns, dtype=np.int64),
        **prior_values,
        **sampling,
        "maximum_tree_depth": maximum_tree_depth,
        "maximum_output_bytes": maximum_output_bytes,
        **_policy_thresholds(policy),
    }


class SinglePatternPosterior(NamedTuple):
    sections: dict[str, Any]
    completed_draws: int
    finite: bool
    identifiable: bool


def single_pattern_posterior(
    config: dict[str, Any], intercept: np.ndarray, potential: np.ndarray
) -> SinglePatternPosterior:
    """Summarize single-pattern draws of full intercepts and potentials.

    The predictive check scores each observed type under its one-step conditional given the
    observed neighbours, and compares it with an independent-label model that uses the
    observed type proportions.
    """
    chains = config["chains"]
    types = len(config["type_ids"])
    observed = config["observed"]
    edges = config["edges"]
    flat_intercept = intercept.reshape(-1, types)
    flat_potential = potential.reshape(-1, types, types)
    draws = len(flat_intercept)
    score = np.empty(draws, dtype=np.float64)
    expected_counts = np.empty((draws, types), dtype=np.float64)
    expected_same = np.empty(draws, dtype=np.float64)
    sites = np.arange(len(observed))
    for begin in range(0, draws, SUMMARY_CHUNK_DRAWS):
        end = min(begin + SUMMARY_CHUNK_DRAWS, draws)
        neighbour_terms = np.einsum(
            "nl,dkl->dnk", config["neighbor_counts"], flat_potential[begin:end]
        )
        probability = softmax(flat_intercept[begin:end, None, :] + neighbour_terms)
        score[begin:end] = np.log(probability[:, sites, observed]).sum(axis=1)
        expected_counts[begin:end] = probability.sum(axis=1)
        same_type = probability[:, edges[:, 0], :] * probability[:, edges[:, 1], :]
        expected_same[begin:end] = np.sum(same_type, axis=(1, 2))
    observed_counts = np.bincount(observed, minlength=types)
    proportions = observed_counts / observed_counts.sum()
    null_score = float(np.sum(observed_counts * np.log(proportions)))
    observed_same = int(np.sum(observed[edges[:, 0]] == observed[edges[:, 1]]))
    type_ids = config["type_ids"]
    sections = {
        "type_intercepts": [
            {"type_id": type_id, "intercept": _summary(flat_intercept[:, index], chains)}
            for index, type_id in enumerate(type_ids)
        ],
        "pair_potentials": [
            {
                "type_a": type_ids[left],
                "type_b": type_ids[right],
                "potential": _summary(flat_potential[:, left, right], chains),
            }
            for left in range(types)
            for right in range(left, types)
        ],
        "pair_affinity_contrasts": affinity_rows(flat_potential, type_ids, chains),
        "comparison": {
            "independent_label_composite_log_score": null_score,
            "spatial_composite_log_score": _summary(score, chains),
            "mean_composite_log_score_improvement": float(score.mean() - null_score),
        },
        "posterior_predictive": {
            "mode": "one_step_conditionals_given_observed_neighbors",
            "observed_same_type_edges": observed_same,
            "expected_same_type_edges": _summary(expected_same, chains),
            "observed_type_counts": observed_counts.astype(int).tolist(),
            "expected_type_counts": [
                _summary(expected_counts[:, index], chains) for index in range(types)
            ],
        },
    }
    finite = bool(
        np.isfinite(intercept).all()
        and np.isfinite(potential).all()
        and np.isfinite(score).all()
        and np.isfinite(expected_counts).all()
        and np.isfinite(expected_same).all()
    )
    identifiable = bool(np.all(intercept[..., 0] == 0.0) and np.all(potential[..., 0, 0] == 0.0))
    return SinglePatternPosterior(sections, draws, finite, identifiable)


class ReplicatedPosterior(NamedTuple):
    sections: dict[str, Any]
    completed_draws: int
    finite: bool
    identifiable: bool


def replicated_posterior(
    config: dict[str, Any], draws: dict[str, np.ndarray]
) -> ReplicatedPosterior:
    """Summarize replicated-model draws, flattened over chains, keyed by parameter name.

    Reports baseline affinities, two-group affinity shifts, per-patient affinities, the four
    hierarchy scales, and per-pattern one-step conditional predictive checks.
    """
    chains = config["chains"]
    types = len(config["types"])
    patterns = len(config["pattern_ids"])
    design = potential_design(types)
    pattern_patients = config["pattern_patients"]
    completed = len(draws["baseline_intercept"])
    baseline_potential = np.einsum("df,fkl->dkl", draws["baseline_potential"], design)
    group_potential = np.einsum("df,fkl->dkl", draws["group_potential"], design)
    patient_potential_free = (
        draws["baseline_potential"][:, None, :]
        + config["patient_groups"][None, :, None] * draws["group_potential"][:, None, :]
        + draws["patient_potential_sd"][:, None, None] * draws["patient_potential_z"]
    )
    patient_potential = np.einsum("dpf,fkl->dpkl", patient_potential_free, design)
    pattern_intercept_free = (
        draws["baseline_intercept"][:, None, :]
        + config["pattern_groups"][None, :, None] * draws["group_intercept"][:, None, :]
        + draws["patient_intercept_sd"][:, None, None]
        * draws["patient_intercept_z"][:, pattern_patients, :]
        + draws["pattern_intercept_sd"][:, None, None] * draws["pattern_intercept_z"]
    )
    pattern_potential_free = (
        draws["baseline_potential"][:, None, :]
        + config["pattern_groups"][None, :, None] * draws["group_potential"][:, None, :]
        + draws["patient_potential_sd"][:, None, None]
        * draws["patient_potential_z"][:, pattern_patients, :]
        + draws["pattern_potential_sd"][:, None, None] * draws["pattern_potential_z"]
    )
    pattern_intercept = np.zeros((completed, patterns, types), dtype=np.float64)
    pattern_intercept[:, :, 1:] = pattern_intercept_free
    pattern_potential = np.einsum("dpf,fkl->dpkl", pattern_potential_free, design)

    observed = config["observed"]
    point_patterns = config["point_patterns"]
    edges = config["edges"]
    score = np.zeros(completed, dtype=np.float64)
    expected_counts = np.zeros((completed, patterns, types), dtype=np.float64)
    expected_same = np.zeros((completed, patterns), dtype=np.float64)
    elements_per_draw = len(observed) * types + len(edges) * types
    chunk = max(1, min(64, 4_000_000 // max(1, elements_per_draw)))
    sites = np.arange(len(observed))
    for begin in range(0, completed, chunk):
        end = min(begin + chunk, completed)
        neighbour_terms = np.einsum(
            "nl,dnkl->dnk",
            config["neighbor_counts"],
            pattern_potential[begin:end, point_patterns, :, :],
        )
        probability = softmax(pattern_intercept[begin:end, point_patterns, :] + neighbour_terms)
        score[begin:end] = np.log(probability[:, sites, observed]).sum(axis=1)
        for pattern in range(patterns):
            point_mask = point_patterns == pattern
            expected_counts[begin:end, pattern, :] = probability[:, point_mask, :].sum(axis=1)
            endpoints = edges[config["edge_patterns"] == pattern]
            same_type = probability[:, endpoints[:, 0], :] * probability[:, endpoints[:, 1], :]
            expected_same[begin:end, pattern] = np.sum(same_type, axis=(1, 2))

    null_score = 0.0
    pattern_checks = []
    for pattern, pattern_id in enumerate(config["pattern_ids"]):
        observed_counts = np.bincount(observed[point_patterns == pattern], minlength=types)
        proportions = observed_counts / observed_counts.sum()
        null_score += float(np.sum(observed_counts * np.log(proportions)))
        endpoints = edges[config["edge_patterns"] == pattern]
        observed_same = int(np.sum(observed[endpoints[:, 0]] == observed[endpoints[:, 1]]))
        pattern_checks.append(
            {
                "pattern_id": pattern_id,
                "observed_same_type_edges": observed_same,
                "expected_same_type_edges": _summary(expected_same[:, pattern], chains),
                "observed_type_counts": [int(value) for value in observed_counts],
                "expected_type_counts": [
                    _summary(expected_counts[:, pattern, type_index], chains)
                    for type_index in range(types)
                ],
            }
        )

    patient_affinities = []
    for patient, patient_id in enumerate(config["patient_ids"]):
        group = config["groups"][int(config["patient_groups"][patient])]
        for row in affinity_rows(patient_potential[:, patient, :, :], config["types"], chains):
            patient_affinities.append({"patient_id": patient_id, "group": group, **row})
    hierarchy_scales = [
        {"level": level, "component": component, "scale": _summary(draws[name], chains)}
        for level, component, name in [
            ("patient", "composition_intercept", "patient_intercept_sd"),
            ("patient", "spatial_pair_potential", "patient_potential_sd"),
            ("pattern_within_patient", "composition_intercept", "pattern_intercept_sd"),
            ("pattern_within_patient", "spatial_pair_potential", "pattern_potential_sd"),
        ]
    ]
    sections = {
        "baseline_affinities": affinity_rows(baseline_potential, config["types"], chains),
        "group_affinity_shifts": affinity_rows(group_potential, config["types"], chains),
        "patient_affinities": patient_affinities,
        "hierarchy_scales": hierarchy_scales,
        "comparison": {
            "independent_pattern_label_composite_log_score": null_score,
            "spatial_hierarchical_composite_log_score": _summary(score, chains),
            "mean_composite_log_score_improvement": float(score.mean() - null_score),
        },
        "pattern_checks": pattern_checks,
    }
    finite = bool(
        all(
            np.isfinite(value).all()
            for value in (
                baseline_potential,
                group_potential,
                patient_potential,
                pattern_intercept,
                pattern_potential,
                score,
                expected_counts,
                expected_same,
            )
        )
    )
    identifiable = bool(
        np.all(pattern_intercept[..., 0] == 0.0) and np.all(pattern_potential[..., 0, 0] == 0.0)
    )
    return ReplicatedPosterior(sections, completed, finite, identifiable)
