#!/usr/bin/env python3

import hashlib
import json
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
        raise ContractError("LDDMM backend version drift")
    jax.config.update("jax_enable_x64", True)
    request_bytes = sys.stdin.buffer.read(4 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.jax_lddmm_landmark_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    source = jnp.asarray(request["source_landmarks"], dtype=jnp.float64)
    target = jnp.asarray(request["target_landmarks"], dtype=jnp.float64)
    count = source.shape[0]
    scale_squared = request["kernel_scale_um"] ** 2
    time_steps = request["time_steps"]
    dt = 1.0 / time_steps

    def derivatives(position, momentum):
        differences = position[:, None, :] - position[None, :, :]
        kernel = jnp.exp(-jnp.sum(differences * differences, axis=2) / (2.0 * scale_squared))
        position_derivative = kernel @ momentum
        momentum_dot = momentum @ momentum.T
        momentum_derivative = jnp.sum(
            momentum_dot[:, :, None] * kernel[:, :, None] * differences / scale_squared,
            axis=1,
        )
        return position_derivative, momentum_derivative

    def step(state, _):
        position, momentum = state
        k1_q, k1_p = derivatives(position, momentum)
        k2_q, k2_p = derivatives(position + 0.5 * dt * k1_q, momentum + 0.5 * dt * k1_p)
        k3_q, k3_p = derivatives(position + 0.5 * dt * k2_q, momentum + 0.5 * dt * k2_p)
        k4_q, k4_p = derivatives(position + dt * k3_q, momentum + dt * k3_p)
        next_position = position + dt * (k1_q + 2.0 * k2_q + 2.0 * k3_q + k4_q) / 6.0
        next_momentum = momentum + dt * (k1_p + 2.0 * k2_p + 2.0 * k3_p + k4_p) / 6.0
        return (next_position, next_momentum), (next_position, next_momentum)

    def shoot(initial_momentum):
        (_, _), trajectory = jax.lax.scan(step, (source, initial_momentum), xs=None, length=time_steps)
        return trajectory

    def kinetic_energy(position, momentum):
        differences = position[:, None, :] - position[None, :, :]
        kernel = jnp.exp(-jnp.sum(differences * differences, axis=2) / (2.0 * scale_squared))
        return 0.5 * jnp.sum((momentum @ momentum.T) * kernel)

    def objective(flat_momentum):
        momentum = flat_momentum.reshape(count, 2)
        positions, _ = shoot(momentum)
        data_loss = jnp.mean(jnp.sum((positions[-1] - target) ** 2, axis=1))
        return kinetic_energy(source, momentum) + request["data_weight"] * data_loss

    displacement = np.asarray(target - source).mean(axis=0)
    source_array = np.asarray(source)
    differences = source_array[:, None, :] - source_array[None, :, :]
    kernel = np.exp(-np.sum(differences * differences, axis=2) / (2.0 * scale_squared))
    initial_momentum = np.linalg.solve(kernel + 1e-6 * np.eye(count), np.broadcast_to(displacement, (count, 2)))
    value_and_gradient = jax.jit(jax.value_and_grad(objective))

    def scipy_objective(parameters):
        value, gradient = value_and_gradient(jnp.asarray(parameters))
        return float(value), np.asarray(gradient, dtype=float)

    initial = initial_momentum.ravel()
    initial_objective = scipy_objective(initial)[0]
    fit = minimize(
        scipy_objective,
        initial,
        jac=True,
        method="L-BFGS-B",
        options={"maxiter": request["maximum_iterations"], "ftol": 1e-12, "gtol": 1e-8},
    )
    if not fit.success or not np.isfinite(fit.x).all():
        raise ContractError(f"LDDMM shooting optimization did not converge: {fit.message}")
    fitted_momentum = jnp.asarray(fit.x.reshape(count, 2))
    positions_tail, momenta_tail = shoot(fitted_momentum)
    positions = np.concatenate([np.asarray(source)[None, ...], np.asarray(positions_tail)], axis=0)
    momenta = np.concatenate([np.asarray(fitted_momentum)[None, ...], np.asarray(momenta_tail)], axis=0)
    energies = np.asarray(
        [float(kinetic_energy(jnp.asarray(q), jnp.asarray(p))) for q, p in zip(positions, momenta)]
    )
    residuals = positions[-1] - np.asarray(target)
    result = {
        "format": "marklab.lddmm_landmark_registration",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "source_frame": request["source_frame"],
        "target_frame": request["target_frame"],
        "kernel": {"family": "gaussian", "scale_um": request["kernel_scale_um"]},
        "geodesic": {
            "integration": "fixed_step_rk4_hamiltonian_shooting",
            "times": np.linspace(0.0, 1.0, time_steps + 1).tolist(),
            "positions": positions.tolist(),
            "momenta": momenta.tolist(),
            "initial_momentum": np.asarray(fitted_momentum).tolist(),
            "kinetic_energy_initial": float(energies[0]),
            "kinetic_energy_final": float(energies[-1]),
            "maximum_relative_energy_drift": float(np.max(np.abs(energies - energies[0])) / max(abs(energies[0]), 1e-12)),
        },
        "transformed_source_landmarks": positions[-1].tolist(),
        "quality": {
            "landmark_rmse_um": float(np.sqrt(np.mean(np.sum(residuals * residuals, axis=1)))),
            "maximum_landmark_error_um": float(np.sqrt(np.sum(residuals * residuals, axis=1)).max()),
        },
        "diagnostics": {
            "objective_initial": initial_objective,
            "objective_final": float(fit.fun),
            "iterations": int(fit.nit),
            "gradient_max_absolute": float(np.max(np.abs(fit.jac))),
        },
        "fit_state": "complete",
        "claim_status": "experimental_synthetic_landmark_lddmm",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError, np.linalg.LinAlgError) as error:
        print(f"LDDMM landmark worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
