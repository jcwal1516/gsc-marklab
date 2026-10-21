#!/usr/bin/env python3
"""JAX worker for stationary-velocity-field diffeomorphic registration of two images."""

import json
import sys
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parent))

import jax
import jax.numpy as jnp
import numpy as np
import scipy
from jax.scipy.ndimage import map_coordinates
from scipy.optimize import minimize

from marklab_worker_support.protocol import (
    CONTRACT_ERRORS,
    MIB,
    ContractError,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    write_result,
)
from marklab_worker_support.registration import (
    center_of_mass_translation,
    pixel_grid,
    scipy_objective,
)


def register(request, request_sha256):
    """Fit a stationary velocity field by penalized least squares.

    The transform is the exponential of the velocity field computed by scaling and squaring.
    The objective is the mean squared difference between the fixed image and the bilinearly
    warped moving image plus ``regularization_weight`` times the mean squared first differences
    of the velocity field, minimized by L-BFGS-B from the centre-of-mass translation.
    """
    fixed = np.asarray(request["fixed"]["pixels"], dtype=float)
    moving = np.asarray(request["moving"]["pixels"], dtype=float)
    height, width = fixed.shape
    yy, xx = pixel_grid(height, width)
    grid = jnp.asarray(np.stack([yy, xx], axis=-1))
    fixed_jax = jnp.asarray(fixed)
    moving_jax = jnp.asarray(moving)
    squaring_steps = request["squaring_steps"]

    def sample_scalar(image, coordinates):
        return map_coordinates(
            image,
            [coordinates[..., 0], coordinates[..., 1]],
            order=1,
            mode="nearest",
        )

    def sample_vector(field, coordinates):
        return jnp.stack(
            [sample_scalar(field[..., component], coordinates) for component in range(2)],
            axis=-1,
        )

    def compose_displacements(left, right):
        return right + sample_vector(left, grid + right)

    def exponentiate(velocity):
        displacement = velocity / (2**squaring_steps)
        for _ in range(squaring_steps):
            displacement = compose_displacements(displacement, displacement)
        return grid + displacement

    translation = center_of_mass_translation(fixed, moving, yy, xx)
    initial_velocity = np.broadcast_to(translation, (height, width, 2)).copy()

    def objective(parameters):
        velocity = parameters.reshape(height, width, 2)
        transform = exponentiate(velocity)
        warped = sample_scalar(moving_jax, transform)
        data_loss = jnp.mean((fixed_jax - warped) ** 2)
        dy = velocity[1:, :, :] - velocity[:-1, :, :]
        dx = velocity[:, 1:, :] - velocity[:, :-1, :]
        regularization = jnp.mean(dy * dy) + jnp.mean(dx * dx)
        return data_loss + request["regularization_weight"] * regularization

    fit_objective = scipy_objective(jax.jit(jax.value_and_grad(objective)))

    initial = initial_velocity.ravel()
    initial_objective = fit_objective(initial)[0]
    fit = minimize(
        fit_objective,
        initial,
        jac=True,
        method="L-BFGS-B",
        options={"maxiter": request["maximum_iterations"], "ftol": 1e-12, "gtol": 1e-7},
    )
    if not fit.success or not np.isfinite(fit.x).all():
        raise ContractError(f"SVF optimization did not converge: {fit.message}")
    velocity = np.asarray(fit.x.reshape(height, width, 2), dtype=float)
    transform = np.asarray(exponentiate(jnp.asarray(velocity)), dtype=float)
    inverse = np.asarray(exponentiate(jnp.asarray(-velocity)), dtype=float)
    warped = np.asarray(sample_scalar(moving_jax, jnp.asarray(transform)), dtype=float)
    forward_displacement = jnp.asarray(transform) - grid
    inverse_displacement = jnp.asarray(inverse) - grid
    inverse_after_forward = np.asarray(
        compose_displacements(inverse_displacement, forward_displacement), dtype=float
    )
    inverse_error = np.linalg.norm(inverse_after_forward, axis=-1)
    dphi_y_dy, dphi_y_dx = np.gradient(transform[..., 0])
    dphi_x_dy, dphi_x_dx = np.gradient(transform[..., 1])
    jacobian = dphi_y_dy * dphi_x_dx - dphi_y_dx * dphi_x_dy
    mse_before = float(np.mean((fixed - moving) ** 2))
    mse_after = float(np.mean((fixed - warped) ** 2))
    velocity_xy = velocity[..., [1, 0]]
    transform_xy = transform[..., [1, 0]]
    inverse_xy = inverse[..., [1, 0]]
    tolerance = request["jacobian_tolerance"]
    converged = jacobian.min() > tolerance and mse_after < mse_before
    return {
        "format": "marklab.svf_diffeomorphic_registration",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": request_sha256,
        "fixed_frame": request["fixed"]["frame"],
        "moving_frame": request["moving"]["frame"],
        "transform": {
            "type": "stationary_velocity_diffeomorphism",
            "direction": "fixed_pixel_to_moving_pixel",
            "exponentiation": "scaling_and_squaring",
            "squaring_steps": squaring_steps,
            "velocity_field_xy_pixels": velocity_xy.tolist(),
            "forward_map_xy_pixels": transform_xy.tolist(),
            "inverse_map_xy_pixels": inverse_xy.tolist(),
        },
        "warped_moving_pixels": warped.tolist(),
        "quality": {
            "mse_before": mse_before,
            "mse_after": mse_after,
            "minimum_jacobian_determinant": float(jacobian.min()),
            "maximum_jacobian_determinant": float(jacobian.max()),
            "nonpositive_jacobian_fraction": float(np.mean(jacobian <= tolerance)),
            "inverse_consistency_max_pixels": float(inverse_error.max()),
            "inverse_consistency_mean_pixels": float(inverse_error.mean()),
        },
        "diagnostics": {
            "objective_initial": initial_objective,
            "objective_final": float(fit.fun),
            "iterations": int(fit.nit),
            "gradient_max_absolute": float(np.max(np.abs(fit.jac))),
            "metric": request["metric"],
            "regularization": "first_difference_velocity_energy",
        },
        "fit_state": "complete" if converged else "nonconverged",
        "claim_status": "experimental_synthetic_svf_diffeomorphism",
    }


def main():
    require_versions(jax=jax.__version__, numpy=np.__version__, scipy=scipy.__version__)
    jax.config.update("jax_enable_x64", True)
    request_bytes = read_request(32 * MIB)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.jax_svf_registration_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    write_result(register(request, sha256_hex(request_bytes)))


if __name__ == "__main__":
    run_worker(main, "SVF registration worker contract error", errors=CONTRACT_ERRORS)
