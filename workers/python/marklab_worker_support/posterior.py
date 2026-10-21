"""Posterior and posterior-predictive summaries shared by the Bayesian workers."""

from __future__ import annotations

from typing import Any

import numpy as np


def summary(
    values: Any, dtype: Any = None, *, flatten: bool = True, chains: int | None = None
) -> dict[str, float]:
    """Return the mean, SD (ddof=1), and central 95% interval of all draws in ``values``.

    ``dtype`` optionally converts the draws first (for example to float64 for float32 JAX
    draws); by default the draws keep their dtype. With ``flatten=False`` the moments are
    reduced over the array as given rather than over a flattened copy, which can differ in the
    last bits for non-contiguous views; workers keep whichever their results were defined with.

    With ``chains``, ``values`` must be the draws of one scalar quantity from ``chains``
    equally long chains in chain-major order (a ``(chain, draw)`` array or its flattening), and
    the result adds ``mcse_mean``, the Monte Carlo standard error of this quantity's mean.
    """
    flat = np.asarray(values, dtype=dtype)
    if flatten:
        flat = flat.reshape(-1)
    result = {
        "mean": float(flat.mean()),
        "sd": float(flat.std(ddof=1)),
        "interval_lower": float(np.quantile(flat, 0.025)),
        "interval_upper": float(np.quantile(flat, 0.975)),
    }
    if chains is not None:
        # Imported here so workers that never report MCSE do not pay for importing ArviZ.
        from .diagnostics import mean_mcse

        result["mcse_mean"] = mean_mcse(values, chains)
    return result


def two_sided_tail(replicated: Any, observed: float) -> float:
    """Two-sided posterior predictive tail probability of ``observed`` among ``replicated``."""
    lower = float(np.mean(replicated <= observed))
    upper = float(np.mean(replicated >= observed))
    return min(1.0, 2.0 * min(lower, upper))


def sigmoid(values: Any) -> Any:
    """Logistic function ``1 / (1 + exp(-x))``."""
    return 1.0 / (1.0 + np.exp(-values))


def clipped_sigmoid(values: Any) -> Any:
    """Logistic function with the argument clipped to ``[-700, 700]`` to avoid overflow."""
    return 1.0 / (1.0 + np.exp(-np.clip(values, -700.0, 700.0)))


def softmax(values: np.ndarray) -> np.ndarray:
    """Softmax over the last axis, shifted by the row maximum for numerical stability."""
    shifted = values - values.max(axis=-1, keepdims=True)
    weights = np.exp(shifted)
    return weights / weights.sum(axis=-1, keepdims=True)
