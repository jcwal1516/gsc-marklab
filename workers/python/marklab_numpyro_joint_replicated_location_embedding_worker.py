#!/usr/bin/env python3
"""Patient-replicated joint exact-window location and projected-embedding model."""

from __future__ import annotations

import csv
import hashlib
import io
import json
import math
from pathlib import Path
import sys
from typing import Any, Callable

import arviz as az
import jax

jax.config.update("jax_enable_x64", True)
import jax.numpy as jnp
import numpy as np
import numpyro
import numpyro.distributions as dist
from numpyro.infer import MCMC, NUTS


REQUEST_FORMAT = "marklab.numpyro_joint_replicated_location_embedding_request"
RESULT_FORMAT = "marklab.numpyro_joint_replicated_location_embedding_result"
NUMPYRO_VERSION = "0.21.0"
JAX_VERSION = "0.11.1"
LOCATION_FIELDS = (
    "pattern_id",
    "patient_id",
    "group",
    "cohort",
    "node_id",
    "type_id",
    "x_um",
    "y_um",
    "weight_um2",
    "window_area_um2",
    "covariate",
    "count",
    "window_sha256",
    "event_sha256",
)
EMBEDDING_PREFIX = (
    "pattern_id",
    "patient_id",
    "group",
    "point_id",
    "x_um",
    "y_um",
)


class ContractError(ValueError):
    pass


def finite(value: str, name: str) -> float:
    result = float(value)
    if not math.isfinite(result):
        raise ContractError(f"{name} is nonfinite")
    return result


def parse_csv(text: str, name: str) -> tuple[tuple[str, ...], list[dict[str, str]]]:
    reader = csv.DictReader(io.StringIO(text))
    fields = tuple(reader.fieldnames or ())
    rows = list(reader)
    if not fields or not rows:
        raise ContractError(f"{name} is empty")
    return fields, rows


def validate(request: dict[str, Any], lock_sha: str, worker_sha: str) -> dict[str, Any]:
    expected = {
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
        "priors",
        "sampling",
        "resources",
    }
    if not isinstance(request, dict) or set(request) != expected:
        raise ContractError("request fields differ")
    if request["format"] != REQUEST_FORMAT or request["version"] != 1:
        raise ContractError("request identity differs")
    if request["backend"] != {
        "name": "numpyro",
        "version": NUMPYRO_VERSION,
        "python_version": "3.12",
        "environment_lock_sha256": lock_sha,
        "worker_sha256": worker_sha,
    } or request["jax_version"] != JAX_VERSION:
        raise ContractError("backend identity differs")
    location_text, embedding_text = request["location_csv"], request["embedding_csv"]
    if not isinstance(location_text, str) or not isinstance(embedding_text, str):
        raise ContractError("CSV payload differs")
    if hashlib.sha256(location_text.encode()).hexdigest() != request["location_sha256"]:
        raise ContractError("location digest differs")
    if hashlib.sha256(embedding_text.encode()).hexdigest() != request["embedding_sha256"]:
        raise ContractError("embedding digest differs")
    projection = request["embedding_projection_identity"]
    if (
        not isinstance(projection, str)
        or len(projection) != 64
        or any(character not in "0123456789abcdefABCDEF" for character in projection)
    ):
        raise ContractError("projection identity differs")
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
        not all(isinstance(value, str) and value and value.strip() == value for value in (reference, comparison))
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
        patient_group[patient], pattern_patient[pattern], pattern_group[pattern] = group, patient, group
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
    estimated = len(location_rows) * 256 + len(embeddings) * dimension * 32 + nearest_visits * 16 + 384 * 1024**2
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
    }


def seed_for(seed: int, purpose: str) -> int:
    digest = hashlib.sha256(
        f"marklab-joint-location-embedding-v1\0{seed}\0{purpose}".encode()
    ).digest()
    return int.from_bytes(digest[:4], "little")


def prepare_arrays(config: dict[str, Any]) -> dict[str, Any]:
    patient_index = {value: index for index, value in enumerate(config["patients"])}
    pattern_index = {value: index for index, value in enumerate(config["patterns"])}
    nodes = config["nodes"]
    arrays: dict[str, Any] = {
        "patient_groups": np.asarray(
            [
                config["patient_group"][patient]
                == config["request"]["comparison_group"]
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
            (index, row)
            for index, row in enumerate(nodes)
            if row["pattern"] == point["pattern"]
        ]
        nearest.append(
            min(
                candidates,
                key=lambda item: (
                    (item[1]["x"] - point["x"]) ** 2
                    + (item[1]["y"] - point["y"]) ** 2,
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
        distances.append(
            np.linalg.norm(coordinates[:, None, :] - coordinates[None, :, :], axis=2)
        )
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


def joint_model(config: dict[str, Any], data: dict[str, Any]) -> Callable[[], None]:
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
            dist.Normal(0.0, 1.0)
            .expand([factors, len(data["node_count"])])
            .to_event(2),
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
        location_group = numpyro.sample(
            "location_group", dist.Normal(0.0, priors["group_sd"])
        )
        location_covariate = numpyro.sample(
            "location_covariate", dist.Normal(0.0, priors["location_sd"])
        )
        location_loading = numpyro.sample(
            "location_factor_loading",
            dist.Normal(0.0, priors["location_factor_loading_sd"])
            .expand([factors])
            .to_event(1),
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
            dist.HalfNormal(priors["embedding_noise_scale"])
            .expand([dimension])
            .to_event(1),
        )
        point_factor = spatial[:, enearest].T + patient_factor[epatient]
        point_mean = embedding_mean[None, :] + point_factor @ loading.T
        numpyro.sample(
            "embedding_value",
            dist.Normal(point_mean[etrain], noise).to_event(1),
            obs=evalues[etrain],
        )
    return model


def nonspatial_model(config: dict[str, Any], data: dict[str, Any]) -> Callable[[], None]:
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
            dist.HalfNormal(priors["patient_factor_scale"])
            .expand([dimension])
            .to_event(1),
        )
        patient_raw = numpyro.sample(
            "embedding_patient_raw",
            dist.Normal(0.0, 1.0).expand([patients, dimension]).to_event(2),
        )
        noise = numpyro.sample(
            "embedding_noise",
            dist.HalfNormal(priors["embedding_noise_scale"])
            .expand([dimension])
            .to_event(1),
        )
        mean = intercept[None, :] + patient_sd[None, :] * patient_raw[epatient]
        numpyro.sample(
            "embedding_value", dist.Normal(mean[etrain], noise).to_event(1), obs=evalues[etrain]
        )

    return model


def sample(
    config: dict[str, Any], model: Callable[[], None], purpose: str, diagnostic_names: list[str]
) -> tuple[dict[str, np.ndarray], dict[str, float | int]]:
    sampler = MCMC(
        NUTS(
            model,
            target_accept_prob=config["target_accept"],
            max_tree_depth=config["depth"],
        ),
        num_warmup=config["tune"],
        num_samples=config["draws"],
        num_chains=config["chains"],
        chain_method="sequential",
        progress_bar=False,
    )
    sampler.run(
        jax.random.PRNGKey(seed_for(config["seed"], purpose)),
        extra_fields=("diverging", "num_steps", "energy"),
    )
    samples = {
        name: np.asarray(value, dtype=float)
        for name, value in sampler.get_samples(group_by_chain=True).items()
    }
    posterior = az.from_dict(
        {"posterior": {name: samples[name] for name in diagnostic_names}}
    )
    flatten = lambda tree: np.concatenate(
        [np.asarray(tree[name].values).reshape(-1) for name in diagnostic_names]
    )
    extra = sampler.get_extra_fields(group_by_chain=True)
    energy = np.asarray(extra["energy"], dtype=float)
    diagnostics = {
        "r_hat": float(flatten(az.rhat(posterior, var_names=diagnostic_names, method="rank")).max()),
        "ess_bulk": float(flatten(az.ess(posterior, var_names=diagnostic_names, method="bulk")).min()),
        "ess_tail": float(flatten(az.ess(posterior, var_names=diagnostic_names, method="tail")).min()),
        "minimum_ebfmi": float(
            np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1))
        ),
        "divergences": int(np.asarray(extra["diverging"]).sum()),
        "max_tree_depth_hits": int(
            (
                np.asarray(extra["num_steps"])
                >= 2 ** config["depth"] - 1
            ).sum()
        ),
    }
    return {
        name: value.reshape((-1, *value.shape[2:])) for name, value in samples.items()
    }, diagnostics


def summary(values: np.ndarray) -> dict[str, float]:
    values = np.asarray(values, dtype=float).reshape(-1)
    return {
        "mean": float(values.mean()),
        "sd": float(values.std(ddof=1)),
        "interval_lower": float(np.quantile(values, 0.025)),
        "interval_upper": float(np.quantile(values, 0.975)),
    }


def normal_log_density(values: np.ndarray, mean: np.ndarray, scale: np.ndarray) -> np.ndarray:
    residual = (values[None, :, :] - mean) / scale[:, None, :]
    return -0.5 * residual**2 - np.log(scale[:, None, :]) - 0.5 * math.log(2.0 * math.pi)


def log_mean_exp(values: np.ndarray, axis: int) -> np.ndarray:
    maximum = np.max(values, axis=axis, keepdims=True)
    return np.squeeze(
        maximum + np.log(np.mean(np.exp(values - maximum), axis=axis, keepdims=True)),
        axis=axis,
    )


def tail(values: np.ndarray, observed: float) -> float:
    return min(
        1.0,
        2.0
        * min(float(np.mean(values <= observed)), float(np.mean(values >= observed))),
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
    joint_point_lpd = log_mean_exp(
        normal_log_density(values, joint_mean, joint["embedding_noise"]), axis=0
    ).sum(axis=1)
    baseline_mean = baseline["embedding_intercept"][:, None, :] + (
        baseline["embedding_patient_sd"][:, None, :]
        * baseline["embedding_patient_raw"][:, patients, :]
    )
    baseline_point_lpd = log_mean_exp(
        normal_log_density(values, baseline_mean, baseline["embedding_noise"]), axis=0
    ).sum(axis=1)
    patient_scores_joint = np.asarray(
        [joint_point_lpd[patients == patient].sum() for patient in range(len(config["patients"]))]
    )
    patient_scores_baseline = np.asarray(
        [baseline_point_lpd[patients == patient].sum() for patient in range(len(config["patients"]))]
    )
    comparison = {
        "joint_spatial": {
            "mean_log_predictive_density": float(patient_scores_joint.mean()),
            "uncertainty": summary(patient_scores_joint),
        },
        "nonspatial": {
            "mean_log_predictive_density": float(patient_scores_baseline.mean()),
            "uncertainty": summary(patient_scores_baseline),
        },
        "joint_minus_nonspatial": summary(patient_scores_joint - patient_scores_baseline),
    }
    rng = np.random.default_rng(seed_for(config["seed"], "posterior_predictive"))
    expected = joint["location_expected"]
    replicated_counts = rng.poisson(expected)
    count_values = replicated_counts.sum(axis=1).astype(float)
    observed_count = float(data["node_count"].sum())
    observed_rmse_draws = np.sqrt(np.mean((values[None, :, :] - joint_mean) ** 2, axis=(1, 2)))
    replicated_embeddings = rng.normal(joint_mean, joint["embedding_noise"][:, None, :])
    replicated_rmse = np.sqrt(
        np.mean((replicated_embeddings - joint_mean) ** 2, axis=(1, 2))
    )
    observed_rmse = float(observed_rmse_draws.mean())
    ppc = {
        "total_location_count": {
            "observed": observed_count,
            "replicated_mean": float(count_values.mean()),
            "two_sided_tail_probability": tail(count_values, observed_count),
        },
        "heldout_embedding_rmse": {
            "observed": observed_rmse,
            "replicated_mean": float(replicated_rmse.mean()),
            "two_sided_tail_probability": tail(replicated_rmse, observed_rmse),
        },
    }
    return comparison, ppc


def main() -> int:
    if (
        numpyro.__version__ != NUMPYRO_VERSION
        or jax.__version__ != JAX_VERSION
        or sys.version_info[:2] != (3, 12)
    ):
        raise ContractError("backend version drift")
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    raw = sys.stdin.buffer.read(32 * 1024 * 1024 + 1)
    if not raw or len(raw) > 32 * 1024 * 1024:
        raise ContractError("request size differs")
    request_sha = hashlib.sha256(raw).hexdigest()
    config = validate(json.loads(raw), lock_sha, worker_sha)
    data = prepare_arrays(config)
    joint_names = [
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
    baseline_names = [
        "embedding_intercept",
        "embedding_patient_sd",
        "embedding_patient_raw",
        "embedding_noise",
    ]
    joint, joint_diagnostics = sample(
        config, joint_model(config, data), "joint", joint_names
    )
    baseline, baseline_diagnostics = sample(
        config, nonspatial_model(config, data), "nonspatial", baseline_names
    )
    comparison, ppc = evaluate(config, data, joint, baseline)
    diagnostics = {
        "maximum_r_hat": max(
            joint_diagnostics["r_hat"], baseline_diagnostics["r_hat"]
        ),
        "minimum_bulk_ess": min(
            joint_diagnostics["ess_bulk"], baseline_diagnostics["ess_bulk"]
        ),
        "minimum_tail_ess": min(
            joint_diagnostics["ess_tail"], baseline_diagnostics["ess_tail"]
        ),
        "minimum_ebfmi": min(
            joint_diagnostics["minimum_ebfmi"],
            baseline_diagnostics["minimum_ebfmi"],
        ),
        "divergences": int(joint_diagnostics["divergences"])
        + int(baseline_diagnostics["divergences"]),
        "max_tree_depth_hits": int(joint_diagnostics["max_tree_depth_hits"])
        + int(baseline_diagnostics["max_tree_depth_hits"]),
    }
    complete = bool(
        diagnostics["maximum_r_hat"] <= 1.01
        and diagnostics["minimum_bulk_ess"] >= 100.0
        and diagnostics["minimum_tail_ess"] >= 100.0
        and diagnostics["minimum_ebfmi"] >= 0.3
        and diagnostics["divergences"] == 0
        and diagnostics["max_tree_depth_hits"] == 0
    )
    loading_norms = np.sqrt(np.sum(joint["embedding_loading"] ** 2, axis=1))
    result = {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": {
            "name": "numpyro",
            "version": numpyro.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha,
            "worker_sha256": worker_sha,
        },
        "jax_version": jax.__version__,
        "request_sha256": request_sha,
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
                summary(joint["patient_factor_scale"][:, index])
                for index in range(config["factors"])
            ],
            "location_factor_loadings": [
                summary(joint["location_factor_loading"][:, index])
                for index in range(config["factors"])
            ],
            "embedding_loading_norms": [
                summary(loading_norms[:, index]) for index in range(config["factors"])
            ],
            "embedding_noise_scales": [
                summary(joint["embedding_noise"][:, index])
                for index in range(config["dimension"])
            ],
        },
        "heldout_embedding_comparison": comparison,
        "posterior_predictive": ppc,
        "diagnostics": diagnostics,
        "fit_state": "complete" if complete else "nonconverged",
    }
    output = json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")) + "\n"
    if len(output.encode()) > int(config["request"]["resources"]["maximum_output_bytes"]):
        raise ContractError("output ceiling exceeded")
    sys.stdout.write(output)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(
            f"marklab joint location-embedding failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
