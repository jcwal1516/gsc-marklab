#!/usr/bin/env python3
"""NumPyro agreement worker for the inferred-kernel replicated patient LGCP."""

from __future__ import annotations

import hashlib
import importlib.util
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


REQUEST_FORMAT = "marklab.numpyro_replicated_arbitrary_window_lgcp_inferred_kernel_request"
RESULT_FORMAT = "marklab.numpyro_replicated_arbitrary_window_lgcp_inferred_kernel_result"
NUMPYRO_VERSION = "0.21.0"
JAX_VERSION = "0.11.1"


class ContractError(ValueError):
    pass


def load_module(filename: str, name: str) -> Any:
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
    lock_digest: str,
    worker_digest: str,
    pymc_worker_digest: str,
    fixed_worker_digest: str,
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
    exact(backend["environment_lock_sha256"], lock_digest, "backend.lock")
    exact(backend["worker_sha256"], worker_digest, "backend.worker")
    exact(request["jax_version"], JAX_VERSION, "request.jax_version")
    maximum_tree_depth = request["maximum_tree_depth"]
    if (
        isinstance(maximum_tree_depth, bool)
        or not isinstance(maximum_tree_depth, int)
        or not 10 <= maximum_tree_depth <= 14
    ):
        raise ContractError("maximum_tree_depth is outside [10, 14]")
    source_sha = request["source_request_sha256"]
    if not isinstance(source_sha, str) or len(source_sha) != 64:
        raise ContractError("source request digest is invalid")
    pymc = load_module(
        "marklab_pymc_replicated_arbitrary_window_lgcp_inferred_kernel_worker.py",
        "marklab_pymc_inferred_kernel_contract",
    )
    config = pymc.validate(
        request["source_request"], lock_digest, pymc_worker_digest, fixed_worker_digest
    )
    config["numpyro_maximum_tree_depth"] = maximum_tree_depth
    return request, config


def seed_for(seed: int, purpose: str) -> int:
    digest = hashlib.sha256(
        f"marklab-numpyro-replicated-inferred-kernel-v1\0{seed}\0{purpose}".encode()
    ).digest()
    return int.from_bytes(digest[:4], "little")


def make_model(config: dict[str, Any]) -> Callable[[], None]:
    patient_index = jnp.asarray(config["patient_index"])
    pattern_index = jnp.asarray(config["pattern_index"])
    pattern_patient_index = jnp.asarray(config["pattern_patient_index"])
    group = jnp.asarray(config["group"])
    covariate = jnp.asarray(config["covariate"])
    weight = jnp.asarray(config["weight"])
    counts = jnp.asarray(config["counts"])
    priors = config["priors"]
    slices = []
    distances = []
    start = 0
    for pattern in range(len(config["pattern_ids"])):
        selected = np.flatnonzero(config["pattern_index"] == pattern)
        size = len(selected)
        nodes = [config["request"]["nodes"][index] for index in selected]
        coordinates = np.asarray([[row["x_um"], row["y_um"]] for row in nodes])
        distances.append(jnp.asarray(np.linalg.norm(
            coordinates[:, None, :] - coordinates[None, :, :], axis=2
        )))
        slices.append((start, start + size))
        start += size

    def model() -> None:
        intercept = numpyro.sample("intercept", dist.Normal(priors["intercept_mean"], priors["intercept_sd"]))
        group_effect = numpyro.sample("group_effect", dist.Normal(0.0, priors["group_effect_sd"]))
        covariate_effect = numpyro.sample("covariate_effect", dist.Normal(0.0, priors["covariate_effect_sd"]))
        patient_sd = numpyro.sample("patient_sd", dist.HalfNormal(priors["patient_sd_scale"]))
        pattern_sd = numpyro.sample("pattern_sd", dist.HalfNormal(priors["pattern_sd_scale"]))
        amplitude = numpyro.sample("field_amplitude", dist.HalfNormal(config["field_amplitude_scale"]))
        length = numpyro.sample("field_length_scale_um", dist.HalfNormal(config["field_length_scale_scale_um"]))
        patient_raw = numpyro.sample("patient_raw", dist.Normal(0.0, 1.0).expand([len(config["patient_ids"])]).to_event(1))
        pattern_raw = numpyro.sample("pattern_raw", dist.Normal(0.0, 1.0).expand([len(config["pattern_ids"])]).to_event(1))
        field_raw = numpyro.sample("field_raw", dist.Normal(0.0, 1.0).expand([len(config["counts"])]).to_event(1))
        patient_effect = numpyro.deterministic("patient_effect", patient_sd * patient_raw)
        sums = jnp.zeros(len(config["patient_ids"])).at[pattern_patient_index].add(pattern_raw)
        sizes = jnp.zeros(len(config["patient_ids"])).at[pattern_patient_index].add(1.0)
        pattern_effect = numpyro.deterministic(
            "pattern_effect", pattern_sd * (pattern_raw - (sums / sizes)[pattern_patient_index])
        )
        blocks = []
        for (begin, end), distance in zip(slices, distances, strict=True):
            scaled = math.sqrt(3.0) * distance / length
            covariance = amplitude**2 * (1.0 + scaled) * jnp.exp(-scaled)
            covariance += config["kernel_jitter"] * jnp.eye(end - begin)
            block = jnp.linalg.cholesky(covariance) @ field_raw[begin:end]
            blocks.append(block - jnp.mean(block))
        latent = numpyro.deterministic("latent_effect", jnp.concatenate(blocks))
        expected = numpyro.deterministic(
            "expected_count",
            weight * jnp.exp(
                intercept + group_effect * group[patient_index]
                + covariate_effect * covariate + patient_effect[patient_index]
                + pattern_effect[pattern_index] + latent
            ),
        )
        numpyro.sample("observed_count", dist.Poisson(expected), obs=counts)

    return model


def summary(values: np.ndarray) -> dict[str, float]:
    values = np.asarray(values, dtype=np.float64).reshape(-1)
    return {
        "mean": float(values.mean()), "sd": float(values.std(ddof=1)),
        "interval_lower": float(np.quantile(values, 0.025)),
        "interval_upper": float(np.quantile(values, 0.975)),
    }


def flattened(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def fit(config: dict[str, Any], request_sha: str, source_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    kernel = NUTS(
        make_model(config), target_accept_prob=config["target_accept"],
        max_tree_depth=config["numpyro_maximum_tree_depth"],
    )
    sampler = MCMC(
        kernel, num_warmup=config["tune"], num_samples=config["draws"],
        num_chains=config["chains"], chain_method="sequential", progress_bar=False,
    )
    sampler.run(
        jax.random.PRNGKey(seed_for(config["seed"], "nuts")),
        extra_fields=("diverging", "num_steps", "energy"),
    )
    samples = {name: np.asarray(value, dtype=np.float64) for name, value in sampler.get_samples(group_by_chain=True).items()}
    posterior = az.from_dict({"posterior": samples})
    names = [
        "intercept", "group_effect", "covariate_effect", "patient_sd", "pattern_sd",
        "field_amplitude", "field_length_scale_um", "patient_raw", "pattern_raw", "field_raw",
    ]
    r_hat = float(flattened(az.rhat(posterior, var_names=names, method="rank"), names).max())
    bulk = float(flattened(az.ess(posterior, var_names=names, method="bulk"), names).min())
    tail = float(flattened(az.ess(posterior, var_names=names, method="tail"), names).min())
    mcse_mean = float(flattened(az.mcse(posterior, var_names=names, method="mean"), names).max())
    mcse_sd = float(flattened(az.mcse(posterior, var_names=names, method="sd"), names).max())
    extra = sampler.get_extra_fields(group_by_chain=True)
    energy = np.asarray(extra["energy"], dtype=np.float64)
    ebfmi = float(np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1)))
    divergences = int(np.asarray(extra["diverging"]).sum())
    depth_hits = int((np.asarray(extra["num_steps"]) >= 2 ** config["numpyro_maximum_tree_depth"] - 1).sum())
    patient_effect = samples["patient_sd"][..., None] * samples["patient_raw"]
    pattern_centered = samples["pattern_raw"].copy()
    for patient in range(len(config["patient_ids"])):
        selected = config["pattern_patient_index"] == patient
        pattern_centered[..., selected] -= pattern_centered[..., selected].mean(axis=-1, keepdims=True)
    pattern_effect = samples["pattern_sd"][..., None] * pattern_centered
    latent = np.asarray(samples["latent_effect"])
    expected = np.asarray(samples["expected_count"])
    policy = config["policy"]
    prior_finite = load_module(
        "marklab_pymc_replicated_arbitrary_window_lgcp_inferred_kernel_worker.py",
        "marklab_pymc_inferred_prior",
    ).prior_is_finite(config)
    finite = all(np.isfinite(value).all() for value in samples.values())
    complete = (
        prior_finite and finite and r_hat <= policy["maximum_r_hat"]
        and bulk >= policy["minimum_bulk_ess"] and tail >= policy["minimum_tail_ess"]
        and ebfmi >= policy["minimum_ebfmi"] and divergences == 0 and depth_hits == 0
    )
    flat_latent = latent.reshape(-1, latent.shape[-1])
    flat_expected = expected.reshape(-1, expected.shape[-1])
    return {
        "format": RESULT_FORMAT, "version": 1,
        "backend": {
            "name": "numpyro", "version": numpyro.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha, "worker_sha256": worker_sha,
        },
        "jax_version": jax.__version__, "request_sha256": request_sha,
        "source_request_sha256": source_sha,
        "input_sha256": config["request"]["input_sha256"],
        "maximum_tree_depth": config["numpyro_maximum_tree_depth"],
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"], "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"], "completed_draws": config["chains"] * config["draws"],
        },
        "posterior": {name: summary(samples[name]) for name in names[:7]},
        "patient_effects": [
            {"patient_id": identity, "effect": summary(patient_effect[..., index])}
            for index, identity in enumerate(config["patient_ids"])
        ],
        "pattern_effects": [
            {"pattern_id": identity, "effect": summary(pattern_effect[..., index])}
            for index, identity in enumerate(config["pattern_ids"])
        ],
        "nodes": [
            {
                "pattern_id": config["request"]["nodes"][index]["pattern_id"],
                "node_id": config["request"]["nodes"][index]["node_id"],
                "latent_effect": summary(flat_latent[:, index]),
                "expected_count": summary(flat_expected[:, index]),
            }
            for index in range(len(config["counts"]))
        ],
        "diagnostics": {
            "prior_predictive_finite": prior_finite, "posterior_finite": finite,
            "r_hat": r_hat, "ess_bulk": bulk, "ess_tail": tail,
            "mcse_mean": mcse_mean, "mcse_sd": mcse_sd, "minimum_ebfmi": ebfmi,
            "divergences": divergences, "max_tree_depth_hits": depth_hits,
            "constraints_valid": finite, "identifiability_checks_passed": True,
        },
    }


def main() -> None:
    if numpyro.__version__ != NUMPYRO_VERSION or jax.__version__ != JAX_VERSION or sys.version_info[:2] != (3, 12):
        raise ContractError("NumPyro, JAX, or Python version drift")
    script = Path(__file__)
    lock = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker = hashlib.sha256(script.read_bytes()).hexdigest()
    pymc_worker = hashlib.sha256(script.with_name(
        "marklab_pymc_replicated_arbitrary_window_lgcp_inferred_kernel_worker.py"
    ).read_bytes()).hexdigest()
    fixed_worker = hashlib.sha256(script.with_name(
        "marklab_pymc_replicated_arbitrary_window_lgcp_worker.py"
    ).read_bytes()).hexdigest()
    raw = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if not raw or len(raw) > 16 * 1024 * 1024:
        raise ContractError("request size is invalid")
    request_sha = hashlib.sha256(raw).hexdigest()
    request, config = validate(json.loads(raw), lock, worker, pymc_worker, fixed_worker)
    result = fit(config, request_sha, request["source_request_sha256"], lock, worker)
    sys.stdout.write(json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")) + "\n")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"marklab NumPyro inferred-kernel LGCP failed: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from error
