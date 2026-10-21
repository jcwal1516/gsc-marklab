#!/usr/bin/env python3
"""NumPy prior-generator calibration of the fixed-kernel replicated multitype LGCP."""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path
from typing import Any

import numpy as np

sys.path.append(str(Path(__file__).resolve().parent))

from marklab_worker_support import multitype_lgcp_calibration as calibration
from marklab_worker_support.multitype_lgcp import validate_fixed_kernel_request
from marklab_worker_support.protocol import (
    MIB,
    derive_seed,
    file_sha256,
    integer,
    obj,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    worker_digests,
    write_result,
)

REQUEST_FORMAT = (
    "marklab.numpy_replicated_arbitrary_window_multitype_lgcp_prior_calibration_request"
)
RESULT_FORMAT = "marklab.numpy_replicated_arbitrary_window_multitype_lgcp_prior_calibration_result"
SOURCE_WORKER = "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_worker.py"


def validate(request: Any, lock_sha: str, worker_sha: str, source_sha: str) -> dict[str, Any]:
    """Validate the calibration request; ``source_sha`` identifies the PyMC source worker."""
    request, config = calibration.validate_request(
        request,
        REQUEST_FORMAT,
        lock_sha,
        worker_sha,
        lambda source: validate_fixed_kernel_request(source, lock_sha, source_sha),
        "maximum_field_covariance_rmse_scale",
    )
    resources = obj(
        request["resources"],
        {
            "node_type_rows",
            "simulation_node_type_work",
            "maximum_simulation_node_type_work",
            "estimated_working_bytes",
            "maximum_working_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    calibration.check_work(resources, config, bytes_per_node=96)
    integer(resources["timeout_seconds"], "resources.timeout", 1, 86400)
    return config


def seed_for(seed: int) -> int:
    return derive_seed("marklab-replicated-multitype-lgcp-prior-calibration-v1", seed, width=8)


def field_oracle(config: dict[str, Any], latent: np.ndarray) -> dict[str, Any]:
    """Check that each pattern's field is centred and has the centred kernel covariance.

    The target covariance of pattern nodes ``S`` is ``C K_SS C`` with ``K = L L^T`` and the
    centring matrix ``C = I - 11^T / |S|``; the error is the root mean square deviation of the
    empirical covariance, in units of the squared field amplitude.
    """
    replicates, types = latent.shape[:2]
    covariance = config["cholesky"] @ config["cholesky"].T
    squared_error = 0.0
    covariance_elements = 0
    for pattern in range(len(config["pattern_ids"])):
        selected = np.flatnonzero(config["node_patterns"] == pattern)
        size = len(selected)
        centering = np.eye(size) - np.ones((size, size)) / size
        target = centering @ covariance[np.ix_(selected, selected)] @ centering
        values = latent[:, :, selected].reshape(replicates * types, size)
        empirical = np.cov(values, rowvar=False, ddof=1)
        if size == 1:
            empirical = np.asarray([[float(empirical)]])
        squared_error += float(np.square(empirical - target).sum())
        covariance_elements += target.size
    return {
        "covariance_rmse_scale": math.sqrt(squared_error / covariance_elements)
        / (config["priors"]["field_amplitude"] ** 2),
    }


def calibrate(
    config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str
) -> dict[str, Any]:
    """Draw from the model prior and compare the draws with their analytic moments.

    Prior: per type, ``intercept ~ Normal(intercept_mean, intercept_sd)``, group and covariate
    effects ``~ Normal(0, sd)``, and patient and pattern SDs ``~ HalfNormal(scale)``; patient
    effects ``sd * z`` and pattern effects ``sd * z`` with ``z`` centred within each patient;
    the latent field is ``L w`` with ``w ~ Normal(0, I)`` for the fixed Matern-3/2 Cholesky
    factor ``L``, centred within each pattern. Counts are Poisson with the log intensity of
    ``calibration.log_expected_counts``.
    """
    rng = np.random.default_rng(seed_for(config["seed"]))
    replicates = config["replicates"]
    types = len(config["types"])
    draws = calibration.hierarchy_prior_draws(rng, config["priors"], replicates, types)
    patient_raw = rng.normal(size=(replicates, len(config["patient_ids"]), types))
    pattern_raw = rng.normal(size=(replicates, len(config["pattern_ids"]), types))
    patient_effect = draws["patient_sd"][:, None, :] * patient_raw
    calibration.center_patterns_within_patients(pattern_raw, config)
    pattern_effect = draws["pattern_sd"][:, None, :] * pattern_raw
    field_raw = rng.normal(size=(replicates, types, len(config["weights"])))
    latent = np.einsum("rtn,mn->rtm", field_raw, config["cholesky"])
    maximum_centering_error = 0.0
    for pattern in range(len(config["pattern_ids"])):
        selected = config["node_patterns"] == pattern
        latent[:, :, selected] -= latent[:, :, selected].mean(axis=2, keepdims=True)
        maximum_centering_error = max(
            maximum_centering_error,
            float(np.max(np.abs(latent[:, :, selected].sum(axis=2)))),
        )
    log_expected = calibration.log_expected_counts(
        config, draws, patient_effect, pattern_effect, latent
    )
    poisson = calibration.poisson_oracle(config, rng, log_expected)
    moments = calibration.hierarchy_moment_checks(config, draws)
    field = {"maximum_centering_error": maximum_centering_error, **field_oracle(config, latent)}
    field["passes"] = bool(
        maximum_centering_error <= 1e-12
        and field["covariance_rmse_scale"] <= config["maximum_field_covariance_rmse_scale"]
    )
    return calibration.result(
        RESULT_FORMAT, config, request_sha, lock_sha, worker_sha, moments, field, poisson
    )


def main() -> None:
    require_versions(numpy=np.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    source_sha = file_sha256(Path(__file__).with_name(SOURCE_WORKER))
    raw = read_request(64 * MIB)
    config = validate(json.loads(raw), lock_sha, worker_sha, source_sha)
    result = calibrate(config, sha256_hex(raw), lock_sha, worker_sha)
    write_result(result, max_bytes=MIB)


if __name__ == "__main__":
    run_worker(main, "marklab NumPy replicated multitype LGCP prior calibration failed")
