#!/usr/bin/env python3
"""Spatial posterior-predictive check for the weighted exact-window IPP."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import math
from pathlib import Path
import sys
from typing import Any

import numpy as np
import pymc as pm


REQUEST_FORMAT = "marklab.pymc_arbitrary_window_ipp_spatial_ppc_request"
RESULT_FORMAT = "marklab.arbitrary_window_ipp_spatial_ppc"
PYMC_VERSION = "6.3.0"


class ContractError(ValueError):
    pass


def exact_object(value: Any, keys: set[str], path: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        actual = set(value) if isinstance(value, dict) else set()
        raise ContractError(
            f"{path} fields differ: missing={sorted(keys - actual)}, unknown={sorted(actual - keys)}"
        )
    return value


def exact(value: Any, expected: Any, path: str) -> None:
    if value != expected:
        raise ContractError(f"{path} must equal {expected!r}")


def integer(value: Any, path: str, low: int, high: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not low <= value <= high:
        raise ContractError(f"{path} must be an integer in [{low},{high}]")
    return value


def number(value: Any, path: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ContractError(f"{path} must be numeric")
    result = float(value)
    if not math.isfinite(result):
        raise ContractError(f"{path} must be finite")
    return result


def digest(value: Any, path: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise ContractError(f"{path} must be a lowercase SHA-256 digest")
    return value


def load_source_worker() -> Any:
    path = Path(__file__).with_name("marklab_pymc_inhomogeneous_poisson_worker.py")
    specification = importlib.util.spec_from_file_location("marklab_pymc_ipp", path)
    if specification is None or specification.loader is None:
        raise ContractError("cannot load the PyMC IPP worker")
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


def validate(
    request: Any, lock_digest: str, worker_digest: str, source_worker_digest: str
) -> tuple[dict[str, Any], dict[str, Any], np.ndarray, np.ndarray, list[tuple[int, int]]]:
    request = exact_object(
        request,
        {
            "format",
            "version",
            "backend",
            "source_request_sha256",
            "source_request",
            "input",
            "observed_node_counts",
            "node_weights_um2",
            "neighbor_pairs",
            "neighbor_radius_um",
            "prediction_seed",
            "maximum_predictive_work",
            "maximum_neighbor_pairs",
        },
        "request",
    )
    exact(request["format"], REQUEST_FORMAT, "request.format")
    exact(request["version"], 1, "request.version")
    backend = exact_object(
        request["backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "backend",
    )
    exact(backend["name"], "pymc", "backend.name")
    exact(backend["version"], PYMC_VERSION, "backend.version")
    exact(backend["python_version"], "3.12", "backend.python_version")
    exact(backend["environment_lock_sha256"], lock_digest, "backend.lock")
    exact(backend["worker_sha256"], worker_digest, "backend.worker")
    digest(request["source_request_sha256"], "source_request_sha256")
    input_identity = exact_object(
        request["input"],
        {
            "events_digest",
            "event_membership_digest",
            "quadrature_digest",
            "window_logical_digest",
        },
        "input",
    )
    for name, value in input_identity.items():
        digest(value, f"input.{name}")
    source = load_source_worker()
    config = source.validate(request["source_request"], lock_digest, source_worker_digest)
    node_count = config["grid_covariate"].size
    observed = request["observed_node_counts"]
    if not isinstance(observed, list) or len(observed) != node_count:
        raise ContractError("observed node-count dimensions differ")
    observed_counts = np.asarray(
        [integer(value, "observed_node_counts[]", 0, 2**63 - 1) for value in observed],
        dtype=np.int64,
    )
    if int(observed_counts.sum()) != config["event_covariate"].size:
        raise ContractError("observed node counts do not sum to events")
    weights = request["node_weights_um2"]
    if not isinstance(weights, list) or len(weights) != node_count:
        raise ContractError("node-weight dimensions differ")
    node_weights = np.asarray(
        [number(value, "node_weights_um2[]") for value in weights], dtype=np.float64
    )
    if np.any(node_weights <= 0.0):
        raise ContractError("node weights must be positive")
    raw_pairs = request["neighbor_pairs"]
    if not isinstance(raw_pairs, list) or not raw_pairs:
        raise ContractError("at least one physical neighbor pair is required")
    pairs = []
    previous = None
    for raw in raw_pairs:
        if not isinstance(raw, list) or len(raw) != 2:
            raise ContractError("neighbor pairs must have two indices")
        pair = (
            integer(raw[0], "neighbor_pairs[].left", 0, node_count - 1),
            integer(raw[1], "neighbor_pairs[].right", 0, node_count - 1),
        )
        if pair[0] >= pair[1] or (previous is not None and pair <= previous):
            raise ContractError("neighbor pairs must be unique, ordered, and canonical")
        pairs.append(pair)
        previous = pair
    if number(request["neighbor_radius_um"], "neighbor_radius_um") <= 0.0:
        raise ContractError("neighbor_radius_um must be positive")
    integer(request["prediction_seed"], "prediction_seed", 0, 2**64 - 1)
    maximum_work = integer(
        request["maximum_predictive_work"], "maximum_predictive_work", 1, 100_000_000
    )
    maximum_pairs = integer(
        request["maximum_neighbor_pairs"], "maximum_neighbor_pairs", 1, 1_000_000
    )
    if len(pairs) > maximum_pairs:
        raise ContractError("spatial PPC neighbor-pair ceiling exceeded")
    work = config["chains"] * config["draws"] * (node_count + len(pairs))
    if work > maximum_work:
        raise ContractError("spatial PPC work exceeds the declared maximum")
    config["observed_counts"] = observed_counts
    return request, config, observed_counts, node_weights, pairs


def seed_for(seed: int) -> int:
    digest = hashlib.sha256(
        f"marklab-pymc-arbitrary-window-ipp-spatial-ppc-v1\0{seed}".encode()
    ).digest()
    return int.from_bytes(digest[:4], "little")


def statistics(
    counts: np.ndarray, weights: np.ndarray, pairs: list[tuple[int, int]]
) -> tuple[np.ndarray, np.ndarray]:
    density = counts / weights
    variance = np.var(density, axis=-1)
    contrast = np.mean(
        np.stack([np.abs(density[..., left] - density[..., right]) for left, right in pairs]),
        axis=0,
    )
    return variance, contrast


def summarize(observed: float, replicated: np.ndarray) -> dict[str, float]:
    return {
        "observed": observed,
        "replicated_mean": float(replicated.mean()),
        "replicated_sd": float(replicated.std(ddof=1)),
        "probability_replicated_at_least_observed": float(np.mean(replicated >= observed)),
    }


def run_ppc(
    request: dict[str, Any],
    config: dict[str, Any],
    observed_counts: np.ndarray,
    weights: np.ndarray,
    pairs: list[tuple[int, int]],
    request_sha256: str,
    lock_digest: str,
    worker_digest: str,
    source_worker_digest: str,
) -> dict[str, Any]:
    source = load_source_worker()
    sampled = source.sample_model(config)
    source_result = source.result_from_sample(
        config,
        sampled,
        request["source_request_sha256"],
        lock_digest,
        source_worker_digest,
    )
    expected = sampled["expected"]
    rng = np.random.default_rng(seed_for(request["prediction_seed"]))
    replicated = rng.poisson(expected)
    observed_variance, observed_contrast = statistics(observed_counts, weights, pairs)
    replicated_variance, replicated_contrast = statistics(replicated, weights, pairs)
    values = np.concatenate(
        [
            np.asarray([observed_variance, observed_contrast]),
            replicated_variance,
            replicated_contrast,
        ]
    )
    if not np.isfinite(values).all():
        raise ContractError("spatial PPC produced a non-finite result")
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": {
            "name": "pymc",
            "version": pm.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_digest,
            "worker_sha256": worker_digest,
        },
        "input": request["input"],
        "request_sha256": request_sha256,
        "source_request_sha256": request["source_request_sha256"],
        "fit_state": source_result["fit_state"],
        "sampling": source_result["sampling"],
        "diagnostics": source_result["diagnostics"],
        "observed_event_count": int(observed_counts.sum()),
        "quadrature_node_count": int(observed_counts.size),
        "neighbor_radius_um": request["neighbor_radius_um"],
        "neighbor_pair_count": len(pairs),
        "posterior_predictive_replicates": int(replicated.shape[0]),
        "prediction_seed": request["prediction_seed"],
        "total_predictive_work": int(replicated.shape[0] * (observed_counts.size + len(pairs))),
        "maximum_predictive_work": request["maximum_predictive_work"],
        "maximum_neighbor_pairs": request["maximum_neighbor_pairs"],
        "summaries": {
            "node_density_variance": summarize(
                float(observed_variance), replicated_variance
            ),
            "neighbor_density_mean_absolute_difference": summarize(
                float(observed_contrast), replicated_contrast
            ),
        },
        "statistical_unit": "one_observed_point_pattern",
        "null_model": "fitted_log_linear_inhomogeneous_poisson_process_conditional_on_fixed_covariate",
        "assumptions": [
            "event_to_quadrature_membership_is_exact_and_complete",
            "quadrature_weights_partition_the_exact_window_area",
            "neighbor_pairs_use_fixed_physical_representative_distance",
            "node_density_is_count_per_square_micrometer",
        ],
        "finite_result_policy": "reject_non_finite_nonconverged_is_diagnostic_only",
        "claim_status": (
            "experimental_single_pattern_spatial_ppc"
            if source_result["fit_state"] == "complete"
            else "diagnostic_only_nonconverged"
        ),
    }


def main() -> None:
    if pm.__version__ != PYMC_VERSION or sys.version_info[:2] != (3, 12):
        raise ContractError("PyMC or Python version drift")
    script = Path(__file__)
    lock_digest = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_digest = hashlib.sha256(script.read_bytes()).hexdigest()
    source_worker_digest = hashlib.sha256(
        script.with_name("marklab_pymc_inhomogeneous_poisson_worker.py").read_bytes()
    ).hexdigest()
    raw = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if len(raw) > 16 * 1024 * 1024:
        raise ContractError("request exceeds 16 MiB")
    request_sha256 = hashlib.sha256(raw).hexdigest()
    request, config, observed, weights, pairs = validate(
        json.loads(raw), lock_digest, worker_digest, source_worker_digest
    )
    result = run_ppc(
        request,
        config,
        observed,
        weights,
        pairs,
        request_sha256,
        lock_digest,
        worker_digest,
        source_worker_digest,
    )
    sys.stdout.write(json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")))
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(
            f"marklab PyMC arbitrary-window IPP spatial PPC failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
