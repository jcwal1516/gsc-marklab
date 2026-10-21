"""Penalized logistic regression and Platt scaling shared by the SciPy prediction workers."""

from __future__ import annotations

import math

import numpy as np
from scipy.optimize import minimize
from scipy.special import expit

from .protocol import ContractError


def fit_penalized_logistic(x: np.ndarray, y: np.ndarray, penalty: float) -> np.ndarray:
    """Fit an L2-penalized logistic regression by BFGS from zero.

    Model: ``P(y = 1) = expit(b0 + x @ b)``. The objective is the Bernoulli negative
    log-likelihood plus ``penalty / 2 * ||b||^2``; the intercept ``b0`` is unpenalized.
    Returns ``[b0, *b]``.
    """

    def objective(parameters: np.ndarray) -> tuple[float, np.ndarray]:
        z = parameters[0] + x @ parameters[1:]
        coefficients = parameters[1:]
        log_likelihood = np.sum(np.logaddexp(0.0, z) - y * z)
        value = float(log_likelihood + 0.5 * penalty * np.dot(coefficients, coefficients))
        residual = expit(z) - y
        gradient = np.concatenate(
            [np.asarray([np.sum(residual)]), x.T @ residual + penalty * coefficients]
        )
        return value, gradient

    fitted = minimize(
        objective,
        np.zeros(x.shape[1] + 1),
        method="BFGS",
        jac=True,
        options={"maxiter": 1_000, "gtol": 1e-8},
    )
    if not fitted.success or not np.all(np.isfinite(fitted.x)):
        raise ContractError(f"logistic optimizer failed: {fitted.message}")
    return np.asarray(fitted.x, dtype=np.float64)


def platt_targets(labels: np.ndarray) -> np.ndarray:
    """Platt's smoothed 0/1 targets: (n+ + 1) / (n+ + 2) for positives, 1 / (n- + 2) otherwise."""
    positives = int(np.sum(labels))
    negatives = len(labels) - positives
    return np.where(labels == 1, (positives + 1.0) / (positives + 2.0), 1.0 / (negatives + 2.0))


def clipped_logit(probabilities: np.ndarray) -> np.ndarray:
    """Logit of probabilities clipped to ``[1e-12, 1 - 1e-12]``."""
    return np.log(
        np.clip(probabilities, 1e-12, 1.0 - 1e-12) / np.clip(1.0 - probabilities, 1e-12, 1.0)
    )


def fit_platt_to_probabilities(probabilities: np.ndarray, labels: np.ndarray) -> np.ndarray:
    """Platt-scale probabilities by BFGS from the identity map ``(0, 1)``.

    Model: ``P(y = 1) = expit(a + b * logit(p))`` fitted to Platt's smoothed targets by
    minimizing the Bernoulli cross-entropy. Returns ``[a, b]``.
    """
    targets = platt_targets(labels)
    logits = clipped_logit(probabilities)

    def objective(parameters: np.ndarray) -> tuple[float, np.ndarray]:
        z = parameters[0] + parameters[1] * logits
        residual = expit(z) - targets
        value = float(np.sum(np.logaddexp(0.0, z) - targets * z))
        return value, np.asarray([np.sum(residual), np.dot(residual, logits)])

    fitted = minimize(
        objective,
        np.asarray([0.0, 1.0]),
        method="BFGS",
        jac=True,
        options={"maxiter": 1_000, "gtol": 1e-8},
    )
    if not fitted.success or not np.all(np.isfinite(fitted.x)):
        raise ContractError(f"Platt calibration failed: {fitted.message}")
    return np.asarray(fitted.x, dtype=np.float64)


def apply_platt(calibrator: np.ndarray, probability: float) -> float:
    """Map one probability through a calibrator returned by ``fit_platt_to_probabilities``."""
    clipped = min(max(probability, 1e-12), 1.0 - 1e-12)
    return float(expit(calibrator[0] + calibrator[1] * math.log(clipped / (1.0 - clipped))))
