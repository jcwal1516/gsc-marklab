#!/usr/bin/env python3
"""Bayesian CP or Tucker factorization of a partially observed three-mode tensor."""

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
    delta_method_variance,
    entry_gradient,
    inverse_hessian_covariance,
    jax_objective,
    minimize_lbfgs,
)
from marklab_worker_support.protocol import (
    CONTRACT_ERRORS,
    MIB,
    exact,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    write_result,
)


def mode_singular_vectors(tensor, mode, rank):
    unfolded = np.moveaxis(tensor, mode, 0).reshape(tensor.shape[mode], -1)
    return np.linalg.svd(unfolded, full_matrices=False)[0][:, :rank]


def initial_decomposition(training, values, observed, decomposition, ranks):
    """Initialize factors from mode-wise leading singular vectors of the zero-filled tensor.

    CP components are rescaled by their least-squares amplitude on the observed entries;
    the Tucker core is the projection of the zero-filled tensor onto the factor bases.
    Returns the factor shapes, the flattened initial parameter blocks, and the core shape
    (``None`` for CP).
    """
    if decomposition == "cp":
        rank = ranks[0]
        factors = [mode_singular_vectors(training, mode, rank) for mode in range(3)]
        for component in range(rank):
            component_basis = np.einsum(
                "i,j,k->ijk",
                factors[0][:, component],
                factors[1][:, component],
                factors[2][:, component],
            )
            denominator = np.sum(component_basis[observed] ** 2)
            projection = np.sum(values[observed] * component_basis[observed])
            factors[0][:, component] *= projection / max(denominator, 1e-12)
        factor_shapes = [factor.shape for factor in factors]
        initial_parts = [factor.ravel() for factor in factors]
        return factor_shapes, initial_parts, None
    factors = [mode_singular_vectors(training, mode, ranks[mode]) for mode in range(3)]
    core = np.einsum("ia,jb,kc,ijk->abc", factors[0], factors[1], factors[2], training)
    factor_shapes = [factor.shape for factor in factors]
    initial_parts = [factor.ravel() for factor in factors]
    initial_parts.append(core.ravel())
    return factor_shapes, initial_parts, tuple(ranks)


def main() -> None:
    require_versions(jax=jax.__version__, numpy=np.__version__, scipy=scipy.__version__)
    jax.config.update("jax_enable_x64", True)
    request_bytes = read_request(16 * MIB)
    request = json.loads(request_bytes)
    exact(request["format"], "marklab.jax_scipy_tensor_factor_request", "request.format")
    exact(request["version"], 1, "request.version")
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
    factor_shapes, initial_parts, core_shape = initial_decomposition(
        training, values, observed, decomposition, ranks
    )
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
        """Negative log posterior (up to a constant) of the CP or Tucker factorization.

        Observed entries ``y_ijk ~ Normal(f_ijk, sigma^2)`` where ``f`` is the CP sum of rank-one
        terms ``sum_r a_ir b_jr c_kr`` or the Tucker product ``G x1 A x2 B x3 C``; every factor
        (and Tucker core) entry has a ``Normal(0, 1 / prior_precision)`` prior.
        """
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

    objective_and_gradient = jax_objective(objective)
    initial_objective, fit = minimize_lbfgs(
        objective_and_gradient,
        initial,
        maximum_iterations=request["maximum_iterations"],
        ftol=1e-10,
        gtol=1e-6,
        failure="tensor Laplace MAP did not converge",
    )
    rotations = []
    if decomposition == "tucker":
        fitted_factors, fitted_core = unpack(np.asarray(fit.x))
        offset = 0
        for mode, factor in enumerate(fitted_factors):
            rank = factor.shape[1]
            for left in range(rank):
                for right in range(left + 1, rank):
                    generator = np.zeros((rank, rank))
                    generator[left, right], generator[right, left] = 1.0, -1.0
                    direction = np.zeros_like(fit.x)
                    direction[offset : offset + factor.size] = (factor @ generator).ravel()
                    core_direction = np.moveaxis(
                        -np.tensordot(generator, fitted_core, axes=(1, mode)), 0, mode
                    )
                    direction[-fitted_core.size :] = core_direction.ravel()
                    rotations.append(direction)
            offset += factor.size
    covariance = inverse_hessian_covariance(
        fit,
        "tensor posterior curvature is unavailable",
        objective_and_gradient,
        np.asarray(rotations).T if rotations else None,
    )
    prediction = np.asarray(reconstruction(jnp.asarray(fit.x)), dtype=float)
    masked_predictions = []
    errors = []
    for index in np.ndindex(shape):
        if observed[index]:
            continue
        gradient = entry_gradient(reconstruction, jnp.asarray(fit.x), index)
        variance = delta_method_variance(
            gradient, covariance, request["noise_standard_deviation"] ** 2
        )
        errors.append((prediction[index] - values[index]) ** 2)
        masked_predictions.append(
            {
                "indices": list(index),
                "posterior_mean": float(prediction[index]),
                "posterior_standard_deviation": math.sqrt(variance),
                "evaluation_target": float(values[index]),
                "evaluation_role": "masked_before_fit",
            }
        )
    fitted_factors, fitted_core = unpack(np.asarray(fit.x))
    fitted_factors = [np.asarray(factor, dtype=float).copy() for factor in fitted_factors]
    alignment = {"sign": "max_mode_loading_positive"}
    if decomposition == "cp":
        component_energy = np.asarray(
            [
                np.prod([np.sum(factor[:, component] ** 2) for factor in fitted_factors])
                for component in range(ranks[0])
            ]
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
        extra = {
            "core_map": fitted_core.tolist(),
            "factor_matrices": [factor.tolist() for factor in fitted_factors],
        }
        output_format = "marklab.bayesian_tucker_factorization"
    result = {
        "format": output_format,
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(request_bytes),
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
            "covariance": "posterior_curvature_identified_subspace",
        },
        "fit_state": "approximate_only",
        "claim_status": f"experimental_synthetic_bayesian_{decomposition}_factorization",
    }
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "tensor factor worker contract error", errors=CONTRACT_ERRORS)
