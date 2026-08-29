#!/usr/bin/env python3
"""Analytic prior-generator calibration for inferred-kernel multitype LGCPs."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import math
from pathlib import Path
import sys
from typing import Any

import numpy as np


REQUEST_FORMAT = (
    "marklab.numpy_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_prior_calibration_request"
)
RESULT_FORMAT = (
    "marklab.numpy_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_prior_calibration_result"
)
NUMPY_VERSION = "2.4.6"


class ContractError(ValueError):
    pass


def load_pymc() -> Any:
    path = Path(__file__).with_name(
        "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_worker.py"
    )
    specification = importlib.util.spec_from_file_location(
        "marklab_pymc_inferred_multitype_calibration_contract", path
    )
    if specification is None or specification.loader is None:
        raise ContractError("cannot load inferred multitype contract")
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


def number(value: Any, path: str, low: float, high: float) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ContractError(f"{path} must be numeric")
    result = float(value)
    if not math.isfinite(result) or not low <= result <= high:
        raise ContractError(f"{path} must be finite in [{low}, {high}]")
    return result


def integer(value: Any, path: str, low: int, high: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not low <= value <= high:
        raise ContractError(f"{path} must be an integer in [{low}, {high}]")
    return value


def validate(
    request: Any, lock_sha: str, worker_sha: str, pymc_inferred_sha: str, pymc_fixed_sha: str
) -> tuple[dict[str, Any], dict[str, Any]]:
    request = obj(
        request,
        {"format", "version", "backend", "numpy_version", "source_request_sha256",
         "source_request", "calibration", "resources"},
        "request",
    )
    exact(request["format"], REQUEST_FORMAT, "request.format")
    exact(request["version"], 1, "request.version")
    backend = obj(request["backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "backend")
    exact(backend["name"], "numpy", "backend.name")
    exact(backend["version"], NUMPY_VERSION, "backend.version")
    exact(backend["python_version"], "3.12", "backend.python_version")
    exact(backend["environment_lock_sha256"], lock_sha, "backend.lock")
    exact(backend["worker_sha256"], worker_sha, "backend.worker")
    exact(request["numpy_version"], NUMPY_VERSION, "request.numpy_version")
    source_sha = request["source_request_sha256"]
    if not isinstance(source_sha, str) or len(source_sha) != 64:
        raise ContractError("source request digest is invalid")
    pymc = load_pymc()
    _, config = pymc.validate(
        request["source_request"], lock_sha, pymc_inferred_sha, pymc_fixed_sha
    )
    calibration = obj(request["calibration"],
        {"replicates", "maximum_mean_error_sd", "maximum_relative_sd_error",
         "maximum_field_whitened_moment_error", "maximum_poisson_residual_mean",
         "maximum_poisson_residual_second_moment_error"}, "calibration")
    config["replicates"] = integer(calibration["replicates"], "replicates", 1024, 65536)
    for name in (
        "maximum_mean_error_sd", "maximum_relative_sd_error",
        "maximum_field_whitened_moment_error", "maximum_poisson_residual_mean",
        "maximum_poisson_residual_second_moment_error",
    ):
        config[name] = number(calibration[name], f"calibration.{name}", 1e-12, 1.0)
    resources = obj(request["resources"],
        {"node_type_rows", "simulation_node_type_work", "maximum_simulation_node_type_work",
         "kernel_cube_work_per_replicate", "kernel_simulation_work",
         "maximum_kernel_simulation_work", "estimated_working_bytes",
         "maximum_working_bytes", "timeout_seconds"}, "resources")
    replicates = config["replicates"]
    types = len(config["types"])
    nodes = len(config["weights"])
    patients = len(config["patient_ids"])
    patterns = len(config["pattern_ids"])
    rows = len(config["observed"])
    exact(resources["node_type_rows"], rows, "resources.rows")
    work = replicates * rows
    exact(resources["simulation_node_type_work"], work, "resources.work")
    if work > integer(resources["maximum_simulation_node_type_work"], "resources.max_work", 1, 10**12):
        raise ContractError("simulation-work ceiling exceeded")
    kernel_work = int(request["source_request"]["resources"]["kernel_cube_work"])
    exact(resources["kernel_cube_work_per_replicate"], kernel_work, "resources.kernel_work")
    total_kernel_work = replicates * kernel_work
    exact(resources["kernel_simulation_work"], total_kernel_work, "resources.total_kernel_work")
    if total_kernel_work > integer(resources["maximum_kernel_simulation_work"], "resources.max_kernel_work", 1, 10**12):
        raise ContractError("kernel-work ceiling exceeded")
    estimated = (replicates * types * nodes * 112
        + replicates * types * (patients + patterns) * 32 + 384 * 1024**2)
    exact(resources["estimated_working_bytes"], estimated, "resources.estimated_bytes")
    if estimated > integer(resources["maximum_working_bytes"], "resources.max_bytes", 1, 8 * 1024**3):
        raise ContractError("working-byte ceiling exceeded")
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3600)
    config["source_request_sha256"] = source_sha
    return request, config


def seed_for(seed: int) -> int:
    digest = hashlib.sha256(
        f"marklab-inferred-multitype-lgcp-prior-calibration-v1\0{seed}".encode()
    ).digest()
    return int.from_bytes(digest[:8], "little")


def moment_check(
    component: str, values: np.ndarray, expected_mean: float, expected_sd: float,
    maximum_mean: float, maximum_sd: float,
) -> dict[str, Any]:
    flat = np.asarray(values, dtype=np.float64).reshape(-1)
    observed_mean = float(flat.mean())
    observed_sd = float(flat.std(ddof=1))
    mean_error = abs(observed_mean - expected_mean) / expected_sd
    sd_error = abs(observed_sd - expected_sd) / expected_sd
    return {
        "component": component, "expected_mean": expected_mean,
        "observed_mean": observed_mean, "mean_error_sd": mean_error,
        "expected_sd": expected_sd, "observed_sd": observed_sd,
        "relative_sd_error": sd_error,
        "passes": bool(mean_error <= maximum_mean and sd_error <= maximum_sd),
    }


def calibrate(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    rng = np.random.default_rng(seed_for(config["seed"]))
    r = config["replicates"]
    t = len(config["types"])
    p = len(config["patient_ids"])
    s = len(config["pattern_ids"])
    n = len(config["weights"])
    priors = config["priors"]
    intercept = rng.normal(priors["intercept_mean"], priors["intercept_sd"], (r, t))
    group = rng.normal(0.0, priors["group_effect_sd"], (r, t))
    covariate = rng.normal(0.0, priors["covariate_effect_sd"], (r, t))
    patient_sd = np.abs(rng.normal(0.0, priors["patient_sd_scale"], (r, t)))
    pattern_sd = np.abs(rng.normal(0.0, priors["pattern_sd_scale"], (r, t)))
    amplitude = np.abs(rng.normal(0.0, config["amplitude_scale"], r))
    length = np.abs(rng.normal(0.0, config["length_scale"], r))
    patient_effect = patient_sd[:, None, :] * rng.normal(size=(r, p, t))
    pattern_raw = rng.normal(size=(r, s, t))
    for patient in range(p):
        selected = config["pattern_patients"] == patient
        pattern_raw[:, selected, :] -= pattern_raw[:, selected, :].mean(axis=1, keepdims=True)
    pattern_effect = pattern_sd[:, None, :] * pattern_raw
    latent = np.empty((r, t, n), dtype=np.float64)
    white_sum = 0.0
    white_square = 0.0
    white_count = 0
    maximum_centering = 0.0
    for pattern in range(s):
        selected = np.flatnonzero(config["node_patterns"] == pattern)
        rows = [config["request"]["nodes"][index] for index in selected]
        coordinates = np.asarray([[row["x_um"], row["y_um"]] for row in rows])
        distance = np.linalg.norm(coordinates[:, None, :] - coordinates[None, :, :], axis=2)
        scaled = math.sqrt(3.0) * distance[None, :, :] / length[:, None, None]
        covariance = amplitude[:, None, None] ** 2 * (1.0 + scaled) * np.exp(-scaled)
        covariance += config["kernel_jitter"] * np.eye(len(selected))[None, :, :]
        cholesky = np.linalg.cholesky(covariance)
        white = rng.normal(size=(r, t, len(selected)))
        uncentered = np.einsum("rij,rtj->rti", cholesky, white)
        recovered = np.linalg.solve(
            cholesky[:, None, :, :], uncentered[:, :, :, None]
        )[:, :, :, 0]
        white_sum += float(recovered.sum())
        white_square += float(np.square(recovered).sum())
        white_count += recovered.size
        block = uncentered - uncentered.mean(axis=2, keepdims=True)
        latent[:, :, selected] = block
        maximum_centering = max(maximum_centering, float(np.max(np.abs(block.sum(axis=2)))))
    log_expected = (
        intercept[:, :, None]
        + group[:, :, None] * config["patient_groups"][config["node_patients"]][None, None, :]
        + covariate[:, :, None] * config["covariate"][None, None, :]
        + np.transpose(patient_effect[:, config["node_patients"], :], (0, 2, 1))
        + np.transpose(pattern_effect[:, config["node_patterns"], :], (0, 2, 1))
        + latent + np.log(config["weights"])[None, None, :]
    )
    expected_matrix = np.exp(log_expected)
    expected = expected_matrix[:, config["type_index"], config["node_index"]]
    if not np.isfinite(expected).all() or np.any(expected <= 0.0):
        raise ContractError("prior generator produced non-finite intensities")
    observed = rng.poisson(expected)
    residual = (observed - expected) / np.sqrt(expected)
    half_mean = math.sqrt(2.0 / math.pi)
    half_sd = math.sqrt(1.0 - 2.0 / math.pi)
    values = [
        ("intercept", intercept, priors["intercept_mean"], priors["intercept_sd"]),
        ("group_effect", group, 0.0, priors["group_effect_sd"]),
        ("covariate_effect", covariate, 0.0, priors["covariate_effect_sd"]),
        ("patient_sd", patient_sd, priors["patient_sd_scale"] * half_mean, priors["patient_sd_scale"] * half_sd),
        ("pattern_sd", pattern_sd, priors["pattern_sd_scale"] * half_mean, priors["pattern_sd_scale"] * half_sd),
        ("field_amplitude", amplitude, config["amplitude_scale"] * half_mean, config["amplitude_scale"] * half_sd),
        ("field_length_scale_um", length, config["length_scale"] * half_mean, config["length_scale"] * half_sd),
    ]
    moments = [moment_check(name, value, mean, sd, config["maximum_mean_error_sd"],
        config["maximum_relative_sd_error"]) for name, value, mean, sd in values]
    white_mean = white_sum / white_count
    white_second = white_square / white_count
    field_passes = bool(maximum_centering <= 1e-12
        and abs(white_mean) <= config["maximum_field_whitened_moment_error"]
        and abs(white_second - 1.0) <= config["maximum_field_whitened_moment_error"])
    residual_mean = float(residual.mean())
    residual_second = float(np.square(residual).mean())
    poisson_passes = bool(abs(residual_mean) <= config["maximum_poisson_residual_mean"]
        and abs(residual_second - 1.0) <= config["maximum_poisson_residual_second_moment_error"])
    complete = all(row["passes"] for row in moments) and field_passes and poisson_passes
    return {
        "format": RESULT_FORMAT, "version": 1,
        "backend": {"name": "numpy", "version": np.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha, "worker_sha256": worker_sha},
        "numpy_version": np.__version__, "input_sha256": config["input_sha"],
        "request_sha256": request_sha, "source_request_sha256": config["source_request_sha256"],
        "fit_state": "complete" if complete else "nonconverged", "moment_checks": moments,
        "field_oracle": {"maximum_centering_error": maximum_centering,
            "whitened_mean": white_mean, "whitened_second_moment": white_second,
            "passes": field_passes},
        "poisson_oracle": {"standardized_residual_count": int(residual.size),
            "standardized_residual_mean": residual_mean,
            "standardized_residual_second_moment": residual_second,
            "passes": poisson_passes},
    }


def main() -> int:
    if np.__version__ != NUMPY_VERSION or sys.version_info[:2] != (3, 12):
        raise ContractError("NumPy or Python version drift")
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    pymc_inferred_sha = hashlib.sha256(script.with_name(
        "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_worker.py").read_bytes()).hexdigest()
    pymc_fixed_sha = hashlib.sha256(script.with_name(
        "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_worker.py").read_bytes()).hexdigest()
    raw = sys.stdin.buffer.read(64 * 1_048_576 + 1)
    if not raw or len(raw) > 64 * 1_048_576:
        raise ContractError("request size is invalid")
    request_sha = hashlib.sha256(raw).hexdigest()
    _, config = validate(json.loads(raw), lock_sha, worker_sha, pymc_inferred_sha, pymc_fixed_sha)
    output = json.dumps(calibrate(config, request_sha, lock_sha, worker_sha),
        allow_nan=False, sort_keys=True, separators=(",", ":")) + "\n"
    if len(output.encode()) > 1_048_576:
        raise ContractError("result exceeds output ceiling")
    sys.stdout.write(output)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"marklab inferred multitype calibration failed: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from error
