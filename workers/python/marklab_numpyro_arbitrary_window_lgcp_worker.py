#!/usr/bin/env python3
"""NumPyro adapter for Marklab's weighted exact-window fixed-kernel LGCP."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import jax
import numpyro

sys.path.append(str(Path(__file__).resolve().parent))

import marklab_numpyro_gridded_lgcp_worker as numpyro_source
import marklab_pymc_arbitrary_window_lgcp_worker as pymc_adapter
import marklab_pymc_gridded_lgcp_worker as pymc_source
from marklab_worker_support import versions
from marklab_worker_support.protocol import (
    MIB,
    exact,
    file_sha256,
    obj,
    read_request,
    require_versions,
    run_worker,
    sha256_digest,
    sha256_hex,
    validate_backend,
    worker_digests,
    write_result,
)

REQUEST_FORMAT = "marklab.numpyro_arbitrary_window_lgcp_request"
RESULT_FORMAT = "marklab.numpyro_arbitrary_window_lgcp_result"


def validate(
    request: Any, lock_digest: str, worker_digest: str, numpyro_source_digest: str
) -> dict[str, Any]:
    request = obj(
        request,
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
    for path, digest in (("backend", worker_digest), ("source_backend", numpyro_source_digest)):
        validate_backend(
            request[path],
            f"request.{path}",
            name="numpyro",
            version=versions.NUMPYRO,
            lock_sha256=lock_digest,
            worker_sha256=digest,
        )
    exact(request["jax_version"], versions.JAX, "request.jax_version")
    sha256_digest(request["source_request_sha256"], "request.source_request_sha256")
    return request


def main() -> None:
    require_versions(numpyro=numpyro.__version__, jax=jax.__version__)
    lock_digest, worker_digest = worker_digests(__file__)
    numpyro_source_digest = file_sha256(numpyro_source.__file__)
    pymc_adapter_digest = file_sha256(pymc_adapter.__file__)
    pymc_source_digest = file_sha256(pymc_source.__file__)
    raw = read_request(16 * MIB)
    request_sha256 = sha256_hex(raw)
    request = validate(json.loads(raw), lock_digest, worker_digest, numpyro_source_digest)
    source_request, config, nodes, _, _ = pymc_adapter.validate(
        request["source_request"],
        lock_digest,
        pymc_adapter_digest,
        pymc_source_digest,
    )
    fitted = numpyro_source.fit(
        config,
        request_sha256,
        request["source_request_sha256"],
        lock_digest,
        numpyro_source_digest,
    )
    result = {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": request["backend"],
        "source_backend": fitted["backend"],
        "jax_version": versions.JAX,
        "input": source_request["input"],
        "request_sha256": request_sha256,
        "source_request_sha256": request["source_request_sha256"],
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
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "marklab NumPyro arbitrary-window LGCP failed")
