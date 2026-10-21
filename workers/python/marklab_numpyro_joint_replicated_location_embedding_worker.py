#!/usr/bin/env python3
"""NumPyro worker for a patient-replicated joint location and projected-embedding model."""

from __future__ import annotations

import json
import math
import sys
from collections.abc import Callable
from functools import partial
from pathlib import Path
from typing import Any

import jax

jax.config.update("jax_enable_x64", True)

import jax.numpy as jnp
import numpy as np
import numpyro
import numpyro.distributions as dist

sys.path.append(str(Path(__file__).resolve().parent))

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
from marklab_worker_support.posterior import summary as posterior_summary
from marklab_worker_support.posterior import two_sided_tail
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

REQUEST_FORMAT = "marklab.numpyro_joint_replicated_location_embedding_request"
RESULT_FORMAT = "marklab.numpyro_joint_replicated_location_embedding_result"
REQUEST_FIELDS = {
    "format",
    "version",
    "backend",
    "jax_version",
    "location_csv",
    "embedding_csv",
    "location_sha256",
    "embedding_sha256",
    "reference_group",
    "comparison_group",
    "embedding_projection_identity",
    "embedding_residual",
    "priors",
    "sampling",
    "resources",
}
EMBEDDING_PREFIX = (
    "pattern_id",
    "patient_id",
    "group",
    "point_id",
    "x_um",
    "y_um",
)
JOINT_MONITORED = [
    "field_length_scale_um",
    "patient_factor_scale",
    "patient_factor_raw",
    "field_raw",
    "location_intercept",
    "location_group",
    "location_covariate",
    "location_factor_loading",
    "embedding_intercept",
    "embedding_loading_raw",
    "embedding_noise",
]
BASELINE_MONITORED = [
    "embedding_intercept",
    "embedding_patient_sd",
    "embedding_patient_raw",
    "embedding_noise",
]


summary = partial(posterior_summary, dtype=float)
"""Draw summary after conversion to float64."""


def validate(request: dict[str, Any], lock_sha: str, worker_sha: str) -> dict[str, Any]:
    validate_request_header(
        request,
        fields=REQUEST_FIELDS,
        request_format=REQUEST_FORMAT,
        version=2,
        lock_sha=lock_sha,
        worker_sha=worker_sha,
    )
    location_text = verified_csv(request, "location")
    embedding_text = verified_csv(request, "embedding")
    projection = request["embedding_projection_identity"]
    if (
        not isinstance(projection, str)
        or len(projection) != 64
        or any(character not in "0123456789abcdefABCDEF" for character in projection)
    ):
        raise ContractError("projection identity differs")
    residual = request["embedding_residual"]
    if not isinstance(residual, dict) or set(residual) != {
        "family",
        "student_t_degrees_of_freedom",
    }:
        raise ContractError("embedding residual contract differs")
    family = residual["family"]
    degrees_of_freedom = residual["student_t_degrees_of_freedom"]
    if family == "gaussian":
        if degrees_of_freedom is not None:
            raise ContractError("Gaussian residual cannot declare Student-t degrees of freedom")
    elif family == "student_t":
        if (
            not isinstance(degrees_of_freedom, (int, float))
            or not math.isfinite(float(degrees_of_freedom))
            or not 2.0 < float(degrees_of_freedom) <= 100.0
        ):
            raise ContractError("Student-t degrees of freedom must be in (2, 100]")
    else:
        raise ContractError("embedding residual family differs")
    location_fields, location_rows = parse_csv(location_text, "location")
    embedding_fields, embedding_rows = parse_csv(embedding_text, "embedding")
    if location_fields != LOCATION_FIELDS:
        raise ContractError("location header differs")
    if (
        embedding_fields[:6] != EMBEDDING_PREFIX
        or len(embedding_fields) < 8
        or any(not name.startswith("embedding_") for name in embedding_fields[6:])
        or len(set(embedding_fields)) != len(embedding_fields)
    ):
        raise ContractError("embedding header differs")
    dimension = len(embedding_fields) - 6
    reference, comparison = request["reference_group"], request["comparison_group"]
    if (
        not all(
            isinstance(value, str) and value and value.strip() == value
            for value in (reference, comparison)
        )
        or reference == comparison
    ):
        raise ContractError("group identity differs")
    priors = request["priors"]
    prior_fields = {
        "location_sd",
        "group_sd",
        "patient_factor_scale",
        "location_factor_loading_sd",
        "embedding_loading_sd",
        "embedding_noise_scale",
        "field_length_scale_um",
        "jitter",
    }
    if set(priors) != prior_fields or any(
        not math.isfinite(float(value)) or float(value) <= 0.0 for value in priors.values()
    ):
        raise ContractError("prior controls differ")
    sampling = request["sampling"]
    if set(sampling) != {"chains", "tune", "draws", "target_accept", "seed"}:
        raise ContractError("sampling fields differ")
    chains, tune, draws = int(sampling["chains"]), int(sampling["tune"]), int(sampling["draws"])
    if (
        not 2 <= chains <= 8
        or not 100 <= tune <= 100000
        or not 100 <= draws <= 100000
        or not 0.5 <= float(sampling["target_accept"]) < 1.0
        or not 0 <= int(sampling["seed"]) < 2**64
    ):
        raise ContractError("sampling controls differ")
    resources = request["resources"]
    resource_fields = {
        "maximum_patients",
        "maximum_patterns",
        "maximum_location_rows",
        "maximum_embedding_points",
        "maximum_embedding_dimension",
        "maximum_factor_count",
        "location_node_count",
        "embedding_point_count",
        "embedding_dimension",
        "factor_count",
        "nearest_node_visits",
        "maximum_nearest_node_visits",
        "kernel_cube_work",
        "maximum_kernel_cube_work",
        "draw_observation_work",
        "maximum_draw_observation_work",
        "estimated_working_bytes",
        "maximum_working_bytes",
        "maximum_tree_depth",
        "timeout_seconds",
        "maximum_output_bytes",
    }
    if set(resources) != resource_fields:
        raise ContractError("resource fields differ")
    patient_group: dict[str, str] = {}
    pattern_patient: dict[str, str] = {}
    pattern_group: dict[str, str] = {}
    nodes: dict[tuple[str, str], dict[str, Any]] = {}
    for row in location_rows:
        patient, pattern, group = row["patient_id"], row["pattern_id"], row["group"]
        if group not in (reference, comparison) or not patient or not pattern or not row["node_id"]:
            raise ContractError("location identity differs")
        if patient in patient_group and patient_group[patient] != group:
            raise ContractError("patient group changes")
        if pattern in pattern_patient and pattern_patient[pattern] != patient:
            raise ContractError("pattern patient changes")
        patient_group[patient], pattern_patient[pattern], pattern_group[pattern] = (
            group,
            patient,
            group,
        )
        key = (pattern, row["node_id"])
        values = {
            "pattern": pattern,
            "patient": patient,
            "group": group,
            "x": finite(row["x_um"], "location.x"),
            "y": finite(row["y_um"], "location.y"),
            "weight": finite(row["weight_um2"], "location.weight"),
            "covariate": finite(row["covariate"], "location.covariate"),
        }
        if values["weight"] <= 0.0 or int(row["count"]) < 0:
            raise ContractError("location count or weight differs")
        if key in nodes:
            for name in values:
                if nodes[key][name] != values[name]:
                    raise ContractError("location node metadata changes across types")
            nodes[key]["count"] += int(row["count"])
        else:
            nodes[key] = {**values, "node": row["node_id"], "count": int(row["count"])}
    if set(patient_group.values()) != {reference, comparison}:
        raise ContractError("both groups are required")
    feature_names = embedding_fields[6:]
    point_ids: set[str] = set()
    embeddings: list[dict[str, Any]] = []
    for row in embedding_rows:
        if not row["point_id"] or row["point_id"] in point_ids:
            raise ContractError("embedding point identity differs")
        point_ids.add(row["point_id"])
        if (
            pattern_patient.get(row["pattern_id"]) != row["patient_id"]
            or pattern_group.get(row["pattern_id"]) != row["group"]
        ):
            raise ContractError("embedding/location hierarchy differs")
        embeddings.append(
            {
                "pattern": row["pattern_id"],
                "patient": row["patient_id"],
                "x": finite(row["x_um"], "embedding.x"),
                "y": finite(row["y_um"], "embedding.y"),
                "values": [finite(row[name], name) for name in feature_names],
            }
        )
    patients, patterns = sorted(patient_group), sorted(pattern_patient)
    patient_patterns = {
        patient: sorted(pattern for pattern, owner in pattern_patient.items() if owner == patient)
        for patient in patients
    }
    if any(len(values) != 2 for values in patient_patterns.values()):
        raise ContractError("each patient requires exactly two patterns")
    train = {values[0] for values in patient_patterns.values()}
    heldout = {values[1] for values in patient_patterns.values()}
    if any(not any(row["pattern"] == pattern for row in embeddings) for pattern in patterns):
        raise ContractError("every pattern requires embeddings")
    node_counts = {
        pattern: sum(node["pattern"] == pattern for node in nodes.values()) for pattern in patterns
    }
    point_counts = {
        pattern: sum(row["pattern"] == pattern for row in embeddings) for pattern in patterns
    }
    nearest_visits = sum(node_counts[pattern] * point_counts[pattern] for pattern in patterns)
    kernel_work = sum(node_counts[pattern] ** 3 for pattern in patterns)
    draw_work = chains * draws * (len(nodes) + len(embeddings) * dimension) * 2
    estimated = (
        len(location_rows) * 256
        + len(embeddings) * dimension * 32
        + nearest_visits * 16
        + 384 * 1024**2
    )
    exact_resources = {
        "location_node_count": len(nodes),
        "embedding_point_count": len(embeddings),
        "embedding_dimension": dimension,
        "nearest_node_visits": nearest_visits,
        "kernel_cube_work": kernel_work,
        "draw_observation_work": draw_work,
        "estimated_working_bytes": estimated,
    }
    if any(int(resources[name]) != value for name, value in exact_resources.items()):
        raise ContractError("derived resources differ")
    factors = int(resources["factor_count"])
    if (
        len(patients) > int(resources["maximum_patients"])
        or len(patterns) > int(resources["maximum_patterns"])
        or len(location_rows) > int(resources["maximum_location_rows"])
        or len(embeddings) > int(resources["maximum_embedding_points"])
        or dimension > int(resources["maximum_embedding_dimension"])
        or not 1 <= factors <= int(resources["maximum_factor_count"])
        or factors >= dimension
        or nearest_visits > int(resources["maximum_nearest_node_visits"])
        or kernel_work > int(resources["maximum_kernel_cube_work"])
        or draw_work > int(resources["maximum_draw_observation_work"])
        or estimated > int(resources["maximum_working_bytes"])
        or not 10 <= int(resources["maximum_tree_depth"]) <= 14
    ):
        raise ContractError("resource ceiling exceeded")
    return {
        "request": request,
        "nodes": [nodes[key] for key in sorted(nodes)],
        "embeddings": embeddings,
        "patients": patients,
        "patterns": patterns,
        "patient_group": patient_group,
        "train": train,
        "heldout": heldout,
        "dimension": dimension,
        "factors": factors,
        "chains": chains,
        "tune": tune,
        "draws": draws,
        "target_accept": float(sampling["target_accept"]),
        "seed": int(sampling["seed"]),
        "depth": int(resources["maximum_tree_depth"]),
        "embedding_residual_family": family,
        "student_t_degrees_of_freedom": (
            None if degrees_of_freedom is None else float(degrees_of_freedom)
        ),
    }


def seed_for(seed: int, purpose: str) -> int:
    return derive_seed("marklab-joint-location-embedding-v1", seed, purpose)


def prepare_arrays(config: dict[str, Any]) -> dict[str, Any]:
    patient_index = {value: index for index, value in enumerate(config["patients"])}
    pattern_index = {value: index for index, value in enumerate(config["patterns"])}
    nodes = config["nodes"]
    arrays: dict[str, Any] = {
        "patient_groups": np.asarray(
            [
                config["patient_group"][patient] == config["request"]["comparison_group"]
                for patient in config["patients"]
            ],
            dtype=float,
        ),
        "node_patient": np.asarray(
            [patient_index[row["patient"]] for row in nodes], dtype=np.int64
        ),
        "node_pattern": np.asarray(
            [pattern_index[row["pattern"]] for row in nodes], dtype=np.int64
        ),
        "node_weight": np.asarray([row["weight"] for row in nodes], dtype=float),
        "node_covariate": np.asarray([row["covariate"] for row in nodes], dtype=float),
        "node_count": np.asarray([row["count"] for row in nodes], dtype=np.int64),
    }
    nearest = []
    for point in config["embeddings"]:
        candidates = [
            (index, row) for index, row in enumerate(nodes) if row["pattern"] == point["pattern"]
        ]
        nearest.append(
            min(
                candidates,
                key=lambda item: (
                    (item[1]["x"] - point["x"]) ** 2 + (item[1]["y"] - point["y"]) ** 2,
                    item[1]["node"],
                ),
            )[0]
        )
    arrays["embedding_patient"] = np.asarray(
        [patient_index[row["patient"]] for row in config["embeddings"]], dtype=np.int64
    )
    arrays["embedding_nearest"] = np.asarray(nearest, dtype=np.int64)
    arrays["embedding_values"] = np.asarray(
        [row["values"] for row in config["embeddings"]], dtype=float
    )
    arrays["embedding_train"] = np.asarray(
        [row["pattern"] in config["train"] for row in config["embeddings"]], dtype=bool
    )
    arrays["embedding_heldout"] = ~arrays["embedding_train"]
    distances, selections = [], []
    for pattern in config["patterns"]:
        selected = np.asarray(
            [index for index, row in enumerate(nodes) if row["pattern"] == pattern],
            dtype=np.int64,
        )
        coordinates = np.asarray([[nodes[index]["x"], nodes[index]["y"]] for index in selected])
        distances.append(np.linalg.norm(coordinates[:, None, :] - coordinates[None, :, :], axis=2))
        selections.append(selected)
    arrays["distances"], arrays["selections"] = distances, selections
    return arrays


def identified_loading(raw: jnp.ndarray, factors: int) -> jnp.ndarray:
    dimension = raw.shape[0]
    rows = jnp.arange(dimension)[:, None]
    columns = jnp.arange(factors)[None, :]
    loading = jnp.where((rows < factors) & (columns > rows), 0.0, raw)
    diagonal = jnp.arange(factors)
    return loading.at[diagonal, diagonal].set(jnp.exp(raw[diagonal, diagonal]))


def embedding_distribution(
    config: dict[str, Any], mean: jnp.ndarray, scale: jnp.ndarray
) -> dist.Distribution:
    if config["embedding_residual_family"] == "gaussian":
        return dist.Normal(mean, scale).to_event(1)
    return dist.StudentT(config["student_t_degrees_of_freedom"], mean, scale).to_event(1)


def joint_model(config: dict[str, Any], data: dict[str, Any]) -> Callable[[], None]:
    """Joint Poisson location and embedding model with shared patient and spatial factors.

    Location counts at quadrature nodes: ``n ~ Poisson(w * exp(a + b * group + c * covariate
    + (s + p) . lambda))`` with node weight ``w``, latent spatial factors ``s`` (unit-amplitude
    Matern-3/2 fields per pattern with ``field_length_scale_um ~ HalfNormal``, centred within
    the pattern), and patient factors ``p = scale * z`` (``scale ~ HalfNormal``,
    ``z ~ Normal(0, 1)``). Training embeddings: ``e ~ Normal`` or ``StudentT(mu, noise)`` with
    ``mu = m + L (s_nearest + p)``, a lower-triangular loading matrix ``L`` with positive
    diagonal, ``m ~ Normal(0, 2)``, and ``noise ~ HalfNormal``. Coefficient priors are normal
    with the request scales.
    """
    priors = config["request"]["priors"]
    patients, dimension, factors = len(config["patients"]), config["dimension"], config["factors"]
    group = jnp.asarray(data["patient_groups"])
    npatient = jnp.asarray(data["node_patient"])
    nweight = jnp.asarray(data["node_weight"])
    ncovariate = jnp.asarray(data["node_covariate"])
    ncount = jnp.asarray(data["node_count"])
    epatient = jnp.asarray(data["embedding_patient"])
    enearest = jnp.asarray(data["embedding_nearest"])
    evalues = jnp.asarray(data["embedding_values"])
    etrain = jnp.asarray(data["embedding_train"])
    distances = [jnp.asarray(value) for value in data["distances"]]
    selections = data["selections"]

    def model() -> None:
        length = numpyro.sample(
            "field_length_scale_um", dist.HalfNormal(priors["field_length_scale_um"])
        )
        patient_scale = numpyro.sample(
            "patient_factor_scale",
            dist.HalfNormal(priors["patient_factor_scale"]).expand([factors]).to_event(1),
        )
        patient_raw = numpyro.sample(
            "patient_factor_raw",
            dist.Normal(0.0, 1.0).expand([patients, factors]).to_event(2),
        )
        patient_factor = numpyro.deterministic(
            "patient_factor", patient_scale[None, :] * patient_raw
        )
        field_raw = numpyro.sample(
            "field_raw",
            dist.Normal(0.0, 1.0).expand([factors, len(data["node_count"])]).to_event(2),
        )
        blocks = []
        for distance, selected in zip(distances, selections, strict=True):
            scaled = math.sqrt(3.0) * distance / length
            covariance = (1.0 + scaled) * jnp.exp(-scaled)
            covariance += priors["jitter"] * jnp.eye(len(selected))
            block = field_raw[:, selected] @ jnp.linalg.cholesky(covariance).T
            blocks.append(block - block.mean(axis=1, keepdims=True))
        spatial = numpyro.deterministic("spatial_factor", jnp.concatenate(blocks, axis=1))
        location_intercept = numpyro.sample(
            "location_intercept", dist.Normal(0.0, priors["location_sd"])
        )
        location_group = numpyro.sample("location_group", dist.Normal(0.0, priors["group_sd"]))
        location_covariate = numpyro.sample(
            "location_covariate", dist.Normal(0.0, priors["location_sd"])
        )
        location_loading = numpyro.sample(
            "location_factor_loading",
            dist.Normal(0.0, priors["location_factor_loading_sd"]).expand([factors]).to_event(1),
        )
        node_factor = spatial.T + patient_factor[npatient]
        location_log = (
            jnp.log(nweight)
            + location_intercept
            + location_group * group[npatient]
            + location_covariate * ncovariate
            + node_factor @ location_loading
        )
        location_expected = numpyro.deterministic("location_expected", jnp.exp(location_log))
        numpyro.sample("location_count", dist.Poisson(location_expected), obs=ncount)
        embedding_mean = numpyro.sample(
            "embedding_intercept", dist.Normal(0.0, 2.0).expand([dimension]).to_event(1)
        )
        loading_raw = numpyro.sample(
            "embedding_loading_raw",
            dist.Normal(0.0, priors["embedding_loading_sd"])
            .expand([dimension, factors])
            .to_event(2),
        )
        loading = numpyro.deterministic(
            "embedding_loading", identified_loading(loading_raw, factors)
        )
        noise = numpyro.sample(
            "embedding_noise",
            dist.HalfNormal(priors["embedding_noise_scale"]).expand([dimension]).to_event(1),
        )
        point_factor = spatial[:, enearest].T + patient_factor[epatient]
        point_mean = embedding_mean[None, :] + point_factor @ loading.T
        numpyro.sample(
            "embedding_value",
            embedding_distribution(config, point_mean[etrain], noise),
            obs=evalues[etrain],
        )

    return model


def nonspatial_model(config: dict[str, Any], data: dict[str, Any]) -> Callable[[], None]:
    """Baseline embedding model with patient random intercepts and no spatial factors.

    Training embeddings: ``e ~ Normal`` or ``StudentT(m + sd * z_patient, noise)`` with
    ``m ~ Normal(0, 2)``, ``sd ~ HalfNormal(patient_factor_scale)``, ``z ~ Normal(0, 1)``, and
    ``noise ~ HalfNormal(embedding_noise_scale)`` per dimension.
    """
    priors = config["request"]["priors"]
    patients, dimension = len(config["patients"]), config["dimension"]
    epatient = jnp.asarray(data["embedding_patient"])
    evalues = jnp.asarray(data["embedding_values"])
    etrain = jnp.asarray(data["embedding_train"])

    def model() -> None:
        intercept = numpyro.sample(
            "embedding_intercept", dist.Normal(0.0, 2.0).expand([dimension]).to_event(1)
        )
        patient_sd = numpyro.sample(
            "embedding_patient_sd",
            dist.HalfNormal(priors["patient_factor_scale"]).expand([dimension]).to_event(1),
        )
        patient_raw = numpyro.sample(
            "embedding_patient_raw",
            dist.Normal(0.0, 1.0).expand([patients, dimension]).to_event(2),
        )
        noise = numpyro.sample(
            "embedding_noise",
            dist.HalfNormal(priors["embedding_noise_scale"]).expand([dimension]).to_event(1),
        )
        mean = intercept[None, :] + patient_sd[None, :] * patient_raw[epatient]
        numpyro.sample(
            "embedding_value",
            embedding_distribution(config, mean[etrain], noise),
            obs=evalues[etrain],
        )

    return model


def embedding_log_density(
    config: dict[str, Any], values: np.ndarray, mean: np.ndarray, scale: np.ndarray
) -> np.ndarray:
    residual = (values[None, :, :] - mean) / scale[:, None, :]
    if config["embedding_residual_family"] == "gaussian":
        return -0.5 * residual**2 - np.log(scale[:, None, :]) - 0.5 * math.log(2.0 * math.pi)
    degrees = config["student_t_degrees_of_freedom"]
    constant = (
        math.lgamma((degrees + 1.0) / 2.0)
        - math.lgamma(degrees / 2.0)
        - 0.5 * math.log(degrees * math.pi)
    )
    return (
        constant
        - np.log(scale[:, None, :])
        - 0.5 * (degrees + 1.0) * np.log1p(residual**2 / degrees)
    )


def log_mean_exp(values: np.ndarray, axis: int) -> np.ndarray:
    maximum = np.max(values, axis=axis, keepdims=True)
    return np.squeeze(
        maximum + np.log(np.mean(np.exp(values - maximum), axis=axis, keepdims=True)),
        axis=axis,
    )


def joint_vector_log_predictive_density(log_density: np.ndarray) -> np.ndarray:
    """Integrate each complete embedding vector over shared posterior draws."""
    log_density = np.asarray(log_density, dtype=float)
    if log_density.ndim != 3 or log_density.shape[0] == 0:
        raise ContractError("embedding log-density shape differs")
    return log_mean_exp(log_density.sum(axis=2), axis=0)


def paired_two_sided_tail(replicated: np.ndarray, observed: np.ndarray) -> float:
    replicated = np.asarray(replicated, dtype=float).reshape(-1)
    observed = np.asarray(observed, dtype=float).reshape(-1)
    if replicated.shape != observed.shape or replicated.size == 0:
        raise ContractError("posterior-predictive discrepancy pairing differs")
    return min(
        1.0,
        2.0
        * min(
            float(np.mean(replicated <= observed)),
            float(np.mean(replicated >= observed)),
        ),
    )


def evaluate(
    config: dict[str, Any],
    data: dict[str, Any],
    joint: dict[str, np.ndarray],
    baseline: dict[str, np.ndarray],
) -> tuple[dict[str, Any], dict[str, Any]]:
    heldout = data["embedding_heldout"]
    patients = data["embedding_patient"][heldout]
    nearest = data["embedding_nearest"][heldout]
    values = data["embedding_values"][heldout]
    patient_factor = joint["patient_factor"][:, patients, :]
    spatial_factor = joint["spatial_factor"][:, :, nearest].transpose(0, 2, 1)
    point_factor = patient_factor + spatial_factor
    joint_mean = joint["embedding_intercept"][:, None, :] + np.einsum(
        "dnk,dfk->dnf", point_factor, joint["embedding_loading"]
    )
    joint_point_lpd = joint_vector_log_predictive_density(
        embedding_log_density(config, values, joint_mean, joint["embedding_noise"])
    )
    baseline_mean = baseline["embedding_intercept"][:, None, :] + (
        baseline["embedding_patient_sd"][:, None, :]
        * baseline["embedding_patient_raw"][:, patients, :]
    )
    baseline_point_lpd = joint_vector_log_predictive_density(
        embedding_log_density(config, values, baseline_mean, baseline["embedding_noise"])
    )
    patient_scores_joint = np.asarray(
        [joint_point_lpd[patients == patient].sum() for patient in range(len(config["patients"]))]
    )
    patient_scores_baseline = np.asarray(
        [
            baseline_point_lpd[patients == patient].sum()
            for patient in range(len(config["patients"]))
        ]
    )
    heldout_point_counts = np.asarray(
        [np.count_nonzero(patients == patient) for patient in range(len(config["patients"]))],
        dtype=np.int64,
    )
    comparison = {
        "score_definition": (
            "pointwise_joint_embedding_vector_log_predictive_density_summed_within_patient"
        ),
        "score_version": 2,
        "score_unit": "heldout_embedding_point_complete_vector",
        "patient_aggregation": "sum_point_scores_within_patient_then_equal_patient_mean",
        "patient_count": len(config["patients"]),
        "heldout_embedding_point_count": int(heldout_point_counts.sum()),
        "heldout_embedding_point_counts_by_patient": heldout_point_counts.tolist(),
        "joint_spatial": {
            "mean_log_predictive_density": float(patient_scores_joint.mean()),
            "patient_score_spread": summary(patient_scores_joint),
        },
        "nonspatial": {
            "mean_log_predictive_density": float(patient_scores_baseline.mean()),
            "patient_score_spread": summary(patient_scores_baseline),
        },
        "joint_minus_nonspatial_patient_score_spread": summary(
            patient_scores_joint - patient_scores_baseline
        ),
    }
    rng = np.random.default_rng(seed_for(config["seed"], "posterior_predictive"))
    expected = joint["location_expected"]
    replicated_counts = rng.poisson(expected)
    count_values = replicated_counts.sum(axis=1).astype(float)
    observed_count = float(data["node_count"].sum())
    observed_rmse_draws = np.sqrt(np.mean((values[None, :, :] - joint_mean) ** 2, axis=(1, 2)))
    if config["embedding_residual_family"] == "gaussian":
        residual_draws = rng.normal(size=joint_mean.shape)
    else:
        residual_draws = rng.standard_t(
            config["student_t_degrees_of_freedom"], size=joint_mean.shape
        )
    replicated_embeddings = joint_mean + residual_draws * joint["embedding_noise"][:, None, :]
    replicated_rmse = np.sqrt(np.mean((replicated_embeddings - joint_mean) ** 2, axis=(1, 2)))
    observed_rmse = float(observed_rmse_draws.mean())
    ppc = {
        "total_location_count": {
            "observed": observed_count,
            "replicated_mean": float(count_values.mean()),
            "two_sided_tail_probability": two_sided_tail(count_values, observed_count),
        },
        "heldout_embedding_rmse": {
            "observed": observed_rmse,
            "replicated_mean": float(replicated_rmse.mean()),
            "two_sided_tail_probability": paired_two_sided_tail(
                replicated_rmse, observed_rmse_draws
            ),
        },
    }
    return comparison, ppc


def main() -> None:
    require_versions(numpyro=numpyro.__version__, jax=jax.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(32 * MIB)
    config = validate(json.loads(raw), lock_sha, worker_sha)
    data = prepare_arrays(config)
    joint, joint_diagnostics = sample_model(
        config, joint_model(config, data), seed_for(config["seed"], "joint"), JOINT_MONITORED
    )
    baseline, baseline_diagnostics = sample_model(
        config,
        nonspatial_model(config, data),
        seed_for(config["seed"], "nonspatial"),
        BASELINE_MONITORED,
    )
    comparison, ppc = evaluate(config, data, joint, baseline)
    diagnostics = combined_diagnostics(joint_diagnostics, baseline_diagnostics)
    complete = diagnostics_complete(diagnostics)
    loading_norms = np.sqrt(np.sum(joint["embedding_loading"] ** 2, axis=1))
    factors = range(config["factors"])
    result = {
        "format": RESULT_FORMAT,
        "version": 2,
        "backend": backend_identity("numpyro", numpyro.__version__, lock_sha, worker_sha),
        "jax_version": jax.__version__,
        "request_sha256": sha256_hex(raw),
        "location_sha256": config["request"]["location_sha256"],
        "embedding_sha256": config["request"]["embedding_sha256"],
        "patient_count": len(config["patients"]),
        "pattern_count": len(config["patterns"]),
        "training_pattern_count": len(config["train"]),
        "heldout_pattern_count": len(config["heldout"]),
        "embedding_dimension": config["dimension"],
        "factor_count": config["factors"],
        "training_pattern_ids": sorted(config["train"]),
        "heldout_pattern_ids": sorted(config["heldout"]),
        "joint_posterior": {
            "field_length_scale_um": summary(joint["field_length_scale_um"]),
            "patient_factor_scales": [
                summary(joint["patient_factor_scale"][:, index]) for index in factors
            ],
            "location_factor_loadings": [
                summary(joint["location_factor_loading"][:, index]) for index in factors
            ],
            "embedding_loading_norms": [summary(loading_norms[:, index]) for index in factors],
            "embedding_noise_scales": [
                summary(joint["embedding_noise"][:, index]) for index in range(config["dimension"])
            ],
        },
        "heldout_embedding_comparison": comparison,
        "posterior_predictive": ppc,
        "diagnostics": diagnostics,
        "fit_state": "complete" if complete else "nonconverged",
    }
    write_result(result, max_bytes=int(config["request"]["resources"]["maximum_output_bytes"]))


if __name__ == "__main__":
    run_worker(main, "marklab joint location-embedding failed")
