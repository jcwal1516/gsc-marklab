#!/usr/bin/env python3

import hashlib
import json
import math
import sys

import jax
import jax.numpy as jnp
import numpy as np
import scipy
from scipy.optimize import minimize


class ContractError(Exception):
    pass


def main():
    if (
        jax.__version__ != "0.11.1"
        or np.__version__ != "2.4.6"
        or scipy.__version__ != "1.18.1"
        or sys.version_info[:2] != (3, 12)
    ):
        raise ContractError("multiresolution backend version drift")
    jax.config.update("jax_enable_x64", True)
    request_bytes = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.jax_multiresolution_factor_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    rows = request["rows"]
    values = np.asarray([row["values"] for row in rows], dtype=float)
    observed = np.asarray([row["observed"] for row in rows], dtype=bool)
    n, p = values.shape
    means = np.asarray([values[:, feature][observed[:, feature]].mean() for feature in range(p)])
    scales = np.asarray([values[:, feature][observed[:, feature]].std(ddof=0) for feature in range(p)])
    if not np.isfinite(scales).all() or np.any(scales <= 0.0):
        raise ContractError("every multiresolution feature must vary")
    standardized = (values - means) / scales
    training = np.where(observed, standardized, 0.0)
    bases = [np.asarray(scale["basis_columns"], dtype=float).T for scale in request["scales"]]
    coefficient_shapes = []
    loading_shapes = []
    initial_parts = []
    residual = training.copy()
    for scale, basis in zip(request["scales"], bases):
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
        for coefficient_shape, loading_shape in zip(coefficient_shapes, loading_shapes):
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
            for basis, (coefficients, loadings) in zip(bases_jax, unpack(parameters))
        ]

    def reconstruction(parameters):
        maps = scale_maps(parameters)
        return sum(maps[1:], maps[0])

    def objective(parameters):
        prediction = reconstruction(parameters)
        residuals = jnp.where(observed_jax, prediction - standardized_jax, 0.0)
        penalty = 0.0
        for scale, (coefficients, loadings) in zip(request["scales"], unpack(parameters)):
            penalty += scale["coefficient_precision"] * jnp.sum(coefficients * coefficients)
            penalty += request["loading_precision"] * jnp.sum(loadings * loadings)
        return 0.5 * jnp.sum(residuals * residuals) / request["noise_standard_deviation"] ** 2 + 0.5 * penalty

    value_and_gradient = jax.jit(jax.value_and_grad(objective))

    def scipy_objective(parameters):
        value, gradient = value_and_gradient(jnp.asarray(parameters))
        return float(value), np.asarray(gradient, dtype=float)

    initial_objective = scipy_objective(initial)[0]
    fit = minimize(
        scipy_objective,
        initial,
        jac=True,
        method="L-BFGS-B",
        options={"maxiter": request["maximum_iterations"], "ftol": 1e-10, "gtol": 1e-6},
    )
    if not fit.success or not np.isfinite(fit.x).all():
        raise ContractError(f"multiresolution Laplace MAP did not converge: {fit.message}")
    covariance = np.asarray(fit.hess_inv.todense(), dtype=float)
    if covariance.shape != (len(fit.x), len(fit.x)) or not np.isfinite(covariance).all():
        raise ContractError("multiresolution inverse-Hessian approximation is unavailable")
    prediction = np.asarray(reconstruction(jnp.asarray(fit.x)), dtype=float)
    fitted_maps = [np.asarray(value, dtype=float) for value in scale_maps(jnp.asarray(fit.x))]
    variances = np.asarray([np.mean(value * value) for value in fitted_maps], dtype=float)
    variance_total = float(variances.sum())
    if variance_total <= 0.0:
        raise ContractError("multiresolution fit has no scale contribution")
    fitted_parts = unpack(np.asarray(fit.x))
    scale_contributions = []
    for scale_index, (scale, (coefficients, loadings), variance) in enumerate(
        zip(request["scales"], fitted_parts, variances)
    ):
        coefficients = np.asarray(coefficients, dtype=float).copy()
        loadings = np.asarray(loadings, dtype=float).copy()
        factor_energy = np.var(bases[scale_index] @ coefficients, axis=0) * np.sum(loadings * loadings, axis=0)
        order = np.argsort(-factor_energy, kind="stable")
        coefficients = coefficients[:, order]
        loadings = loadings[:, order]
        for factor in range(loadings.shape[1]):
            pivot = int(np.argmax(np.abs(loadings[:, factor])))
            if loadings[pivot, factor] < 0.0:
                loadings[:, factor] *= -1.0
                coefficients[:, factor] *= -1.0
        scale_contributions.append({
            "scale_id": scale["scale_id"],
            "physical_scale_um": scale["physical_scale_um"],
            "variance_contribution": float(variance),
            "variance_fraction": float(variance / variance_total),
            "basis_coefficients_map": coefficients.tolist(),
            "feature_loadings_map": loadings.tolist(),
        })
    masked_predictions = []
    errors = []
    for row in range(n):
        for feature in range(p):
            if observed[row, feature]:
                continue
            entry_gradient = np.asarray(
                jax.grad(lambda parameters: reconstruction(parameters)[row, feature])(jnp.asarray(fit.x)),
                dtype=float,
            )
            variance_standardized = max(
                float(entry_gradient @ covariance @ entry_gradient)
                + request["noise_standard_deviation"] ** 2,
                1e-12,
            )
            prediction_value = prediction[row, feature] * scales[feature] + means[feature]
            target = values[row, feature]
            errors.append((prediction_value - target) ** 2)
            masked_predictions.append({
                "entity_id": rows[row]["entity_id"],
                "feature_name": request["feature_names"][feature],
                "posterior_mean": float(prediction_value),
                "posterior_standard_deviation": math.sqrt(variance_standardized) * scales[feature],
                "evaluation_target": float(target),
                "evaluation_role": "masked_before_fit",
            })
    result = {
        "format": "marklab.multiresolution_spatial_factor_model",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "matrix_id": request["matrix_id"],
        "entity_level": request["entity_level"],
        "standardization": {"fit_entries": "observed_only", "means": means.tolist(), "scales": scales.tolist()},
        "scale_contributions": scale_contributions,
        "alignment": {"order": "descending_within_scale_energy", "sign": "max_loading_positive"},
        "masked_predictions": masked_predictions,
        "masked_rmse": math.sqrt(sum(errors) / len(errors)),
        "diagnostics": {
            "objective_initial": initial_objective,
            "objective_final": float(fit.fun),
            "iterations": int(fit.nit),
            "gradient_max_absolute": float(np.max(np.abs(fit.jac))),
            "covariance": "limited_memory_inverse_hessian",
        },
        "fit_state": "approximate_only",
        "claim_status": "experimental_synthetic_multiresolution_spatial_factors",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError, np.linalg.LinAlgError) as error:
        print(f"multiresolution factor worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
