#!/usr/bin/env python3

import hashlib
import json
import sys

import jax
import jax.numpy as jnp
import numpy as np
import scipy
from jax.scipy.ndimage import map_coordinates
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
        raise ContractError("SVF backend version drift")
    jax.config.update("jax_enable_x64", True)
    request_bytes = sys.stdin.buffer.read(32 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.jax_svf_registration_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    fixed = np.asarray(request["fixed"]["pixels"], dtype=float)
    moving = np.asarray(request["moving"]["pixels"], dtype=float)
    height, width = fixed.shape
    yy, xx = np.meshgrid(np.arange(height, dtype=float), np.arange(width, dtype=float), indexing="ij")
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
            [sample_scalar(field[..., component], coordinates) for component in range(2)], axis=-1
        )

    def compose_displacements(left, right):
        return right + sample_vector(left, grid + right)

    def exponentiate(velocity):
        displacement = velocity / (2**squaring_steps)
        for _ in range(squaring_steps):
            displacement = compose_displacements(displacement, displacement)
        return grid + displacement

    fixed_mass = fixed.sum()
    moving_mass = moving.sum()
    fixed_center = np.asarray([(fixed * yy).sum() / fixed_mass, (fixed * xx).sum() / fixed_mass])
    moving_center = np.asarray([(moving * yy).sum() / moving_mass, (moving * xx).sum() / moving_mass])
    translation = moving_center - fixed_center
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

    value_and_gradient = jax.jit(jax.value_and_grad(objective))

    def scipy_objective(parameters):
        value, gradient = value_and_gradient(jnp.asarray(parameters))
        return float(value), np.asarray(gradient, dtype=float)

    initial = initial_velocity.ravel()
    initial_objective = scipy_objective(initial)[0]
    fit = minimize(
        scipy_objective,
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
    result = {
        "format": "marklab.svf_diffeomorphic_registration",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
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
            "nonpositive_jacobian_fraction": float(np.mean(jacobian <= request["jacobian_tolerance"])),
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
        "fit_state": "complete" if jacobian.min() > request["jacobian_tolerance"] and mse_after < mse_before else "nonconverged",
        "claim_status": "experimental_synthetic_svf_diffeomorphism",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError, np.linalg.LinAlgError) as error:
        print(f"SVF registration worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
