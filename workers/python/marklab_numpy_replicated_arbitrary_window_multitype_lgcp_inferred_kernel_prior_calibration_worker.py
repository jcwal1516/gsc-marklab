#!/usr/bin/env python3
"""NumPy prior-generator calibration of the inferred-kernel replicated multitype LGCP."""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path
from typing import Any

import numpy as np

sys.path.append(str(Path(__file__).resolve().parent))

from marklab_worker_support import multitype_lgcp_calibration as calibration
from marklab_worker_support.multitype_lgcp import validate_inferred_kernel_request
from marklab_worker_support.multitype_lgcp_calibration import HALF_NORMAL_MEAN, HALF_NORMAL_SD
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
    derive_seed,
    exact,
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
    "marklab.numpy_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_"
    "prior_calibration_request"
)
RESULT_FORMAT = (
    "marklab.numpy_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_"
    "prior_calibration_result"
)
INFERRED_WORKER = (
    "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_worker.py"
)
FIXED_WORKER = "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_worker.py"


def validate(
    request: Any, lock_sha: str, worker_sha: str, inferred_sha: str, fixed_sha: str
) -> dict[str, Any]:
    """Validate the calibration request; the digests identify the two PyMC source workers."""

    def validate_source(source: Any) -> dict[str, Any]:
        return validate_inferred_kernel_request(source, lock_sha, inferred_sha, fixed_sha)[1]

    request, config = calibration.validate_request(
        request,
        REQUEST_FORMAT,
        lock_sha,
        worker_sha,
        validate_source,
        "maximum_field_whitened_moment_error",
    )
    resources = obj(
        request["resources"],
        {
            "node_type_rows",
            "simulation_node_type_work",
            "maximum_simulation_node_type_work",
            "kernel_cube_work_per_replicate",
            "kernel_simulation_work",
            "maximum_kernel_simulation_work",
            "estimated_working_bytes",
            "maximum_working_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    calibration.check_work(resources, config, bytes_per_node=112)
    kernel_work = int(request["source_request"]["resources"]["kernel_cube_work"])
    exact(resources["kernel_cube_work_per_replicate"], kernel_work, "resources.kernel_work")
    total_kernel_work = config["replicates"] * kernel_work
    exact(resources["kernel_simulation_work"], total_kernel_work, "resources.total_kernel_work")
    maximum_kernel_work = integer(
        resources["maximum_kernel_simulation_work"], "resources.max_kernel_work", 1, 10**12
    )
    if total_kernel_work > maximum_kernel_work:
        raise ContractError("kernel-work ceiling exceeded")
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3600)
    return config


def seed_for(seed: int) -> int:
    return derive_seed("marklab-inferred-multitype-lgcp-prior-calibration-v1", seed, width=8)


def calibrate(
    config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str
) -> dict[str, Any]:
    """Draw from the model prior and compare the draws with their analytic moments.

    Prior: the hierarchy of the fixed-kernel calibration, plus a shared Matern-3/2 kernel with
    amplitude ``~ HalfNormal(amplitude_scale)`` and length scale ``~ HalfNormal(length_scale)``
    per replicate. Each pattern's field is ``L w`` with ``w ~ Normal(0, I)`` and ``L`` the
    Cholesky factor of the jittered kernel over the pattern's nodes, centred within the pattern.
    The field oracle recovers ``w`` by solving ``L w = f`` and checks its first two moments.
    """
    rng = np.random.default_rng(seed_for(config["seed"]))
    replicates = config["replicates"]
    types = len(config["types"])
    draws = calibration.hierarchy_prior_draws(rng, config["priors"], replicates, types)
    amplitude = np.abs(rng.normal(0.0, config["amplitude_scale"], replicates))
    length = np.abs(rng.normal(0.0, config["length_scale"], replicates))
    patient_raw = rng.normal(size=(replicates, len(config["patient_ids"]), types))
    patient_effect = draws["patient_sd"][:, None, :] * patient_raw
    pattern_raw = rng.normal(size=(replicates, len(config["pattern_ids"]), types))
    calibration.center_patterns_within_patients(pattern_raw, config)
    pattern_effect = draws["pattern_sd"][:, None, :] * pattern_raw
    latent = np.empty((replicates, types, len(config["weights"])), dtype=np.float64)
    white_sum = 0.0
    white_square = 0.0
    white_count = 0
    maximum_centering = 0.0
    for pattern in range(len(config["pattern_ids"])):
        selected = np.flatnonzero(config["node_patterns"] == pattern)
        rows = [config["request"]["nodes"][index] for index in selected]
        coordinates = np.asarray([[row["x_um"], row["y_um"]] for row in rows])
        distance = np.linalg.norm(coordinates[:, None, :] - coordinates[None, :, :], axis=2)
        scaled = math.sqrt(3.0) * distance[None, :, :] / length[:, None, None]
        covariance = amplitude[:, None, None] ** 2 * (1.0 + scaled) * np.exp(-scaled)
        covariance += config["kernel_jitter"] * np.eye(len(selected))[None, :, :]
        cholesky = np.linalg.cholesky(covariance)
        white = rng.normal(size=(replicates, types, len(selected)))
        uncentered = np.einsum("rij,rtj->rti", cholesky, white)
        recovered = np.linalg.solve(cholesky[:, None, :, :], uncentered[:, :, :, None])[:, :, :, 0]
        white_sum += float(recovered.sum())
        white_square += float(np.square(recovered).sum())
        white_count += recovered.size
        block = uncentered - uncentered.mean(axis=2, keepdims=True)
        latent[:, :, selected] = block
        maximum_centering = max(maximum_centering, float(np.max(np.abs(block.sum(axis=2)))))
    log_expected = calibration.log_expected_counts(
        config, draws, patient_effect, pattern_effect, latent
    )
    poisson = calibration.poisson_oracle(config, rng, log_expected)
    moments = [
        *calibration.hierarchy_moment_checks(config, draws),
        calibration.prior_moment_check(
            config,
            "field_amplitude",
            amplitude,
            config["amplitude_scale"] * HALF_NORMAL_MEAN,
            config["amplitude_scale"] * HALF_NORMAL_SD,
        ),
        calibration.prior_moment_check(
            config,
            "field_length_scale_um",
            length,
            config["length_scale"] * HALF_NORMAL_MEAN,
            config["length_scale"] * HALF_NORMAL_SD,
        ),
    ]
    white_mean = white_sum / white_count
    white_second = white_square / white_count
    tolerance = config["maximum_field_whitened_moment_error"]
    field = {
        "maximum_centering_error": maximum_centering,
        "whitened_mean": white_mean,
        "whitened_second_moment": white_second,
        "passes": bool(
            maximum_centering <= 1e-12
            and abs(white_mean) <= tolerance
            and abs(white_second - 1.0) <= tolerance
        ),
    }
    return calibration.result(
        RESULT_FORMAT, config, request_sha, lock_sha, worker_sha, moments, field, poisson
    )


def main() -> None:
    require_versions(numpy=np.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    inferred_sha = file_sha256(Path(__file__).with_name(INFERRED_WORKER))
    fixed_sha = file_sha256(Path(__file__).with_name(FIXED_WORKER))
    raw = read_request(64 * MIB)
    config = validate(json.loads(raw), lock_sha, worker_sha, inferred_sha, fixed_sha)
    result = calibrate(config, sha256_hex(raw), lock_sha, worker_sha)
    write_result(result, max_bytes=MIB)


if __name__ == "__main__":
    run_worker(main, "marklab inferred multitype calibration failed")
