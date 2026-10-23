#!/usr/bin/env python3
"""SBC adapter for Marklab's weighted exact-window fixed-kernel LGCP."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import jax
import numpy as np
import numpyro

sys.path.append(str(Path(__file__).resolve().parent))

import marklab_numpyro_gridded_lgcp_sbc_worker as source_sbc
from marklab_worker_support import arbitrary_window_lgcp as adapter_contract
from marklab_worker_support import gridded_lgcp as source_contract
from marklab_worker_support import versions
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
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

REQUEST_FORMAT = "marklab.numpyro_arbitrary_window_lgcp_sbc_request"
RESULT_FORMAT = "marklab.numpyro_arbitrary_window_lgcp_sbc_result"
SOURCE_RESULT_FORMAT = "marklab.numpyro_gridded_lgcp_sbc_worker_result"


def validate(
    request: Any, lock_digest: str, worker_digest: str, source_sbc_digest: str
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
            "source_sbc_request_sha256",
            "source_sbc_request",
        },
        "request",
    )
    exact(request["format"], REQUEST_FORMAT, "request.format")
    exact(request["version"], 1, "request.version")
    for path, digest in (("backend", worker_digest), ("source_backend", source_sbc_digest)):
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
    sha256_digest(request["source_sbc_request_sha256"], "request.source_sbc_request_sha256")
    return request


def main() -> None:
    require_versions(numpyro=numpyro.__version__, jax=jax.__version__)
    lock_digest, worker_digest = worker_digests(__file__)
    source_sbc_digest = file_sha256(source_sbc.__file__)
    pymc_adapter_digest = file_sha256(Path(__file__).with_name(adapter_contract.SOURCE_WORKER))
    pymc_source_digest = file_sha256(Path(__file__).with_name(source_contract.SOURCE_WORKER))
    raw = read_request(16 * MIB)
    request = validate(json.loads(raw), lock_digest, worker_digest, source_sbc_digest)
    source_request, physical_config, nodes, _, _ = adapter_contract.validate_request(
        request["source_request"],
        lock_digest,
        pymc_adapter_digest,
        pymc_source_digest,
    )
    config = source_sbc.validate(request["source_sbc_request"], lock_digest, source_sbc_digest)
    if request["source_sbc_request"]["source_request"] != source_request["source_request"]:
        raise ContractError("SBC and physical source requests differ")
    for name in ("covariate", "offset", "counts"):
        if not np.array_equal(config[name], physical_config[name]):
            raise ContractError(f"SBC {name} differs from physical source")
    config["cholesky"] = physical_config["cholesky"]
    completed, failures = source_sbc.run_sbc(config)
    diagnostics, passes = source_sbc.calibration_summary(config, completed, failures)
    source_result = {
        "format": SOURCE_RESULT_FORMAT,
        "version": 1,
        "backend": request["source_backend"],
        "jax_version": versions.JAX,
        "request_sha256": request["source_sbc_request_sha256"],
        "fit_state": "complete" if passes else "nonconverged",
        "replicates": completed,
        "failures": failures,
        "diagnostics": diagnostics,
    }
    result = {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": request["backend"],
        "jax_version": versions.JAX,
        "input": source_request["input"],
        "covariance_sha256": source_request["covariance_sha256"],
        "physical_latent_node_id": nodes[config["latent_cell_index"]]["node_id"],
        "request_sha256": sha256_hex(raw),
        "source_request_sha256": request["source_request_sha256"],
        "source_sbc_request_sha256": request["source_sbc_request_sha256"],
        "source_result": source_result,
    }
    write_result(result, max_bytes=config["maximum_output_bytes"], newline=False)


if __name__ == "__main__":
    run_worker(main, "marklab NumPyro arbitrary-window LGCP SBC failed")
