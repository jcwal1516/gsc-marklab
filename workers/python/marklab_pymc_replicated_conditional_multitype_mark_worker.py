#!/usr/bin/env python3
"""Pinned PyMC patient-hierarchical conditional model for hard multitype marks."""

from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path
import sys
from typing import Any

import numpy as np
import pymc as pm
import pytensor.tensor as pt


REQUEST_FORMAT = "marklab.pymc_replicated_conditional_multitype_mark_request"
RESULT_FORMAT = "marklab.pymc_replicated_conditional_multitype_mark_result"
PYMC_VERSION = "6.3.0"


class ContractError(ValueError):
    pass


def obj(value: Any, keys: set[str], path: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        actual = set(value) if isinstance(value, dict) else set()
        raise ContractError(
            f"{path} fields differ: missing={sorted(keys - actual)}, unknown={sorted(actual - keys)}"
        )
    return value


def exact(value: Any, expected: Any, path: str) -> None:
    if value != expected:
        raise ContractError(f"{path} must equal {expected!r}")


def number(value: Any, path: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ContractError(f"{path} must be numeric")
    result = float(value)
    if not math.isfinite(result):
        raise ContractError(f"{path} must be finite")
    return result


def integer(value: Any, path: str, low: int, high: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not low <= value <= high:
        raise ContractError(f"{path} must be an integer in [{low}, {high}]")
    return value


def digest(value: Any, path: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise ContractError(f"{path} is not lowercase SHA-256")
    return value


def strings(value: Any, path: str, low: int, high: int) -> list[str]:
    if (
        not isinstance(value, list)
        or not low <= len(value) <= high
        or any(not isinstance(item, str) or not item or len(item) > 256 for item in value)
        or len(set(value)) != len(value)
    ):
        raise ContractError(f"{path} identities are invalid")
    return value


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format", "version", "backend", "input_sha256", "group_ids",
            "reference_group", "type_ids", "reference_type", "radius_um", "patients",
            "patterns", "points", "edges", "priors", "sampling", "diagnostic_policy",
            "resources",
        },
        "request",
    )
    exact(request["format"], REQUEST_FORMAT, "request.format")
    exact(request["version"], 1, "request.version")
    backend = obj(
        request["backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "backend",
    )
    exact(backend["name"], "pymc", "backend.name")
    exact(backend["version"], PYMC_VERSION, "backend.version")
    exact(backend["python_version"], "3.12", "backend.python_version")
    exact(backend["environment_lock_sha256"], lock_digest, "backend.lock")
    exact(backend["worker_sha256"], worker_digest, "backend.worker")
    input_sha = digest(request["input_sha256"], "input_sha256")
    groups = strings(request["group_ids"], "group_ids", 2, 2)
    types = strings(request["type_ids"], "type_ids", 3, 8)
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
    if len(set(patient_ids)) != len(patient_ids) or any(patient_groups.count(group) < 2 for group in (0, 1)):
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
        declared_pattern_points.append(integer(row["point_count"], "pattern.point_count", 6, 100_000))
        declared_pattern_edges.append(integer(row["edge_count"], "pattern.edge_count", 1, 10_000_000))
    if pattern_ids != sorted(pattern_ids) or len(set(pattern_ids)) != len(pattern_ids):
        raise ContractError("pattern identities must be unique and sorted")
    if any(pattern_patients.count(patient) < 2 for patient in range(len(patients))):
        raise ContractError("every patient requires at least two patterns")

    points = request["points"]
    if not isinstance(points, list) or not 24 <= len(points) <= 100_000:
        raise ContractError("point count is invalid")
    point_ids: list[str] = []
    point_patterns: list[int] = []
    point_patients: list[int] = []
    point_groups: list[int] = []
    observed: list[int] = []
    neighbor_counts: list[list[int]] = []
    for index, raw in enumerate(points):
        row = obj(
            raw,
            {
                "point_id", "pattern_index", "patient_index", "group_index", "type_index",
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
        point_patients.append(patient)
        point_groups.append(group)
        observed.append(integer(row["type_index"], "point.type_index", 0, len(types) - 1))
        neighbor_counts.append([
            integer(value, "point.neighbor_count", 0, len(points) - 1) for value in vector
        ])
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
        {"intercept_sd", "interaction_sd", "group_effect_sd", "patient_sd_scale", "pattern_sd_scale"},
        "priors",
    )
    prior_values = {name: number(value, f"priors.{name}") for name, value in priors.items()}
    if min(prior_values.values()) <= 0.0:
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
        raise ContractError("target_accept is invalid")
    seed = integer(sampling["seed"], "sampling.seed", 0, 2**64 - 1)
    policy = obj(
        request["diagnostic_policy"],
        {
            "maximum_r_hat", "minimum_bulk_ess", "minimum_tail_ess", "minimum_ebfmi",
            "maximum_divergences", "maximum_tree_depth_hits",
        },
        "diagnostic_policy",
    )
    resources = obj(
        request["resources"],
        {
            "maximum_patients", "maximum_patterns", "maximum_points", "maximum_types",
            "maximum_neighbor_visits", "neighbor_visits", "maximum_edges",
            "maximum_draw_parameter_work", "draw_parameter_work", "maximum_working_bytes",
            "estimated_working_bytes", "memory_scope", "maximum_output_bytes",
            "maximum_tree_depth", "timeout_seconds",
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
    if visits > integer(resources["maximum_neighbor_visits"], "resources.maximum_neighbor_visits", 1, 2**63 - 1):
        raise ContractError("neighbor-visit ceiling exceeded")
    if len(edges) > integer(resources["maximum_edges"], "resources.maximum_edges", 1, 10_000_000):
        raise ContractError("edge ceiling exceeded")
    free = len(types) - 1 + len(types) * (len(types) + 1) // 2 - 1
    parameter_count = 2 * free + (len(patients) + len(patterns)) * free + 4
    work = chains * draws * parameter_count
    exact(resources["draw_parameter_work"], work, "resources.draw_parameter_work")
    if work > integer(resources["maximum_draw_parameter_work"], "resources.maximum_draw_parameter_work", 1, 2**63 - 1):
        raise ContractError("draw-parameter ceiling exceeded")
    estimated = integer(resources["estimated_working_bytes"], "resources.estimated_working_bytes", 1, 2**63 - 1)
    if estimated > integer(resources["maximum_working_bytes"], "resources.maximum_working_bytes", 1, 16 * 1024**3):
        raise ContractError("working-byte ceiling exceeded")
    exact(
        resources["memory_scope"],
        "retained_request_graph_and_posterior_parameters_excluding_pinned_runtime_rss",
        "resources.memory_scope",
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
        "chains": chains,
        "tune": tune,
        "draws": draws,
        "target_accept": target_accept,
        "seed": seed,
        "maximum_tree_depth": integer(resources["maximum_tree_depth"], "resources.maximum_tree_depth", 10, 14),
        "maximum_output_bytes": integer(resources["maximum_output_bytes"], "resources.maximum_output_bytes", 1, 8 * 1_048_576),
        "maximum_r_hat": number(policy["maximum_r_hat"], "policy.rhat"),
        "minimum_bulk_ess": number(policy["minimum_bulk_ess"], "policy.bulk"),
        "minimum_tail_ess": number(policy["minimum_tail_ess"], "policy.tail"),
        "minimum_ebfmi": number(policy["minimum_ebfmi"], "policy.ebfmi"),
        "maximum_divergences": integer(policy["maximum_divergences"], "policy.divergences", 0, 2**63 - 1),
        "maximum_tree_depth_hits": integer(policy["maximum_tree_depth_hits"], "policy.depth_hits", 0, 2**63 - 1),
    }


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    value = hashlib.sha256(
        f"marklab-pymc-replicated-conditional-multitype-mark-v1\0{seed}\0{purpose}\0{index}".encode()
    ).digest()
    return int.from_bytes(value[:4], "little")


def summary(values: np.ndarray) -> dict[str, float]:
    flat = np.asarray(values, dtype=np.float64).reshape(-1)
    return {
        "mean": float(flat.mean()),
        "sd": float(flat.std(ddof=1)),
        "interval_lower": float(np.quantile(flat, 0.025)),
        "interval_upper": float(np.quantile(flat, 0.975)),
    }


def tree_values(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def softmax(values: np.ndarray) -> np.ndarray:
    shifted = values - values.max(axis=-1, keepdims=True)
    weights = np.exp(shifted)
    return weights / weights.sum(axis=-1, keepdims=True)


def design(types: int) -> np.ndarray:
    result = np.zeros((types * (types + 1) // 2 - 1, types, types), dtype=np.float64)
    index = 0
    for left in range(types):
        for right in range(left, types):
            if left == 0 and right == 0:
                continue
            result[index, left, right] = 1.0
            result[index, right, left] = 1.0
            index += 1
    return result


def affinity_rows(values: np.ndarray, type_ids: list[str]) -> list[dict[str, Any]]:
    rows = []
    for left in range(len(type_ids)):
        for right in range(left + 1, len(type_ids)):
            contrast = values[:, left, right] - 0.5 * (
                values[:, left, left] + values[:, right, right]
            )
            rows.append({
                "type_a": type_ids[left],
                "type_b": type_ids[right],
                "contrast": summary(contrast),
            })
    return rows


def flatten(trace: Any, name: str) -> np.ndarray:
    value = np.asarray(trace["posterior"][name].values, dtype=np.float64)
    return value.reshape((-1, *value.shape[2:]))


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    types = len(config["types"])
    patients = len(config["patient_ids"])
    patterns = len(config["pattern_ids"])
    free_intercepts = types - 1
    free_potentials = types * (types + 1) // 2 - 1
    potential_design = design(types)
    with pm.Model():
        baseline_intercept = pm.Normal(
            "baseline_intercept", 0.0, config["intercept_sd"], shape=free_intercepts
        )
        group_intercept = pm.Normal(
            "group_intercept", 0.0, config["group_effect_sd"], shape=free_intercepts
        )
        baseline_potential = pm.Normal(
            "baseline_potential", 0.0, config["interaction_sd"], shape=free_potentials
        )
        group_potential = pm.Normal(
            "group_potential", 0.0, config["group_effect_sd"], shape=free_potentials
        )
        patient_intercept_sd = pm.HalfNormal(
            "patient_intercept_sd", config["patient_sd_scale"]
        )
        patient_potential_sd = pm.HalfNormal(
            "patient_potential_sd", config["patient_sd_scale"]
        )
        pattern_intercept_sd = pm.HalfNormal(
            "pattern_intercept_sd", config["pattern_sd_scale"]
        )
        pattern_potential_sd = pm.HalfNormal(
            "pattern_potential_sd", config["pattern_sd_scale"]
        )
        patient_intercept_z = pm.Normal(
            "patient_intercept_z", 0.0, 1.0, shape=(patients, free_intercepts)
        )
        patient_potential_z = pm.Normal(
            "patient_potential_z", 0.0, 1.0, shape=(patients, free_potentials)
        )
        pattern_intercept_z = pm.Normal(
            "pattern_intercept_z", 0.0, 1.0, shape=(patterns, free_intercepts)
        )
        pattern_potential_z = pm.Normal(
            "pattern_potential_z", 0.0, 1.0, shape=(patterns, free_potentials)
        )
        pattern_group = pt.as_tensor_variable(config["pattern_groups"][:, None])
        pattern_patient = pt.as_tensor_variable(config["pattern_patients"])
        pattern_intercept_free = (
            baseline_intercept
            + pattern_group * group_intercept
            + patient_intercept_sd * patient_intercept_z[pattern_patient]
            + pattern_intercept_sd * pattern_intercept_z
        )
        pattern_potential_free = (
            baseline_potential
            + pattern_group * group_potential
            + patient_potential_sd * patient_potential_z[pattern_patient]
            + pattern_potential_sd * pattern_potential_z
        )
        pattern_intercept = pt.concatenate(
            [pt.zeros((patterns, 1)), pattern_intercept_free], axis=1
        )
        pattern_potential = pt.tensordot(
            pattern_potential_free, pt.as_tensor_variable(potential_design), axes=1
        )
        owner = pt.as_tensor_variable(config["point_patterns"])
        logits = pattern_intercept[owner] + pt.sum(
            pattern_potential[owner]
            * pt.as_tensor_variable(config["neighbor_counts"])[:, None, :],
            axis=2,
        )
        pm.Categorical("observed_type", logit_p=logits, observed=config["observed"])
        prior = pm.sample_prior_predictive(
            draws=500, random_seed=seed_for(config["seed"], "prior")
        )
        posterior = pm.sample(
            draws=config["draws"],
            tune=config["tune"],
            chains=config["chains"],
            cores=1,
            blas_cores=1,
            random_seed=[
                seed_for(config["seed"], "chain", index)
                for index in range(config["chains"])
            ],
            target_accept=config["target_accept"],
            nuts_sampler="pymc",
            nuts={"max_treedepth": config["maximum_tree_depth"]},
            progressbar=False,
            quiet=True,
            compute_convergence_checks=False,
        )

    base_intercept = flatten(posterior, "baseline_intercept")
    group_intercept = flatten(posterior, "group_intercept")
    base_potential_free = flatten(posterior, "baseline_potential")
    group_potential_free = flatten(posterior, "group_potential")
    patient_intercept_sd = flatten(posterior, "patient_intercept_sd")
    patient_potential_sd = flatten(posterior, "patient_potential_sd")
    pattern_intercept_sd = flatten(posterior, "pattern_intercept_sd")
    pattern_potential_sd = flatten(posterior, "pattern_potential_sd")
    patient_intercept_z = flatten(posterior, "patient_intercept_z")
    patient_potential_z = flatten(posterior, "patient_potential_z")
    pattern_intercept_z = flatten(posterior, "pattern_intercept_z")
    pattern_potential_z = flatten(posterior, "pattern_potential_z")
    completed = len(base_intercept)
    base_potential = np.einsum("df,fkl->dkl", base_potential_free, potential_design)
    group_potential = np.einsum("df,fkl->dkl", group_potential_free, potential_design)
    patient_potential_free = (
        base_potential_free[:, None, :]
        + config["patient_groups"][None, :, None] * group_potential_free[:, None, :]
        + patient_potential_sd[:, None, None] * patient_potential_z
    )
    patient_potential = np.einsum("dpf,fkl->dpkl", patient_potential_free, potential_design)
    pattern_intercept_free = (
        base_intercept[:, None, :]
        + config["pattern_groups"][None, :, None] * group_intercept[:, None, :]
        + patient_intercept_sd[:, None, None]
        * patient_intercept_z[:, config["pattern_patients"], :]
        + pattern_intercept_sd[:, None, None] * pattern_intercept_z
    )
    pattern_potential_free = (
        base_potential_free[:, None, :]
        + config["pattern_groups"][None, :, None] * group_potential_free[:, None, :]
        + patient_potential_sd[:, None, None]
        * patient_potential_z[:, config["pattern_patients"], :]
        + pattern_potential_sd[:, None, None] * pattern_potential_z
    )
    pattern_intercept = np.zeros((completed, patterns, types), dtype=np.float64)
    pattern_intercept[:, :, 1:] = pattern_intercept_free
    pattern_potential = np.einsum("dpf,fkl->dpkl", pattern_potential_free, potential_design)

    score = np.zeros(completed, dtype=np.float64)
    expected_counts = np.zeros((completed, patterns, types), dtype=np.float64)
    expected_same = np.zeros((completed, patterns), dtype=np.float64)
    elements_per_draw = len(config["observed"]) * types + len(config["edges"]) * types
    chunk = max(1, min(64, 4_000_000 // max(1, elements_per_draw)))
    for begin in range(0, completed, chunk):
        end = min(begin + chunk, completed)
        logits = (
            pattern_intercept[begin:end, config["point_patterns"], :]
            + np.einsum(
                "nl,dnkl->dnk",
                config["neighbor_counts"],
                pattern_potential[begin:end, config["point_patterns"], :, :],
            )
        )
        probability = softmax(logits)
        score[begin:end] = np.log(
            probability[:, np.arange(len(config["observed"])), config["observed"]]
        ).sum(axis=1)
        for pattern in range(patterns):
            mask = config["point_patterns"] == pattern
            expected_counts[begin:end, pattern, :] = probability[:, mask, :].sum(axis=1)
            edge_mask = config["edge_patterns"] == pattern
            endpoints = config["edges"][edge_mask]
            expected_same[begin:end, pattern] = np.sum(
                probability[:, endpoints[:, 0], :] * probability[:, endpoints[:, 1], :],
                axis=(1, 2),
            )

    null_score = 0.0
    pattern_checks = []
    for pattern, pattern_id in enumerate(config["pattern_ids"]):
        mask = config["point_patterns"] == pattern
        labels = config["observed"][mask]
        observed_counts = np.bincount(labels, minlength=types)
        proportions = observed_counts / observed_counts.sum()
        null_score += float(np.sum(observed_counts * np.log(proportions)))
        edge_mask = config["edge_patterns"] == pattern
        endpoints = config["edges"][edge_mask]
        observed_same = int(np.sum(config["observed"][endpoints[:, 0]] == config["observed"][endpoints[:, 1]]))
        pattern_checks.append({
            "pattern_id": pattern_id,
            "observed_same_type_edges": observed_same,
            "expected_same_type_edges": summary(expected_same[:, pattern]),
            "observed_type_counts": [int(value) for value in observed_counts],
            "expected_type_counts": [
                summary(expected_counts[:, pattern, type_index])
                for type_index in range(types)
            ],
        })

    patient_affinities = []
    for patient, patient_id in enumerate(config["patient_ids"]):
        rows = affinity_rows(patient_potential[:, patient, :, :], config["types"])
        for row in rows:
            patient_affinities.append({
                "patient_id": patient_id,
                "group": config["groups"][int(config["patient_groups"][patient])],
                **row,
            })
    hierarchy_scales = [
        {"level": "patient", "component": "composition_intercept", "scale": summary(patient_intercept_sd)},
        {"level": "patient", "component": "spatial_pair_potential", "scale": summary(patient_potential_sd)},
        {"level": "pattern_within_patient", "component": "composition_intercept", "scale": summary(pattern_intercept_sd)},
        {"level": "pattern_within_patient", "component": "spatial_pair_potential", "scale": summary(pattern_potential_sd)},
    ]
    monitored = [
        "baseline_intercept", "group_intercept", "baseline_potential", "group_potential",
        "patient_intercept_sd", "patient_potential_sd", "pattern_intercept_sd",
        "pattern_potential_sd", "patient_intercept_z", "patient_potential_z",
        "pattern_intercept_z", "pattern_potential_z",
    ]
    prior_finite = bool(
        all(np.isfinite(np.asarray(value.values)).all() for value in prior["prior"].values())
    )
    posterior_finite = bool(
        all(
            np.isfinite(value).all()
            for value in (
                base_intercept, group_intercept, base_potential, group_potential,
                patient_potential, pattern_intercept, pattern_potential, score,
                expected_counts, expected_same,
            )
        )
    )
    r_hat = float(tree_values(pm.stats.rhat(posterior, var_names=monitored, method="rank"), monitored).max())
    bulk = float(tree_values(pm.stats.ess(posterior, var_names=monitored, method="bulk"), monitored).min())
    tail = float(tree_values(pm.stats.ess(posterior, var_names=monitored, method="tail"), monitored).min())
    mcse_mean = float(tree_values(pm.stats.mcse(posterior, var_names=monitored, method="mean"), monitored).max())
    mcse_sd = float(tree_values(pm.stats.mcse(posterior, var_names=monitored, method="sd"), monitored).max())
    energy = np.asarray(posterior["sample_stats"]["energy"].values, dtype=np.float64)
    ebfmi = float(np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1)))
    divergences = int(np.asarray(posterior["sample_stats"]["diverging"].values).sum())
    depth_hits = int(np.asarray(posterior["sample_stats"]["reached_max_treedepth"].values).sum())
    complete = bool(
        prior_finite
        and posterior_finite
        and r_hat <= config["maximum_r_hat"]
        and bulk >= config["minimum_bulk_ess"]
        and tail >= config["minimum_tail_ess"]
        and ebfmi >= config["minimum_ebfmi"]
        and divergences <= config["maximum_divergences"]
        and depth_hits <= config["maximum_tree_depth_hits"]
    )
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": {
            "name": "pymc", "version": pm.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha, "worker_sha256": worker_sha,
        },
        "input_sha256": config["input_sha"],
        "request_sha256": request_sha,
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"], "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"], "completed_draws": completed,
        },
        "baseline_affinities": affinity_rows(base_potential, config["types"]),
        "group_affinity_shifts": affinity_rows(group_potential, config["types"]),
        "patient_affinities": patient_affinities,
        "hierarchy_scales": hierarchy_scales,
        "comparison": {
            "independent_pattern_label_composite_log_score": null_score,
            "spatial_hierarchical_composite_log_score": summary(score),
            "mean_composite_log_score_improvement": float(score.mean() - null_score),
        },
        "pattern_checks": pattern_checks,
        "diagnostics": {
            "prior_predictive_finite": prior_finite,
            "posterior_finite": posterior_finite,
            "r_hat": r_hat, "ess_bulk": bulk, "ess_tail": tail,
            "mcse_mean": mcse_mean, "mcse_sd": mcse_sd,
            "minimum_ebfmi": ebfmi, "divergences": divergences,
            "max_tree_depth_hits": depth_hits,
            "constraints_valid": True, "identifiability_checks_passed": True,
        },
    }


def main() -> int:
    if pm.__version__ != PYMC_VERSION or sys.version_info[:2] != (3, 12):
        raise ContractError("PyMC or Python version drift")
    script = Path(__file__)
    worker_bytes = script.read_bytes()
    lock_bytes = script.with_name("uv.lock").read_bytes()
    request_bytes = sys.stdin.buffer.read(64 * 1_048_576 + 1)
    if not request_bytes or len(request_bytes) > 64 * 1_048_576:
        raise ContractError("request size is invalid")
    config = validate(
        json.loads(request_bytes), hashlib.sha256(lock_bytes).hexdigest(),
        hashlib.sha256(worker_bytes).hexdigest(),
    )
    result = fit(config, hashlib.sha256(request_bytes).hexdigest(), hashlib.sha256(lock_bytes).hexdigest(), hashlib.sha256(worker_bytes).hexdigest())
    encoded = json.dumps(result, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()
    if len(encoded) > config["maximum_output_bytes"]:
        raise ContractError("result exceeds output ceiling")
    sys.stdout.buffer.write(encoded + b"\n")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ContractError, ValueError, OSError, json.JSONDecodeError) as error:
        print(f"replicated conditional multitype worker failed: {error}", file=sys.stderr)
        raise SystemExit(2) from error
