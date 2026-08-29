#!/usr/bin/env python3
"""SBC adapter for Marklab's weighted exact-window fixed-kernel LGCP."""

from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
import sys
from typing import Any

import jax
import numpy as np
import numpyro


REQUEST_FORMAT = "marklab.numpyro_arbitrary_window_lgcp_sbc_request"
RESULT_FORMAT = "marklab.numpyro_arbitrary_window_lgcp_sbc_result"
SOURCE_RESULT_FORMAT = "marklab.numpyro_gridded_lgcp_sbc_worker_result"
NUMPYRO_VERSION = "0.21.0"
JAX_VERSION = "0.11.1"


class ContractError(ValueError):
    pass


def load(name: str, filename: str) -> Any:
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


def digest(value: Any, path: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise ContractError(f"{path} must be a lowercase SHA-256 digest")
    return value


def backend(
    value: Any, lock_digest: str, worker_digest: str, path: str
) -> dict[str, Any]:
    value = obj(
        value,
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        path,
    )
    exact(value["name"], "numpyro", f"{path}.name")
    exact(value["version"], NUMPYRO_VERSION, f"{path}.version")
    exact(value["python_version"], "3.12", f"{path}.python_version")
    exact(value["environment_lock_sha256"], lock_digest, f"{path}.lock")
    exact(value["worker_sha256"], worker_digest, f"{path}.worker")
    return value


def main() -> None:
    if (
        numpyro.__version__ != NUMPYRO_VERSION
        or jax.__version__ != JAX_VERSION
        or sys.version_info[:2] != (3, 12)
    ):
        raise ContractError("NumPyro, JAX, or Python version drift")
    script = Path(__file__)
    lock_digest = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_digest = hashlib.sha256(script.read_bytes()).hexdigest()
    source_sbc_path = script.with_name("marklab_numpyro_gridded_lgcp_sbc_worker.py")
    source_sbc_digest = hashlib.sha256(source_sbc_path.read_bytes()).hexdigest()
    pymc_adapter_path = script.with_name("marklab_pymc_arbitrary_window_lgcp_worker.py")
    pymc_adapter_digest = hashlib.sha256(pymc_adapter_path.read_bytes()).hexdigest()
    pymc_source_digest = hashlib.sha256(
        script.with_name("marklab_pymc_gridded_lgcp_worker.py").read_bytes()
    ).hexdigest()
    raw = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if not raw or len(raw) > 16 * 1024 * 1024:
        raise ContractError("request size is invalid")
    request_sha256 = hashlib.sha256(raw).hexdigest()
    request = obj(
        json.loads(raw),
        {
            "format",
            "version",
            "backend",
            "source_backend",
            "jax_version",
            "source_request_sha256",
            "source_request",
            "source_sbc_request_sha256",
            "source_sbc_request",
        },
        "request",
    )
    exact(request["format"], REQUEST_FORMAT, "request.format")
    exact(request["version"], 1, "request.version")
    backend(request["backend"], lock_digest, worker_digest, "request.backend")
    backend(
        request["source_backend"],
        lock_digest,
        source_sbc_digest,
        "request.source_backend",
    )
    exact(request["jax_version"], JAX_VERSION, "request.jax_version")
    source_request_sha256 = digest(
        request["source_request_sha256"], "request.source_request_sha256"
    )
    source_sbc_request_sha256 = digest(
        request["source_sbc_request_sha256"], "request.source_sbc_request_sha256"
    )
    pymc_adapter = load("marklab_pymc_arbitrary_lgcp", pymc_adapter_path.name)
    source_request, physical_config, nodes, _, _ = pymc_adapter.validate(
        request["source_request"],
        lock_digest,
        pymc_adapter_digest,
        pymc_source_digest,
    )
    source_sbc = load("marklab_numpyro_gridded_lgcp_sbc", source_sbc_path.name)
    config = source_sbc.validate(
        request["source_sbc_request"], lock_digest, source_sbc_digest
    )
    if request["source_sbc_request"]["source_request"] != source_request["source_request"]:
        raise ContractError("SBC and physical source requests differ")
    for name in ("covariate", "offset", "counts"):
        if not np.array_equal(config[name], physical_config[name]):
            raise ContractError(f"SBC {name} differs from physical source")
    config["cholesky"] = physical_config["cholesky"]
    completed, failures = source_sbc.run_sbc(config)
    draws = config["chains"] * config["draws"]
    diagnostics = {
        "intercept": source_sbc.aggregate(
            completed, "intercept_rank", "intercept_covered", draws, config["rank_bins"]
        ),
        "coefficient": source_sbc.aggregate(
            completed,
            "coefficient_rank",
            "coefficient_covered",
            draws,
            config["rank_bins"],
        ),
        "latent_cell": source_sbc.aggregate(
            completed,
            "latent_cell_rank",
            "latent_cell_covered",
            draws,
            config["rank_bins"],
        ),
    }
    passes = not failures and all(
        config["minimum_rank_p"] <= value["rank_uniformity_p_value"]
        and config["minimum_coverage"] <= value["coverage_90"] <= config["maximum_coverage"]
        for value in diagnostics.values()
    )
    source_result = {
        "format": SOURCE_RESULT_FORMAT,
        "version": 1,
        "backend": request["source_backend"],
        "jax_version": JAX_VERSION,
        "request_sha256": source_sbc_request_sha256,
        "fit_state": "complete" if passes else "nonconverged",
        "replicates": completed,
        "failures": failures,
        "diagnostics": diagnostics,
    }
    result = {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": request["backend"],
        "jax_version": JAX_VERSION,
        "input": source_request["input"],
        "covariance_sha256": source_request["covariance_sha256"],
        "physical_latent_node_id": nodes[config["latent_cell_index"]]["node_id"],
        "request_sha256": request_sha256,
        "source_request_sha256": source_request_sha256,
        "source_sbc_request_sha256": source_sbc_request_sha256,
        "source_result": source_result,
    }
    encoded = json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")).encode()
    if len(encoded) > config["maximum_output_bytes"]:
        raise ContractError("result exceeds output limit")
    sys.stdout.buffer.write(encoded)


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(
            f"marklab NumPyro arbitrary-window LGCP SBC failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
