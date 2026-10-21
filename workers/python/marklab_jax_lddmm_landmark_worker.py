#!/usr/bin/env python3
"""JAX worker for Gaussian-kernel LDDMM landmark registration by geodesic shooting."""

import json
import sys
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parent))

import jax
import jax.numpy as jnp
import numpy as np
import scipy
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
from marklab_worker_support.registration import scipy_objective


def register(request, request_sha256):
    """Fit the initial momentum of a landmark geodesic and report the deformation.

    Landmarks ``q`` with momenta ``p`` follow Hamiltonian geodesics of the Gaussian kernel
    ``K(x, y) = exp(-|x - y|^2 / (2 s^2))``, integrated by fixed-step RK4 over unit time. The
    objective is the kinetic energy ``0.5 * sum_ij (p_i . p_j) K(q_i, q_j)`` at the source plus
    ``data_weight`` times the mean squared endpoint landmark error, minimized by L-BFGS-B from
    the kernel-smoothed mean displacement.
    """
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
        (_, _), trajectory = jax.lax.scan(
            step, (source, initial_momentum), xs=None, length=time_steps
        )
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
    initial_momentum = np.linalg.solve(
        kernel + 1e-6 * np.eye(count), np.broadcast_to(displacement, (count, 2))
    )
    fit_objective = scipy_objective(jax.jit(jax.value_and_grad(objective)))

    initial = initial_momentum.ravel()
    initial_objective = fit_objective(initial)[0]
    fit = minimize(
        fit_objective,
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
    momenta = np.concatenate(
        [np.asarray(fitted_momentum)[None, ...], np.asarray(momenta_tail)], axis=0
    )
    energies = np.asarray(
        [
            float(kinetic_energy(jnp.asarray(q), jnp.asarray(p)))
            for q, p in zip(positions, momenta, strict=True)
        ]
    )
    energy_drift = np.max(np.abs(energies - energies[0])) / max(abs(energies[0]), 1e-12)
    residuals = positions[-1] - np.asarray(target)
    landmark_errors = np.sqrt(np.sum(residuals * residuals, axis=1))
    return {
        "format": "marklab.lddmm_landmark_registration",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": request_sha256,
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
            "maximum_relative_energy_drift": float(energy_drift),
        },
        "transformed_source_landmarks": positions[-1].tolist(),
        "quality": {
            "landmark_rmse_um": float(np.sqrt(np.mean(np.sum(residuals * residuals, axis=1)))),
            "maximum_landmark_error_um": float(landmark_errors.max()),
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


def main():
    require_versions(jax=jax.__version__, numpy=np.__version__, scipy=scipy.__version__)
    jax.config.update("jax_enable_x64", True)
    request_bytes = read_request(4 * MIB)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.jax_lddmm_landmark_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    write_result(register(request, sha256_hex(request_bytes)))


if __name__ == "__main__":
    run_worker(main, "LDDMM landmark worker contract error", errors=CONTRACT_ERRORS)
