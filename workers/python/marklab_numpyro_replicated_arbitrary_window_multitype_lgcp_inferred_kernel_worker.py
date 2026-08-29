#!/usr/bin/env python3
"""Independent NumPyro replicated multitype LGCP with inferred shared kernel."""

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


REQUEST_FORMAT = (
    "marklab.numpyro_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_request"
)
RESULT_FORMAT = (
    "marklab.numpyro_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_result"
)
NUMPYRO_VERSION = "0.21.0"
JAX_VERSION = "0.11.1"


class ContractError(ValueError):
    pass


def load(filename: str, name: str) -> Any:
    path = Path(__file__).with_name(filename)
    specification = importlib.util.spec_from_file_location(name, path)
    if specification is None or specification.loader is None:
        raise ContractError(f"cannot load {filename}")
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
    request: Any,
    lock_sha: str,
    worker_sha: str,
    pymc_inferred_sha: str,
    pymc_fixed_sha: str,
) -> tuple[dict[str, Any], dict[str, Any]]:
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "jax_version",
            "source_request_sha256",
            "source_request",
            "maximum_tree_depth",
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
    if not isinstance(source_sha, str) or len(source_sha) != 64:
        raise ContractError("source request digest is invalid")
    pymc = load(
        "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_worker.py",
        "marklab_pymc_inferred_multitype_contract",
    )
    _, config = pymc.validate(
        request["source_request"], lock_sha, pymc_inferred_sha, pymc_fixed_sha
    )
    depth = request["maximum_tree_depth"]
    if isinstance(depth, bool) or not isinstance(depth, int) or not 10 <= depth <= 14:
        raise ContractError("maximum tree depth is outside [10, 14]")
    config["numpyro_maximum_tree_depth"] = depth
    config["source_request_sha256"] = source_sha
    return request, config


def seed_for(seed: int, purpose: str) -> int:
    digest = hashlib.sha256(
        f"marklab-numpyro-replicated-multitype-inferred-kernel-v1\0{seed}\0{purpose}".encode()
    ).digest()
    return int.from_bytes(digest[:4], "little")


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
    node_index = jnp.asarray(config["node_index"])
    type_index = jnp.asarray(config["type_index"])
    observed = jnp.asarray(config["observed"])
    priors = config["priors"]
    distances = []
    selections = []
    for pattern in range(patterns):
        selected = np.flatnonzero(config["node_patterns"] == pattern)
        rows = [config["request"]["nodes"][index] for index in selected]
        coordinates = np.asarray([[row["x_um"], row["y_um"]] for row in rows])
        distances.append(
            jnp.asarray(
                np.linalg.norm(
                    coordinates[:, None, :] - coordinates[None, :, :], axis=2
                )
            )
        )
        selections.append(selected)

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
            dist.Normal(0.0, priors["covariate_effect_sd"])
            .expand([types])
            .to_event(1),
        )
        patient_sd = numpyro.sample(
            "patient_sd",
            dist.HalfNormal(priors["patient_sd_scale"]).expand([types]).to_event(1),
        )
        pattern_sd = numpyro.sample(
            "pattern_sd",
            dist.HalfNormal(priors["pattern_sd_scale"]).expand([types]).to_event(1),
        )
        amplitude = numpyro.sample("field_amplitude", dist.HalfNormal(config["amplitude_scale"]))
        length = numpyro.sample("field_length_scale_um", dist.HalfNormal(config["length_scale"]))
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
        blocks = []
        for distance, selected in zip(distances, selections, strict=True):
            scaled = math.sqrt(3.0) * distance / length
            covariance = amplitude**2 * (1.0 + scaled) * jnp.exp(-scaled)
            covariance += config["kernel_jitter"] * jnp.eye(len(selected))
            block = field_raw[:, selected] @ jnp.linalg.cholesky(covariance).T
            blocks.append(block - block.mean(axis=1, keepdims=True))
        latent = numpyro.deterministic("latent_effect", jnp.concatenate(blocks, axis=1))
        log_expected = (
            intercept[:, None]
            + group_effect[:, None] * patient_groups[node_patients][None, :]
            + covariate_effect[:, None] * covariate[None, :]
            + patient_effect[node_patients, :].T
            + pattern_effect[node_patterns, :].T
            + latent
            + jnp.log(weights)[None, :]
        )
        expected_matrix = numpyro.deterministic("expected_matrix", jnp.exp(log_expected))
        expected = numpyro.deterministic(
            "expected_count", expected_matrix[type_index, node_index]
        )
        numpyro.sample("observed_count", dist.Poisson(expected), obs=observed)

    return model


def fit(
    config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str
) -> dict[str, Any]:
    base = load(
        "marklab_numpyro_replicated_arbitrary_window_multitype_lgcp_worker.py",
        "marklab_numpyro_fixed_multitype_helpers",
    )
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
    flat = {name: value.reshape((-1, *value.shape[2:])) for name, value in samples.items()}
    patient_effect = flat["patient_sd"][:, None, :] * flat["patient_raw"]
    pattern_centered = flat["pattern_raw"].copy()
    for patient in range(len(config["patient_ids"])):
        selected = config["pattern_patients"] == patient
        pattern_centered[:, selected, :] -= pattern_centered[:, selected, :].mean(
            axis=1, keepdims=True
        )
    pattern_effect = flat["pattern_sd"][:, None, :] * pattern_centered
    latent = np.asarray(samples["latent_effect"]).reshape(
        (-1, len(config["types"]), len(config["weights"]))
    )
    expected = np.asarray(samples["expected_count"]).reshape(
        (-1, len(config["observed"]))
    )
    type_rows = [
        {
            "type_id": identity,
            "intercept": base.summary(flat["intercept"][:, index]),
            "group_effect": base.summary(flat["group_effect"][:, index]),
            "covariate_effect": base.summary(flat["covariate_effect"][:, index]),
            "patient_sd": base.summary(flat["patient_sd"][:, index]),
            "pattern_sd": base.summary(flat["pattern_sd"][:, index]),
        }
        for index, identity in enumerate(config["types"])
    ]
    differences = [
        {
            "type_a": config["types"][left],
            "type_b": config["types"][right],
            "difference": base.summary(
                flat["group_effect"][:, left] - flat["group_effect"][:, right]
            ),
        }
        for left in range(len(config["types"]))
        for right in range(left + 1, len(config["types"]))
    ]
    patient_rows = [
        {
            "owner_id": owner,
            "type_id": identity,
            "effect": base.summary(patient_effect[:, owner_index, type_index]),
        }
        for owner_index, owner in enumerate(config["patient_ids"])
        for type_index, identity in enumerate(config["types"])
    ]
    pattern_rows = [
        {
            "owner_id": owner,
            "type_id": identity,
            "effect": base.summary(pattern_effect[:, owner_index, type_index]),
        }
        for owner_index, owner in enumerate(config["pattern_ids"])
        for type_index, identity in enumerate(config["types"])
    ]
    node_rows = [
        {
            "pattern_id": config["request"]["nodes"][int(node)]["pattern_id"],
            "node_id": config["request"]["nodes"][int(node)]["node_id"],
            "type_id": config["types"][int(type_index)],
            "latent_effect": base.summary(latent[:, int(type_index), int(node)]),
            "expected_count": base.summary(expected[:, row]),
        }
        for row, (node, type_index) in enumerate(
            zip(config["node_index"], config["type_index"], strict=True)
        )
    ]
    names = [
        "intercept", "group_effect", "covariate_effect", "patient_sd", "pattern_sd",
        "field_amplitude", "field_length_scale_um", "patient_raw", "pattern_raw", "field_raw",
    ]
    posterior = az.from_dict({"posterior": {name: samples[name] for name in names}})
    r_hat = float(base.flattened(az.rhat(posterior, var_names=names, method="rank"), names).max())
    bulk = float(base.flattened(az.ess(posterior, var_names=names, method="bulk"), names).min())
    tail = float(base.flattened(az.ess(posterior, var_names=names, method="tail"), names).min())
    mcse_mean = float(base.flattened(az.mcse(posterior, var_names=names, method="mean"), names).max())
    mcse_sd = float(base.flattened(az.mcse(posterior, var_names=names, method="sd"), names).max())
    extra = sampler.get_extra_fields(group_by_chain=True)
    energy = np.asarray(extra["energy"], dtype=np.float64)
    ebfmi = float(np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1)))
    divergences = int(np.asarray(extra["diverging"]).sum())
    depth_hits = int(
        (np.asarray(extra["num_steps"]) >= 2 ** config["numpyro_maximum_tree_depth"] - 1).sum()
    )
    constraints = bool(
        all(
            np.max(np.abs(pattern_effect[:, config["pattern_patients"] == patient, :].sum(axis=1)))
            < 1e-10
            for patient in range(len(config["patient_ids"]))
        )
        and all(
            np.max(np.abs(latent[:, :, config["node_patterns"] == pattern].sum(axis=2))) < 1e-10
            for pattern in range(len(config["pattern_ids"]))
        )
    )
    finite = bool(
        all(np.isfinite(value).all() for value in (patient_effect, pattern_effect, latent, expected))
    )
    policy = config["policy"]
    complete = bool(
        finite and constraints and r_hat <= policy["maximum_r_hat"]
        and bulk >= policy["minimum_bulk_ess"] and tail >= policy["minimum_tail_ess"]
        and ebfmi >= policy["minimum_ebfmi"] and divergences <= policy["maximum_divergences"]
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
        "source_request_sha256": config["source_request_sha256"],
        "maximum_tree_depth": config["numpyro_maximum_tree_depth"],
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"], "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"],
            "completed_draws": config["chains"] * config["draws"],
        },
        "kernel_posterior": {
            "field_amplitude": base.summary(flat["field_amplitude"]),
            "field_length_scale_um": base.summary(flat["field_length_scale_um"]),
        },
        "type_posteriors": type_rows,
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
    pymc_inferred_sha = hashlib.sha256(
        script.with_name(
            "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_worker.py"
        ).read_bytes()
    ).hexdigest()
    pymc_fixed_sha = hashlib.sha256(
        script.with_name(
            "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_worker.py"
        ).read_bytes()
    ).hexdigest()
    raw = sys.stdin.buffer.read(64 * 1_048_576 + 1)
    if not raw or len(raw) > 64 * 1_048_576:
        raise ContractError("request size is invalid")
    request_sha = hashlib.sha256(raw).hexdigest()
    _, config = validate(
        json.loads(raw), lock_sha, worker_sha, pymc_inferred_sha, pymc_fixed_sha
    )
    output = json.dumps(
        fit(config, request_sha, lock_sha, worker_sha),
        allow_nan=False, sort_keys=True, separators=(",", ":"),
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
            f"marklab NumPyro inferred multitype LGCP failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
