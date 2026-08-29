#!/usr/bin/env python3
"""Pinned PyMC replicated multitype LGCP with one inferred shared Matérn kernel."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import math
from pathlib import Path
import sys
from typing import Any

import arviz as az
import numpy as np
import pymc as pm
import pytensor.tensor as pt


PYMC_VERSION = "6.3.0"
REQUEST_FORMAT = (
    "marklab.pymc_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_request"
)
RESULT_FORMAT = (
    "marklab.bayesian_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_fit"
)


class ContractError(ValueError):
    pass


def load_source() -> Any:
    path = Path(__file__).with_name(
        "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_worker.py"
    )
    specification = importlib.util.spec_from_file_location(
        "marklab_multitype_lgcp_inferred_source", path
    )
    if specification is None or specification.loader is None:
        raise ContractError("cannot load multitype LGCP source contract")
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


def positive(value: Any, path: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ContractError(f"{path} must be numeric")
    result = float(value)
    if not math.isfinite(result) or result <= 0.0:
        raise ContractError(f"{path} must be finite and positive")
    return result


def validate(
    request: Any, lock_sha: str, worker_sha: str, source_worker_sha: str
) -> tuple[dict[str, Any], dict[str, Any]]:
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
    exact(backend["environment_lock_sha256"], lock_sha, "backend.lock")
    exact(backend["worker_sha256"], worker_sha, "backend.worker")
    source_backend = obj(
        request["source_backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "source_backend",
    )
    exact(source_backend, request["source_request"]["backend"], "source backend")
    source_sha = request["source_request_sha256"]
    if not isinstance(source_sha, str) or len(source_sha) != 64:
        raise ContractError("source request digest is invalid")
    source = load_source()
    config = source.validate(request["source_request"], lock_sha, source_worker_sha)
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
            "source_module": source,
            "source_request_sha256": source_sha,
            "amplitude_scale": amplitude,
            "length_scale": length,
            "kernel_jitter": jitter,
            "kernel_cube_work": work,
            "maximum_tree_depth": depth,
        }
    )
    return request, config


def seed_for(seed: int, purpose: str) -> int:
    digest = hashlib.sha256(
        f"marklab-replicated-multitype-lgcp-inferred-kernel-v1\0{seed}\0{purpose}".encode()
    ).digest()
    return int.from_bytes(digest[:4], "little")


def dynamic_latent(config: dict[str, Any], amplitude: Any, length: Any, field_raw: Any) -> Any:
    blocks = []
    for pattern in range(len(config["pattern_ids"])):
        selected = np.flatnonzero(config["node_patterns"] == pattern)
        nodes = [config["request"]["nodes"][index] for index in selected]
        coordinates = np.asarray([[row["x_um"], row["y_um"]] for row in nodes])
        distance = np.linalg.norm(
            coordinates[:, None, :] - coordinates[None, :, :], axis=2
        )
        scaled = math.sqrt(3.0) * distance / length
        covariance = amplitude**2 * (1.0 + scaled) * pt.exp(-scaled)
        covariance += config["kernel_jitter"] * np.eye(len(selected))
        block = field_raw[:, selected] @ pt.linalg.cholesky(covariance).T
        blocks.append(block - pt.mean(block, axis=1, keepdims=True))
    return pt.concatenate(blocks, axis=1)


def prior_is_finite(config: dict[str, Any]) -> bool:
    rng = np.random.default_rng(seed_for(config["seed"], "prior"))
    for _ in range(int(config["policy"]["prior_predictive_draws"])):
        amplitude = abs(rng.normal(0.0, config["amplitude_scale"]))
        length = abs(rng.normal(0.0, config["length_scale"]))
        if amplitude <= 0.0 or length <= 0.0:
            return False
        for pattern in range(len(config["pattern_ids"])):
            selected = np.flatnonzero(config["node_patterns"] == pattern)
            nodes = [config["request"]["nodes"][index] for index in selected]
            coordinates = np.asarray([[row["x_um"], row["y_um"]] for row in nodes])
            distance = np.linalg.norm(
                coordinates[:, None, :] - coordinates[None, :, :], axis=2
            )
            scaled = math.sqrt(3.0) * distance / length
            covariance = amplitude**2 * (1.0 + scaled) * np.exp(-scaled)
            covariance += config["kernel_jitter"] * np.eye(len(selected))
            if not np.isfinite(np.linalg.cholesky(covariance)).all():
                return False
    return True


def fit(
    config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str
) -> dict[str, Any]:
    source = config["source_module"]
    priors = config["priors"]
    types = len(config["types"])
    patients = len(config["patient_ids"])
    patterns = len(config["pattern_ids"])
    nodes = len(config["weights"])
    prior_finite = prior_is_finite(config)
    with pm.Model():
        intercept = pm.Normal(
            "intercept", priors["intercept_mean"], priors["intercept_sd"], shape=types
        )
        group_effect = pm.Normal(
            "group_effect", 0.0, priors["group_effect_sd"], shape=types
        )
        covariate_effect = pm.Normal(
            "covariate_effect", 0.0, priors["covariate_effect_sd"], shape=types
        )
        patient_sd = pm.HalfNormal(
            "patient_sd", priors["patient_sd_scale"], shape=types
        )
        pattern_sd = pm.HalfNormal(
            "pattern_sd", priors["pattern_sd_scale"], shape=types
        )
        field_amplitude = pm.HalfNormal("field_amplitude", config["amplitude_scale"])
        field_length_scale_um = pm.HalfNormal(
            "field_length_scale_um", config["length_scale"]
        )
        patient_raw = pm.Normal("patient_raw", 0.0, 1.0, shape=(patients, types))
        pattern_raw = pm.Normal("pattern_raw", 0.0, 1.0, shape=(patterns, types))
        field_raw = pm.Normal("field_raw", 0.0, 1.0, shape=(types, nodes))
        patient_effect = pm.Deterministic(
            "patient_effect", patient_sd[None, :] * patient_raw
        )
        patient_pattern_mean = pt.stack(
            [
                pt.mean(
                    pattern_raw[np.flatnonzero(config["pattern_patients"] == patient), :],
                    axis=0,
                )
                for patient in range(patients)
            ]
        )
        pattern_effect = pm.Deterministic(
            "pattern_effect",
            pattern_sd[None, :]
            * (pattern_raw - patient_pattern_mean[config["pattern_patients"], :]),
        )
        latent_effect = pm.Deterministic(
            "latent_effect",
            dynamic_latent(config, field_amplitude, field_length_scale_um, field_raw),
        )
        log_expected = (
            intercept[:, None]
            + group_effect[:, None]
            * config["patient_groups"][config["node_patients"]][None, :]
            + covariate_effect[:, None] * config["covariate"][None, :]
            + patient_effect[config["node_patients"], :].T
            + pattern_effect[config["node_patterns"], :].T
            + latent_effect
            + np.log(config["weights"])[None, :]
        )
        expected_matrix = pm.Deterministic("expected_matrix", pm.math.exp(log_expected))
        expected_count = pm.Deterministic(
            "expected_count", expected_matrix[config["type_index"], config["node_index"]]
        )
        pm.Poisson("observed_count", mu=expected_count, observed=config["observed"])
        trace = pm.sample(
            draws=config["draws"],
            tune=config["tune"],
            chains=config["chains"],
            cores=1,
            blas_cores=1,
            target_accept=config["target_accept"],
            random_seed=config["seed"],
            progressbar=False,
            quiet=True,
            compute_convergence_checks=False,
            return_inferencedata=True,
            nuts={"max_treedepth": config["maximum_tree_depth"]},
        )
    names = [
        "intercept",
        "group_effect",
        "covariate_effect",
        "patient_sd",
        "pattern_sd",
        "field_amplitude",
        "field_length_scale_um",
        "patient_raw",
        "pattern_raw",
        "field_raw",
    ]
    posterior = trace.posterior
    flat = {
        name: np.asarray(posterior[name], dtype=np.float64).reshape(
            (-1, *np.asarray(posterior[name]).shape[2:])
        )
        for name in [
            "intercept",
            "group_effect",
            "covariate_effect",
            "patient_sd",
            "pattern_sd",
            "field_amplitude",
            "field_length_scale_um",
            "patient_effect",
            "pattern_effect",
            "latent_effect",
            "expected_count",
        ]
    }
    type_rows = [
        {
            "type_id": identity,
            "intercept": source.summary(flat["intercept"][:, index]),
            "group_effect": source.summary(flat["group_effect"][:, index]),
            "covariate_effect": source.summary(flat["covariate_effect"][:, index]),
            "patient_sd": source.summary(flat["patient_sd"][:, index]),
            "pattern_sd": source.summary(flat["pattern_sd"][:, index]),
        }
        for index, identity in enumerate(config["types"])
    ]
    differences = [
        {
            "type_a": config["types"][left],
            "type_b": config["types"][right],
            "difference": source.summary(
                flat["group_effect"][:, left] - flat["group_effect"][:, right]
            ),
        }
        for left in range(types)
        for right in range(left + 1, types)
    ]
    patient_rows = [
        {
            "owner_id": owner,
            "type_id": identity,
            "effect": source.summary(flat["patient_effect"][:, owner_index, type_index]),
        }
        for owner_index, owner in enumerate(config["patient_ids"])
        for type_index, identity in enumerate(config["types"])
    ]
    pattern_rows = [
        {
            "owner_id": owner,
            "type_id": identity,
            "effect": source.summary(flat["pattern_effect"][:, owner_index, type_index]),
        }
        for owner_index, owner in enumerate(config["pattern_ids"])
        for type_index, identity in enumerate(config["types"])
    ]
    node_rows = [
        {
            "pattern_id": config["request"]["nodes"][int(node)]["pattern_id"],
            "node_id": config["request"]["nodes"][int(node)]["node_id"],
            "type_id": config["types"][int(type_index)],
            "latent_effect": source.summary(flat["latent_effect"][:, int(type_index), int(node)]),
            "expected_count": source.summary(flat["expected_count"][:, row]),
        }
        for row, (node, type_index) in enumerate(
            zip(config["node_index"], config["type_index"], strict=True)
        )
    ]
    rng = np.random.default_rng(seed_for(config["seed"], "posterior_predictive"))
    replicated = rng.poisson(flat["expected_count"])
    predictive = []
    for pattern_index, pattern_id in enumerate(config["pattern_ids"]):
        for type_index, type_id in enumerate(config["types"]):
            selected = (
                (config["node_patterns"][config["node_index"]] == pattern_index)
                & (config["type_index"] == type_index)
            )
            observed_nodes = config["observed"][selected]
            replicated_nodes = replicated[:, selected]
            observed_total = int(observed_nodes.sum())
            totals = replicated_nodes.sum(axis=1)
            observed_variance = float(np.var(observed_nodes))
            variances = np.var(replicated_nodes, axis=1)
            predictive.append(
                {
                    "pattern_id": pattern_id,
                    "type_id": type_id,
                    "observed_total_count": observed_total,
                    "replicate_count": int(len(totals)),
                    "replicated_total_count_mean": float(totals.mean()),
                    "replicated_total_count_sd": float(totals.std(ddof=1)),
                    "replicated_total_count_interval_lower": float(np.quantile(totals, 0.025)),
                    "replicated_total_count_interval_upper": float(np.quantile(totals, 0.975)),
                    "total_count_two_sided_tail_probability": source.two_sided_tail(
                        totals, observed_total
                    ),
                    "observed_node_count_variance": observed_variance,
                    "replicated_node_count_variance_mean": float(variances.mean()),
                    "node_variance_two_sided_tail_probability": source.two_sided_tail(
                        variances, observed_variance
                    ),
                }
            )
    tree = az.from_dict(
        {
            "posterior": {
                name: np.asarray(trace.posterior[name], dtype=np.float64) for name in names
            }
        }
    )
    r_hat = float(source.flattened(az.rhat(tree, var_names=names, method="rank"), names).max())
    bulk = float(source.flattened(az.ess(tree, var_names=names, method="bulk"), names).min())
    tail = float(source.flattened(az.ess(tree, var_names=names, method="tail"), names).min())
    mcse_mean = float(source.flattened(az.mcse(tree, var_names=names, method="mean"), names).max())
    mcse_sd = float(source.flattened(az.mcse(tree, var_names=names, method="sd"), names).max())
    stats = trace.sample_stats
    energy = np.asarray(stats["energy"], dtype=np.float64)
    ebfmi = float(np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1)))
    divergences = int(np.asarray(stats["diverging"]).sum())
    depth_hits = int(np.asarray(stats["reached_max_treedepth"]).sum())
    constraints = bool(
        all(
            np.max(
                np.abs(
                    flat["pattern_effect"][:, config["pattern_patients"] == patient, :].sum(
                        axis=1
                    )
                )
            )
            < 1e-10
            for patient in range(patients)
        )
        and all(
            np.max(
                np.abs(
                    flat["latent_effect"][:, :, config["node_patterns"] == pattern].sum(axis=2)
                )
            )
            < 1e-10
            for pattern in range(patterns)
        )
    )
    finite = bool(all(np.isfinite(value).all() for value in flat.values()))
    policy = config["policy"]
    complete = bool(
        prior_finite
        and finite
        and constraints
        and r_hat <= policy["maximum_r_hat"]
        and bulk >= policy["minimum_bulk_ess"]
        and tail >= policy["minimum_tail_ess"]
        and ebfmi >= policy["minimum_ebfmi"]
        and divergences <= policy["maximum_divergences"]
        and depth_hits <= policy["maximum_tree_depth_hits"]
    )
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": {
            "name": "pymc",
            "version": pm.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha,
            "worker_sha256": worker_sha,
        },
        "source_backend": config["outer_request"]["source_backend"],
        "input_sha256": config["input_sha"],
        "request_sha256": request_sha,
        "source_request_sha256": config["source_request_sha256"],
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"],
            "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"],
            "completed_draws": config["chains"] * config["draws"],
        },
        "kernel_posterior": {
            "field_amplitude": source.summary(flat["field_amplitude"]),
            "field_length_scale_um": source.summary(flat["field_length_scale_um"]),
        },
        "type_posteriors": type_rows,
        "group_effect_differences": differences,
        "patient_type_effects": patient_rows,
        "pattern_type_effects": pattern_rows,
        "node_type_posteriors": node_rows,
        "pattern_type_posterior_predictive": predictive,
        "diagnostics": {
            "prior_predictive_finite": prior_finite,
            "posterior_finite": finite,
            "r_hat": r_hat,
            "ess_bulk": bulk,
            "ess_tail": tail,
            "mcse_mean": mcse_mean,
            "mcse_sd": mcse_sd,
            "minimum_ebfmi": ebfmi,
            "divergences": divergences,
            "max_tree_depth_hits": depth_hits,
            "constraints_valid": constraints,
            "identifiability_checks_passed": constraints,
        },
        "kernel_cube_work": config["kernel_cube_work"],
    }


def main() -> int:
    if pm.__version__ != PYMC_VERSION or sys.version_info[:2] != (3, 12):
        raise ContractError("PyMC or Python version drift")
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    source_worker_sha = hashlib.sha256(
        script.with_name(
            "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_worker.py"
        ).read_bytes()
    ).hexdigest()
    raw = sys.stdin.buffer.read(64 * 1_048_576 + 1)
    if not raw or len(raw) > 64 * 1_048_576:
        raise ContractError("request size is invalid")
    request_sha = hashlib.sha256(raw).hexdigest()
    _, config = validate(json.loads(raw), lock_sha, worker_sha, source_worker_sha)
    output = json.dumps(
        fit(config, request_sha, lock_sha, worker_sha),
        allow_nan=False,
        sort_keys=True,
        separators=(",", ":"),
    ) + "\n"
    if len(output.encode()) > 16 * 1_048_576:
        raise ContractError("result exceeds output ceiling")
    sys.stdout.write(output)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(
            f"marklab PyMC replicated multitype inferred-kernel LGCP failed: "
            f"{type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
