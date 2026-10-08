#!/usr/bin/env python3
"""Static ArviZ PSIS-LOO worker for complete pointwise log-likelihood matrices."""

from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path
import sys
from typing import Any

import arviz as az
import arviz_stats
import numpy as np

BACKEND_VERSION = "arviz-1.3.0+arviz-stats-1.3.1"


class ContractError(ValueError):
    pass


def obj(value: Any, keys: set[str], path: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        actual = set(value) if isinstance(value, dict) else set()
        raise ContractError(
            f"{path} fields differ: missing={sorted(keys - actual)}, unknown={sorted(actual - keys)}"
        )
    return value


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


def exact(value: Any, expected: Any, path: str) -> None:
    if value != expected:
        raise ContractError(f"{path} must equal {expected!r}")


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "model_name",
            "likelihood_target",
            "data_identity_sha256",
            "preprocessing_identity_sha256",
            "heldout_unit",
            "unit_ids",
            "chains",
            "draws_per_chain",
            "relative_efficiency",
            "log_likelihood",
            "resources",
        },
        "request",
    )
    exact(request["format"], "marklab.pymc_worker_request", "request.format")
    integer(request["version"], "request.version", 1, 1)
    backend = obj(
        request["backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "backend",
    )
    exact(backend["name"], "arviz", "backend.name")
    exact(backend["version"], BACKEND_VERSION, "backend.version")
    exact(backend["python_version"], "3.12", "backend.python")
    exact(backend["environment_lock_sha256"], lock_digest, "backend.lock")
    exact(backend["worker_sha256"], worker_digest, "backend.worker")

    for field in ["model_name", "likelihood_target"]:
        value = request[field]
        if (
            not isinstance(value, str)
            or not value
            or len(value) > 128
            or value.strip() != value
        ):
            raise ContractError(f"{field} must be an exact bounded name")
    for field in ["data_identity_sha256", "preprocessing_identity_sha256"]:
        value = request[field]
        if (
            not isinstance(value, str)
            or len(value) != 64
            or any(character not in "0123456789abcdef" for character in value)
        ):
            raise ContractError(f"{field} must be a lowercase SHA-256")

    heldout_unit = request["heldout_unit"]
    if (
        not isinstance(heldout_unit, str)
        or not heldout_unit
        or heldout_unit.strip() != heldout_unit
    ):
        raise ContractError("held-out unit kind must be an exact nonempty string")
    unit_ids = request["unit_ids"]
    if not isinstance(unit_ids, list) or not 2 <= len(unit_ids) <= 10_000:
        raise ContractError("unit IDs must contain 2-10000 entries")
    if any(
        not isinstance(unit, str)
        or not unit
        or unit.strip() != unit
        or (index > 0 and unit_ids[index - 1] >= unit)
        for index, unit in enumerate(unit_ids)
    ):
        raise ContractError("unit IDs must be exact and increasing")
    chains = integer(request["chains"], "request.chains", 2, 8)
    draws = integer(request["draws_per_chain"], "request.draws", 100, 100_000)
    relative_efficiency = number(request["relative_efficiency"], "request.reff")
    if not 0.0 < relative_efficiency <= 1.0:
        raise ContractError("relative efficiency must be in (0,1]")
    values = request["log_likelihood"]
    expected_values = chains * draws * len(unit_ids)
    if not isinstance(values, list) or len(values) != expected_values:
        raise ContractError("log-likelihood matrix dimensions mismatch")
    log_likelihood = np.asarray(
        [number(value, "request.log_likelihood[]") for value in values], dtype=np.float64
    ).reshape(chains, draws, len(unit_ids))

    resources = obj(
        request["resources"],
        {"maximum_log_likelihood_values", "maximum_output_bytes", "timeout_seconds"},
        "resources",
    )
    maximum_values = integer(
        resources["maximum_log_likelihood_values"], "resources.values", 1, 500_000
    )
    if expected_values > maximum_values:
        raise ContractError("log-likelihood matrix exceeds resource limit")
    integer(resources["maximum_output_bytes"], "resources.output", 1, 1_048_576)
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3_600)
    return {
        "unit_ids": unit_ids,
        "chains": chains,
        "draws": draws,
        "relative_efficiency": relative_efficiency,
        "log_likelihood": log_likelihood,
    }


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    data = az.from_dict(
        {
            "posterior": {
                "sample_index": np.zeros(
                    (config["chains"], config["draws"]), dtype=np.float64
                )
            },
            "log_likelihood": {"observed": config["log_likelihood"]},
        },
        coords={"unit": config["unit_ids"]},
        dims={"observed": ["unit"]},
    )
    loo = az.loo(
        data,
        pointwise=True,
        var_name="observed",
        reff=config["relative_efficiency"],
    )
    elpd_i = np.asarray(loo.elpd_i.values, dtype=np.float64).reshape(-1)
    pareto_k = np.asarray(loo.pareto_k.values, dtype=np.float64).reshape(-1)
    aggregate = np.asarray(
        [loo.elpd, loo.se, loo.p, loo.good_k, *elpd_i, *pareto_k], dtype=np.float64
    )
    if not np.isfinite(aggregate).all():
        raise ContractError("ArviZ returned non-finite PSIS-LOO output")
    good_k = float(loo.good_k)
    refit_units = [
        unit_id
        for unit_id, pareto in zip(config["unit_ids"], pareto_k)
        if float(pareto) > good_k
    ]
    return {
        "format": "marklab.arviz_psis_loo_worker_result",
        "version": 1,
        "backend": {
            "name": "arviz",
            "version": BACKEND_VERSION,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha,
            "worker_sha256": worker_sha,
        },
        "request_sha256": request_sha,
        "elpd_loo": float(loo.elpd),
        "standard_error": float(loo.se),
        "p_loo": float(loo.p),
        "good_pareto_k": good_k,
        "maximum_pareto_k": float(pareto_k.max()),
        "warning": bool(refit_units),
        "pointwise": [
            {
                "unit_id": unit_id,
                "elpd_loo": float(elpd_i[index]),
                "pareto_k": float(pareto_k[index]),
                "reliability": (
                    "reliable" if float(pareto_k[index]) <= good_k else "requires_refit_or_kfold"
                ),
            }
            for index, unit_id in enumerate(config["unit_ids"])
        ],
        "refit_or_kfold_units": refit_units,
    }


def main() -> None:
    if (
        az.__version__ != "1.3.0"
        or arviz_stats.__version__ != "1.3.1"
        or sys.version_info[:2] != (3, 12)
    ):
        raise ContractError("ArviZ, arviz-stats, or Python version drift")
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    raw = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if len(raw) > 16 * 1024 * 1024:
        raise ContractError("request exceeds 16 MiB")
    request_sha = hashlib.sha256(raw).hexdigest()
    result = fit(validate(json.loads(raw), lock_sha, worker_sha), request_sha, lock_sha, worker_sha)
    sys.stdout.write(json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")))
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"marklab PSIS-LOO worker failed: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from error
