"""Prior-generator calibration shared by the replicated multitype LGCP calibration workers.

The fixed-kernel and inferred-kernel workers draw the hierarchy parameters, patient and
pattern effects, and the pattern-centred latent field from the model prior with NumPy, simulate
Poisson counts at the quadrature nodes, and compare the draws with analytic values: the means
and SDs of the hierarchy priors, a worker-specific latent-field oracle, and the first two
moments of the standardized Poisson residuals.
"""

from __future__ import annotations

import math
from collections.abc import Callable
from typing import Any

import numpy as np

from . import versions
from .multitype_lgcp import moment_check
from .protocol import (
    ContractError,
    backend_identity,
    exact,
    integer,
    number,
    obj,
    sha256_digest,
    validate_backend,
)

REQUEST_KEYS = frozenset(
    {
        "format",
        "version",
        "backend",
        "numpy_version",
        "source_request_sha256",
        "source_request",
        "calibration",
        "resources",
    }
)
HALF_NORMAL_MEAN = math.sqrt(2.0 / math.pi)
"""Mean of a unit-scale half-normal distribution."""
HALF_NORMAL_SD = math.sqrt(1.0 - 2.0 / math.pi)
"""SD of a unit-scale half-normal distribution."""


def validate_request(
    request: Any,
    request_format: str,
    lock_sha: str,
    worker_sha: str,
    validate_source: Callable[[Any], dict[str, Any]],
    field_threshold: str,
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Validate the calibration request envelope, its source request, and the thresholds.

    ``validate_source`` parses the embedded PyMC source request into the model configuration;
    ``field_threshold`` names the worker's latent-field tolerance. The ``resources`` block is
    left to the caller. Returns the request and the configuration extended with the
    replicate count, the thresholds, and the source request digest.
    """
    request = obj(request, REQUEST_KEYS, "request")
    exact(request["format"], request_format, "request.format")
    exact(request["version"], 1, "request.version")
    validate_backend(
        request["backend"],
        "backend",
        name="numpy",
        version=versions.NUMPY,
        lock_sha256=lock_sha,
        worker_sha256=worker_sha,
    )
    exact(request["numpy_version"], versions.NUMPY, "request.numpy_version")
    source_sha = sha256_digest(request["source_request_sha256"], "request.source_request_sha256")
    config = validate_source(request["source_request"])
    thresholds = (
        "maximum_mean_error_sd",
        "maximum_relative_sd_error",
        field_threshold,
        "maximum_poisson_residual_mean",
        "maximum_poisson_residual_second_moment_error",
    )
    calibration = obj(request["calibration"], {"replicates", *thresholds}, "calibration")
    config["replicates"] = integer(calibration["replicates"], "replicates", 1024, 65536)
    for name in thresholds:
        config[name] = number(calibration[name], f"calibration.{name}", 1e-12, 1.0)
    config["source_request_sha256"] = source_sha
    return request, config


def check_work(resources: dict[str, Any], config: dict[str, Any], bytes_per_node: int) -> None:
    """Check the simulation work and working-memory figures of the ``resources`` block.

    Both must equal what this request implies; the memory estimate allots ``bytes_per_node``
    bytes per replicate, type, and node, 32 per replicate, type, and patient or pattern, and a
    fixed 384 MiB.
    """
    replicates = config["replicates"]
    types = len(config["types"])
    nodes = len(config["weights"])
    rows = len(config["observed"])
    exact(resources["node_type_rows"], rows, "resources.node_type_rows")
    work = replicates * rows
    exact(resources["simulation_node_type_work"], work, "resources.simulation_node_type_work")
    maximum_work = integer(
        resources["maximum_simulation_node_type_work"],
        "resources.maximum_simulation_node_type_work",
        1,
        10**12,
    )
    if work > maximum_work:
        raise ContractError("simulation-work ceiling exceeded")
    groups = len(config["patient_ids"]) + len(config["pattern_ids"])
    estimated_bytes = (
        replicates * types * nodes * bytes_per_node
        + replicates * types * groups * 32
        + 384 * 1024**2
    )
    exact(resources["estimated_working_bytes"], estimated_bytes, "resources.estimated_bytes")
    maximum_bytes = integer(
        resources["maximum_working_bytes"], "resources.maximum_working_bytes", 1, 8 * 1024**3
    )
    if estimated_bytes > maximum_bytes:
        raise ContractError("working-byte ceiling exceeded")


def hierarchy_prior_draws(
    rng: np.random.Generator, priors: dict[str, float], replicates: int, types: int
) -> dict[str, np.ndarray]:
    """Draw the per-type intercept, effects, and random-effect SDs, ``(replicate, type)`` each."""
    shape = (replicates, types)
    return {
        "intercept": rng.normal(priors["intercept_mean"], priors["intercept_sd"], shape),
        "group_effect": rng.normal(0.0, priors["group_effect_sd"], shape),
        "covariate_effect": rng.normal(0.0, priors["covariate_effect_sd"], shape),
        "patient_sd": np.abs(rng.normal(0.0, priors["patient_sd_scale"], shape)),
        "pattern_sd": np.abs(rng.normal(0.0, priors["pattern_sd_scale"], shape)),
    }


def center_patterns_within_patients(pattern_raw: np.ndarray, config: dict[str, Any]) -> None:
    """Subtract, in place, each patient's mean over its patterns (axis 1)."""
    for patient in range(len(config["patient_ids"])):
        selected = config["pattern_patients"] == patient
        pattern_raw[:, selected, :] -= pattern_raw[:, selected, :].mean(axis=1, keepdims=True)


def log_expected_counts(
    config: dict[str, Any],
    draws: dict[str, np.ndarray],
    patient_effect: np.ndarray,
    pattern_effect: np.ndarray,
    latent: np.ndarray,
) -> np.ndarray:
    """Log expected count of each ``(replicate, type, node)``.

    The sum of the intercept, the group and covariate effects, the node's patient and pattern
    effects, the latent field, and the log quadrature weight.
    """
    node_groups = config["patient_groups"][config["node_patients"]]
    node_patient_effect = np.transpose(patient_effect[:, config["node_patients"], :], (0, 2, 1))
    node_pattern_effect = np.transpose(pattern_effect[:, config["node_patterns"], :], (0, 2, 1))
    return (
        draws["intercept"][:, :, None]
        + draws["group_effect"][:, :, None] * node_groups[None, None, :]
        + draws["covariate_effect"][:, :, None] * config["covariate"][None, None, :]
        + node_patient_effect
        + node_pattern_effect
        + latent
        + np.log(config["weights"])[None, None, :]
    )


def poisson_oracle(
    config: dict[str, Any], rng: np.random.Generator, log_expected: np.ndarray
) -> dict[str, Any]:
    """Simulate Poisson counts at the observed node-type rows and check standardized residuals.

    The residuals ``(y - mu) / sqrt(mu)`` should have mean zero and second moment one.
    """
    expected_matrix = np.exp(log_expected)
    expected = expected_matrix[:, config["type_index"], config["node_index"]]
    if not np.isfinite(expected).all() or np.any(expected <= 0.0):
        raise ContractError("prior generator produced non-finite intensities")
    observed = rng.poisson(expected)
    residual = (observed - expected) / np.sqrt(expected)
    residual_mean = float(residual.mean())
    residual_second = float(np.square(residual).mean())
    return {
        "standardized_residual_count": int(residual.size),
        "standardized_residual_mean": residual_mean,
        "standardized_residual_second_moment": residual_second,
        "passes": bool(
            abs(residual_mean) <= config["maximum_poisson_residual_mean"]
            and abs(residual_second - 1.0) <= config["maximum_poisson_residual_second_moment_error"]
        ),
    }


def prior_moment_check(
    config: dict[str, Any], component: str, values: np.ndarray, mean: float, sd: float
) -> dict[str, Any]:
    """``moment_check`` with the request's mean and SD tolerances."""
    return moment_check(
        component,
        values,
        mean,
        sd,
        config["maximum_mean_error_sd"],
        config["maximum_relative_sd_error"],
    )


def hierarchy_moment_checks(
    config: dict[str, Any], draws: dict[str, np.ndarray]
) -> list[dict[str, Any]]:
    """Moment checks of the normal effects and the half-normal random-effect SDs."""
    priors = config["priors"]
    return [
        prior_moment_check(
            config,
            "intercept",
            draws["intercept"],
            priors["intercept_mean"],
            priors["intercept_sd"],
        ),
        prior_moment_check(
            config, "group_effect", draws["group_effect"], 0.0, priors["group_effect_sd"]
        ),
        prior_moment_check(
            config,
            "covariate_effect",
            draws["covariate_effect"],
            0.0,
            priors["covariate_effect_sd"],
        ),
        prior_moment_check(
            config,
            "patient_sd",
            draws["patient_sd"],
            priors["patient_sd_scale"] * HALF_NORMAL_MEAN,
            priors["patient_sd_scale"] * HALF_NORMAL_SD,
        ),
        prior_moment_check(
            config,
            "pattern_sd",
            draws["pattern_sd"],
            priors["pattern_sd_scale"] * HALF_NORMAL_MEAN,
            priors["pattern_sd_scale"] * HALF_NORMAL_SD,
        ),
    ]


def result(
    result_format: str,
    config: dict[str, Any],
    request_sha: str,
    lock_sha: str,
    worker_sha: str,
    moments: list[dict[str, Any]],
    field_oracle: dict[str, Any],
    poisson: dict[str, Any],
) -> dict[str, Any]:
    """The calibration result; it is complete when every check passes."""
    complete = (
        all(row["passes"] for row in moments) and field_oracle["passes"] and poisson["passes"]
    )
    return {
        "format": result_format,
        "version": 1,
        "backend": backend_identity("numpy", np.__version__, lock_sha, worker_sha),
        "numpy_version": np.__version__,
        "input_sha256": config["input_sha"],
        "request_sha256": request_sha,
        "source_request_sha256": config["source_request_sha256"],
        "fit_state": "complete" if complete else "nonconverged",
        "moment_checks": moments,
        "field_oracle": field_oracle,
        "poisson_oracle": poisson,
    }
