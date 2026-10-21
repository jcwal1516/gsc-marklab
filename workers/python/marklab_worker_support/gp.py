"""Matern-3/2 Gaussian-process pieces shared by the exact and variational PyMC GP workers."""

from __future__ import annotations

import math
from typing import Any

import numpy as np

from .protocol import ContractError


def matern32(left: np.ndarray, right: np.ndarray, amplitude: float, length: float) -> np.ndarray:
    """Matern-3/2 covariance between one-dimensional coordinate vectors."""
    distance = np.abs(left[:, None] - right[None, :])
    scaled = math.sqrt(3.0) * distance / length
    return amplitude**2 * (1.0 + scaled) * np.exp(-scaled)


def anisotropic_matern32(
    left: np.ndarray, right: np.ndarray, amplitude: float, length_scales: np.ndarray
) -> np.ndarray:
    """Matern-3/2 covariance between ``(n, d)`` coordinates with one length scale per axis."""
    differences = (left[:, None, :] - right[None, :, :]) / length_scales
    scaled = math.sqrt(3.0) * np.sqrt(np.sum(differences**2, axis=2))
    return amplitude**2 * (1.0 + scaled) * np.exp(-scaled)


def condition(
    observed_covariance: np.ndarray,
    cross_covariance: np.ndarray,
    prediction_covariance: np.ndarray,
    residual: np.ndarray,
) -> tuple[np.ndarray, np.ndarray]:
    """Condition a joint Gaussian on observed values, using a Cholesky factorization.

    Returns the mean shift ``K_*x K_xx^-1 r`` for the residual ``r = y - m(x)`` and the
    conditional covariance ``K_** - K_*x K_xx^-1 K_x*``.
    """
    cholesky = np.linalg.cholesky(observed_covariance)
    alpha = np.linalg.solve(cholesky.T, np.linalg.solve(cholesky, residual))
    shift = cross_covariance @ alpha
    solved = np.linalg.solve(cholesky, cross_covariance.T)
    return shift, prediction_covariance - solved.T @ solved


def append_id(ids: list[str], value: Any, label: str) -> None:
    """Append a trimmed, non-empty ID that is strictly greater than the previous one."""
    if (
        not isinstance(value, str)
        or not value
        or value.strip() != value
        or (ids and value <= ids[-1])
    ):
        raise ContractError(f"{label} IDs must be exact and strictly increasing")
    ids.append(value)


def coordinate_key(value: float) -> int:
    """Bit pattern of a float64 coordinate, used to reject exactly repeated coordinates."""
    return np.float64(value).view(np.uint64).item()


def observed_value_check(observed: np.ndarray, replicated: np.ndarray) -> dict[str, float]:
    """Compare the observed mean and SD with those of replicated data sets."""
    replicated_means = replicated.mean(axis=-1)
    replicated_sds = replicated.std(axis=-1, ddof=1)
    return {
        "observed_mean": float(observed.mean()),
        "replicated_mean": float(replicated_means.mean()),
        "replicated_mean_sd": float(replicated_means.std(ddof=1)),
        "observed_sd": float(observed.std(ddof=1)),
        "replicated_sd_mean": float(replicated_sds.mean()),
    }
