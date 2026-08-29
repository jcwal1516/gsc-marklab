#!/usr/bin/env python3
"""Independent NumPyro agreement worker for conditional hard multitype marks."""

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


REQUEST_FORMAT = "marklab.numpyro_conditional_multitype_mark_request"
RESULT_FORMAT = "marklab.numpyro_conditional_multitype_mark_result"
NUMPYRO_VERSION = "0.21.0"
JAX_VERSION = "0.11.1"


class ContractError(ValueError):
    pass


def load_pymc() -> Any:
    path = Path(__file__).with_name("marklab_pymc_conditional_multitype_mark_worker.py")
    specification = importlib.util.spec_from_file_location("marklab_pymc_conditional_mark_contract", path)
    if specification is None or specification.loader is None:
        raise ContractError("cannot load PyMC conditional-mark contract")
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
    exact(backend["python_version"], "3.12", "backend.python")
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
        f"marklab-numpyro-conditional-multitype-mark-v1\0{seed}\0{purpose}".encode()
    ).digest()
    return int.from_bytes(value[:4], "little")


def make_model(config: dict[str, Any]) -> Any:
    types = len(config["type_ids"])
    free_pairs = types * (types + 1) // 2 - 1
    counts = jnp.asarray(config["neighbor_counts"])
    observed = jnp.asarray(config["observed"])

    def model() -> None:
        intercept_free = numpyro.sample(
            "intercept_free",
            dist.Normal(0.0, config["intercept_sd"]).expand([types - 1]).to_event(1),
        )
        potential_free = numpyro.sample(
            "potential_free",
            dist.Normal(0.0, config["interaction_sd"]).expand([free_pairs]).to_event(1),
        )
        intercept = jnp.concatenate([jnp.zeros(1), intercept_free])
        potential = jnp.zeros((types, types))
        index = 0
        for left in range(types):
            for right in range(left, types):
                if left == 0 and right == 0:
                    continue
                potential = potential.at[left, right].set(potential_free[index])
                potential = potential.at[right, left].set(potential_free[index])
                index += 1
        logits = intercept + counts @ potential.T
        numpyro.sample("observed_type", dist.Categorical(logits=logits), obs=observed)

    return model


def summary(values: np.ndarray) -> dict[str, float]:
    flat = np.asarray(values, dtype=np.float64).reshape(-1)
    return {
        "mean": float(flat.mean()), "sd": float(flat.std(ddof=1)),
        "interval_lower": float(np.quantile(flat, 0.025)),
        "interval_upper": float(np.quantile(flat, 0.975)),
    }


def matrices(
    intercept_free: np.ndarray, potential_free: np.ndarray, types: int
) -> tuple[np.ndarray, np.ndarray]:
    intercept = np.zeros((*intercept_free.shape[:-1], types), dtype=np.float64)
    intercept[..., 1:] = intercept_free
    potential = np.zeros((*potential_free.shape[:-1], types, types), dtype=np.float64)
    index = 0
    for left in range(types):
        for right in range(left, types):
            if left == 0 and right == 0:
                continue
            potential[..., left, right] = potential_free[..., index]
            potential[..., right, left] = potential_free[..., index]
            index += 1
    return intercept, potential


def softmax(values: np.ndarray) -> np.ndarray:
    shifted = values - values.max(axis=-1, keepdims=True)
    weights = np.exp(shifted)
    return weights / weights.sum(axis=-1, keepdims=True)


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
    samples = {
        name: np.asarray(value, dtype=np.float64)
        for name, value in sampler.get_samples(group_by_chain=True).items()
    }
    types = len(config["type_ids"])
    intercept, potential = matrices(samples["intercept_free"], samples["potential_free"], types)
    flat_intercept = intercept.reshape(-1, types)
    flat_potential = potential.reshape(-1, types, types)
    draws = len(flat_intercept)
    scores = np.empty(draws, dtype=np.float64)
    expected_counts = np.empty((draws, types), dtype=np.float64)
    expected_same = np.empty(draws, dtype=np.float64)
    edges = config["edges"]
    for begin in range(0, draws, 128):
        end = min(begin + 128, draws)
        logits = flat_intercept[begin:end, None, :] + np.einsum(
            "nl,dkl->dnk", config["neighbor_counts"], flat_potential[begin:end]
        )
        probability = softmax(logits)
        scores[begin:end] = np.log(
            probability[:, np.arange(len(config["observed"])), config["observed"]]
        ).sum(axis=1)
        expected_counts[begin:end] = probability.sum(axis=1)
        expected_same[begin:end] = np.sum(
            probability[:, edges[:, 0], :] * probability[:, edges[:, 1], :], axis=(1, 2)
        )
    observed_counts = np.bincount(config["observed"], minlength=types)
    proportions = observed_counts / observed_counts.sum()
    null_score = float(np.sum(observed_counts * np.log(proportions)))
    observed_same = int(np.sum(config["observed"][edges[:, 0]] == config["observed"][edges[:, 1]]))
    posterior = az.from_dict({"posterior": samples})
    names = ["intercept_free", "potential_free"]
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
    prior_rng = np.random.default_rng(seed_for(config["seed"], "prior"))
    prior_intercept = prior_rng.normal(0.0, config["intercept_sd"], (500, types - 1))
    free_pairs = types * (types + 1) // 2 - 1
    prior_potential = prior_rng.normal(0.0, config["interaction_sd"], (500, free_pairs))
    prior_finite = bool(np.isfinite(prior_intercept).all() and np.isfinite(prior_potential).all())
    posterior_finite = bool(
        np.isfinite(intercept).all() and np.isfinite(potential).all()
        and np.isfinite(scores).all() and np.isfinite(expected_counts).all()
        and np.isfinite(expected_same).all()
    )
    complete = bool(
        prior_finite and posterior_finite and r_hat <= config["maximum_r_hat"]
        and bulk >= config["minimum_bulk_ess"] and tail >= config["minimum_tail_ess"]
        and ebfmi >= config["minimum_ebfmi"]
        and divergences <= config["maximum_divergences"]
        and depth_hits <= config["maximum_tree_depth_hits"]
    )
    type_intercepts = [
        {"type_id": identity, "intercept": summary(flat_intercept[:, index])}
        for index, identity in enumerate(config["type_ids"])
    ]
    pair_potentials = []
    for left in range(types):
        for right in range(left, types):
            pair_potentials.append({
                "type_a": config["type_ids"][left], "type_b": config["type_ids"][right],
                "potential": summary(flat_potential[:, left, right]),
            })
    affinities = []
    for left in range(types):
        for right in range(left + 1, types):
            contrast = flat_potential[:, left, right] - 0.5 * (
                flat_potential[:, left, left] + flat_potential[:, right, right]
            )
            affinities.append({
                "type_a": config["type_ids"][left], "type_b": config["type_ids"][right],
                "contrast": summary(contrast),
            })
    return {
        "format": RESULT_FORMAT, "version": 1,
        "backend": {
            "name": "numpyro", "version": numpyro.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha, "worker_sha256": worker_sha,
        },
        "jax_version": jax.__version__, "input_sha256": config["input_sha"],
        "request_sha256": request_sha, "source_request_sha256": source_sha,
        "maximum_tree_depth": config["numpyro_maximum_tree_depth"],
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"], "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"], "completed_draws": draws,
        },
        "type_intercepts": type_intercepts, "pair_potentials": pair_potentials,
        "pair_affinity_contrasts": affinities,
        "comparison": {
            "independent_label_composite_log_score": null_score,
            "spatial_composite_log_score": summary(scores),
            "mean_composite_log_score_improvement": float(scores.mean() - null_score),
        },
        "posterior_predictive": {
            "mode": "one_step_conditionals_given_observed_neighbors",
            "observed_same_type_edges": observed_same,
            "expected_same_type_edges": summary(expected_same),
            "observed_type_counts": observed_counts.astype(int).tolist(),
            "expected_type_counts": [summary(expected_counts[:, index]) for index in range(types)],
        },
        "diagnostics": {
            "prior_predictive_finite": prior_finite, "posterior_finite": posterior_finite,
            "r_hat": r_hat, "ess_bulk": bulk, "ess_tail": tail,
            "mcse_mean": mcse_mean, "mcse_sd": mcse_sd, "minimum_ebfmi": ebfmi,
            "divergences": divergences, "max_tree_depth_hits": depth_hits,
            "constraints_valid": posterior_finite,
            "identifiability_checks_passed": bool(
                np.all(intercept[..., 0] == 0.0) and np.all(potential[..., 0, 0] == 0.0)
            ),
        },
    }


def main() -> int:
    if numpyro.__version__ != NUMPYRO_VERSION or jax.__version__ != JAX_VERSION or sys.version_info[:2] != (3, 12):
        raise ContractError("NumPyro, JAX, or Python version drift")
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    pymc_worker_sha = hashlib.sha256(
        script.with_name("marklab_pymc_conditional_multitype_mark_worker.py").read_bytes()
    ).hexdigest()
    raw = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if not raw or len(raw) > 16 * 1024 * 1024:
        raise ContractError("request size is invalid")
    request_sha = hashlib.sha256(raw).hexdigest()
    request, config = validate(json.loads(raw), lock_sha, worker_sha, pymc_worker_sha)
    result = fit(config, request_sha, request["source_request_sha256"], lock_sha, worker_sha)
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
            f"marklab NumPyro conditional multitype mark failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
