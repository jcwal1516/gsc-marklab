"""Pieces shared by the multimodal factor-model workers.

Feature standardization from training entries, factor sign alignment, and the L-BFGS
maximum-a-posteriori fit with posterior curvature calculated from the objective's gradient.
Delta-method variances use that curvature on the identified parameter subspace.
"""

from __future__ import annotations

from collections.abc import Callable, Sequence
from dataclasses import dataclass
from typing import Any

import numpy as np
from scipy.optimize import OptimizeResult, minimize

from .protocol import ContractError

ObjectiveAndGradient = Callable[[np.ndarray], tuple[float, np.ndarray]]


def observed_feature_standardization(
    values: np.ndarray, observed: np.ndarray, label: str
) -> tuple[np.ndarray, np.ndarray]:
    """Return per-feature means and population SDs computed from the observed entries only."""
    features = range(values.shape[1])
    means = np.asarray([values[:, feature][observed[:, feature]].mean() for feature in features])
    scales = np.asarray(
        [values[:, feature][observed[:, feature]].std(ddof=0) for feature in features]
    )
    if not np.isfinite(scales).all() or np.any(scales <= 0.0):
        raise ContractError(f"every {label} feature must vary")
    return means, scales


def align_factor_signs(loadings: np.ndarray, scores: np.ndarray) -> int:
    """Flip factors in place so each column's largest-magnitude loading is positive.

    The matching column of ``scores`` is flipped with it, leaving their product unchanged.
    Returns the number of flipped factors.
    """
    flips = 0
    for factor in range(loadings.shape[1]):
        pivot = int(np.argmax(np.abs(loadings[:, factor])))
        if loadings[pivot, factor] < 0.0:
            loadings[:, factor] *= -1.0
            scores[:, factor] *= -1.0
            flips += 1
    return flips


def jax_objective(objective: Callable[[Any], Any]) -> ObjectiveAndGradient:
    """Wrap a scalar JAX objective as a SciPy objective returning (value, gradient)."""
    import jax
    import jax.numpy as jnp

    value_and_gradient = jax.jit(jax.value_and_grad(objective))

    def objective_and_gradient(parameters: np.ndarray) -> tuple[float, np.ndarray]:
        value, gradient = value_and_gradient(jnp.asarray(parameters))
        return float(value), np.asarray(gradient, dtype=float)

    return objective_and_gradient


def entry_gradient(reconstruction: Callable[[Any], Any], parameters: Any, index: Any) -> np.ndarray:
    """Gradient, by JAX, of the reconstructed entry ``index`` with respect to the parameters."""
    import jax

    gradient = jax.grad(lambda values: reconstruction(values)[index])(parameters)
    return np.asarray(gradient, dtype=float)


def minimize_lbfgs(
    objective_and_gradient: ObjectiveAndGradient,
    initial: np.ndarray,
    *,
    maximum_iterations: int,
    ftol: float,
    gtol: float,
    failure: str,
) -> tuple[float, OptimizeResult]:
    """Minimize with L-BFGS-B from ``initial``; return the initial objective and the fit.

    A fit that does not report success or has non-finite parameters raises
    ``ContractError(f"{failure}: <optimizer message>")``.
    """
    initial_objective = objective_and_gradient(initial)[0]
    fit = minimize(
        objective_and_gradient,
        initial,
        jac=True,
        method="L-BFGS-B",
        options={"maxiter": maximum_iterations, "ftol": ftol, "gtol": gtol},
    )
    if not fit.success or not np.isfinite(fit.x).all():
        raise ContractError(f"{failure}: {fit.message}")
    return initial_objective, fit


def inverse_hessian_covariance(
    fit: OptimizeResult,
    failure: str,
    objective_and_gradient: ObjectiveAndGradient,
    invariant_directions: np.ndarray | None = None,
) -> tuple[np.ndarray, np.ndarray]:
    """Return inverse posterior curvature and the unidentified parameter directions.

    Compute the central-difference Jacobian of the objective's exact gradient. Factor
    rotations can leave both the objective and predictions unchanged. Invert only positive
    curvature; prediction gradients must be orthogonal to the null directions.
    Materially negative curvature is a saddle, so it cannot supply Laplace uncertainty.
    """
    # Use the declared bounded model dimensions, as for the former dense covariance.
    steps = np.cbrt(np.finfo(np.float64).eps) * np.maximum(1.0, np.abs(fit.x))
    hessian = np.empty((len(fit.x), len(fit.x)), dtype=np.float64)
    for index, step in enumerate(steps):
        displacement = np.zeros_like(fit.x)
        displacement[index] = step
        above = objective_and_gradient(fit.x + displacement)[1]
        below = objective_and_gradient(fit.x - displacement)[1]
        hessian[:, index] = (above - below) / (2.0 * step)
    hessian = 0.5 * (hessian + hessian.T)
    if hessian.shape != (len(fit.x), len(fit.x)) or not np.isfinite(hessian).all():
        raise ContractError(failure)
    basis = np.eye(len(fit.x))
    rotations = np.empty((len(fit.x), 0))
    if invariant_directions is not None and invariant_directions.shape[1] > 0:
        left, singular, _ = np.linalg.svd(invariant_directions, full_matrices=True)
        rank = int(np.count_nonzero(singular > 1e-12 * singular[0]))
        rotations, basis = left[:, :rank], left[:, rank:]
    # Remove known factor rotations explicitly: a finite optimization tolerance can give
    # these invariant directions small nonzero curvature even with exact derivatives.
    eigenvalues, eigenvectors = np.linalg.eigh(basis.T @ hessian @ basis)
    # Account for central-difference error and near-stationary rotational directions.
    tolerance = 1e-8 * float(np.max(np.abs(eigenvalues)))
    if np.any(eigenvalues < -tolerance):
        raise ContractError(f"{failure}: negative posterior curvature")
    identified = eigenvalues > tolerance
    vectors = basis @ eigenvectors[:, identified]
    covariance = (vectors / eigenvalues[identified]) @ vectors.T
    if not np.isfinite(covariance).all():
        raise ContractError(failure)
    return covariance, np.column_stack([rotations, basis @ eigenvectors[:, ~identified]])


def delta_method_variance(
    gradient: np.ndarray, covariance: tuple[np.ndarray, np.ndarray], noise_variance: float
) -> float:
    """Predictive variance ``g' C g + noise_variance`` for an identified fitted entry."""
    matrix, null_directions = covariance
    if np.linalg.norm(null_directions.T @ gradient) > 1e-7 * np.linalg.norm(gradient):
        raise ContractError("prediction depends on unidentified posterior directions")
    variance = float(gradient @ matrix @ gradient) + noise_variance
    if not np.isfinite(variance) or variance < 0.0:
        raise ContractError("prediction variance must be finite and nonnegative")
    return max(variance, 1e-12)


@dataclass
class PairedViews:
    """Two views of the same rows standardized with training-row means and SDs."""

    x: np.ndarray
    y: np.ndarray
    train: np.ndarray
    test: np.ndarray
    means_x: np.ndarray
    means_y: np.ndarray
    scales_x: np.ndarray
    scales_y: np.ndarray
    standardized_x: np.ndarray
    standardized_y: np.ndarray

    def standardization_record(self) -> dict[str, Any]:
        return {
            "fit_split": "train_only",
            "fit_row_count": int(self.train.sum()),
            "mean_x": self.means_x.tolist(),
            "scale_x": self.scales_x.tolist(),
            "mean_y": self.means_y.tolist(),
            "scale_y": self.scales_y.tolist(),
        }


def standardize_paired_rows(rows: Sequence[dict[str, Any]]) -> PairedViews:
    """Split paired rows into train/test and standardize both views on the training rows."""
    x = np.asarray([row["x"] for row in rows], dtype=float)
    y = np.asarray([row["y"] for row in rows], dtype=float)
    train = np.asarray([row["split"] == "train" for row in rows], dtype=bool)
    test = np.asarray([row["split"] == "test" for row in rows], dtype=bool)
    if train.sum() < 8 or test.sum() < 1 or not np.isfinite(x).all() or not np.isfinite(y).all():
        raise ContractError("pCCA requires finite training and held-out paired rows")
    means_x = x[train].mean(axis=0)
    means_y = y[train].mean(axis=0)
    scales_x = x[train].std(axis=0, ddof=0)
    scales_y = y[train].std(axis=0, ddof=0)
    if np.any(scales_x <= 0.0) or np.any(scales_y <= 0.0):
        raise ContractError("training features must vary")
    return PairedViews(
        x=x,
        y=y,
        train=train,
        test=test,
        means_x=means_x,
        means_y=means_y,
        scales_x=scales_x,
        scales_y=scales_y,
        standardized_x=(x - means_x) / scales_x,
        standardized_y=(y - means_y) / scales_y,
    )
