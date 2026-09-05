#!/usr/bin/env python3
"""Pinned PyMC conditional pseudolikelihood for hard multitype marks."""

from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path
import sys
from typing import Any

import numpy as np
import pymc as pm
import pytensor.tensor as pt


REQUEST_FORMAT = "marklab.pymc_conditional_multitype_mark_request"
RESULT_FORMAT = "marklab.pymc_conditional_multitype_mark_result"
PYMC_VERSION = "6.3.0"


class ContractError(ValueError):
    pass


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


def digest(value: Any, path: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise ContractError(f"{path} is not lowercase SHA-256")
    return value


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format", "version", "backend", "input_sha256", "type_ids", "reference_type",
            "radius_um", "points", "edges", "priors", "sampling", "diagnostic_policy",
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
    exact(backend["python_version"], "3.12", "backend.python")
    exact(backend["environment_lock_sha256"], lock_digest, "backend.lock")
    exact(backend["worker_sha256"], worker_digest, "backend.worker")
    input_sha = digest(request["input_sha256"], "input_sha256")
    type_ids = request["type_ids"]
    if (
        not isinstance(type_ids, list)
        or not 3 <= len(type_ids) <= 8
        or any(not isinstance(value, str) or not value or len(value) > 128 for value in type_ids)
        or len(set(type_ids)) != len(type_ids)
        or request["reference_type"] != type_ids[0]
    ):
        raise ContractError("type identities or reference are invalid")
    radius = number(request["radius_um"], "radius_um")
    if radius <= 0.0:
        raise ContractError("radius must be positive")
    points = request["points"]
    if not isinstance(points, list) or not 6 <= len(points) <= 10_000:
        raise ContractError("point count is invalid")
    point_ids: list[str] = []
    observed: list[int] = []
    neighbor_counts: list[list[int]] = []
    for index, raw in enumerate(points):
        row = obj(raw, {"point_id", "type_index", "neighbor_counts"}, f"points[{index}]")
        point_id = row["point_id"]
        if not isinstance(point_id, str) or not point_id or len(point_id) > 256:
            raise ContractError("point identity is invalid")
        vector = row["neighbor_counts"]
        if not isinstance(vector, list) or len(vector) != len(type_ids):
            raise ContractError("neighbor-count dimensions are invalid")
        point_ids.append(point_id)
        observed.append(integer(row["type_index"], "point.type_index", 0, len(type_ids) - 1))
        neighbor_counts.append([
            integer(value, "point.neighbor_count", 0, len(points) - 1) for value in vector
        ])
    if point_ids != sorted(point_ids) or len(set(point_ids)) != len(point_ids):
        raise ContractError("point identities must be unique and sorted")
    if any(observed.count(index) < 2 for index in range(len(type_ids))):
        raise ContractError("every type requires at least two points")
    edges = request["edges"]
    if not isinstance(edges, list) or not edges:
        raise ContractError("edges are invalid")
    edge_rows: list[tuple[int, int]] = []
    previous = (-1, -1)
    recomputed = np.zeros((len(points), len(type_ids)), dtype=np.int64)
    for index, raw in enumerate(edges):
        row = obj(raw, {"left", "right"}, f"edges[{index}]")
        left = integer(row["left"], "edge.left", 0, len(points) - 1)
        right = integer(row["right"], "edge.right", 0, len(points) - 1)
        if left >= right or (left, right) <= previous:
            raise ContractError("edges must be unique and lexicographically sorted")
        previous = (left, right)
        edge_rows.append((left, right))
        recomputed[left, observed[right]] += 1
        recomputed[right, observed[left]] += 1
    counts = np.asarray(neighbor_counts, dtype=np.int64)
    if not np.array_equal(counts, recomputed):
        raise ContractError("neighbor counts differ from exact edges and observed types")
    priors = obj(request["priors"], {"intercept_sd", "interaction_sd"}, "priors")
    intercept_sd = number(priors["intercept_sd"], "priors.intercept_sd")
    interaction_sd = number(priors["interaction_sd"], "priors.interaction_sd")
    if min(intercept_sd, interaction_sd) <= 0.0:
        raise ContractError("prior scales must be positive")
    sampling = obj(
        request["sampling"],
        {"chains", "tune_per_chain", "draws_per_chain", "target_accept", "seed"},
        "sampling",
    )
    chains = integer(sampling["chains"], "sampling.chains", 2, 8)
    tune = integer(sampling["tune_per_chain"], "sampling.tune", 100, 100_000)
    draws = integer(sampling["draws_per_chain"], "sampling.draws", 100, 100_000)
    target_accept = number(sampling["target_accept"], "sampling.target_accept")
    if not 0.5 <= target_accept < 1.0:
        raise ContractError("target_accept is invalid")
    seed = integer(sampling["seed"], "sampling.seed", 0, 2**64 - 1)
    policy = obj(
        request["diagnostic_policy"],
        {
            "maximum_r_hat", "minimum_bulk_ess", "minimum_tail_ess", "minimum_ebfmi",
            "maximum_divergences", "maximum_tree_depth_hits",
        },
        "diagnostic_policy",
    )
    resources = obj(
        request["resources"],
        {
            "maximum_points", "maximum_types", "maximum_neighbor_visits", "neighbor_visits",
            "maximum_edges", "maximum_draw_parameter_work", "draw_parameter_work",
            "maximum_working_bytes", "estimated_working_bytes", "maximum_output_bytes",
            "memory_scope", "maximum_tree_depth", "timeout_seconds",
        },
        "resources",
    )
    if len(points) > integer(resources["maximum_points"], "resources.points", 6, 10_000):
        raise ContractError("point ceiling exceeded")
    if len(type_ids) > integer(resources["maximum_types"], "resources.types", 3, 8):
        raise ContractError("type ceiling exceeded")
    expected_visits = len(points) * (len(points) - 1) // 2
    exact(resources["neighbor_visits"], expected_visits, "resources.neighbor_visits")
    if expected_visits > integer(
        resources["maximum_neighbor_visits"], "resources.maximum_neighbor_visits", 1, 100_000_000
    ):
        raise ContractError("neighbor-visit ceiling exceeded")
    if len(edges) > integer(resources["maximum_edges"], "resources.maximum_edges", 1, 10_000_000):
        raise ContractError("edge ceiling exceeded")
    parameter_count = len(type_ids) - 1 + len(type_ids) * (len(type_ids) + 1) // 2 - 1
    work = chains * draws * parameter_count
    exact(resources["draw_parameter_work"], work, "resources.draw_parameter_work")
    if work > integer(
        resources["maximum_draw_parameter_work"], "resources.maximum_draw_parameter_work", 1, 100_000_000
    ):
        raise ContractError("draw-parameter ceiling exceeded")
    working = integer(resources["estimated_working_bytes"], "resources.estimated_working_bytes", 1, 2**63 - 1)
    if working > integer(resources["maximum_working_bytes"], "resources.maximum_working_bytes", 1, 8 * 1024**3):
        raise ContractError("working-byte ceiling exceeded")
    exact(
        resources["memory_scope"],
        "retained_request_graph_and_posterior_summaries_excluding_pinned_runtime_rss",
        "resources.memory_scope",
    )
    maximum_output_bytes = integer(
        resources["maximum_output_bytes"], "resources.maximum_output_bytes", 1, 2 * 1_048_576
    )
    maximum_tree_depth = integer(resources["maximum_tree_depth"], "resources.maximum_tree_depth", 10, 14)
    return {
        "request": request,
        "input_sha": input_sha,
        "type_ids": type_ids,
        "observed": np.asarray(observed, dtype=np.int64),
        "neighbor_counts": counts.astype(np.float64),
        "edges": np.asarray(edge_rows, dtype=np.int64),
        "intercept_sd": intercept_sd,
        "interaction_sd": interaction_sd,
        "chains": chains,
        "tune": tune,
        "draws": draws,
        "target_accept": target_accept,
        "seed": seed,
        "maximum_tree_depth": maximum_tree_depth,
        "maximum_r_hat": number(policy["maximum_r_hat"], "policy.rhat"),
        "minimum_bulk_ess": number(policy["minimum_bulk_ess"], "policy.bulk"),
        "minimum_tail_ess": number(policy["minimum_tail_ess"], "policy.tail"),
        "minimum_ebfmi": number(policy["minimum_ebfmi"], "policy.ebfmi"),
        "maximum_divergences": integer(policy["maximum_divergences"], "policy.divergences", 0, 2**63 - 1),
        "maximum_tree_depth_hits": integer(policy["maximum_tree_depth_hits"], "policy.depth_hits", 0, 2**63 - 1),
        "maximum_output_bytes": maximum_output_bytes,
    }


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    value = hashlib.sha256(
        f"marklab-pymc-conditional-multitype-mark-v1\0{seed}\0{purpose}\0{index}".encode()
    ).digest()
    return int.from_bytes(value[:4], "little")


def summary(values: np.ndarray) -> dict[str, float]:
    flat = np.asarray(values, dtype=np.float64).reshape(-1)
    return {
        "mean": float(flat.mean()),
        "sd": float(flat.std(ddof=1)),
        "interval_lower": float(np.quantile(flat, 0.025)),
        "interval_upper": float(np.quantile(flat, 0.975)),
    }


def tree_values(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def softmax(values: np.ndarray) -> np.ndarray:
    shifted = values - values.max(axis=-1, keepdims=True)
    weights = np.exp(shifted)
    return weights / weights.sum(axis=-1, keepdims=True)


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


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    types = len(config["type_ids"])
    free_pairs = types * (types + 1) // 2 - 1
    with pm.Model():
        intercept_free = pm.Normal("intercept_free", 0.0, config["intercept_sd"], shape=types - 1)
        potential_free = pm.Normal("potential_free", 0.0, config["interaction_sd"], shape=free_pairs)
        intercept = pt.concatenate([pt.zeros((1,)), intercept_free])
        potential = pt.zeros((types, types))
        index = 0
        for left in range(types):
            for right in range(left, types):
                if left == 0 and right == 0:
                    continue
                potential = pt.set_subtensor(potential[left, right], potential_free[index])
                potential = pt.set_subtensor(potential[right, left], potential_free[index])
                index += 1
        logits = intercept + pt.dot(config["neighbor_counts"], potential.T)
        pm.Categorical("observed_type", logit_p=logits, observed=config["observed"])
        prior = pm.sample_prior_predictive(
            draws=500, random_seed=seed_for(config["seed"], "prior")
        )
        posterior = pm.sample(
            draws=config["draws"],
            tune=config["tune"],
            chains=config["chains"],
            cores=1,
            blas_cores=1,
            random_seed=[
                seed_for(config["seed"], "chain", index) for index in range(config["chains"])
            ],
            target_accept=config["target_accept"],
            nuts_sampler="pymc",
            nuts={"max_treedepth": config["maximum_tree_depth"]},
            progressbar=False,
            quiet=True,
            compute_convergence_checks=False,
        )
    free_intercept = np.asarray(posterior["posterior"]["intercept_free"].values, dtype=np.float64)
    free_potential = np.asarray(posterior["posterior"]["potential_free"].values, dtype=np.float64)
    intercept, potential = matrices(free_intercept, free_potential, types)
    flat_intercept = intercept.reshape(-1, types)
    flat_potential = potential.reshape(-1, types, types)
    draws = len(flat_intercept)
    score = np.empty(draws, dtype=np.float64)
    expected_counts = np.empty((draws, types), dtype=np.float64)
    expected_same = np.empty(draws, dtype=np.float64)
    edges = config["edges"]
    for begin in range(0, draws, 128):
        end = min(begin + 128, draws)
        logits = (
            flat_intercept[begin:end, None, :]
            + np.einsum(
                "nl,dkl->dnk", config["neighbor_counts"], flat_potential[begin:end]
            )
        )
        probability = softmax(logits)
        score[begin:end] = np.log(
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
    monitored = ["intercept_free", "potential_free"]
    prior_finite = bool(
        all(np.isfinite(np.asarray(value.values)).all() for value in prior["prior"].values())
    )
    posterior_finite = bool(
        np.isfinite(intercept).all()
        and np.isfinite(potential).all()
        and np.isfinite(score).all()
        and np.isfinite(expected_counts).all()
        and np.isfinite(expected_same).all()
    )
    r_hat = float(tree_values(pm.stats.rhat(posterior, var_names=monitored, method="rank"), monitored).max())
    bulk = float(tree_values(pm.stats.ess(posterior, var_names=monitored, method="bulk"), monitored).min())
    tail = float(tree_values(pm.stats.ess(posterior, var_names=monitored, method="tail"), monitored).min())
    mcse_mean = float(tree_values(pm.stats.mcse(posterior, var_names=monitored, method="mean"), monitored).max())
    mcse_sd = float(tree_values(pm.stats.mcse(posterior, var_names=monitored, method="sd"), monitored).max())
    energy = np.asarray(posterior["sample_stats"]["energy"].values, dtype=np.float64)
    ebfmi = float(np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1)))
    divergences = int(np.asarray(posterior["sample_stats"]["diverging"].values).sum())
    depth_hits = int(np.asarray(posterior["sample_stats"]["reached_max_treedepth"].values).sum())
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
    type_intercepts = [
        {"type_id": type_id, "intercept": summary(flat_intercept[:, index])}
        for index, type_id in enumerate(config["type_ids"])
    ]
    pair_potentials = []
    for left in range(types):
        for right in range(left, types):
            pair_potentials.append({
                "type_a": config["type_ids"][left],
                "type_b": config["type_ids"][right],
                "potential": summary(flat_potential[:, left, right]),
            })
    affinities = []
    for left in range(types):
        for right in range(left + 1, types):
            contrast = (
                flat_potential[:, left, right]
                - 0.5 * (flat_potential[:, left, left] + flat_potential[:, right, right])
            )
            affinities.append({
                "type_a": config["type_ids"][left],
                "type_b": config["type_ids"][right],
                "contrast": summary(contrast),
            })
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": {
            "name": "pymc", "version": pm.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha, "worker_sha256": worker_sha,
        },
        "input_sha256": config["input_sha"],
        "request_sha256": request_sha,
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"], "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"], "completed_draws": draws,
        },
        "type_intercepts": type_intercepts,
        "pair_potentials": pair_potentials,
        "pair_affinity_contrasts": affinities,
        "comparison": {
            "independent_label_composite_log_score": null_score,
            "spatial_composite_log_score": summary(score),
            "mean_composite_log_score_improvement": float(score.mean() - null_score),
        },
        "posterior_predictive": {
            "mode": "one_step_conditionals_given_observed_neighbors",
            "observed_same_type_edges": observed_same,
            "expected_same_type_edges": summary(expected_same),
            "observed_type_counts": observed_counts.astype(int).tolist(),
            "expected_type_counts": [summary(expected_counts[:, index]) for index in range(types)],
        },
        "diagnostics": {
            "prior_predictive_finite": prior_finite,
            "posterior_finite": posterior_finite,
            "r_hat": r_hat,
            "ess_bulk": bulk,
            "ess_tail": tail,
            "mcse_mean": mcse_mean,
            "mcse_sd": mcse_sd,
            "minimum_ebfmi": ebfmi,
            "divergences": divergences,
            "max_tree_depth_hits": depth_hits,
            "constraints_valid": posterior_finite,
            "identifiability_checks_passed": bool(
                np.all(intercept[..., 0] == 0.0) and np.all(potential[..., 0, 0] == 0.0)
            ),
        },
    }


def main() -> int:
    if pm.__version__ != PYMC_VERSION or sys.version_info[:2] != (3, 12):
        raise ContractError("PyMC or Python version drift")
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    raw = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if not raw or len(raw) > 16 * 1024 * 1024:
        raise ContractError("request size is invalid")
    request_sha = hashlib.sha256(raw).hexdigest()
    config = validate(json.loads(raw), lock_sha, worker_sha)
    result = fit(config, request_sha, lock_sha, worker_sha)
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
            f"marklab conditional multitype mark failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
