#!/usr/bin/env python3
"""JAX worker for variational Bayesian translation-SVF registration of two images."""

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

Z_975 = 1.959963984540054


def register(request, request_sha256):
    """Fit a Gaussian variational posterior over a constant translation velocity.

    Model: translation ``t ~ Normal(0, prior_sd^2 I)`` (pixels) and fixed image
    ``F ~ Normal(M(x + t), noise_sd^2 I)`` with bilinear warping of the moving image ``M``.
    The mean-field Gaussian ``q(t)`` is fitted by maximizing a reparameterized ELBO over a
    fixed set of antithetic standard-normal draws.
    """
    fixed = np.asarray(request["fixed"]["pixels"], dtype=float)
    moving = np.asarray(request["moving"]["pixels"], dtype=float)
    height, width = fixed.shape
    yy, xx = pixel_grid(height, width)
    known_translation_yx = center_of_mass_translation(fixed, moving, yy, xx)
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
            log_prior = -0.5 * jnp.sum(translation * translation) / prior_sd**2 - 2.0 * jnp.log(
                prior_sd
            )
            log_q = -0.5 * jnp.sum(draw_epsilon * draw_epsilon) - jnp.sum(log_sd)
            return log_likelihood + log_prior - log_q

        return jnp.mean(jax.vmap(draw_term)(translations, epsilon))

    def negative_elbo(parameters):
        return -elbo(parameters)

    fit_objective = scipy_objective(jax.jit(jax.value_and_grad(negative_elbo)))

    initial = np.concatenate([known_translation_yx, np.log([0.5, 0.5])])
    initial_elbo = float(elbo(jnp.asarray(initial)))
    fit = minimize(
        fit_objective,
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
    lower_yx = mean_yx - Z_975 * sd_yx
    upper_yx = mean_yx + Z_975 * sd_yx
    calibrated = bool(
        np.all((known_translation_yx >= lower_yx) & (known_translation_yx <= upper_yx))
    )
    draw_payload = [
        {
            "draw": index,
            "velocity_xy_pixels": [float(draw[1]), float(draw[0])],
            "transform": "translation_exp_constant_svf",
            "minimum_jacobian_determinant": 1.0,
        }
        for index, draw in enumerate(draws_yx)
    ]
    return {
        "format": "marklab.probabilistic_diffeomorphic_registration",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": request_sha256,
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
            "known_translation_xy_pixels": [
                float(known_translation_yx[1]),
                float(known_translation_yx[0]),
            ],
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


def main():
    require_versions(jax=jax.__version__, numpy=np.__version__, scipy=scipy.__version__)
    jax.config.update("jax_enable_x64", True)
    request_bytes = read_request(32 * MIB)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.jax_probabilistic_svf_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    write_result(register(request, sha256_hex(request_bytes)))


if __name__ == "__main__":
    run_worker(main, "probabilistic SVF worker contract error", errors=CONTRACT_ERRORS)
