#!/usr/bin/env python3
"""Multiresolution spatial factor model: per-scale basis factors with Laplace uncertainty."""

import json
import math
import sys
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parent))

import jax
import jax.numpy as jnp
import numpy as np
import scipy

from marklab_worker_support.factor_models import (
    align_factor_signs,
    delta_method_variance,
    entry_gradient,
    inverse_hessian_covariance,
    jax_objective,
    minimize_lbfgs,
    observed_feature_standardization,
)
from marklab_worker_support.protocol import (
    CONTRACT_ERRORS,
    MIB,
    ContractError,
    exact,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    write_result,
)


def main() -> None:
    require_versions(jax=jax.__version__, numpy=np.__version__, scipy=scipy.__version__)
    jax.config.update("jax_enable_x64", True)
    request_bytes = read_request(16 * MIB)
    request = json.loads(request_bytes)
    exact(request["format"], "marklab.jax_multiresolution_factor_request", "request.format")
    exact(request["version"], 1, "request.version")
    rows = request["rows"]
    values = np.asarray([row["values"] for row in rows], dtype=float)
    observed = np.asarray([row["observed"] for row in rows], dtype=bool)
    n, p = values.shape
    means, scales = observed_feature_standardization(values, observed, "multiresolution")
    standardized = (values - means) / scales
    training = np.where(observed, standardized, 0.0)
    bases = [np.asarray(scale["basis_columns"], dtype=float).T for scale in request["scales"]]
    coefficient_shapes = []
    loading_shapes = []
    initial_parts = []
    residual = training.copy()
    for scale, basis in zip(request["scales"], bases, strict=True):
        coefficient_target = np.linalg.pinv(basis) @ residual
        left, singular, right_t = np.linalg.svd(coefficient_target, full_matrices=False)
        factors = scale["factors"]
        root = np.sqrt(np.maximum(singular[:factors], 1e-8))
        coefficients = left[:, :factors] * root
        loadings = right_t[:factors].T * root
        coefficient_shapes.append(coefficients.shape)
        loading_shapes.append(loadings.shape)
        initial_parts.extend([coefficients.ravel(), loadings.ravel()])
        residual -= basis @ coefficients @ loadings.T
    initial = np.concatenate(initial_parts)
    bases_jax = [jnp.asarray(basis) for basis in bases]
    observed_jax = jnp.asarray(observed)
    standardized_jax = jnp.asarray(standardized)

    def unpack(parameters):
        offset = 0
        parts = []
        for coefficient_shape, loading_shape in zip(
            coefficient_shapes, loading_shapes, strict=True
        ):
            coefficient_size = int(np.prod(coefficient_shape))
            loading_size = int(np.prod(loading_shape))
            coefficients = parameters[offset : offset + coefficient_size].reshape(coefficient_shape)
            offset += coefficient_size
            loadings = parameters[offset : offset + loading_size].reshape(loading_shape)
            offset += loading_size
            parts.append((coefficients, loadings))
        return parts

    def scale_maps(parameters):
        return [
            basis @ coefficients @ loadings.T
            for basis, (coefficients, loadings) in zip(bases_jax, unpack(parameters), strict=True)
        ]

    def reconstruction(parameters):
        maps = scale_maps(parameters)
        return sum(maps[1:], maps[0])

    def objective(parameters):
        """Negative log posterior (up to a constant) of the multiresolution factor model.

        Standardized entries ``y_ij ~ Normal(sum_s (B_s C_s L_s')_ij, sigma^2)`` over observed
        entries, with declared spatial basis ``B_s``, basis coefficients
        ``C_s ~ Normal(0, 1 / coefficient_precision_s)``, and feature loadings
        ``L_s ~ Normal(0, 1 / loading_precision)`` for every scale ``s``.
        """
        prediction = reconstruction(parameters)
        residuals = jnp.where(observed_jax, prediction - standardized_jax, 0.0)
        penalty = 0.0
        for scale, (coefficients, loadings) in zip(
            request["scales"], unpack(parameters), strict=True
        ):
            penalty += scale["coefficient_precision"] * jnp.sum(coefficients * coefficients)
            penalty += request["loading_precision"] * jnp.sum(loadings * loadings)
        return (
            0.5 * jnp.sum(residuals * residuals) / request["noise_standard_deviation"] ** 2
            + 0.5 * penalty
        )

    objective_and_gradient = jax_objective(objective)
    initial_objective, fit = minimize_lbfgs(
        objective_and_gradient,
        initial,
        maximum_iterations=request["maximum_iterations"],
        ftol=1e-10,
        gtol=1e-6,
        failure="multiresolution Laplace MAP did not converge",
    )
    rotations = []
    offset = 0
    for coefficients, loadings in unpack(np.asarray(fit.x)):
        rank = coefficients.shape[1]
        for left in range(rank):
            for right in range(left + 1, rank):
                generator = np.zeros((rank, rank))
                generator[left, right], generator[right, left] = 1.0, -1.0
                direction = np.zeros_like(fit.x)
                direction[offset : offset + coefficients.size + loadings.size] = np.concatenate(
                    [(coefficients @ generator).ravel(), (loadings @ generator).ravel()]
                )
                rotations.append(direction)
        offset += coefficients.size + loadings.size
    covariance = inverse_hessian_covariance(
        fit,
        "multiresolution posterior curvature is unavailable",
        objective_and_gradient,
        np.asarray(rotations).T if rotations else None,
    )
    prediction = np.asarray(reconstruction(jnp.asarray(fit.x)), dtype=float)
    fitted_maps = [np.asarray(value, dtype=float) for value in scale_maps(jnp.asarray(fit.x))]
    variances = np.asarray([np.mean(value * value) for value in fitted_maps], dtype=float)
    variance_total = float(variances.sum())
    if variance_total <= 0.0:
        raise ContractError("multiresolution fit has no scale contribution")
    fitted_parts = unpack(np.asarray(fit.x))
    scale_contributions = []
    for scale_index, (scale, (coefficients, loadings), variance) in enumerate(
        zip(request["scales"], fitted_parts, variances, strict=True)
    ):
        coefficients = np.asarray(coefficients, dtype=float).copy()
        loadings = np.asarray(loadings, dtype=float).copy()
        factor_energy = np.var(bases[scale_index] @ coefficients, axis=0) * np.sum(
            loadings * loadings, axis=0
        )
        order = np.argsort(-factor_energy, kind="stable")
        coefficients = coefficients[:, order]
        loadings = loadings[:, order]
        align_factor_signs(loadings, coefficients)
        scale_contributions.append(
            {
                "scale_id": scale["scale_id"],
                "physical_scale_um": scale["physical_scale_um"],
                "variance_contribution": float(variance),
                "variance_fraction": float(variance / variance_total),
                "basis_coefficients_map": coefficients.tolist(),
                "feature_loadings_map": loadings.tolist(),
            }
        )
    masked_predictions = []
    errors = []
    for row in range(n):
        for feature in range(p):
            if observed[row, feature]:
                continue
            gradient = entry_gradient(reconstruction, jnp.asarray(fit.x), (row, feature))
            variance_standardized = delta_method_variance(
                gradient, covariance, request["noise_standard_deviation"] ** 2
            )
            prediction_value = prediction[row, feature] * scales[feature] + means[feature]
            target = values[row, feature]
            errors.append((prediction_value - target) ** 2)
            masked_predictions.append(
                {
                    "entity_id": rows[row]["entity_id"],
                    "feature_name": request["feature_names"][feature],
                    "posterior_mean": float(prediction_value),
                    "posterior_standard_deviation": math.sqrt(variance_standardized)
                    * scales[feature],
                    "evaluation_target": float(target),
                    "evaluation_role": "masked_before_fit",
                }
            )
    result = {
        "format": "marklab.multiresolution_spatial_factor_model",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(request_bytes),
        "matrix_id": request["matrix_id"],
        "entity_level": request["entity_level"],
        "standardization": {
            "fit_entries": "observed_only",
            "means": means.tolist(),
            "scales": scales.tolist(),
        },
        "scale_contributions": scale_contributions,
        "alignment": {"order": "descending_within_scale_energy", "sign": "max_loading_positive"},
        "masked_predictions": masked_predictions,
        "masked_rmse": math.sqrt(sum(errors) / len(errors)),
        "diagnostics": {
            "objective_initial": initial_objective,
            "objective_final": float(fit.fun),
            "iterations": int(fit.nit),
            "gradient_max_absolute": float(np.max(np.abs(fit.jac))),
            "covariance": "posterior_curvature_identified_subspace",
        },
        "fit_state": "approximate_only",
        "claim_status": "experimental_synthetic_multiresolution_spatial_factors",
    }
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "multiresolution factor worker contract error", errors=CONTRACT_ERRORS)
