#!/usr/bin/env python3
"""Pinned PyMC replicated multitype LGCP with one inferred shared Matérn kernel."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

sys.path.append(str(Path(__file__).resolve().parent))

# Configure PyTensor before PyMC is imported.
from marklab_worker_support import pytensor_config  # noqa: F401

# isort: split
import pymc as pm

from marklab_worker_support import multitype_lgcp_pymc
from marklab_worker_support.multitype_lgcp import summarize, validate_inferred_kernel_request
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
    backend_identity,
    derive_seed,
    file_sha256,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    worker_digests,
    write_result,
)

RESULT_FORMAT = "marklab.bayesian_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_fit"
SOURCE_WORKER = "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_worker.py"

# Tests read these names from this module.
__all__ = ["ContractError", "build_model", "fit", "prior_is_finite"]


def seed_for(seed: int, purpose: str) -> int:
    return derive_seed("marklab-replicated-multitype-lgcp-inferred-kernel-v1", seed, purpose)


def build_model(config: dict[str, Any]) -> pm.Model:
    """Inferred-kernel model; see ``multitype_lgcp_pymc.build_model``."""
    return multitype_lgcp_pymc.build_model(config, inferred_kernel=True)


def prior_is_finite(config: dict[str, Any], model: pm.Model) -> bool:
    return multitype_lgcp_pymc.prior_is_finite(config, model, seed_for)


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    flat, fields = multitype_lgcp_pymc.fit(
        config,
        inferred_kernel=True,
        maximum_tree_depth=config["maximum_tree_depth"],
        seed_for=seed_for,
    )
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": backend_identity("pymc", pm.__version__, lock_sha, worker_sha),
        "source_backend": config["outer_request"]["source_backend"],
        "request_sha256": request_sha,
        "source_request_sha256": config["source_request_sha256"],
        "kernel_posterior": {
            "field_amplitude": summarize(flat["field_amplitude"], config["chains"]),
            "field_length_scale_um": summarize(flat["field_length_scale_um"], config["chains"]),
        },
        "kernel_cube_work": config["kernel_cube_work"],
        **fields,
    }


def main() -> None:
    require_versions(pymc=pm.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    source_worker_sha = file_sha256(Path(__file__).with_name(SOURCE_WORKER))
    raw = read_request(64 * MIB)
    _, config = validate_inferred_kernel_request(
        json.loads(raw), lock_sha, worker_sha, source_worker_sha
    )
    write_result(fit(config, sha256_hex(raw), lock_sha, worker_sha))


if __name__ == "__main__":
    run_worker(main, "marklab PyMC replicated multitype inferred-kernel LGCP failed")
