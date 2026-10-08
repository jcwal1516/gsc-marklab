#!/usr/bin/env python3
"""Patient-replicated joint point-location and conditional categorical-mark model."""

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


REQUEST_FORMAT = "marklab.numpyro_joint_replicated_location_mark_request"
RESULT_FORMAT = "marklab.numpyro_joint_replicated_location_mark_result"
NUMPYRO_VERSION = "0.21.0"
JAX_VERSION = "0.11.1"
LOCATION_FIELDS = (
    "pattern_id", "patient_id", "group", "cohort", "node_id", "type_id", "x_um", "y_um",
    "weight_um2", "window_area_um2", "covariate", "count", "window_sha256", "event_sha256",
)
MARK_FIELDS = ("pattern_id", "patient_id", "group", "point_id", "x_um", "y_um", "type_id")


class ContractError(ValueError):
    pass


def rows(text: str, fields: tuple[str, ...], name: str) -> list[dict[str, str]]:
    reader = csv.DictReader(io.StringIO(text))
    if tuple(reader.fieldnames or ()) != fields:
        raise ContractError(f"{name} header differs")
    result = list(reader)
    if not result:
        raise ContractError(f"{name} is empty")
    return result


def finite(value: str, name: str) -> float:
    result = float(value)
    if not math.isfinite(result):
        raise ContractError(f"{name} is nonfinite")
    return result


def validate(request: dict[str, Any], lock_sha: str, worker_sha: str) -> dict[str, Any]:
    expected = {
        "format", "version", "backend", "jax_version", "location_csv", "mark_csv",
        "location_sha256", "mark_sha256", "reference_group", "comparison_group",
        "reference_type", "neighbor_radius_um", "priors", "sampling", "resources",
    }
    if not isinstance(request, dict) or set(request) != expected:
        raise ContractError("request fields differ")
    if request["format"] != REQUEST_FORMAT or request["version"] != 1:
        raise ContractError("request identity differs")
    backend = request["backend"]
    if backend != {
        "name": "numpyro", "version": NUMPYRO_VERSION, "python_version": "3.12",
        "environment_lock_sha256": lock_sha, "worker_sha256": worker_sha,
    } or request["jax_version"] != JAX_VERSION:
        raise ContractError("backend identity differs")
    location_text = request["location_csv"]
    mark_text = request["mark_csv"]
    if not isinstance(location_text, str) or not isinstance(mark_text, str):
        raise ContractError("CSV payload differs")
    if hashlib.sha256(location_text.encode()).hexdigest() != request["location_sha256"]:
        raise ContractError("location digest differs")
    if hashlib.sha256(mark_text.encode()).hexdigest() != request["mark_sha256"]:
        raise ContractError("mark digest differs")
    location = rows(location_text, LOCATION_FIELDS, "location")
    marks = rows(mark_text, MARK_FIELDS, "mark")
    reference = request["reference_group"]
    comparison = request["comparison_group"]
    if not all(isinstance(value, str) and value and value.strip() == value for value in (reference, comparison, request["reference_type"])):
        raise ContractError("group or type identity differs")
    if reference == comparison:
        raise ContractError("groups must differ")
    radius = float(request["neighbor_radius_um"])
    if not math.isfinite(radius) or radius <= 0.0:
        raise ContractError("neighbor radius differs")
    priors = request["priors"]
    if set(priors) != {
        "location_sd", "group_sd", "patient_ecology_scale", "mark_intercept_sd",
        "mark_group_sd", "mark_loading_sd", "interaction_sd",
    } or any(not math.isfinite(float(value)) or float(value) <= 0.0 for value in priors.values()):
        raise ContractError("prior controls differ")
    sampling = request["sampling"]
    if set(sampling) != {"chains", "tune", "draws", "target_accept", "seed"}:
        raise ContractError("sampling fields differ")
    chains, tune, draws = int(sampling["chains"]), int(sampling["tune"]), int(sampling["draws"])
    if not 2 <= chains <= 8 or not 100 <= tune <= 100000 or not 100 <= draws <= 100000:
        raise ContractError("sampling bounds differ")
    if not 0.5 <= float(sampling["target_accept"]) < 1.0 or not 0 <= int(sampling["seed"]) < 2**64:
        raise ContractError("sampling controls differ")
    resources = request["resources"]
    required = {
        "maximum_patients", "maximum_patterns", "maximum_types", "maximum_location_rows",
        "maximum_mark_points", "maximum_neighbor_visits", "neighbor_visits",
        "maximum_draw_observation_work", "draw_observation_work", "maximum_working_bytes",
        "estimated_working_bytes", "maximum_tree_depth", "timeout_seconds", "maximum_output_bytes",
    }
    if set(resources) != required:
        raise ContractError("resource fields differ")

    patient_group: dict[str, str] = {}
    pattern_patient: dict[str, str] = {}
    pattern_group: dict[str, str] = {}
    types: set[str] = set()
    location_keys: set[tuple[str, str, str]] = set()
    for row in location:
        patient, pattern, group, kind = row["patient_id"], row["pattern_id"], row["group"], row["type_id"]
        if group not in (reference, comparison) or not patient or not pattern or not kind:
            raise ContractError("location identity differs")
        if patient in patient_group and patient_group[patient] != group:
            raise ContractError("patient group changes")
        if pattern in pattern_patient and pattern_patient[pattern] != patient:
            raise ContractError("pattern patient changes")
        patient_group[patient], pattern_patient[pattern], pattern_group[pattern] = group, patient, group
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
        if pattern_patient.get(row["pattern_id"]) != row["patient_id"] or pattern_group.get(row["pattern_id"]) != row["group"]:
            raise ContractError("mark/location hierarchy differs")
        if row["type_id"] not in types:
            raise ContractError("mark type differs")
        finite(row["x_um"], "mark.x")
        finite(row["y_um"], "mark.y")
    patients = sorted(patient_group)
    patterns = sorted(pattern_patient)
    if set(patient_group.values()) != {reference, comparison}:
        raise ContractError("both groups are required")
    patient_patterns = {patient: sorted(pattern for pattern, owner in pattern_patient.items() if owner == patient) for patient in patients}
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
    neighbor_visits = sum(
        count * (count - 1)
        for pattern in patterns
        for count in [sum(row["pattern_id"] == pattern for row in marks)]
    )
    if neighbor_visits != int(resources["neighbor_visits"]) or neighbor_visits > int(resources["maximum_neighbor_visits"]):
        raise ContractError("neighbor work differs")
    work = chains * draws * (len(location) + len(marks)) * 3
    if work != int(resources["draw_observation_work"]) or work > int(resources["maximum_draw_observation_work"]):
        raise ContractError("draw-observation work differs")
    estimated = (len(location) * 256 + len(marks) * 512 + neighbor_visits * 16 + 384 * 1024**2)
    if estimated != int(resources["estimated_working_bytes"]) or estimated > int(resources["maximum_working_bytes"]):
        raise ContractError("memory estimate differs")
    depth = int(resources["maximum_tree_depth"])
    if not 10 <= depth <= 14:
        raise ContractError("tree depth differs")
    return {
        "request": request, "location": location, "marks": marks, "patients": patients,
        "patterns": patterns, "patient_group": patient_group, "pattern_patient": pattern_patient,
        "type_ids": type_ids, "train": train, "heldout": heldout, "chains": chains,
        "tune": tune, "draws": draws, "target_accept": float(sampling["target_accept"]),
        "seed": int(sampling["seed"]), "depth": depth,
    }


def seed_for(seed: int, purpose: str) -> int:
    return int.from_bytes(hashlib.sha256(f"marklab-joint-location-mark-v1\0{seed}\0{purpose}".encode()).digest()[:4], "little")


def prepare_arrays(config: dict[str, Any]) -> dict[str, Any]:
    patient_index = {value: index for index, value in enumerate(config["patients"])}
    type_index = {value: index for index, value in enumerate(config["type_ids"])}
    groups = np.asarray([config["patient_group"][patient] == config["request"]["comparison_group"] for patient in config["patients"]], dtype=float)
    nodes: dict[tuple[str, str], dict[str, Any]] = {}
    for row in config["location"]:
        key = (row["pattern_id"], row["node_id"])
        node = nodes.setdefault(key, {"pattern": row["pattern_id"], "patient": row["patient_id"], "weight": finite(row["weight_um2"], "weight"), "covariate": finite(row["covariate"], "covariate"), "count": 0})
        node["count"] += int(row["count"])
    ordered_nodes = [nodes[key] for key in sorted(nodes)]
    result: dict[str, Any] = {"patient_groups": groups}
    for split, selected_patterns in (("train", config["train"]), ("heldout", config["heldout"])):
        selected = [row for row in ordered_nodes if row["pattern"] in selected_patterns]
        result[f"loc_{split}_patient"] = np.asarray([patient_index[row["patient"]] for row in selected], dtype=np.int64)
        result[f"loc_{split}_weight"] = np.asarray([row["weight"] for row in selected], dtype=float)
        result[f"loc_{split}_covariate"] = np.asarray([row["covariate"] for row in selected], dtype=float)
        result[f"loc_{split}_count"] = np.asarray([row["count"] for row in selected], dtype=np.int64)
    mark_rows: list[dict[str, Any]] = []
    edges: dict[str, list[tuple[int, int]]] = {pattern: [] for pattern in config["patterns"]}
    by_pattern = {pattern: [row for row in config["marks"] if row["pattern_id"] == pattern] for pattern in config["patterns"]}
    radius2 = float(config["request"]["neighbor_radius_um"]) ** 2
    for pattern in config["patterns"]:
        pattern_rows = by_pattern[pattern]
        xy = np.asarray([[finite(row["x_um"], "x"), finite(row["y_um"], "y")] for row in pattern_rows])
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
            mark_rows.append({"pattern": pattern, "patient": row["patient_id"], "type": kinds[index], "features": features[index], "local_index": index})
    for split, selected_patterns in (("train", config["train"]), ("heldout", config["heldout"])):
        selected = [row for row in mark_rows if row["pattern"] in selected_patterns]
        result[f"mark_{split}_patient"] = np.asarray([patient_index[row["patient"]] for row in selected], dtype=np.int64)
        result[f"mark_{split}_type"] = np.asarray([row["type"] for row in selected], dtype=np.int64)
        result[f"mark_{split}_features"] = np.asarray([row["features"] for row in selected], dtype=float)
        result[f"mark_{split}_patterns"] = [row["pattern"] for row in selected]
        result[f"mark_{split}_local"] = np.asarray([row["local_index"] for row in selected], dtype=np.int64)
    result["edges"] = edges
    return result


def joint_model(config: dict[str, Any], data: dict[str, Any]) -> Callable[[], None]:
    priors = config["request"]["priors"]
    patients, types = len(config["patients"]), len(config["type_ids"])
    group = jnp.asarray(data["patient_groups"])
    lp, lw, lc, ly = map(jnp.asarray, (data["loc_train_patient"], data["loc_train_weight"], data["loc_train_covariate"], data["loc_train_count"]))
    mp, mt, mf = map(jnp.asarray, (data["mark_train_patient"], data["mark_train_type"], data["mark_train_features"]))
    def model() -> None:
        ecology_sd = numpyro.sample("patient_ecology_sd", dist.HalfNormal(priors["patient_ecology_scale"]))
        ecology = numpyro.sample("patient_ecology_raw", dist.Normal(0, 1).expand([patients]).to_event(1)) * ecology_sd
        li = numpyro.sample("location_intercept", dist.Normal(0, priors["location_sd"]))
        lg = numpyro.sample("location_group", dist.Normal(0, priors["group_sd"]))
        lc_beta = numpyro.sample("location_covariate", dist.Normal(0, priors["location_sd"]))
        mi = numpyro.sample("mark_intercept", dist.Normal(0, priors["mark_intercept_sd"]).expand([types - 1]).to_event(1))
        mg = numpyro.sample("mark_group", dist.Normal(0, priors["mark_group_sd"]).expand([types - 1]).to_event(1))
        ml = numpyro.sample("mark_loading", dist.Normal(0, priors["mark_loading_sd"]).expand([types - 1]).to_event(1))
        interaction = numpyro.sample("interaction", dist.Normal(0, priors["interaction_sd"]).expand([types - 1, types]).to_event(2))
        loc_log = jnp.log(lw) + li + lg * group[lp] + lc_beta * lc + ecology[lp]
        numpyro.sample("location_count", dist.Poisson(jnp.exp(loc_log)), obs=ly)
        nonref = mi + mg * group[mp, None] + ml * ecology[mp, None] + mf @ interaction.T
        logits = jnp.concatenate([jnp.zeros((len(mp), 1)), nonref], axis=1)
        numpyro.sample("mark_type", dist.Categorical(logits=logits), obs=mt)
    return model


def location_model(config: dict[str, Any], data: dict[str, Any]) -> Callable[[], None]:
    priors = config["request"]["priors"]
    patients = len(config["patients"])
    group = jnp.asarray(data["patient_groups"])
    lp, lw, lc, ly = map(jnp.asarray, (data["loc_train_patient"], data["loc_train_weight"], data["loc_train_covariate"], data["loc_train_count"]))
    def model() -> None:
        sd = numpyro.sample("location_patient_sd", dist.HalfNormal(priors["patient_ecology_scale"]))
        effect = numpyro.sample("location_patient_raw", dist.Normal(0, 1).expand([patients]).to_event(1)) * sd
        intercept = numpyro.sample("location_intercept", dist.Normal(0, priors["location_sd"]))
        beta_group = numpyro.sample("location_group", dist.Normal(0, priors["group_sd"]))
        beta_cov = numpyro.sample("location_covariate", dist.Normal(0, priors["location_sd"]))
        log_expected = jnp.log(lw) + intercept + beta_group * group[lp] + beta_cov * lc + effect[lp]
        numpyro.sample("location_count", dist.Poisson(jnp.exp(log_expected)), obs=ly)
    return model


def mark_model(config: dict[str, Any], data: dict[str, Any]) -> Callable[[], None]:
    priors = config["request"]["priors"]
    patients, types = len(config["patients"]), len(config["type_ids"])
    group = jnp.asarray(data["patient_groups"])
    mp, mt, mf = map(jnp.asarray, (data["mark_train_patient"], data["mark_train_type"], data["mark_train_features"]))
    def model() -> None:
        sd = numpyro.sample("mark_patient_sd", dist.HalfNormal(priors["patient_ecology_scale"]).expand([types - 1]).to_event(1))
        raw = numpyro.sample("mark_patient_raw", dist.Normal(0, 1).expand([patients, types - 1]).to_event(2))
        intercept = numpyro.sample("mark_intercept", dist.Normal(0, priors["mark_intercept_sd"]).expand([types - 1]).to_event(1))
        beta_group = numpyro.sample("mark_group", dist.Normal(0, priors["mark_group_sd"]).expand([types - 1]).to_event(1))
        interaction = numpyro.sample("interaction", dist.Normal(0, priors["interaction_sd"]).expand([types - 1, types]).to_event(2))
        nonref = intercept + beta_group * group[mp, None] + sd * raw[mp] + mf @ interaction.T
        logits = jnp.concatenate([jnp.zeros((len(mp), 1)), nonref], axis=1)
        numpyro.sample("mark_type", dist.Categorical(logits=logits), obs=mt)
    return model


def sample(config: dict[str, Any], model: Callable[[], None], purpose: str) -> tuple[dict[str, np.ndarray], dict[str, float | int]]:
    sampler = MCMC(NUTS(model, target_accept_prob=config["target_accept"], max_tree_depth=config["depth"]),
        num_warmup=config["tune"], num_samples=config["draws"], num_chains=config["chains"],
        chain_method="sequential", progress_bar=False)
    sampler.run(jax.random.PRNGKey(seed_for(config["seed"], purpose)), extra_fields=("diverging", "num_steps", "energy"))
    samples = {name: np.asarray(value, dtype=float) for name, value in sampler.get_samples(group_by_chain=True).items()}
    posterior = az.from_dict({"posterior": samples})
    names = list(samples)
    flat = lambda tree: np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])
    extra = sampler.get_extra_fields(group_by_chain=True)
    energy = np.asarray(extra["energy"], dtype=float)
    diagnostics = {
        "r_hat": float(flat(az.rhat(posterior, var_names=names, method="rank")).max()),
        "ess_bulk": float(flat(az.ess(posterior, var_names=names, method="bulk")).min()),
        "ess_tail": float(flat(az.ess(posterior, var_names=names, method="tail")).min()),
        "minimum_ebfmi": float(np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1))),
        "divergences": int(np.asarray(extra["diverging"]).sum()),
        "max_tree_depth_hits": int((np.asarray(extra["num_steps"]) >= 2**config["depth"] - 1).sum()),
    }
    return {name: value.reshape((-1, *value.shape[2:])) for name, value in samples.items()}, diagnostics


def log_poisson(y: np.ndarray, log_mu: np.ndarray) -> np.ndarray:
    return (y[None, :] * log_mu - np.exp(log_mu) - np.vectorize(math.lgamma)(y + 1)[None, :]).sum(axis=1)


def log_marks(y: np.ndarray, logits: np.ndarray) -> np.ndarray:
    maximum = logits.max(axis=2, keepdims=True)
    log_probability = logits - maximum - np.log(np.exp(logits - maximum).sum(axis=2, keepdims=True))
    return log_probability[:, np.arange(len(y)), y].sum(axis=1)


def summary(values: np.ndarray) -> dict[str, float]:
    values = np.asarray(values, dtype=float).reshape(-1)
    return {"mean": float(values.mean()), "sd": float(values.std(ddof=1)),
        "interval_lower": float(np.quantile(values, 0.025)), "interval_upper": float(np.quantile(values, 0.975))}


def tail(values: np.ndarray, observed: float) -> float:
    return min(1.0, 2 * min(float(np.mean(values <= observed)), float(np.mean(values >= observed))))


def evaluate(config: dict[str, Any], data: dict[str, Any], joint: dict[str, np.ndarray], location: dict[str, np.ndarray], mark: dict[str, np.ndarray]) -> tuple[dict[str, Any], dict[str, Any]]:
    group = data["patient_groups"]
    lp, lw, lc, ly = (data["loc_heldout_patient"], data["loc_heldout_weight"], data["loc_heldout_covariate"], data["loc_heldout_count"])
    mp, mt, mf = (data["mark_heldout_patient"], data["mark_heldout_type"], data["mark_heldout_features"])
    ecology = joint["patient_ecology_raw"] * joint["patient_ecology_sd"][:, None]
    joint_loc_log = np.log(lw)[None, :] + joint["location_intercept"][:, None] + joint["location_group"][:, None] * group[lp][None, :] + joint["location_covariate"][:, None] * lc[None, :] + ecology[:, lp]
    joint_nonref = joint["mark_intercept"][:, None, :] + joint["mark_group"][:, None, :] * group[mp][None, :, None] + joint["mark_loading"][:, None, :] * ecology[:, mp, None] + np.einsum("nf,dkf->dnk", mf, joint["interaction"])
    joint_logits = np.concatenate([np.zeros((len(joint_nonref), len(mp), 1)), joint_nonref], axis=2)
    location_effect = location["location_patient_raw"] * location["location_patient_sd"][:, None]
    base_loc_log = np.log(lw)[None, :] + location["location_intercept"][:, None] + location["location_group"][:, None] * group[lp][None, :] + location["location_covariate"][:, None] * lc[None, :] + location_effect[:, lp]
    mark_effect = mark["mark_patient_raw"] * mark["mark_patient_sd"][:, None, :]
    base_nonref = mark["mark_intercept"][:, None, :] + mark["mark_group"][:, None, :] * group[mp][None, :, None] + mark_effect[:, mp, :] + np.einsum("nf,dkf->dnk", mf, mark["interaction"])
    base_logits = np.concatenate([np.zeros((len(base_nonref), len(mp), 1)), base_nonref], axis=2)
    joint_score = log_poisson(ly, joint_loc_log) + log_marks(mt, joint_logits)
    location_score = log_poisson(ly, base_loc_log)
    mark_score = log_marks(mt, base_logits)
    heldout = {
        "joint": {"mean_log_predictive_density": float(joint_score.mean()), "uncertainty": summary(joint_score)},
        "location_only": {"mean_log_predictive_density": float(location_score.mean()), "uncertainty": summary(location_score)},
        "conditional_mark_only": {"mean_log_predictive_density": float(mark_score.mean()), "uncertainty": summary(mark_score)},
        "joint_minus_separate_baselines": summary(joint_score - location_score - mark_score),
    }
    rng = np.random.default_rng(seed_for(config["seed"], "heldout_ppc"))
    replicated_counts = rng.poisson(np.exp(joint_loc_log))
    probabilities = np.exp(joint_logits - joint_logits.max(axis=2, keepdims=True))
    probabilities /= probabilities.sum(axis=2, keepdims=True)
    replicated_marks = np.asarray([[rng.choice(len(config["type_ids"]), p=probabilities[draw, row]) for row in range(len(mp))] for draw in range(len(probabilities))])
    observed_count = float(ly.sum())
    count_values = replicated_counts.sum(axis=1).astype(float)
    observed_prop = float(np.mean(mt == 0))
    prop_values = np.mean(replicated_marks == 0, axis=1)
    def enrichment(values: np.ndarray) -> float:
        total, cross = 0, 0
        offset = 0
        for pattern in sorted(config["heldout"]):
            n = sum(value == pattern for value in data["mark_heldout_patterns"])
            for left, right in data["edges"][pattern]:
                total += 1
                cross += int({int(values[offset + left]), int(values[offset + right])} == {0, 1})
            offset += n
        return cross / total if total else 0.0
    observed_enrichment = enrichment(mt)
    enrichment_values = np.asarray([enrichment(row) for row in replicated_marks])
    ppc = {
        "total_count": {"observed": observed_count, "replicated_mean": float(count_values.mean()), "two_sided_tail_probability": tail(count_values, observed_count)},
        "mark_proportions": {"observed": observed_prop, "replicated_mean": float(prop_values.mean()), "two_sided_tail_probability": tail(prop_values, observed_prop)},
        "cross_type_enrichment": {"observed": observed_enrichment, "replicated_mean": float(enrichment_values.mean()), "two_sided_tail_probability": tail(enrichment_values, observed_enrichment)},
    }
    return heldout, ppc


def main() -> int:
    if numpyro.__version__ != NUMPYRO_VERSION or jax.__version__ != JAX_VERSION or sys.version_info[:2] != (3, 12):
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
    joint, joint_diagnostics = sample(config, joint_model(config, data), "joint")
    location, location_diagnostics = sample(config, location_model(config, data), "location")
    mark, mark_diagnostics = sample(config, mark_model(config, data), "mark")
    heldout, ppc = evaluate(config, data, joint, location, mark)
    diagnostics = {
        "maximum_r_hat": max(joint_diagnostics["r_hat"], location_diagnostics["r_hat"], mark_diagnostics["r_hat"]),
        "minimum_bulk_ess": min(joint_diagnostics["ess_bulk"], location_diagnostics["ess_bulk"], mark_diagnostics["ess_bulk"]),
        "minimum_tail_ess": min(joint_diagnostics["ess_tail"], location_diagnostics["ess_tail"], mark_diagnostics["ess_tail"]),
        "minimum_ebfmi": min(joint_diagnostics["minimum_ebfmi"], location_diagnostics["minimum_ebfmi"], mark_diagnostics["minimum_ebfmi"]),
        "divergences": sum(int(value["divergences"]) for value in (joint_diagnostics, location_diagnostics, mark_diagnostics)),
        "max_tree_depth_hits": sum(int(value["max_tree_depth_hits"]) for value in (joint_diagnostics, location_diagnostics, mark_diagnostics)),
    }
    complete = bool(
        diagnostics["maximum_r_hat"] <= 1.01
        and diagnostics["minimum_bulk_ess"] >= 100.0
        and diagnostics["minimum_tail_ess"] >= 100.0
        and diagnostics["minimum_ebfmi"] >= 0.3
        and diagnostics["divergences"] == 0
        and diagnostics["max_tree_depth_hits"] == 0
    )
    result = {
        "format": RESULT_FORMAT, "version": 1,
        "backend": {"name": "numpyro", "version": numpyro.__version__, "python_version": f"{sys.version_info.major}.{sys.version_info.minor}", "environment_lock_sha256": lock_sha, "worker_sha256": worker_sha},
        "jax_version": jax.__version__, "request_sha256": request_sha,
        "location_sha256": config["request"]["location_sha256"], "mark_sha256": config["request"]["mark_sha256"],
        "patient_count": len(config["patients"]), "pattern_count": len(config["patterns"]),
        "training_pattern_count": len(config["train"]), "heldout_pattern_count": len(config["heldout"]),
        "type_ids": config["type_ids"], "training_pattern_ids": sorted(config["train"]),
        "heldout_pattern_ids": sorted(config["heldout"]),
        "joint_posterior": {"patient_ecology_sd": summary(joint["patient_ecology_sd"]), "mark_loadings": [summary(joint["mark_loading"][:, index]) for index in range(len(config["type_ids"]) - 1)]},
        "heldout_comparison": heldout, "posterior_predictive": ppc, "diagnostics": diagnostics,
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
        print(f"marklab joint location-mark failed: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from error
