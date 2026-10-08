#!/usr/bin/env python3
"""Exact finite-state SBC for the conditional hard-multitype pseudoposterior."""

from __future__ import annotations

import hashlib
import importlib.util
import itertools
import json
import math
from pathlib import Path
import sys
from typing import Any

import arviz as az
import jax
jax.config.update("jax_enable_x64", True)
import numpy as np
import numpyro
from numpyro.infer import MCMC, NUTS
from scipy import stats


REQUEST_FORMAT = "marklab.numpyro_conditional_multitype_mark_sbc_request"
RESULT_FORMAT = "marklab.numpyro_conditional_multitype_mark_sbc_result"
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


def number(value: Any, path: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ContractError(f"{path} must be numeric")
    result = float(value)
    if not math.isfinite(result):
        raise ContractError(f"{path} must be finite")
    return result


def integer(value: Any, path: str, low: int, high: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not low <= value <= high:
        raise ContractError(f"{path} must be an integer in [{low}, {high}]")
    return value


def validate(request: Any, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format", "version", "backend", "jax_version", "source_request_sha256",
            "source_request", "calibration", "resources",
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
    if not isinstance(source_sha, str) or len(source_sha) != 64:
        raise ContractError("source request digest is invalid")
    pymc_worker_sha = hashlib.sha256(
        Path(__file__).with_name("marklab_pymc_conditional_multitype_mark_worker.py").read_bytes()
    ).hexdigest()
    pymc = load_module(
        "marklab_pymc_conditional_multitype_mark_worker.py",
        "marklab_pymc_conditional_multitype_sbc_contract",
    )
    config = pymc.validate(request["source_request"], lock_sha, pymc_worker_sha)
    if len(config["observed"]) > 10 or len(config["type_ids"]) > 4:
        raise ContractError("exact SBC requires at most ten sites and four types")
    calibration = obj(
        request["calibration"],
        {
            "replicates", "rank_bins", "interval_probability",
            "minimum_rank_uniformity_p_value", "minimum_coverage", "maximum_coverage",
        },
        "calibration",
    )
    config["replicates"] = integer(calibration["replicates"], "calibration.replicates", 20, 100)
    config["rank_bins"] = integer(calibration["rank_bins"], "calibration.rank_bins", 2, 20)
    exact(number(calibration["interval_probability"], "calibration.interval"), 0.9, "interval")
    config["minimum_rank_p"] = number(
        calibration["minimum_rank_uniformity_p_value"], "calibration.minimum_rank_p"
    )
    config["minimum_coverage"] = number(calibration["minimum_coverage"], "calibration.minimum_coverage")
    config["maximum_coverage"] = number(calibration["maximum_coverage"], "calibration.maximum_coverage")
    if not 0 <= config["minimum_rank_p"] <= 1 or not 0 <= config["minimum_coverage"] <= config["maximum_coverage"] <= 1:
        raise ContractError("calibration thresholds are invalid")
    resources = obj(
        request["resources"],
        {
            "maximum_states", "state_count", "maximum_enumeration_work", "enumeration_work",
            "maximum_enumeration_bytes", "enumeration_bytes", "maximum_total_iterations",
            "maximum_output_bytes", "timeout_seconds",
        },
        "resources",
    )
    state_count = len(config["type_ids"]) ** len(config["observed"])
    exact(resources["state_count"], state_count, "resources.state_count")
    if state_count > integer(resources["maximum_states"], "resources.maximum_states", 1, 1_000_000):
        raise ContractError("state ceiling exceeded")
    enumeration_work = state_count * (len(config["observed"]) + len(config["edges"]))
    exact(resources["enumeration_work"], enumeration_work, "resources.enumeration_work")
    if enumeration_work > integer(
        resources["maximum_enumeration_work"], "resources.maximum_enumeration_work", 1, 100_000_000
    ):
        raise ContractError("enumeration-work ceiling exceeded")
    enumeration_bytes = state_count * len(config["observed"]) * 2 + state_count * 16
    exact(resources["enumeration_bytes"], enumeration_bytes, "resources.enumeration_bytes")
    if enumeration_bytes > integer(
        resources["maximum_enumeration_bytes"], "resources.maximum_enumeration_bytes", 1, 1024**3
    ):
        raise ContractError("enumeration-byte ceiling exceeded")
    iterations = config["replicates"] * config["chains"] * (config["tune"] + config["draws"])
    if iterations > integer(
        resources["maximum_total_iterations"], "resources.maximum_total_iterations", 1, 10_000_000
    ):
        raise ContractError("total-iteration ceiling exceeded")
    config["maximum_output_bytes"] = integer(
        resources["maximum_output_bytes"], "resources.maximum_output_bytes", 1, 2 * 1_048_576
    )
    config["source_request_sha256"] = source_sha
    config["state_count"] = state_count
    config["enumeration_work"] = enumeration_work
    config["enumeration_bytes"] = enumeration_bytes
    return config


def seed_for(seed: int, purpose: str, replicate: int) -> int:
    value = hashlib.sha256(
        f"marklab-conditional-multitype-sbc-v1\0{seed}\0{purpose}\0{replicate}".encode()
    ).digest()
    return int.from_bytes(value[:4], "little")


def parameter_names(type_ids: list[str]) -> list[str]:
    names = [f"intercept:{identity}" for identity in type_ids[1:]]
    for left in range(len(type_ids)):
        for right in range(left, len(type_ids)):
            if left != 0 or right != 0:
                names.append(f"potential:{type_ids[left]}|{type_ids[right]}")
    for left in range(len(type_ids)):
        for right in range(left + 1, len(type_ids)):
            names.append(f"affinity:{type_ids[left]}|{type_ids[right]}")
    return names


def matrices(intercept_free: np.ndarray, potential_free: np.ndarray, types: int) -> tuple[np.ndarray, np.ndarray]:
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


def state_table(config: dict[str, Any]) -> np.ndarray:
    return np.asarray(
        list(itertools.product(range(len(config["type_ids"])), repeat=len(config["observed"]))),
        dtype=np.int16,
    )


def simulate(
    config: dict[str, Any], states: np.ndarray, replicate: int
) -> tuple[dict[str, float], np.ndarray, np.ndarray]:
    rng = np.random.default_rng(seed_for(config["seed"], "simulate", replicate))
    types = len(config["type_ids"])
    free_pairs = types * (types + 1) // 2 - 1
    intercept_free = rng.normal(0.0, config["intercept_sd"], types - 1)
    potential_free = rng.normal(0.0, config["interaction_sd"], free_pairs)
    intercept, potential = matrices(intercept_free, potential_free, types)
    intercept = intercept.reshape(types)
    potential = potential.reshape(types, types)
    energy = intercept[states].sum(axis=1)
    for left, right in config["edges"]:
        energy += potential[states[:, left], states[:, right]]
    shifted = energy - energy.max()
    weights = np.exp(shifted)
    probability = weights / weights.sum()
    observed = states[rng.choice(len(states), p=probability)].astype(np.int64)
    counts = np.zeros((len(observed), types), dtype=np.float64)
    for left, right in config["edges"]:
        counts[left, observed[right]] += 1
        counts[right, observed[left]] += 1
    truth: dict[str, float] = {}
    for index, identity in enumerate(config["type_ids"][1:]):
        truth[f"intercept:{identity}"] = float(intercept_free[index])
    index = 0
    for left in range(types):
        for right in range(left, types):
            if left == 0 and right == 0:
                continue
            truth[f"potential:{config['type_ids'][left]}|{config['type_ids'][right]}"] = float(potential_free[index])
            index += 1
    for left in range(types):
        for right in range(left + 1, types):
            truth[f"affinity:{config['type_ids'][left]}|{config['type_ids'][right]}"] = float(
                potential[left, right] - 0.5 * (potential[left, left] + potential[right, right])
            )
    return truth, observed, counts


def flattened(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def run_sbc(config: dict[str, Any], states: np.ndarray) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    model_module = load_module(
        "marklab_numpyro_conditional_multitype_mark_worker.py",
        "marklab_numpyro_conditional_multitype_sbc_model",
    )
    completed: list[dict[str, Any]] = []
    failures: list[dict[str, Any]] = []
    names = parameter_names(config["type_ids"])
    for replicate in range(config["replicates"]):
        try:
            truth, observed, counts = simulate(config, states, replicate)
            fit_config = dict(config)
            fit_config["observed"] = observed
            fit_config["neighbor_counts"] = counts
            sampler = MCMC(
                NUTS(
                    model_module.make_model(fit_config),
                    target_accept_prob=config["target_accept"],
                    max_tree_depth=config["request"]["resources"]["maximum_tree_depth"],
                ),
                num_warmup=config["tune"], num_samples=config["draws"],
                num_chains=config["chains"], chain_method="sequential", progress_bar=False,
            )
            sampler.run(
                jax.random.PRNGKey(seed_for(config["seed"], "fit", replicate)),
                extra_fields=("diverging", "num_steps", "energy"),
            )
            samples = {
                key: np.asarray(value, dtype=np.float64)
                for key, value in sampler.get_samples(group_by_chain=True).items()
            }
            intercept, potential = matrices(
                samples["intercept_free"], samples["potential_free"], len(config["type_ids"])
            )
            draws_by_name: dict[str, np.ndarray] = {}
            for index, identity in enumerate(config["type_ids"][1:]):
                draws_by_name[f"intercept:{identity}"] = samples["intercept_free"][..., index].reshape(-1)
            index = 0
            for left in range(len(config["type_ids"])):
                for right in range(left, len(config["type_ids"])):
                    if left == 0 and right == 0:
                        continue
                    key = f"potential:{config['type_ids'][left]}|{config['type_ids'][right]}"
                    draws_by_name[key] = samples["potential_free"][..., index].reshape(-1)
                    index += 1
            for left in range(len(config["type_ids"])):
                for right in range(left + 1, len(config["type_ids"])):
                    key = f"affinity:{config['type_ids'][left]}|{config['type_ids'][right]}"
                    draws_by_name[key] = (
                        potential[..., left, right]
                        - 0.5 * (potential[..., left, left] + potential[..., right, right])
                    ).reshape(-1)
            posterior = az.from_dict({"posterior": samples})
            raw_names = ["intercept_free", "potential_free"]
            r_hat = float(flattened(az.rhat(posterior, var_names=raw_names, method="rank"), raw_names).max())
            bulk = float(flattened(az.ess(posterior, var_names=raw_names, method="bulk"), raw_names).min())
            tail = float(flattened(az.ess(posterior, var_names=raw_names, method="tail"), raw_names).min())
            extra = sampler.get_extra_fields(group_by_chain=True)
            energy = np.asarray(extra["energy"], dtype=np.float64)
            ebfmi = float(np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1)))
            divergences = int(np.asarray(extra["diverging"]).sum())
            maximum_steps = 2 ** config["request"]["resources"]["maximum_tree_depth"] - 1
            depth_hits = int((np.asarray(extra["num_steps"]) >= maximum_steps).sum())
            if not (
                r_hat <= config["maximum_r_hat"] and bulk >= config["minimum_bulk_ess"]
                and tail >= config["minimum_tail_ess"] and ebfmi >= config["minimum_ebfmi"]
                and divergences <= config["maximum_divergences"]
                and depth_hits <= config["maximum_tree_depth_hits"]
            ):
                failures.append({
                    "replicate": replicate,
                    "reason": (
                        f"diagnostics_failed:r_hat={r_hat:.17g},bulk={bulk:.17g},tail={tail:.17g},"
                        f"ebfmi={ebfmi:.17g},divergences={divergences},depth_hits={depth_hits}"
                    ),
                })
                continue
            parameters = []
            for name in names:
                values = draws_by_name[name]
                parameters.append({
                    "parameter": name, "truth": truth[name],
                    "rank": int(np.sum(values < truth[name])),
                    "covered": bool(np.quantile(values, 0.05) <= truth[name] <= np.quantile(values, 0.95)),
                })
            completed.append({
                "replicate": replicate,
                "simulated_type_counts": np.bincount(observed, minlength=len(config["type_ids"])).tolist(),
                "parameters": parameters, "r_hat": r_hat, "ess_bulk": bulk, "ess_tail": tail,
                "minimum_ebfmi": ebfmi, "divergences": divergences,
                "max_tree_depth_hits": depth_hits,
            })
        except Exception as error:
            failures.append({"replicate": replicate, "reason": f"{type(error).__name__}:{str(error)[:400]}"})
    return completed, failures


def aggregate(completed: list[dict[str, Any]], name: str, draws: int, bins: int) -> dict[str, Any]:
    rows = [next(value for value in row["parameters"] if value["parameter"] == name) for row in completed]
    ranks = np.asarray([row["rank"] for row in rows], dtype=np.int64)
    histogram = np.zeros(bins, dtype=np.int64)
    for value in ranks:
        histogram[min(int(value) * bins // (draws + 1), bins - 1)] += 1
    return {
        "parameter": name,
        "rank_histogram": histogram.tolist(),
        "rank_uniformity_p_value": float(stats.chisquare(histogram).pvalue) if rows else 0.0,
        "coverage_90": float(np.mean([row["covered"] for row in rows])) if rows else 0.0,
        "mean_normalized_rank": float(ranks.mean() / draws) if rows else 0.0,
    }


def main() -> int:
    if numpyro.__version__ != NUMPYRO_VERSION or jax.__version__ != JAX_VERSION or sys.version_info[:2] != (3, 12):
        raise ContractError("NumPyro, JAX, or Python version drift")
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    raw = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if not raw or len(raw) > 16 * 1024 * 1024:
        raise ContractError("request size is invalid")
    request_sha = hashlib.sha256(raw).hexdigest()
    config = validate(json.loads(raw), lock_sha, worker_sha)
    states = state_table(config)
    completed, failures = run_sbc(config, states)
    draws = config["chains"] * config["draws"]
    diagnostics = [
        aggregate(completed, name, draws, config["rank_bins"])
        for name in parameter_names(config["type_ids"])
    ]
    passes = not failures and all(
        config["minimum_rank_p"] <= row["rank_uniformity_p_value"]
        and config["minimum_coverage"] <= row["coverage_90"] <= config["maximum_coverage"]
        for row in diagnostics
    )
    result = {
        "format": RESULT_FORMAT, "version": 1,
        "backend": {
            "name": "numpyro", "version": numpyro.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha, "worker_sha256": worker_sha,
        },
        "jax_version": jax.__version__, "input_sha256": config["input_sha"],
        "request_sha256": request_sha, "source_request_sha256": config["source_request_sha256"],
        "fit_state": "complete" if passes else "nonconverged",
        "enumeration_oracle": {
            "state_count": len(states),
            "zero_parameter_log_normalizer": float(len(config["observed"]) * math.log(len(config["type_ids"]))),
        },
        "replicates": completed, "failures": failures, "diagnostics": diagnostics,
    }
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
            f"marklab conditional multitype SBC failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
