#!/usr/bin/env python3
"""Pinned PyMC fit for patient-replicated exact-window multitype LGCPs."""

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
from marklab_worker_support.multitype_lgcp import validate_fixed_kernel_request
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
    backend_identity,
    derive_seed,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    worker_digests,
    write_result,
)

RESULT_FORMAT = "marklab.bayesian_replicated_arbitrary_window_multitype_lgcp_fit"

# Tests read these names from this module.
__all__ = ["ContractError", "build_model", "fit", "prior_is_finite"]


def seed_for(seed: int, purpose: str) -> int:
    return derive_seed(
        "marklab-replicated-arbitrary-window-multitype-lgcp-v1", seed, purpose, width=8
    )


def build_model(config: dict[str, Any]) -> pm.Model:
    """Fixed-kernel model; see ``multitype_lgcp_pymc.build_model``."""
    return multitype_lgcp_pymc.build_model(config, inferred_kernel=False)


def prior_is_finite(config: dict[str, Any], model: pm.Model) -> bool:
    return multitype_lgcp_pymc.prior_is_finite(config, model, seed_for)


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    _, fields = multitype_lgcp_pymc.fit(
        config,
        inferred_kernel=False,
        maximum_tree_depth=int(config["policy"]["maximum_tree_depth"]),
        seed_for=seed_for,
    )
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": backend_identity("pymc", pm.__version__, lock_sha, worker_sha),
        "request_sha256": request_sha,
        **fields,
    }


def main() -> None:
    require_versions(pymc=pm.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(64 * MIB)
    config = validate_fixed_kernel_request(json.loads(raw), lock_sha, worker_sha)
    write_result(fit(config, sha256_hex(raw), lock_sha, worker_sha))


if __name__ == "__main__":
    run_worker(main, "marklab replicated arbitrary-window multitype LGCP failed")
