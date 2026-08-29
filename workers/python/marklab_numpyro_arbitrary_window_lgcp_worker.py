#!/usr/bin/env python3
"""NumPyro adapter for Marklab's weighted exact-window fixed-kernel LGCP."""

from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
import sys
from typing import Any

import jax
import numpyro


REQUEST_FORMAT = "marklab.numpyro_arbitrary_window_lgcp_request"
RESULT_FORMAT = "marklab.numpyro_arbitrary_window_lgcp_result"
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


def backend(
    value: Any,
    name: str,
    version: str,
    lock_digest: str,
    worker_digest: str,
    path: str,
) -> dict[str, Any]:
    value = obj(
        value,
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        path,
    )
    exact(value["name"], name, f"{path}.name")
    exact(value["version"], version, f"{path}.version")
    exact(value["python_version"], "3.12", f"{path}.python_version")
    exact(value["environment_lock_sha256"], lock_digest, f"{path}.lock")
    exact(value["worker_sha256"], worker_digest, f"{path}.worker")
    return value


def digest(value: Any, path: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise ContractError(f"{path} must be a lowercase SHA-256 digest")
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
    numpyro_source_path = script.with_name("marklab_numpyro_gridded_lgcp_worker.py")
    numpyro_source_digest = hashlib.sha256(numpyro_source_path.read_bytes()).hexdigest()
    pymc_adapter_path = script.with_name("marklab_pymc_arbitrary_window_lgcp_worker.py")
    pymc_adapter_digest = hashlib.sha256(pymc_adapter_path.read_bytes()).hexdigest()
    pymc_source_digest = hashlib.sha256(
        script.with_name("marklab_pymc_gridded_lgcp_worker.py").read_bytes()
    ).hexdigest()
    raw = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if len(raw) > 16 * 1024 * 1024:
        raise ContractError("request exceeds 16 MiB")
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
        },
        "request",
    )
    exact(request["format"], REQUEST_FORMAT, "request.format")
    exact(request["version"], 1, "request.version")
    backend(
        request["backend"],
        "numpyro",
        NUMPYRO_VERSION,
        lock_digest,
        worker_digest,
        "request.backend",
    )
    backend(
        request["source_backend"],
        "numpyro",
        NUMPYRO_VERSION,
        lock_digest,
        numpyro_source_digest,
        "request.source_backend",
    )
    exact(request["jax_version"], JAX_VERSION, "request.jax_version")
    source_request_sha256 = digest(
        request["source_request_sha256"], "request.source_request_sha256"
    )
    pymc_adapter = load("marklab_pymc_arbitrary_lgcp", pymc_adapter_path.name)
    source_request, config, nodes, _, _ = pymc_adapter.validate(
        request["source_request"],
        lock_digest,
        pymc_adapter_digest,
        pymc_source_digest,
    )
    numpyro_source = load("marklab_numpyro_gridded_lgcp", numpyro_source_path.name)
    fitted = numpyro_source.fit(
        config,
        request_sha256,
        source_request_sha256,
        lock_digest,
        numpyro_source_digest,
    )
    result = {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": request["backend"],
        "source_backend": fitted["backend"],
        "jax_version": JAX_VERSION,
        "input": source_request["input"],
        "request_sha256": request_sha256,
        "source_request_sha256": source_request_sha256,
        "covariance_sha256": source_request["covariance_sha256"],
        "fit_state": fitted["fit_state"],
        "sampling": fitted["sampling"],
        "posterior": fitted["posterior"],
        "nodes": [
            {
                "node_id": node["node_id"],
                "latent_effect": cell["latent_effect"],
                "expected_count": cell["expected_count"],
            }
            for node, cell in zip(nodes, fitted["cells"], strict=True)
        ],
        "diagnostics": fitted["diagnostics"],
        "posterior_predictive": fitted["posterior_predictive"],
    }
    sys.stdout.write(json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")))
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(
            f"marklab NumPyro arbitrary-window LGCP failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
