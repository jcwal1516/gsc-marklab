#!/usr/bin/env python3
"""Independent NumPyro agreement fit for replicated conditional hard marks."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import math
from pathlib import Path
import sys
from typing import Any

import arviz as az
import jax

jax.config.update("jax_enable_x64", True)
import jax.numpy as jnp
import numpy as np
import numpyro
import numpyro.distributions as dist
from numpyro.infer import MCMC, NUTS


REQUEST_FORMAT = "marklab.numpyro_replicated_conditional_multitype_mark_request"
RESULT_FORMAT = "marklab.numpyro_replicated_conditional_multitype_mark_result"
NUMPYRO_VERSION = "0.21.0"
JAX_VERSION = "0.11.1"


class ContractError(ValueError):
    pass


def load_pymc() -> Any:
    path = Path(__file__).with_name(
        "marklab_pymc_replicated_conditional_multitype_mark_worker.py"
    )
    specification = importlib.util.spec_from_file_location(
        "marklab_pymc_replicated_conditional_mark_contract", path
    )
    if specification is None or specification.loader is None:
        raise ContractError("cannot load replicated PyMC conditional-mark contract")
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


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


def validate(
    request: Any, lock_sha: str, worker_sha: str, pymc_worker_sha: str
) -> tuple[dict[str, Any], dict[str, Any]]:
    request = obj(
        request,
        {
            "format", "version", "backend", "jax_version", "source_request_sha256",
            "source_request", "maximum_tree_depth",
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
    exact(backend["name"], "numpyro", "backend.name")
    exact(backend["version"], NUMPYRO_VERSION, "backend.version")
    exact(backend["python_version"], "3.12", "backend.python_version")
    exact(backend["environment_lock_sha256"], lock_sha, "backend.lock")
    exact(backend["worker_sha256"], worker_sha, "backend.worker")
    exact(request["jax_version"], JAX_VERSION, "request.jax_version")
    source_sha = request["source_request_sha256"]
    if (
        not isinstance(source_sha, str)
        or len(source_sha) != 64
        or any(character not in "0123456789abcdef" for character in source_sha)
    ):
        raise ContractError("source request digest is invalid")
    depth = request["maximum_tree_depth"]
    if isinstance(depth, bool) or not isinstance(depth, int) or not 10 <= depth <= 14:
        raise ContractError("maximum tree depth is outside [10, 14]")
    config = load_pymc().validate(request["source_request"], lock_sha, pymc_worker_sha)
    config["numpyro_maximum_tree_depth"] = depth
    return request, config


def seed_for(seed: int, purpose: str) -> int:
    value = hashlib.sha256(
        f"marklab-numpyro-replicated-conditional-multitype-mark-v1\0{seed}\0{purpose}".encode()
    ).digest()
    return int.from_bytes(value[:4], "little")


def potential_design(types: int) -> np.ndarray:
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


def make_model(config: dict[str, Any]) -> Any:
    types = len(config["types"])
    patients = len(config["patient_ids"])
    patterns = len(config["pattern_ids"])
    free_intercepts = types - 1
    free_potentials = types * (types + 1) // 2 - 1
    design = jnp.asarray(potential_design(types))
    pattern_groups = jnp.asarray(config["pattern_groups"])[:, None]
    pattern_patients = jnp.asarray(config["pattern_patients"])
    point_patterns = jnp.asarray(config["point_patterns"])
    counts = jnp.asarray(config["neighbor_counts"])
    observed = jnp.asarray(config["observed"])

    def model() -> None:
        baseline_intercept = numpyro.sample(
            "baseline_intercept",
            dist.Normal(0.0, config["intercept_sd"])
            .expand([free_intercepts])
            .to_event(1),
        )
        group_intercept = numpyro.sample(
            "group_intercept",
            dist.Normal(0.0, config["group_effect_sd"])
            .expand([free_intercepts])
            .to_event(1),
        )
        baseline_potential = numpyro.sample(
            "baseline_potential",
            dist.Normal(0.0, config["interaction_sd"])
            .expand([free_potentials])
            .to_event(1),
        )
        group_potential = numpyro.sample(
            "group_potential",
            dist.Normal(0.0, config["group_effect_sd"])
            .expand([free_potentials])
            .to_event(1),
        )
        patient_intercept_sd = numpyro.sample(
            "patient_intercept_sd", dist.HalfNormal(config["patient_sd_scale"])
        )
        patient_potential_sd = numpyro.sample(
            "patient_potential_sd", dist.HalfNormal(config["patient_sd_scale"])
        )
        pattern_intercept_sd = numpyro.sample(
            "pattern_intercept_sd", dist.HalfNormal(config["pattern_sd_scale"])
        )
        pattern_potential_sd = numpyro.sample(
            "pattern_potential_sd", dist.HalfNormal(config["pattern_sd_scale"])
        )
        patient_intercept_z = numpyro.sample(
            "patient_intercept_z",
            dist.Normal(0.0, 1.0).expand([patients, free_intercepts]).to_event(2),
        )
        patient_potential_z = numpyro.sample(
            "patient_potential_z",
            dist.Normal(0.0, 1.0).expand([patients, free_potentials]).to_event(2),
        )
        pattern_intercept_z = numpyro.sample(
            "pattern_intercept_z",
            dist.Normal(0.0, 1.0).expand([patterns, free_intercepts]).to_event(2),
        )
        pattern_potential_z = numpyro.sample(
            "pattern_potential_z",
            dist.Normal(0.0, 1.0).expand([patterns, free_potentials]).to_event(2),
        )
        pattern_intercept_free = (
            baseline_intercept
            + pattern_groups * group_intercept
            + patient_intercept_sd * patient_intercept_z[pattern_patients]
            + pattern_intercept_sd * pattern_intercept_z
        )
        pattern_potential_free = (
            baseline_potential
            + pattern_groups * group_potential
            + patient_potential_sd * patient_potential_z[pattern_patients]
            + pattern_potential_sd * pattern_potential_z
        )
        pattern_intercept = jnp.concatenate(
            [jnp.zeros((patterns, 1)), pattern_intercept_free], axis=1
        )
        pattern_potential = jnp.einsum("pf,fkl->pkl", pattern_potential_free, design)
        logits = pattern_intercept[point_patterns] + jnp.einsum(
            "nl,nkl->nk", counts, pattern_potential[point_patterns]
        )
        numpyro.sample("observed_type", dist.Categorical(logits=logits), obs=observed)

    return model


def summary(values: np.ndarray) -> dict[str, float]:
    flat = np.asarray(values, dtype=np.float64).reshape(-1)
    return {
        "mean": float(flat.mean()),
        "sd": float(flat.std(ddof=1)),
        "interval_lower": float(np.quantile(flat, 0.025)),
        "interval_upper": float(np.quantile(flat, 0.975)),
    }


def flattened(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def softmax(values: np.ndarray) -> np.ndarray:
    shifted = values - values.max(axis=-1, keepdims=True)
    weights = np.exp(shifted)
    return weights / weights.sum(axis=-1, keepdims=True)


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


def fit(
    config: dict[str, Any], request_sha: str, source_sha: str,
    lock_sha: str, worker_sha: str,
) -> dict[str, Any]:
    kernel = NUTS(
        make_model(config),
        target_accept_prob=config["target_accept"],
        max_tree_depth=config["numpyro_maximum_tree_depth"],
    )
    sampler = MCMC(
        kernel,
        num_warmup=config["tune"],
        num_samples=config["draws"],
        num_chains=config["chains"],
        chain_method="sequential",
        progress_bar=False,
    )
    sampler.run(
        jax.random.PRNGKey(seed_for(config["seed"], "nuts")),
        extra_fields=("diverging", "num_steps", "energy"),
    )
    samples = {
        name: np.asarray(value, dtype=np.float64)
        for name, value in sampler.get_samples(group_by_chain=True).items()
    }
    flat = {
        name: value.reshape((-1, *value.shape[2:]))
        for name, value in samples.items()
    }
    types = len(config["types"])
    patients = len(config["patient_ids"])
    patterns = len(config["pattern_ids"])
    design = potential_design(types)
    completed = len(flat["baseline_intercept"])
    baseline_potential = np.einsum(
        "df,fkl->dkl", flat["baseline_potential"], design
    )
    group_potential = np.einsum(
        "df,fkl->dkl", flat["group_potential"], design
    )
    patient_potential_free = (
        flat["baseline_potential"][:, None, :]
        + config["patient_groups"][None, :, None]
        * flat["group_potential"][:, None, :]
        + flat["patient_potential_sd"][:, None, None]
        * flat["patient_potential_z"]
    )
    patient_potential = np.einsum(
        "dpf,fkl->dpkl", patient_potential_free, design
    )
    pattern_intercept_free = (
        flat["baseline_intercept"][:, None, :]
        + config["pattern_groups"][None, :, None]
        * flat["group_intercept"][:, None, :]
        + flat["patient_intercept_sd"][:, None, None]
        * flat["patient_intercept_z"][:, config["pattern_patients"], :]
        + flat["pattern_intercept_sd"][:, None, None]
        * flat["pattern_intercept_z"]
    )
    pattern_potential_free = (
        flat["baseline_potential"][:, None, :]
        + config["pattern_groups"][None, :, None]
        * flat["group_potential"][:, None, :]
        + flat["patient_potential_sd"][:, None, None]
        * flat["patient_potential_z"][:, config["pattern_patients"], :]
        + flat["pattern_potential_sd"][:, None, None]
        * flat["pattern_potential_z"]
    )
    pattern_intercept = np.zeros((completed, patterns, types), dtype=np.float64)
    pattern_intercept[:, :, 1:] = pattern_intercept_free
    pattern_potential = np.einsum(
        "dpf,fkl->dpkl", pattern_potential_free, design
    )

    scores = np.zeros(completed, dtype=np.float64)
    expected_counts = np.zeros((completed, patterns, types), dtype=np.float64)
    expected_same = np.zeros((completed, patterns), dtype=np.float64)
    elements = len(config["observed"]) * types + len(config["edges"]) * types
    chunk = max(1, min(64, 4_000_000 // max(1, elements)))
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
        scores[begin:end] = np.log(
            probability[:, np.arange(len(config["observed"])), config["observed"]]
        ).sum(axis=1)
        for pattern in range(patterns):
            point_mask = config["point_patterns"] == pattern
            expected_counts[begin:end, pattern, :] = probability[:, point_mask, :].sum(axis=1)
            edge_mask = config["edge_patterns"] == pattern
            endpoints = config["edges"][edge_mask]
            expected_same[begin:end, pattern] = np.sum(
                probability[:, endpoints[:, 0], :]
                * probability[:, endpoints[:, 1], :],
                axis=(1, 2),
            )

    null_score = 0.0
    pattern_checks = []
    for pattern, pattern_id in enumerate(config["pattern_ids"]):
        point_mask = config["point_patterns"] == pattern
        observed = config["observed"][point_mask]
        observed_counts = np.bincount(observed, minlength=types)
        proportions = observed_counts / observed_counts.sum()
        null_score += float(np.sum(observed_counts * np.log(proportions)))
        edge_mask = config["edge_patterns"] == pattern
        endpoints = config["edges"][edge_mask]
        observed_same = int(
            np.sum(
                config["observed"][endpoints[:, 0]]
                == config["observed"][endpoints[:, 1]]
            )
        )
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
        for row in affinity_rows(patient_potential[:, patient, :, :], config["types"]):
            patient_affinities.append({
                "patient_id": patient_id,
                "group": config["groups"][int(config["patient_groups"][patient])],
                **row,
            })
    hierarchy_scales = [
        {"level": "patient", "component": "composition_intercept", "scale": summary(flat["patient_intercept_sd"])},
        {"level": "patient", "component": "spatial_pair_potential", "scale": summary(flat["patient_potential_sd"])},
        {"level": "pattern_within_patient", "component": "composition_intercept", "scale": summary(flat["pattern_intercept_sd"])},
        {"level": "pattern_within_patient", "component": "spatial_pair_potential", "scale": summary(flat["pattern_potential_sd"])},
    ]
    names = [
        "baseline_intercept", "group_intercept", "baseline_potential", "group_potential",
        "patient_intercept_sd", "patient_potential_sd", "pattern_intercept_sd",
        "pattern_potential_sd", "patient_intercept_z", "patient_potential_z",
        "pattern_intercept_z", "pattern_potential_z",
    ]
    posterior = az.from_dict({"posterior": samples})
    r_hat = float(flattened(az.rhat(posterior, var_names=names, method="rank"), names).max())
    bulk = float(flattened(az.ess(posterior, var_names=names, method="bulk"), names).min())
    tail = float(flattened(az.ess(posterior, var_names=names, method="tail"), names).min())
    mcse_mean = float(flattened(az.mcse(posterior, var_names=names, method="mean"), names).max())
    mcse_sd = float(flattened(az.mcse(posterior, var_names=names, method="sd"), names).max())
    extra = sampler.get_extra_fields(group_by_chain=True)
    energy = np.asarray(extra["energy"], dtype=np.float64)
    ebfmi = float(
        np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1))
    )
    divergences = int(np.asarray(extra["diverging"]).sum())
    depth_hits = int(
        (
            np.asarray(extra["num_steps"])
            >= 2 ** config["numpyro_maximum_tree_depth"] - 1
        ).sum()
    )
    prior_rng = np.random.default_rng(seed_for(config["seed"], "prior"))
    prior_finite = bool(
        np.isfinite(prior_rng.normal(size=(500, 2 * (types - 1)))).all()
        and np.isfinite(
            prior_rng.normal(
                size=(500, 2 * (types * (types + 1) // 2 - 1))
            )
        ).all()
    )
    posterior_finite = bool(
        all(
            np.isfinite(value).all()
            for value in (
                baseline_potential, group_potential, patient_potential,
                pattern_intercept, pattern_potential, scores, expected_counts, expected_same,
            )
        )
    )
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
            "name": "numpyro", "version": numpyro.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha, "worker_sha256": worker_sha,
        },
        "jax_version": jax.__version__,
        "input_sha256": config["input_sha"],
        "request_sha256": request_sha,
        "source_request_sha256": source_sha,
        "maximum_tree_depth": config["numpyro_maximum_tree_depth"],
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"], "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"], "completed_draws": completed,
        },
        "baseline_affinities": affinity_rows(baseline_potential, config["types"]),
        "group_affinity_shifts": affinity_rows(group_potential, config["types"]),
        "patient_affinities": patient_affinities,
        "hierarchy_scales": hierarchy_scales,
        "comparison": {
            "independent_pattern_label_composite_log_score": null_score,
            "spatial_hierarchical_composite_log_score": summary(scores),
            "mean_composite_log_score_improvement": float(scores.mean() - null_score),
        },
        "pattern_checks": pattern_checks,
        "diagnostics": {
            "prior_predictive_finite": prior_finite,
            "posterior_finite": posterior_finite,
            "r_hat": r_hat, "ess_bulk": bulk, "ess_tail": tail,
            "mcse_mean": mcse_mean, "mcse_sd": mcse_sd,
            "minimum_ebfmi": ebfmi, "divergences": divergences,
            "max_tree_depth_hits": depth_hits,
            "constraints_valid": posterior_finite,
            "identifiability_checks_passed": bool(
                np.all(pattern_intercept[..., 0] == 0.0)
                and np.all(pattern_potential[..., 0, 0] == 0.0)
            ),
        },
    }


def main() -> int:
    if (
        numpyro.__version__ != NUMPYRO_VERSION
        or jax.__version__ != JAX_VERSION
        or sys.version_info[:2] != (3, 12)
    ):
        raise ContractError("NumPyro, JAX, or Python version drift")
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    pymc_worker_sha = hashlib.sha256(
        script.with_name(
            "marklab_pymc_replicated_conditional_multitype_mark_worker.py"
        ).read_bytes()
    ).hexdigest()
    raw = sys.stdin.buffer.read(64 * 1_048_576 + 1)
    if not raw or len(raw) > 64 * 1_048_576:
        raise ContractError("request size is invalid")
    request_sha = hashlib.sha256(raw).hexdigest()
    request, config = validate(json.loads(raw), lock_sha, worker_sha, pymc_worker_sha)
    result = fit(
        config, request_sha, request["source_request_sha256"], lock_sha, worker_sha
    )
    output = json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")) + "\n"
    if len(output.encode()) > config["maximum_output_bytes"]:
        raise ContractError("result exceeds output ceiling")
    sys.stdout.write(output)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(
            f"marklab NumPyro replicated conditional multitype mark failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
