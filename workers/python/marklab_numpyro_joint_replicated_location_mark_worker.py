#!/usr/bin/env python3
"""Patient-replicated joint point-location and conditional categorical-mark model."""

from __future__ import annotations

import json
import math
import sys
from collections.abc import Callable
from pathlib import Path
from typing import Any

import jax

jax.config.update("jax_enable_x64", True)
sys.path.append(str(Path(__file__).resolve().parent))

import jax.numpy as jnp
import numpy as np
import numpyro
import numpyro.distributions as dist

from marklab_worker_support.joint_location import (
    LOCATION_FIELDS,
    combined_diagnostics,
    diagnostics_complete,
    finite,
    parse_csv,
    sample_model,
    validate_request_header,
    verified_csv,
)
from marklab_worker_support.posterior import summary, two_sided_tail
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
    backend_identity,
    derive_seed,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    worker_digests,
    write_result,
)

REQUEST_FORMAT = "marklab.numpyro_joint_replicated_location_mark_request"
RESULT_FORMAT = "marklab.numpyro_joint_replicated_location_mark_result"
MARK_FIELDS = ("pattern_id", "patient_id", "group", "point_id", "x_um", "y_um", "type_id")
REQUEST_FIELDS = {
    "format",
    "version",
    "backend",
    "jax_version",
    "location_csv",
    "mark_csv",
    "location_sha256",
    "mark_sha256",
    "reference_group",
    "comparison_group",
    "reference_type",
    "neighbor_radius_um",
    "priors",
    "sampling",
    "resources",
}
PRIOR_FIELDS = {
    "location_sd",
    "group_sd",
    "patient_ecology_scale",
    "mark_intercept_sd",
    "mark_group_sd",
    "mark_loading_sd",
    "interaction_sd",
}
RESOURCE_FIELDS = {
    "maximum_patients",
    "maximum_patterns",
    "maximum_types",
    "maximum_location_rows",
    "maximum_mark_points",
    "maximum_neighbor_visits",
    "neighbor_visits",
    "maximum_draw_observation_work",
    "draw_observation_work",
    "maximum_working_bytes",
    "estimated_working_bytes",
    "maximum_tree_depth",
    "timeout_seconds",
    "maximum_output_bytes",
}


def rows(text: str, fields: tuple[str, ...], name: str) -> list[dict[str, str]]:
    header, result = parse_csv(text, name)
    if header != fields:
        raise ContractError(f"{name} header differs")
    return result


def validate(request: dict[str, Any], lock_sha: str, worker_sha: str) -> dict[str, Any]:
    validate_request_header(
        request,
        fields=REQUEST_FIELDS,
        request_format=REQUEST_FORMAT,
        version=1,
        lock_sha=lock_sha,
        worker_sha=worker_sha,
    )
    location = rows(verified_csv(request, "location"), LOCATION_FIELDS, "location")
    marks = rows(verified_csv(request, "mark"), MARK_FIELDS, "mark")
    reference = request["reference_group"]
    comparison = request["comparison_group"]
    identities = (reference, comparison, request["reference_type"])
    if not all(isinstance(value, str) and value and value.strip() == value for value in identities):
        raise ContractError("group or type identity differs")
    if reference == comparison:
        raise ContractError("groups must differ")
    radius = float(request["neighbor_radius_um"])
    if not math.isfinite(radius) or radius <= 0.0:
        raise ContractError("neighbor radius differs")
    priors = request["priors"]
    if set(priors) != PRIOR_FIELDS or any(
        not math.isfinite(float(value)) or float(value) <= 0.0 for value in priors.values()
    ):
        raise ContractError("prior controls differ")
    sampling = request["sampling"]
    if set(sampling) != {"chains", "tune", "draws", "target_accept", "seed"}:
        raise ContractError("sampling fields differ")
    chains = int(sampling["chains"])
    tune = int(sampling["tune"])
    draws = int(sampling["draws"])
    if not 2 <= chains <= 8 or not 100 <= tune <= 100000 or not 100 <= draws <= 100000:
        raise ContractError("sampling bounds differ")
    if not 0.5 <= float(sampling["target_accept"]) < 1.0 or not 0 <= int(sampling["seed"]) < 2**64:
        raise ContractError("sampling controls differ")
    resources = request["resources"]
    if set(resources) != RESOURCE_FIELDS:
        raise ContractError("resource fields differ")

    patient_group: dict[str, str] = {}
    pattern_patient: dict[str, str] = {}
    pattern_group: dict[str, str] = {}
    types: set[str] = set()
    location_keys: set[tuple[str, str, str]] = set()
    for row in location:
        patient = row["patient_id"]
        pattern = row["pattern_id"]
        group = row["group"]
        kind = row["type_id"]
        if group not in (reference, comparison) or not patient or not pattern or not kind:
            raise ContractError("location identity differs")
        if patient in patient_group and patient_group[patient] != group:
            raise ContractError("patient group changes")
        if pattern in pattern_patient and pattern_patient[pattern] != patient:
            raise ContractError("pattern patient changes")
        patient_group[patient] = group
        pattern_patient[pattern] = patient
        pattern_group[pattern] = group
        types.add(kind)
        key = (pattern, row["node_id"], kind)
        if key in location_keys:
            raise ContractError("duplicate location node/type row")
        location_keys.add(key)
        for name in ("x_um", "y_um", "weight_um2", "window_area_um2", "covariate"):
            finite(row[name], f"location.{name}")
        if int(row["count"]) < 0 or finite(row["weight_um2"], "weight") <= 0.0:
            raise ContractError("location count or weight differs")
    if request["reference_type"] not in types:
        raise ContractError("reference type absent")
    type_ids = sorted(types)
    if type_ids[0] != request["reference_type"]:
        type_ids.remove(request["reference_type"])
        type_ids.insert(0, request["reference_type"])

    point_ids: set[str] = set()
    for row in marks:
        if row["point_id"] in point_ids:
            raise ContractError("duplicate point identity")
        point_ids.add(row["point_id"])
        if (
            pattern_patient.get(row["pattern_id"]) != row["patient_id"]
            or pattern_group.get(row["pattern_id"]) != row["group"]
        ):
            raise ContractError("mark/location hierarchy differs")
        if row["type_id"] not in types:
            raise ContractError("mark type differs")
        finite(row["x_um"], "mark.x")
        finite(row["y_um"], "mark.y")
    patients = sorted(patient_group)
    patterns = sorted(pattern_patient)
    if set(patient_group.values()) != {reference, comparison}:
        raise ContractError("both groups are required")
    patient_patterns = {
        patient: sorted(pattern for pattern, owner in pattern_patient.items() if owner == patient)
        for patient in patients
    }
    if any(len(value) != 2 for value in patient_patterns.values()):
        raise ContractError("each patient requires exactly two patterns for fixed holdout")
    train = {value[0] for value in patient_patterns.values()}
    heldout = {value[1] for value in patient_patterns.values()}
    if (
        len(patients) > int(resources["maximum_patients"])
        or len(patterns) > int(resources["maximum_patterns"])
        or len(type_ids) > int(resources["maximum_types"])
        or len(location) > int(resources["maximum_location_rows"])
        or len(marks) > int(resources["maximum_mark_points"])
    ):
        raise ContractError("row/resource ceiling exceeded")
    neighbor_visits = 0
    for pattern in patterns:
        count = sum(row["pattern_id"] == pattern for row in marks)
        neighbor_visits += count * (count - 1)
    if neighbor_visits != int(resources["neighbor_visits"]) or neighbor_visits > int(
        resources["maximum_neighbor_visits"]
    ):
        raise ContractError("neighbor work differs")
    work = chains * draws * (len(location) + len(marks)) * 3
    if work != int(resources["draw_observation_work"]) or work > int(
        resources["maximum_draw_observation_work"]
    ):
        raise ContractError("draw-observation work differs")
    estimated = len(location) * 256 + len(marks) * 512 + neighbor_visits * 16 + 384 * 1024**2
    if estimated != int(resources["estimated_working_bytes"]) or estimated > int(
        resources["maximum_working_bytes"]
    ):
        raise ContractError("memory estimate differs")
    depth = int(resources["maximum_tree_depth"])
    if not 10 <= depth <= 14:
        raise ContractError("tree depth differs")
    return {
        "request": request,
        "location": location,
        "marks": marks,
        "patients": patients,
        "patterns": patterns,
        "patient_group": patient_group,
        "pattern_patient": pattern_patient,
        "type_ids": type_ids,
        "train": train,
        "heldout": heldout,
        "chains": chains,
        "tune": tune,
        "draws": draws,
        "target_accept": float(sampling["target_accept"]),
        "seed": int(sampling["seed"]),
        "depth": depth,
    }


def seed_for(seed: int, purpose: str) -> int:
    return derive_seed("marklab-joint-location-mark-v1", seed, purpose)


def prepare_arrays(config: dict[str, Any]) -> dict[str, Any]:
    """Aggregate location counts per node and build radius-neighbour mark features.

    A mark's features are the type proportions among its neighbours within
    ``neighbor_radius_um`` (zero when it has none); ``edges`` keeps the neighbour pairs.
    """
    patient_index = {value: index for index, value in enumerate(config["patients"])}
    type_index = {value: index for index, value in enumerate(config["type_ids"])}
    comparison = config["request"]["comparison_group"]
    groups = np.asarray(
        [config["patient_group"][patient] == comparison for patient in config["patients"]],
        dtype=float,
    )
    nodes: dict[tuple[str, str], dict[str, Any]] = {}
    for row in config["location"]:
        key = (row["pattern_id"], row["node_id"])
        if key not in nodes:
            nodes[key] = {
                "pattern": row["pattern_id"],
                "patient": row["patient_id"],
                "weight": finite(row["weight_um2"], "weight"),
                "covariate": finite(row["covariate"], "covariate"),
                "count": 0,
            }
        nodes[key]["count"] += int(row["count"])
    ordered_nodes = [nodes[key] for key in sorted(nodes)]
    result: dict[str, Any] = {"patient_groups": groups}
    splits = (("train", config["train"]), ("heldout", config["heldout"]))
    for split, selected_patterns in splits:
        selected = [row for row in ordered_nodes if row["pattern"] in selected_patterns]
        result[f"loc_{split}_patient"] = np.asarray(
            [patient_index[row["patient"]] for row in selected], dtype=np.int64
        )
        result[f"loc_{split}_weight"] = np.asarray([row["weight"] for row in selected], dtype=float)
        result[f"loc_{split}_covariate"] = np.asarray(
            [row["covariate"] for row in selected], dtype=float
        )
        result[f"loc_{split}_count"] = np.asarray(
            [row["count"] for row in selected], dtype=np.int64
        )

    mark_rows: list[dict[str, Any]] = []
    edges: dict[str, list[tuple[int, int]]] = {pattern: [] for pattern in config["patterns"]}
    radius2 = float(config["request"]["neighbor_radius_um"]) ** 2
    for pattern in config["patterns"]:
        pattern_rows = [row for row in config["marks"] if row["pattern_id"] == pattern]
        xy = np.asarray(
            [[finite(row["x_um"], "x"), finite(row["y_um"], "y")] for row in pattern_rows]
        )
        kinds = np.asarray([type_index[row["type_id"]] for row in pattern_rows], dtype=np.int64)
        features = np.zeros((len(pattern_rows), len(type_index)), dtype=float)
        for left in range(len(pattern_rows)):
            for right in range(left + 1, len(pattern_rows)):
                if float(np.square(xy[left] - xy[right]).sum()) <= radius2:
                    features[left, kinds[right]] += 1.0
                    features[right, kinds[left]] += 1.0
                    edges[pattern].append((left, right))
        degree = features.sum(axis=1)
        features /= np.maximum(degree[:, None], 1.0)
        for index, row in enumerate(pattern_rows):
            mark_rows.append(
                {
                    "pattern": pattern,
                    "patient": row["patient_id"],
                    "type": kinds[index],
                    "features": features[index],
                    "local_index": index,
                }
            )
    for split, selected_patterns in splits:
        selected = [row for row in mark_rows if row["pattern"] in selected_patterns]
        result[f"mark_{split}_patient"] = np.asarray(
            [patient_index[row["patient"]] for row in selected], dtype=np.int64
        )
        result[f"mark_{split}_type"] = np.asarray([row["type"] for row in selected], dtype=np.int64)
        result[f"mark_{split}_features"] = np.asarray(
            [row["features"] for row in selected], dtype=float
        )
        result[f"mark_{split}_patterns"] = [row["pattern"] for row in selected]
        result[f"mark_{split}_local"] = np.asarray(
            [row["local_index"] for row in selected], dtype=np.int64
        )
    result["edges"] = edges
    return result


def training_location(data: dict[str, Any]) -> tuple[Any, Any, Any, Any]:
    return tuple(
        jnp.asarray(data[f"loc_train_{name}"])
        for name in ("patient", "weight", "covariate", "count")
    )


def training_marks(data: dict[str, Any]) -> tuple[Any, Any, Any]:
    return tuple(
        jnp.asarray(data[f"mark_train_{name}"]) for name in ("patient", "type", "features")
    )


def joint_model(config: dict[str, Any], data: dict[str, Any]) -> Callable[[], None]:
    """Joint location/mark model sharing one patient ecology effect.

    Locations: ``count ~ Poisson(weight * exp(intercept + group_effect * group + beta *
    covariate + ecology[patient]))``. Marks: ``type ~ Categorical(logits)`` with reference type 0
    and non-reference logits ``mark_intercept + mark_group * group + mark_loading *
    ecology[patient] + features @ interaction.T``. Priors: ``ecology = ecology_sd * raw`` with
    ``raw ~ Normal(0, 1)`` and ``ecology_sd ~ HalfNormal(patient_ecology_scale)``; location
    intercept and covariate ``~ Normal(0, location_sd)``; location group ``~ Normal(0,
    group_sd)``; mark intercepts, group effects, loadings, and interactions ``~ Normal(0, ...)``
    with their own prior SDs.
    """
    priors = config["request"]["priors"]
    patients = len(config["patients"])
    types = len(config["type_ids"])
    group = jnp.asarray(data["patient_groups"])
    location_patient, weight, covariate, count = training_location(data)
    mark_patient, mark_type, features = training_marks(data)

    def model() -> None:
        ecology_sd = numpyro.sample(
            "patient_ecology_sd", dist.HalfNormal(priors["patient_ecology_scale"])
        )
        ecology_raw = numpyro.sample(
            "patient_ecology_raw", dist.Normal(0, 1).expand([patients]).to_event(1)
        )
        ecology = ecology_raw * ecology_sd
        location_intercept = numpyro.sample(
            "location_intercept", dist.Normal(0, priors["location_sd"])
        )
        location_group = numpyro.sample("location_group", dist.Normal(0, priors["group_sd"]))
        location_covariate = numpyro.sample(
            "location_covariate", dist.Normal(0, priors["location_sd"])
        )
        mark_intercept = numpyro.sample(
            "mark_intercept",
            dist.Normal(0, priors["mark_intercept_sd"]).expand([types - 1]).to_event(1),
        )
        mark_group = numpyro.sample(
            "mark_group", dist.Normal(0, priors["mark_group_sd"]).expand([types - 1]).to_event(1)
        )
        mark_loading = numpyro.sample(
            "mark_loading",
            dist.Normal(0, priors["mark_loading_sd"]).expand([types - 1]).to_event(1),
        )
        interaction = numpyro.sample(
            "interaction",
            dist.Normal(0, priors["interaction_sd"]).expand([types - 1, types]).to_event(2),
        )
        location_log = (
            jnp.log(weight)
            + location_intercept
            + location_group * group[location_patient]
            + location_covariate * covariate
            + ecology[location_patient]
        )
        numpyro.sample("location_count", dist.Poisson(jnp.exp(location_log)), obs=count)
        nonreference = (
            mark_intercept
            + mark_group * group[mark_patient, None]
            + mark_loading * ecology[mark_patient, None]
            + features @ interaction.T
        )
        logits = jnp.concatenate([jnp.zeros((len(mark_patient), 1)), nonreference], axis=1)
        numpyro.sample("mark_type", dist.Categorical(logits=logits), obs=mark_type)

    return model


def location_model(config: dict[str, Any], data: dict[str, Any]) -> Callable[[], None]:
    """Location-only baseline: the joint model's Poisson count likelihood alone.

    ``count ~ Poisson(weight * exp(intercept + group_effect * group + beta * covariate +
    effect[patient]))`` with ``effect = sd * raw``, ``raw ~ Normal(0, 1)``,
    ``sd ~ HalfNormal(patient_ecology_scale)`` and the joint model's coefficient priors.
    """
    priors = config["request"]["priors"]
    patients = len(config["patients"])
    group = jnp.asarray(data["patient_groups"])
    location_patient, weight, covariate, count = training_location(data)

    def model() -> None:
        sd = numpyro.sample("location_patient_sd", dist.HalfNormal(priors["patient_ecology_scale"]))
        raw = numpyro.sample(
            "location_patient_raw", dist.Normal(0, 1).expand([patients]).to_event(1)
        )
        effect = raw * sd
        intercept = numpyro.sample("location_intercept", dist.Normal(0, priors["location_sd"]))
        beta_group = numpyro.sample("location_group", dist.Normal(0, priors["group_sd"]))
        beta_covariate = numpyro.sample("location_covariate", dist.Normal(0, priors["location_sd"]))
        log_expected = (
            jnp.log(weight)
            + intercept
            + beta_group * group[location_patient]
            + beta_covariate * covariate
            + effect[location_patient]
        )
        numpyro.sample("location_count", dist.Poisson(jnp.exp(log_expected)), obs=count)

    return model


def mark_model(config: dict[str, Any], data: dict[str, Any]) -> Callable[[], None]:
    """Conditional-mark-only baseline with independent patient effects per non-reference type.

    ``type ~ Categorical(logits)`` with reference type 0 and non-reference logits
    ``intercept + group_effect * group + sd * raw[patient] + features @ interaction.T``;
    ``sd ~ HalfNormal(patient_ecology_scale)`` per type, ``raw ~ Normal(0, 1)``, and the joint
    model's coefficient priors.
    """
    priors = config["request"]["priors"]
    patients = len(config["patients"])
    types = len(config["type_ids"])
    group = jnp.asarray(data["patient_groups"])
    mark_patient, mark_type, features = training_marks(data)

    def model() -> None:
        sd = numpyro.sample(
            "mark_patient_sd",
            dist.HalfNormal(priors["patient_ecology_scale"]).expand([types - 1]).to_event(1),
        )
        raw = numpyro.sample(
            "mark_patient_raw", dist.Normal(0, 1).expand([patients, types - 1]).to_event(2)
        )
        intercept = numpyro.sample(
            "mark_intercept",
            dist.Normal(0, priors["mark_intercept_sd"]).expand([types - 1]).to_event(1),
        )
        beta_group = numpyro.sample(
            "mark_group", dist.Normal(0, priors["mark_group_sd"]).expand([types - 1]).to_event(1)
        )
        interaction = numpyro.sample(
            "interaction",
            dist.Normal(0, priors["interaction_sd"]).expand([types - 1, types]).to_event(2),
        )
        nonreference = (
            intercept
            + beta_group * group[mark_patient, None]
            + sd * raw[mark_patient]
            + features @ interaction.T
        )
        logits = jnp.concatenate([jnp.zeros((len(mark_patient), 1)), nonreference], axis=1)
        numpyro.sample("mark_type", dist.Categorical(logits=logits), obs=mark_type)

    return model


def log_poisson(y: np.ndarray, log_mu: np.ndarray) -> np.ndarray:
    """Per-draw Poisson log likelihood of counts ``y`` under ``(draw, node)`` log means."""
    log_factorial = np.vectorize(math.lgamma)(y + 1)[None, :]
    return (y[None, :] * log_mu - np.exp(log_mu) - log_factorial).sum(axis=1)


def log_marks(y: np.ndarray, logits: np.ndarray) -> np.ndarray:
    """Per-draw categorical log likelihood of types ``y`` under ``(draw, point, type)`` logits."""
    maximum = logits.max(axis=2, keepdims=True)
    log_normalizer = np.log(np.exp(logits - maximum).sum(axis=2, keepdims=True))
    log_probability = logits - maximum - log_normalizer
    return log_probability[:, np.arange(len(y)), y].sum(axis=1)


def _summary(values: np.ndarray) -> dict[str, float]:
    return summary(values, dtype=float)


def evaluate(
    config: dict[str, Any],
    data: dict[str, Any],
    joint: dict[str, np.ndarray],
    location: dict[str, np.ndarray],
    mark: dict[str, np.ndarray],
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Score held-out patterns under the joint and separate models; run predictive checks."""
    group = data["patient_groups"]
    location_patient = data["loc_heldout_patient"]
    weight = data["loc_heldout_weight"]
    covariate = data["loc_heldout_covariate"]
    count = data["loc_heldout_count"]
    mark_patient = data["mark_heldout_patient"]
    mark_type = data["mark_heldout_type"]
    features = data["mark_heldout_features"]
    log_weight = np.log(weight)[None, :]
    location_group = group[location_patient][None, :]
    mark_group = group[mark_patient][None, :, None]

    ecology = joint["patient_ecology_raw"] * joint["patient_ecology_sd"][:, None]
    joint_location_log = (
        log_weight
        + joint["location_intercept"][:, None]
        + joint["location_group"][:, None] * location_group
        + joint["location_covariate"][:, None] * covariate[None, :]
        + ecology[:, location_patient]
    )
    joint_nonreference = (
        joint["mark_intercept"][:, None, :]
        + joint["mark_group"][:, None, :] * mark_group
        + joint["mark_loading"][:, None, :] * ecology[:, mark_patient, None]
        + np.einsum("nf,dkf->dnk", features, joint["interaction"])
    )
    joint_reference = np.zeros((len(joint_nonreference), len(mark_patient), 1))
    joint_logits = np.concatenate([joint_reference, joint_nonreference], axis=2)

    location_effect = location["location_patient_raw"] * location["location_patient_sd"][:, None]
    baseline_location_log = (
        log_weight
        + location["location_intercept"][:, None]
        + location["location_group"][:, None] * location_group
        + location["location_covariate"][:, None] * covariate[None, :]
        + location_effect[:, location_patient]
    )
    mark_effect = mark["mark_patient_raw"] * mark["mark_patient_sd"][:, None, :]
    baseline_nonreference = (
        mark["mark_intercept"][:, None, :]
        + mark["mark_group"][:, None, :] * mark_group
        + mark_effect[:, mark_patient, :]
        + np.einsum("nf,dkf->dnk", features, mark["interaction"])
    )
    baseline_reference = np.zeros((len(baseline_nonreference), len(mark_patient), 1))
    baseline_logits = np.concatenate([baseline_reference, baseline_nonreference], axis=2)

    joint_score = log_poisson(count, joint_location_log) + log_marks(mark_type, joint_logits)
    location_score = log_poisson(count, baseline_location_log)
    mark_score = log_marks(mark_type, baseline_logits)
    heldout = {
        "joint": {
            "mean_log_predictive_density": float(joint_score.mean()),
            "uncertainty": _summary(joint_score),
        },
        "location_only": {
            "mean_log_predictive_density": float(location_score.mean()),
            "uncertainty": _summary(location_score),
        },
        "conditional_mark_only": {
            "mean_log_predictive_density": float(mark_score.mean()),
            "uncertainty": _summary(mark_score),
        },
        "joint_minus_separate_baselines": _summary(joint_score - location_score - mark_score),
    }

    rng = np.random.default_rng(seed_for(config["seed"], "heldout_ppc"))
    replicated_counts = rng.poisson(np.exp(joint_location_log))
    probabilities = np.exp(joint_logits - joint_logits.max(axis=2, keepdims=True))
    probabilities /= probabilities.sum(axis=2, keepdims=True)
    types = len(config["type_ids"])
    replicated_marks = np.asarray(
        [
            [rng.choice(types, p=probabilities[draw, row]) for row in range(len(mark_patient))]
            for draw in range(len(probabilities))
        ]
    )
    observed_count = float(count.sum())
    count_values = replicated_counts.sum(axis=1).astype(float)
    observed_proportion = float(np.mean(mark_type == 0))
    proportion_values = np.mean(replicated_marks == 0, axis=1)

    def enrichment(values: np.ndarray) -> float:
        """Fraction of held-out neighbour edges joining type 0 and type 1."""
        total, cross = 0, 0
        offset = 0
        for pattern in sorted(config["heldout"]):
            size = sum(value == pattern for value in data["mark_heldout_patterns"])
            for left, right in data["edges"][pattern]:
                total += 1
                endpoint_types = {int(values[offset + left]), int(values[offset + right])}
                cross += int(endpoint_types == {0, 1})
            offset += size
        return cross / total if total else 0.0

    observed_enrichment = enrichment(mark_type)
    enrichment_values = np.asarray([enrichment(row) for row in replicated_marks])
    ppc = {
        "total_count": {
            "observed": observed_count,
            "replicated_mean": float(count_values.mean()),
            "two_sided_tail_probability": two_sided_tail(count_values, observed_count),
        },
        "mark_proportions": {
            "observed": observed_proportion,
            "replicated_mean": float(proportion_values.mean()),
            "two_sided_tail_probability": two_sided_tail(proportion_values, observed_proportion),
        },
        "cross_type_enrichment": {
            "observed": observed_enrichment,
            "replicated_mean": float(enrichment_values.mean()),
            "two_sided_tail_probability": two_sided_tail(enrichment_values, observed_enrichment),
        },
    }
    return heldout, ppc


def main() -> int:
    require_versions(numpyro=numpyro.__version__, jax=jax.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(32 * MIB)
    config = validate(json.loads(raw), lock_sha, worker_sha)
    data = prepare_arrays(config)
    seed = config["seed"]
    joint, joint_diagnostics = sample_model(
        config, joint_model(config, data), seed_for(seed, "joint")
    )
    location, location_diagnostics = sample_model(
        config, location_model(config, data), seed_for(seed, "location")
    )
    mark, mark_diagnostics = sample_model(config, mark_model(config, data), seed_for(seed, "mark"))
    heldout, ppc = evaluate(config, data, joint, location, mark)
    diagnostics = combined_diagnostics(joint_diagnostics, location_diagnostics, mark_diagnostics)
    complete = diagnostics_complete(diagnostics)
    mark_loadings = [
        _summary(joint["mark_loading"][:, index]) for index in range(len(config["type_ids"]) - 1)
    ]
    result = {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": backend_identity("numpyro", numpyro.__version__, lock_sha, worker_sha),
        "jax_version": jax.__version__,
        "request_sha256": sha256_hex(raw),
        "location_sha256": config["request"]["location_sha256"],
        "mark_sha256": config["request"]["mark_sha256"],
        "patient_count": len(config["patients"]),
        "pattern_count": len(config["patterns"]),
        "training_pattern_count": len(config["train"]),
        "heldout_pattern_count": len(config["heldout"]),
        "type_ids": config["type_ids"],
        "training_pattern_ids": sorted(config["train"]),
        "heldout_pattern_ids": sorted(config["heldout"]),
        "joint_posterior": {
            "patient_ecology_sd": _summary(joint["patient_ecology_sd"]),
            "mark_loadings": mark_loadings,
        },
        "heldout_comparison": heldout,
        "posterior_predictive": ppc,
        "diagnostics": diagnostics,
        "fit_state": "complete" if complete else "nonconverged",
    }
    write_result(result, max_bytes=int(config["request"]["resources"]["maximum_output_bytes"]))
    return 0


if __name__ == "__main__":
    run_worker(main, "marklab joint location-mark failed")
