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


def mode_singular_vectors(tensor, mode, rank):
    unfolded = np.moveaxis(tensor, mode, 0).reshape(tensor.shape[mode], -1)
    return np.linalg.svd(unfolded, full_matrices=False)[0][:, :rank]


def main():
    if (
        jax.__version__ != "0.11.1"
        or np.__version__ != "2.4.6"
        or scipy.__version__ != "1.18.1"
        or sys.version_info[:2] != (3, 12)
    ):
        raise ContractError("tensor factor backend version drift")
    jax.config.update("jax_enable_x64", True)
    request_bytes = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.jax_scipy_tensor_factor_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    shape = tuple(request["shape"])
    values = np.empty(shape, dtype=float)
    observed = np.empty(shape, dtype=bool)
    for entry in request["entries"]:
        index = tuple(entry["indices"])
        values[index] = entry["value"]
        observed[index] = entry["observed"]
    training = np.where(observed, values, 0.0)
    decomposition = request["decomposition"]
    ranks = request["ranks"]
    factor_shapes = []
    initial_parts = []
    if decomposition == "cp":
        rank = ranks[0]
        factors = [mode_singular_vectors(training, mode, rank) for mode in range(3)]
        basis = np.einsum("ir,jr,kr->ijk", *factors)
        for component in range(rank):
            component_basis = np.einsum(
                "i,j,k->ijk", factors[0][:, component], factors[1][:, component], factors[2][:, component]
            )
            denominator = np.sum(component_basis[observed] ** 2)
            amplitude = np.sum(values[observed] * component_basis[observed]) / max(denominator, 1e-12)
            factors[0][:, component] *= amplitude
        for factor in factors:
            factor_shapes.append(factor.shape)
            initial_parts.append(factor.ravel())
        core_shape = None
    else:
        factors = [mode_singular_vectors(training, mode, ranks[mode]) for mode in range(3)]
        core = np.einsum("ia,jb,kc,ijk->abc", factors[0], factors[1], factors[2], training)
        factor_shapes = [factor.shape for factor in factors]
        initial_parts = [factor.ravel() for factor in factors]
        core_shape = tuple(ranks)
        initial_parts.append(core.ravel())
    initial = np.concatenate(initial_parts)
    observed_jax = jnp.asarray(observed)
    values_jax = jnp.asarray(values)

    def unpack(parameters):
        offset = 0
        unpacked = []
        for factor_shape in factor_shapes:
            size = int(np.prod(factor_shape))
            unpacked.append(parameters[offset : offset + size].reshape(factor_shape))
            offset += size
        if core_shape is None:
            return unpacked, None
        size = int(np.prod(core_shape))
        return unpacked, parameters[offset : offset + size].reshape(core_shape)

    def reconstruction(parameters):
        current_factors, current_core = unpack(parameters)
        if decomposition == "cp":
            return jnp.einsum("ir,jr,kr->ijk", *current_factors)
        return jnp.einsum("abc,ia,jb,kc->ijk", current_core, *current_factors)

    def objective(parameters):
        prediction = reconstruction(parameters)
        residual = jnp.where(observed_jax, prediction - values_jax, 0.0)
        penalty = sum(jnp.sum(factor * factor) for factor in unpack(parameters)[0])
        current_core = unpack(parameters)[1]
        if current_core is not None:
            penalty += jnp.sum(current_core * current_core)
        return (
            0.5 * jnp.sum(residual * residual) / request["noise_standard_deviation"] ** 2
            + 0.5 * request["prior_precision"] * penalty
        )

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
        raise ContractError(f"tensor Laplace MAP did not converge: {fit.message}")
    covariance = np.asarray(fit.hess_inv.todense(), dtype=float)
    if covariance.shape != (len(fit.x), len(fit.x)) or not np.isfinite(covariance).all():
        raise ContractError("tensor inverse-Hessian approximation is unavailable")
    prediction = np.asarray(reconstruction(jnp.asarray(fit.x)), dtype=float)
    masked_predictions = []
    errors = []
    for index in np.ndindex(shape):
        if observed[index]:
            continue
        entry_gradient = np.asarray(
            jax.grad(lambda parameters: reconstruction(parameters)[index])(jnp.asarray(fit.x)), dtype=float
        )
        variance = max(
            float(entry_gradient @ covariance @ entry_gradient)
            + request["noise_standard_deviation"] ** 2,
            1e-12,
        )
        errors.append((prediction[index] - values[index]) ** 2)
        masked_predictions.append({
            "indices": list(index),
            "posterior_mean": float(prediction[index]),
            "posterior_standard_deviation": math.sqrt(variance),
            "evaluation_target": float(values[index]),
            "evaluation_role": "masked_before_fit",
        })
    fitted_factors, fitted_core = unpack(np.asarray(fit.x))
    fitted_factors = [np.asarray(factor, dtype=float).copy() for factor in fitted_factors]
    alignment = {"sign": "max_mode_loading_positive"}
    if decomposition == "cp":
        component_energy = np.asarray(
            [np.prod([np.sum(factor[:, component] ** 2) for factor in fitted_factors]) for component in range(ranks[0])]
        )
        order = np.argsort(-component_energy, kind="stable")
        fitted_factors = [factor[:, order] for factor in fitted_factors]
        component_energy = component_energy[order]
        for component in range(ranks[0]):
            for mode in (0, 1):
                pivot = int(np.argmax(np.abs(fitted_factors[mode][:, component])))
                if fitted_factors[mode][pivot, component] < 0.0:
                    fitted_factors[mode][:, component] *= -1.0
                    fitted_factors[2][:, component] *= -1.0
        alignment["order"] = "descending_component_energy"
        factor_payload = [factor.tolist() for factor in fitted_factors]
        extra = {"component_energy": component_energy.tolist(), "factor_matrices": factor_payload}
        output_format = "marklab.bayesian_cp_factorization"
    else:
        fitted_core = np.asarray(fitted_core, dtype=float).copy()
        for mode, factor in enumerate(fitted_factors):
            for component in range(factor.shape[1]):
                pivot = int(np.argmax(np.abs(factor[:, component])))
                if factor[pivot, component] < 0.0:
                    factor[:, component] *= -1.0
                    slices = [slice(None)] * 3
                    slices[mode] = component
                    fitted_core[tuple(slices)] *= -1.0
        alignment["order"] = "declared_tucker_ranks"
        extra = {"core_map": fitted_core.tolist(), "factor_matrices": [factor.tolist() for factor in fitted_factors]}
        output_format = "marklab.bayesian_tucker_factorization"
    result = {
        "format": output_format,
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "tensor_id": request["tensor_id"],
        "mode_names": request["mode_names"],
        "shape": request["shape"],
        "decomposition": decomposition,
        "ranks": ranks,
        **extra,
        "alignment": alignment,
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
        "claim_status": f"experimental_synthetic_bayesian_{decomposition}_factorization",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError, np.linalg.LinAlgError) as error:
        print(f"tensor factor worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
