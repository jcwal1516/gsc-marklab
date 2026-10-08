#!/usr/bin/env python3

import hashlib
import json
import math
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
        raise ContractError("probabilistic SVF backend version drift")
    jax.config.update("jax_enable_x64", True)
    request_bytes = sys.stdin.buffer.read(32 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.jax_probabilistic_svf_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    fixed = np.asarray(request["fixed"]["pixels"], dtype=float)
    moving = np.asarray(request["moving"]["pixels"], dtype=float)
    height, width = fixed.shape
    yy, xx = np.meshgrid(np.arange(height, dtype=float), np.arange(width, dtype=float), indexing="ij")
    fixed_mass = fixed.sum()
    moving_mass = moving.sum()
    fixed_center = np.asarray([(fixed * yy).sum() / fixed_mass, (fixed * xx).sum() / fixed_mass])
    moving_center = np.asarray([(moving * yy).sum() / moving_mass, (moving * xx).sum() / moving_mass])
    known_translation_yx = moving_center - fixed_center
    grid_y = jnp.asarray(yy)
    grid_x = jnp.asarray(xx)
    fixed_jax = jnp.asarray(fixed)
    moving_jax = jnp.asarray(moving)
    half = request["variational_samples"] // 2
    random = np.random.default_rng(request["seed"])
    epsilon_half = random.normal(size=(half, 2))
    epsilon = jnp.asarray(np.concatenate([epsilon_half, -epsilon_half], axis=0))
    prior_sd = request["prior_standard_deviation_pixels"]
    noise_sd = request["likelihood_noise_standard_deviation"]

    def warp(translation_yx):
        return map_coordinates(
            moving_jax,
            [grid_y + translation_yx[0], grid_x + translation_yx[1]],
            order=1,
            mode="nearest",
        )

    def elbo(parameters):
        mean = parameters[:2]
        log_sd = parameters[2:]
        translations = mean[None, :] + jnp.exp(log_sd)[None, :] * epsilon

        def draw_term(translation, draw_epsilon):
            residual = fixed_jax - warp(translation)
            log_likelihood = -0.5 * jnp.sum(residual * residual) / noise_sd**2
            log_prior = -0.5 * jnp.sum(translation * translation) / prior_sd**2 - 2.0 * jnp.log(prior_sd)
            log_q = -0.5 * jnp.sum(draw_epsilon * draw_epsilon) - jnp.sum(log_sd)
            return log_likelihood + log_prior - log_q

        return jnp.mean(jax.vmap(draw_term)(translations, epsilon))

    value_and_gradient = jax.jit(jax.value_and_grad(lambda parameters: -elbo(parameters)))

    def scipy_objective(parameters):
        value, gradient = value_and_gradient(jnp.asarray(parameters))
        return float(value), np.asarray(gradient, dtype=float)

    initial = np.concatenate([known_translation_yx, np.log([0.5, 0.5])])
    initial_elbo = float(elbo(jnp.asarray(initial)))
    fit = minimize(
        scipy_objective,
        initial,
        jac=True,
        method="L-BFGS-B",
        bounds=[(None, None), (None, None), (-8.0, 2.0), (-8.0, 2.0)],
        options={"maxiter": request["maximum_iterations"], "ftol": 1e-12, "gtol": 1e-7},
    )
    if not fit.success or not np.isfinite(fit.x).all():
        raise ContractError(f"probabilistic SVF optimization did not converge: {fit.message}")
    mean_yx = fit.x[:2]
    sd_yx = np.exp(fit.x[2:])
    posterior_random = np.random.default_rng(request["seed"] + 1)
    draws_yx = posterior_random.normal(mean_yx, sd_yx, size=(request["posterior_draws"], 2))
    warped_mean = np.asarray(warp(jnp.asarray(mean_yx)), dtype=float)
    mse_before = float(np.mean((fixed - moving) ** 2))
    mse_after = float(np.mean((fixed - warped_mean) ** 2))
    lower_yx = mean_yx - 1.959963984540054 * sd_yx
    upper_yx = mean_yx + 1.959963984540054 * sd_yx
    calibrated = bool(np.all((known_translation_yx >= lower_yx) & (known_translation_yx <= upper_yx)))
    draw_payload = [
        {
            "draw": index,
            "velocity_xy_pixels": [float(draw[1]), float(draw[0])],
            "transform": "translation_exp_constant_svf",
            "minimum_jacobian_determinant": 1.0,
        }
        for index, draw in enumerate(draws_yx)
    ]
    result = {
        "format": "marklab.probabilistic_diffeomorphic_registration",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "fixed_frame": request["fixed"]["frame"],
        "moving_frame": request["moving"]["frame"],
        "velocity_family": "constant_translation_svf",
        "posterior_velocity_xy_pixels": {
            "mean": [float(mean_yx[1]), float(mean_yx[0])],
            "standard_deviation": [float(sd_yx[1]), float(sd_yx[0])],
            "lower_95": [float(lower_yx[1]), float(lower_yx[0])],
            "upper_95": [float(upper_yx[1]), float(upper_yx[0])],
        },
        "posterior_transform_draws": draw_payload,
        "warped_posterior_mean_pixels": warped_mean.tolist(),
        "quality": {
            "mse_before": mse_before,
            "mse_posterior_mean": mse_after,
            "minimum_sample_jacobian_determinant": 1.0,
            "nonpositive_sample_jacobian_count": 0,
        },
        "calibration": {
            "known_translation_xy_pixels": [float(known_translation_yx[1]), float(known_translation_yx[0])],
            "known_translation_inside_95_percent_interval": calibrated,
            "oracle": "center_of_mass_translation_for_synthetic_same_shape_images",
        },
        "diagnostics": {
            "objective": "fixed_antithetic_reparameterized_elbo",
            "elbo_initial": initial_elbo,
            "elbo_final": float(elbo(jnp.asarray(fit.x))),
            "iterations": int(fit.nit),
            "gradient_max_absolute": float(np.max(np.abs(fit.jac))),
        },
        "fit_state": "approximate_only",
        "claim_status": "experimental_synthetic_probabilistic_diffeomorphism",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError, np.linalg.LinAlgError) as error:
        print(f"probabilistic SVF worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
