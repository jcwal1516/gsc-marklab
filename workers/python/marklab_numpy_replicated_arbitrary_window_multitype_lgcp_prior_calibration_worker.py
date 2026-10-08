#!/usr/bin/env python3
"""Analytic NumPy prior-generator calibration for replicated multitype LGCPs."""

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
    "marklab.numpy_replicated_arbitrary_window_multitype_lgcp_prior_calibration_request"
)
RESULT_FORMAT = (
    "marklab.numpy_replicated_arbitrary_window_multitype_lgcp_prior_calibration_result"
)
NUMPY_VERSION = "2.4.6"


class ContractError(ValueError):
    pass


def load_pymc() -> Any:
    path = Path(__file__).with_name(
        "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_worker.py"
    )
    specification = importlib.util.spec_from_file_location(
        "marklab_pymc_replicated_multitype_prior_calibration_contract", path
    )
    if specification is None or specification.loader is None:
        raise ContractError("cannot load replicated multitype PyMC contract")
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
    request: Any, lock_sha: str, worker_sha: str, pymc_worker_sha: str
) -> tuple[dict[str, Any], dict[str, Any]]:
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "numpy_version",
            "source_request_sha256",
            "source_request",
            "calibration",
            "resources",
        },
        "request",
    )
    exact(request["format"], REQUEST_FORMAT, "request.format")
    exact(request["version"], 1, "request.version")
    backend = obj(
        request["backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "backend",
    )
    exact(backend["name"], "numpy", "backend.name")
    exact(backend["version"], NUMPY_VERSION, "backend.version")
    exact(backend["python_version"], "3.12", "backend.python_version")
    exact(backend["environment_lock_sha256"], lock_sha, "backend.lock")
    exact(backend["worker_sha256"], worker_sha, "backend.worker")
    exact(request["numpy_version"], NUMPY_VERSION, "request.numpy_version")
    source_sha = request["source_request_sha256"]
    if (
        not isinstance(source_sha, str)
        or len(source_sha) != 64
        or any(character not in "0123456789abcdef" for character in source_sha)
    ):
        raise ContractError("source request digest is invalid")
    config = load_pymc().validate(request["source_request"], lock_sha, pymc_worker_sha)

    calibration = obj(
        request["calibration"],
        {
            "replicates",
            "maximum_mean_error_sd",
            "maximum_relative_sd_error",
            "maximum_field_covariance_rmse_scale",
            "maximum_poisson_residual_mean",
            "maximum_poisson_residual_second_moment_error",
        },
        "calibration",
    )
    config["replicates"] = integer(calibration["replicates"], "replicates", 1024, 65536)
    for name in (
        "maximum_mean_error_sd",
        "maximum_relative_sd_error",
        "maximum_field_covariance_rmse_scale",
        "maximum_poisson_residual_mean",
        "maximum_poisson_residual_second_moment_error",
    ):
        config[name] = number(calibration[name], f"calibration.{name}", 1e-12, 1.0)

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
    types = len(config["types"])
    nodes = len(config["weights"])
    patients = len(config["patient_ids"])
    patterns = len(config["pattern_ids"])
    rows = len(config["observed"])
    exact(resources["node_type_rows"], rows, "resources.node_type_rows")
    work = config["replicates"] * rows
    exact(resources["simulation_node_type_work"], work, "resources.work")
    if work > integer(
        resources["maximum_simulation_node_type_work"], "resources.maximum_work", 1, 10**12
    ):
        raise ContractError("simulation-work ceiling exceeded")
    estimated_bytes = (
        config["replicates"] * types * nodes * 96
        + config["replicates"] * types * (patients + patterns) * 32
        + 384 * 1024**2
    )
    exact(resources["estimated_working_bytes"], estimated_bytes, "resources.estimated_bytes")
    if estimated_bytes > integer(
        resources["maximum_working_bytes"], "resources.maximum_bytes", 1, 8 * 1024**3
    ):
        raise ContractError("working-byte ceiling exceeded")
    integer(resources["timeout_seconds"], "resources.timeout", 1, 86400)
    config["source_request_sha256"] = source_sha
    return request, config


def seed_for(seed: int) -> int:
    digest = hashlib.sha256(
        f"marklab-replicated-multitype-lgcp-prior-calibration-v1\0{seed}".encode()
    ).digest()
    return int.from_bytes(digest[:8], "little")


def moment_check(
    component: str,
    values: np.ndarray,
    expected_mean: float,
    expected_sd: float,
    maximum_mean_error_sd: float,
    maximum_relative_sd_error: float,
) -> dict[str, Any]:
    flat = np.asarray(values, dtype=np.float64).reshape(-1)
    observed_mean = float(flat.mean())
    observed_sd = float(flat.std(ddof=1))
    mean_error_sd = abs(observed_mean - expected_mean) / expected_sd
    relative_sd_error = abs(observed_sd - expected_sd) / expected_sd
    return {
        "component": component,
        "expected_mean": expected_mean,
        "observed_mean": observed_mean,
        "mean_error_sd": mean_error_sd,
        "expected_sd": expected_sd,
        "observed_sd": observed_sd,
        "relative_sd_error": relative_sd_error,
        "passes": bool(
            mean_error_sd <= maximum_mean_error_sd
            and relative_sd_error <= maximum_relative_sd_error
        ),
    }


def calibrate(
    config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str
) -> dict[str, Any]:
    rng = np.random.default_rng(seed_for(config["seed"]))
    replicates = config["replicates"]
    types = len(config["types"])
    patients = len(config["patient_ids"])
    patterns = len(config["pattern_ids"])
    nodes = len(config["weights"])
    priors = config["priors"]

    intercept = rng.normal(
        priors["intercept_mean"], priors["intercept_sd"], (replicates, types)
    )
    group = rng.normal(0.0, priors["group_effect_sd"], (replicates, types))
    covariate = rng.normal(0.0, priors["covariate_effect_sd"], (replicates, types))
    patient_sd = np.abs(
        rng.normal(0.0, priors["patient_sd_scale"], (replicates, types))
    )
    pattern_sd = np.abs(
        rng.normal(0.0, priors["pattern_sd_scale"], (replicates, types))
    )
    patient_raw = rng.normal(size=(replicates, patients, types))
    pattern_raw = rng.normal(size=(replicates, patterns, types))
    patient_effect = patient_sd[:, None, :] * patient_raw
    for patient in range(patients):
        selected = config["pattern_patients"] == patient
        pattern_raw[:, selected, :] -= pattern_raw[:, selected, :].mean(
            axis=1, keepdims=True
        )
    pattern_effect = pattern_sd[:, None, :] * pattern_raw
    field_raw = rng.normal(size=(replicates, types, nodes))
    latent = np.einsum("rtn,mn->rtm", field_raw, config["cholesky"])
    maximum_centering_error = 0.0
    for pattern in range(patterns):
        selected = config["node_patterns"] == pattern
        latent[:, :, selected] -= latent[:, :, selected].mean(axis=2, keepdims=True)
        maximum_centering_error = max(
            maximum_centering_error,
            float(np.max(np.abs(latent[:, :, selected].sum(axis=2)))),
        )
    log_expected = (
        intercept[:, :, None]
        + group[:, :, None]
        * config["patient_groups"][config["node_patients"]][None, None, :]
        + covariate[:, :, None] * config["covariate"][None, None, :]
        + np.transpose(patient_effect[:, config["node_patients"], :], (0, 2, 1))
        + np.transpose(pattern_effect[:, config["node_patterns"], :], (0, 2, 1))
        + latent
        + np.log(config["weights"])[None, None, :]
    )
    expected_matrix = np.exp(log_expected)
    expected = expected_matrix[:, config["type_index"], config["node_index"]]
    if not np.isfinite(expected).all() or np.any(expected <= 0.0):
        raise ContractError("prior generator produced non-finite intensities")
    observed = rng.poisson(expected)
    residual = (observed - expected) / np.sqrt(expected)

    half_mean = math.sqrt(2.0 / math.pi)
    half_sd = math.sqrt(1.0 - 2.0 / math.pi)
    moments = [
        moment_check(
            "intercept",
            intercept,
            priors["intercept_mean"],
            priors["intercept_sd"],
            config["maximum_mean_error_sd"],
            config["maximum_relative_sd_error"],
        ),
        moment_check(
            "group_effect",
            group,
            0.0,
            priors["group_effect_sd"],
            config["maximum_mean_error_sd"],
            config["maximum_relative_sd_error"],
        ),
        moment_check(
            "covariate_effect",
            covariate,
            0.0,
            priors["covariate_effect_sd"],
            config["maximum_mean_error_sd"],
            config["maximum_relative_sd_error"],
        ),
        moment_check(
            "patient_sd",
            patient_sd,
            priors["patient_sd_scale"] * half_mean,
            priors["patient_sd_scale"] * half_sd,
            config["maximum_mean_error_sd"],
            config["maximum_relative_sd_error"],
        ),
        moment_check(
            "pattern_sd",
            pattern_sd,
            priors["pattern_sd_scale"] * half_mean,
            priors["pattern_sd_scale"] * half_sd,
            config["maximum_mean_error_sd"],
            config["maximum_relative_sd_error"],
        ),
    ]

    covariance = config["cholesky"] @ config["cholesky"].T
    squared_error = 0.0
    covariance_elements = 0
    for pattern in range(patterns):
        selected = np.flatnonzero(config["node_patterns"] == pattern)
        centering = np.eye(len(selected)) - np.ones((len(selected), len(selected))) / len(
            selected
        )
        target = centering @ covariance[np.ix_(selected, selected)] @ centering
        values = latent[:, :, selected].reshape(replicates * types, len(selected))
        empirical = np.cov(values, rowvar=False, ddof=1)
        if len(selected) == 1:
            empirical = np.asarray([[float(empirical)]])
        squared_error += float(np.square(empirical - target).sum())
        covariance_elements += target.size
    covariance_rmse_scale = math.sqrt(squared_error / covariance_elements) / (
        priors["field_amplitude"] ** 2
    )
    field_passes = bool(
        maximum_centering_error <= 1e-12
        and covariance_rmse_scale <= config["maximum_field_covariance_rmse_scale"]
    )
    residual_mean = float(residual.mean())
    residual_second = float(np.square(residual).mean())
    poisson_passes = bool(
        abs(residual_mean) <= config["maximum_poisson_residual_mean"]
        and abs(residual_second - 1.0)
        <= config["maximum_poisson_residual_second_moment_error"]
    )
    complete = all(row["passes"] for row in moments) and field_passes and poisson_passes
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": {
            "name": "numpy",
            "version": np.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha,
            "worker_sha256": worker_sha,
        },
        "numpy_version": np.__version__,
        "input_sha256": config["input_sha"],
        "request_sha256": request_sha,
        "source_request_sha256": config["source_request_sha256"],
        "fit_state": "complete" if complete else "nonconverged",
        "moment_checks": moments,
        "field_oracle": {
            "maximum_centering_error": maximum_centering_error,
            "covariance_rmse_scale": covariance_rmse_scale,
            "passes": field_passes,
        },
        "poisson_oracle": {
            "standardized_residual_count": int(residual.size),
            "standardized_residual_mean": residual_mean,
            "standardized_residual_second_moment": residual_second,
            "passes": poisson_passes,
        },
    }


def main() -> int:
    if np.__version__ != NUMPY_VERSION or sys.version_info[:2] != (3, 12):
        raise ContractError("NumPy or Python version drift")
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    pymc_worker_sha = hashlib.sha256(
        script.with_name(
            "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_worker.py"
        ).read_bytes()
    ).hexdigest()
    raw = sys.stdin.buffer.read(64 * 1_048_576 + 1)
    if not raw or len(raw) > 64 * 1_048_576:
        raise ContractError("request size is invalid")
    request_sha = hashlib.sha256(raw).hexdigest()
    _, config = validate(json.loads(raw), lock_sha, worker_sha, pymc_worker_sha)
    result = calibrate(config, request_sha, lock_sha, worker_sha)
    output = json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")) + "\n"
    if len(output.encode()) > 1_048_576:
        raise ContractError("result exceeds output ceiling")
    sys.stdout.write(output)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(
            f"marklab NumPy replicated multitype LGCP prior calibration failed: "
            f"{type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
