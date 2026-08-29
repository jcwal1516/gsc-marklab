#!/usr/bin/env python3
"""Independent NumPyro fit for replicated exact-window multitype LGCPs."""

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


REQUEST_FORMAT = "marklab.numpyro_replicated_arbitrary_window_multitype_lgcp_request"
RESULT_FORMAT = "marklab.numpyro_replicated_arbitrary_window_multitype_lgcp_result"
NUMPYRO_VERSION = "0.21.0"
JAX_VERSION = "0.11.1"


class ContractError(ValueError):
    pass


def load_pymc() -> Any:
    path = Path(__file__).with_name(
        "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_worker.py"
    )
    specification = importlib.util.spec_from_file_location(
        "marklab_pymc_replicated_multitype_lgcp_contract", path
    )
    if specification is None or specification.loader is None:
        raise ContractError("cannot load replicated multitype PyMC contract")
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
        f"marklab-numpyro-replicated-multitype-lgcp-v1\0{seed}\0{purpose}".encode()
    ).digest()
    return int.from_bytes(value[:4], "little")


def make_model(config: dict[str, Any]) -> Any:
    types = len(config["types"])
    patients = len(config["patient_ids"])
    patterns = len(config["pattern_ids"])
    nodes = len(config["weights"])
    pattern_patients = jnp.asarray(config["pattern_patients"])
    node_patterns = jnp.asarray(config["node_patterns"])
    node_patients = jnp.asarray(config["node_patients"])
    patient_groups = jnp.asarray(config["patient_groups"])
    covariate = jnp.asarray(config["covariate"])
    weights = jnp.asarray(config["weights"])
    cholesky = jnp.asarray(config["cholesky"])
    node_index = jnp.asarray(config["node_index"])
    type_index = jnp.asarray(config["type_index"])
    observed = jnp.asarray(config["observed"])
    priors = config["priors"]

    def model() -> None:
        intercept = numpyro.sample(
            "intercept",
            dist.Normal(priors["intercept_mean"], priors["intercept_sd"])
            .expand([types])
            .to_event(1),
        )
        group_effect = numpyro.sample(
            "group_effect",
            dist.Normal(0.0, priors["group_effect_sd"]).expand([types]).to_event(1),
        )
        covariate_effect = numpyro.sample(
            "covariate_effect",
            dist.Normal(0.0, priors["covariate_effect_sd"]).expand([types]).to_event(1),
        )
        patient_sd = numpyro.sample(
            "patient_sd", dist.HalfNormal(priors["patient_sd_scale"]).expand([types]).to_event(1)
        )
        pattern_sd = numpyro.sample(
            "pattern_sd", dist.HalfNormal(priors["pattern_sd_scale"]).expand([types]).to_event(1)
        )
        patient_raw = numpyro.sample(
            "patient_raw", dist.Normal(0.0, 1.0).expand([patients, types]).to_event(2)
        )
        pattern_raw = numpyro.sample(
            "pattern_raw", dist.Normal(0.0, 1.0).expand([patterns, types]).to_event(2)
        )
        field_raw = numpyro.sample(
            "field_raw", dist.Normal(0.0, 1.0).expand([types, nodes]).to_event(2)
        )
        patient_effect = numpyro.deterministic(
            "patient_effect", patient_sd[None, :] * patient_raw
        )
        pattern_sums = jnp.zeros((patients, types)).at[pattern_patients].add(pattern_raw)
        pattern_numbers = jnp.zeros(patients).at[pattern_patients].add(1.0)
        pattern_effect = numpyro.deterministic(
            "pattern_effect",
            pattern_sd[None, :]
            * (pattern_raw - pattern_sums[pattern_patients] / pattern_numbers[pattern_patients, None]),
        )
        latent_uncentered = field_raw @ cholesky.T
        latent_sums = jax.vmap(
            lambda values: jnp.zeros(patterns).at[node_patterns].add(values)
        )(latent_uncentered)
        node_numbers = jnp.zeros(patterns).at[node_patterns].add(1.0)
        latent_effect = numpyro.deterministic(
            "latent_effect",
            latent_uncentered - latent_sums[:, node_patterns] / node_numbers[node_patterns][None, :],
        )
        log_expected = (
            intercept[:, None]
            + group_effect[:, None] * patient_groups[node_patients][None, :]
            + covariate_effect[:, None] * covariate[None, :]
            + patient_effect[node_patients, :].T
            + pattern_effect[node_patterns, :].T
            + latent_effect
            + jnp.log(weights)[None, :]
        )
        expected_matrix = numpyro.deterministic("expected_matrix", jnp.exp(log_expected))
        expected_count = numpyro.deterministic(
            "expected_count", expected_matrix[type_index, node_index]
        )
        numpyro.sample("observed_count", dist.Poisson(expected_count), obs=observed)

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


def fit(
    config: dict[str, Any], request_sha: str, source_sha: str, lock_sha: str, worker_sha: str
) -> dict[str, Any]:
    sampler = MCMC(
        NUTS(
            make_model(config),
            target_accept_prob=config["target_accept"],
            max_tree_depth=config["numpyro_maximum_tree_depth"],
        ),
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
    patient_effect = flat["patient_sd"][:, None, :] * flat["patient_raw"]
    pattern_centered = flat["pattern_raw"].copy()
    for patient in range(len(config["patient_ids"])):
        selected = config["pattern_patients"] == patient
        pattern_centered[:, selected, :] -= pattern_centered[:, selected, :].mean(
            axis=1, keepdims=True
        )
    pattern_effect = flat["pattern_sd"][:, None, :] * pattern_centered
    latent = np.einsum("dkn,mn->dkm", flat["field_raw"], config["cholesky"])
    for pattern in range(len(config["pattern_ids"])):
        selected = config["node_patterns"] == pattern
        latent[:, :, selected] -= latent[:, :, selected].mean(axis=2, keepdims=True)
    log_expected = (
        flat["intercept"][:, :, None]
        + flat["group_effect"][:, :, None]
        * config["patient_groups"][config["node_patients"]][None, None, :]
        + flat["covariate_effect"][:, :, None] * config["covariate"][None, None, :]
        + np.transpose(patient_effect[:, config["node_patients"], :], (0, 2, 1))
        + np.transpose(pattern_effect[:, config["node_patterns"], :], (0, 2, 1))
        + latent
        + np.log(config["weights"])[None, None, :]
    )
    expected_matrix = np.exp(log_expected)
    expected = expected_matrix[:, config["type_index"], config["node_index"]]
    type_posteriors = [
        {
            "type_id": identity,
            "intercept": summary(flat["intercept"][:, index]),
            "group_effect": summary(flat["group_effect"][:, index]),
            "covariate_effect": summary(flat["covariate_effect"][:, index]),
            "patient_sd": summary(flat["patient_sd"][:, index]),
            "pattern_sd": summary(flat["pattern_sd"][:, index]),
        }
        for index, identity in enumerate(config["types"])
    ]
    differences = [
        {
            "type_a": config["types"][left],
            "type_b": config["types"][right],
            "difference": summary(flat["group_effect"][:, left] - flat["group_effect"][:, right]),
        }
        for left in range(types)
        for right in range(left + 1, types)
    ]
    patient_rows = [
        {
            "owner_id": owner,
            "type_id": identity,
            "effect": summary(patient_effect[:, owner_index, type_index]),
        }
        for owner_index, owner in enumerate(config["patient_ids"])
        for type_index, identity in enumerate(config["types"])
    ]
    pattern_rows = [
        {
            "owner_id": owner,
            "type_id": identity,
            "effect": summary(pattern_effect[:, owner_index, type_index]),
        }
        for owner_index, owner in enumerate(config["pattern_ids"])
        for type_index, identity in enumerate(config["types"])
    ]
    node_rows = [
        {
            "pattern_id": config["request"]["nodes"][int(node)]["pattern_id"],
            "node_id": config["request"]["nodes"][int(node)]["node_id"],
            "type_id": config["types"][int(type_index)],
            "latent_effect": summary(latent[:, int(type_index), int(node)]),
            "expected_count": summary(expected[:, row]),
        }
        for row, (node, type_index) in enumerate(
            zip(config["node_index"], config["type_index"], strict=True)
        )
    ]
    names = [
        "intercept", "group_effect", "covariate_effect", "patient_sd", "pattern_sd",
        "patient_raw", "pattern_raw", "field_raw",
    ]
    posterior = az.from_dict({"posterior": samples})
    r_hat = float(flattened(az.rhat(posterior, var_names=names, method="rank"), names).max())
    bulk = float(flattened(az.ess(posterior, var_names=names, method="bulk"), names).min())
    tail = float(flattened(az.ess(posterior, var_names=names, method="tail"), names).min())
    mcse_mean = float(flattened(az.mcse(posterior, var_names=names, method="mean"), names).max())
    mcse_sd = float(flattened(az.mcse(posterior, var_names=names, method="sd"), names).max())
    extra = sampler.get_extra_fields(group_by_chain=True)
    energy = np.asarray(extra["energy"], dtype=np.float64)
    ebfmi = float(np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1)))
    divergences = int(np.asarray(extra["diverging"]).sum())
    depth_hits = int(
        (
            np.asarray(extra["num_steps"])
            >= 2 ** config["numpyro_maximum_tree_depth"] - 1
        ).sum()
    )
    policy = config["policy"]
    finite = bool(
        all(np.isfinite(value).all() for value in (patient_effect, pattern_effect, latent, expected))
    )
    constraints = bool(
        all(
            np.max(np.abs(pattern_effect[:, config["pattern_patients"] == patient, :].sum(axis=1))) < 1e-10
            for patient in range(len(config["patient_ids"]))
        )
        and all(
            np.max(np.abs(latent[:, :, config["node_patterns"] == pattern].sum(axis=2))) < 1e-10
            for pattern in range(len(config["pattern_ids"]))
        )
    )
    complete = bool(
        finite
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
            "draws_per_chain": config["draws"],
            "completed_draws": config["chains"] * config["draws"],
        },
        "type_posteriors": type_posteriors,
        "group_effect_differences": differences,
        "patient_type_effects": patient_rows,
        "pattern_type_effects": pattern_rows,
        "node_type_posteriors": node_rows,
        "diagnostics": {
            "prior_predictive_finite": True, "posterior_finite": finite,
            "r_hat": r_hat, "ess_bulk": bulk, "ess_tail": tail,
            "mcse_mean": mcse_mean, "mcse_sd": mcse_sd,
            "minimum_ebfmi": ebfmi, "divergences": divergences,
            "max_tree_depth_hits": depth_hits, "constraints_valid": constraints,
            "identifiability_checks_passed": constraints,
        },
    }


def main() -> int:
    if numpyro.__version__ != NUMPYRO_VERSION or jax.__version__ != JAX_VERSION or sys.version_info[:2] != (3, 12):
        raise ContractError("NumPyro, JAX, or Python version drift")
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    pymc_worker_sha = hashlib.sha256(
        script.with_name("marklab_pymc_replicated_arbitrary_window_multitype_lgcp_worker.py").read_bytes()
    ).hexdigest()
    raw = sys.stdin.buffer.read(64 * 1_048_576 + 1)
    if not raw or len(raw) > 64 * 1_048_576:
        raise ContractError("request size is invalid")
    request_sha = hashlib.sha256(raw).hexdigest()
    request, config = validate(json.loads(raw), lock_sha, worker_sha, pymc_worker_sha)
    result = fit(config, request_sha, request["source_request_sha256"], lock_sha, worker_sha)
    output = json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")) + "\n"
    if len(output.encode()) > 16 * 1_048_576:
        raise ContractError("result exceeds output ceiling")
    sys.stdout.write(output)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(
            f"marklab NumPyro replicated multitype LGCP failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
