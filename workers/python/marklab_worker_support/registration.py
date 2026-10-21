"""Helpers shared by the JAX image and landmark registration workers."""

from __future__ import annotations

from collections.abc import Callable
from typing import Any

import jax.numpy as jnp
import numpy as np


def scipy_objective(
    value_and_gradient: Callable[[Any], tuple[Any, Any]],
) -> Callable[[np.ndarray], tuple[float, np.ndarray]]:
    """Adapt a JAX value-and-gradient function to ``scipy.optimize.minimize(jac=True)``."""

    def objective(parameters: np.ndarray) -> tuple[float, np.ndarray]:
        value, gradient = value_and_gradient(jnp.asarray(parameters))
        return float(value), np.asarray(gradient, dtype=float)

    return objective


def pixel_grid(height: int, width: int) -> tuple[np.ndarray, np.ndarray]:
    """Row and column pixel coordinates of an image, indexed ``[row, column]``."""
    return np.meshgrid(np.arange(height, dtype=float), np.arange(width, dtype=float), indexing="ij")


def center_of_mass_translation(
    fixed: np.ndarray, moving: np.ndarray, yy: np.ndarray, xx: np.ndarray
) -> np.ndarray:
    """Moving-minus-fixed intensity centre of mass, as ``(row, column)`` pixels."""
    fixed_mass = fixed.sum()
    moving_mass = moving.sum()
    fixed_center = np.asarray([(fixed * yy).sum() / fixed_mass, (fixed * xx).sum() / fixed_mass])
    moving_center = np.asarray(
        [(moving * yy).sum() / moving_mass, (moving * xx).sum() / moving_mass]
    )
    return moving_center - fixed_center
