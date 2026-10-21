#!/usr/bin/env python3
"""SciPy worker for clone phylogeography diffusion and clone-niche contrast models."""

import json
import math
import sys
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parent))

import numpy as np
import scipy
from scipy.optimize import minimize
from scipy.special import logsumexp
from scipy.stats import invgamma

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


def fit_niche(values, probabilities, patient_indices, patient_count):
    """Fit a clone contrast, patient intercepts and common residual SD by mixture likelihood.

    The component means are ``patient_intercept +/- contrast/2``. Centering the input only
    sets the origin for the fitted intercepts; it does not constrain them. The contrast SE
    profiles all free nuisance parameters through the observed-information Schur complement.
    At the existing residual-SD floor, the SE is conditional on that active bound.
    """
    counts = np.bincount(patient_indices, minlength=patient_count)
    patient_means = np.bincount(patient_indices, weights=values, minlength=patient_count) / counts
    centered = values - patient_means[patient_indices]
    signs = np.asarray([0.5, -0.5])
    with np.errstate(divide="ignore"):
        log_probabilities = np.log(probabilities)

    def components(parameters):
        residual = centered[:, None] - parameters[2:][patient_indices, None]
        residual = residual - parameters[0] * signs
        precision = math.exp(-2.0 * parameters[1])
        log_components = (
            log_probabilities
            - 0.5 * residual**2 * precision
            - parameters[1]
            - 0.5 * math.log(2.0 * math.pi)
        )
        log_density = logsumexp(log_components, axis=1)
        weights = np.exp(log_components - log_density[:, None])
        scores = np.stack(
            [signs * residual * precision, residual**2 * precision - 1.0, residual * precision],
            axis=2,
        )
        mean_scores = np.sum(weights[:, :, None] * scores, axis=1)
        return log_density, weights, scores, mean_scores, precision

    def objective(parameters):
        log_density, _, _, scores, _ = components(parameters)
        gradient = -np.r_[
            scores[:, :2].sum(axis=0),
            np.bincount(patient_indices, weights=scores[:, 2], minlength=patient_count),
        ]
        return float(-log_density.sum()), gradient

    expected_clone = probabilities[:, 0] - 0.5
    patient_clone = np.bincount(patient_indices, weights=expected_clone) / counts
    within_clone = expected_clone - patient_clone[patient_indices]
    information = float(within_clone @ within_clone)
    initial_contrast = float(within_clone @ centered) / information if information > 0.0 else 2.0
    initial_contrast = float(np.clip(initial_contrast, -20.0, 20.0))
    initial_intercepts = -initial_contrast * patient_clone
    residual = centered - initial_contrast * within_clone
    initial_log_sd = float(np.clip(math.log(max(np.std(residual), 0.05)), -6.0, 4.0))
    initial = np.r_[initial_contrast, initial_log_sd, initial_intercepts]
    fit = minimize(
        objective,
        initial,
        jac=True,
        method="L-BFGS-B",
        bounds=[(-20.0, 20.0), (-6.0, 4.0), *[(None, None)] * patient_count],
        options={"ftol": 1e-12, "gtol": 1e-7, "maxiter": 1000},
    )
    if not fit.success:
        raise ContractError(f"clone niche fit did not converge: {fit.message}")
    if abs(fit.x[0]) >= 20.0 - 1e-8 or fit.x[1] >= 4.0 - 1e-8:
        raise ContractError("clone niche contrast or residual SD reached its bound")

    _, weights, scores, mean_scores, precision = components(fit.x)
    # For a two-component mixture, the missing-information covariance is
    # w_A*w_B*(score_A-score_B)*(score_A-score_B)^T for each observation.
    difference = scores[:, 0, :] - scores[:, 1, :]
    mixture_variance = weights[:, 0] * weights[:, 1]
    cc = len(values) * precision / 4.0 - np.sum(mixture_variance * difference[:, 0] ** 2)
    cs = 2.0 * mean_scores[:, 0].sum() - np.sum(
        mixture_variance * difference[:, 0] * difference[:, 1]
    )
    ss = 2.0 * (mean_scores[:, 1] + 1.0).sum() - np.sum(mixture_variance * difference[:, 1] ** 2)
    global_information = np.asarray([[cc, cs], [cs, ss]])
    patient_information = np.bincount(
        patient_indices,
        weights=precision - mixture_variance * difference[:, 2] ** 2,
        minlength=patient_count,
    )
    cross = np.column_stack(
        [
            np.bincount(
                patient_indices,
                weights=(weights @ signs) * precision
                - mixture_variance * difference[:, 0] * difference[:, 2],
                minlength=patient_count,
            ),
            np.bincount(
                patient_indices,
                weights=2.0 * mean_scores[:, 2]
                - mixture_variance * difference[:, 1] * difference[:, 2],
                minlength=patient_count,
            ),
        ]
    )
    if not np.isfinite(patient_information).all() or np.any(patient_information <= 0.0):
        raise ContractError("clone niche patient effects are not identifiable")
    profiled = global_information - cross.T @ (cross / patient_information[:, None])
    if fit.x[1] <= -6.0 + 1e-8:
        profiled = profiled[:1, :1]
    tolerance = 64.0 * np.finfo(float).eps * np.max(np.abs(global_information))
    if not np.isfinite(profiled).all() or np.linalg.eigvalsh(profiled).min() <= tolerance:
        raise ContractError("clone niche contrast is not identifiable")
    standard_error = math.sqrt(float(np.linalg.inv(profiled)[0, 0]))
    return float(fit.x[0]), standard_error, patient_means.tolist(), float(fit.fun)


def diffusion_posterior(request, location_by_clone, rng):
    """Draw the clone-tree Brownian diffusion coefficient, integrating location uncertainty.

    Clone locations are drawn from their Gaussian posteriors. Given one draw of all locations,
    each branch displacement is ``Normal(0, 2 D length I_2)``, so with an
    ``InverseGamma(shape, scale)`` prior on ``D`` the conditional posterior is
    ``InverseGamma(shape + branches * d / 2, scale + sum(|displacement|^2 / length) / 4)``,
    from which one ``D`` is drawn per location draw.
    """
    tree = request["tree"]
    location_draws = {}
    for clone_id, location in location_by_clone.items():
        location_draws[clone_id] = rng.multivariate_normal(
            location["mean_xy_um"],
            location["covariance_um2"],
            size=request["posterior_draws"],
        )
    diffusion_draws = []
    posterior_shapes = []
    posterior_scales = []
    dimension = 2
    for draw in range(request["posterior_draws"]):
        scaled_sum = 0.0
        for branch in tree["branches"]:
            displacement = (
                location_draws[branch["child"]][draw] - location_draws[branch["parent"]][draw]
            )
            scaled_sum += float(displacement @ displacement) / branch["length"]
        shape = (
            request["diffusion_prior"]["inverse_gamma_shape"]
            + len(tree["branches"]) * dimension / 2.0
        )
        scale = request["diffusion_prior"]["inverse_gamma_scale"] + scaled_sum / 4.0
        posterior_shapes.append(shape)
        posterior_scales.append(scale)
        diffusion_draws.append(float(invgamma.rvs(shape, scale=scale, random_state=rng)))
    return np.asarray(diffusion_draws), posterior_shapes, posterior_scales


def main():
    require_versions(numpy=np.__version__, scipy=scipy.__version__)
    request_bytes = read_request(32 * MIB)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.scipy_clone_models_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    tree = request["tree"]
    location_by_clone = {location["clone_id"]: location for location in request["clone_locations"]}
    rng = np.random.default_rng(request["seed"])
    diffusion_draws, posterior_shapes, posterior_scales = diffusion_posterior(
        request, location_by_clone, rng
    )
    cells = request["cells"]
    probabilities = np.asarray([cell["clone_probabilities"] for cell in cells], dtype=float)
    values = np.asarray([cell["neighborhood_features"] for cell in cells], dtype=float)
    patient_ids = sorted({cell["patient_id"] for cell in cells})
    patient_map = {patient_id: index for index, patient_id in enumerate(patient_ids)}
    patient_indices = np.asarray([patient_map[cell["patient_id"]] for cell in cells], dtype=int)
    contrasts = []
    standard_errors = []
    patient_effects = []
    objectives = []
    for feature in range(values.shape[1]):
        contrast, standard_error, effects, objective = fit_niche(
            values[:, feature], probabilities, patient_indices, len(patient_ids)
        )
        contrasts.append(contrast)
        standard_errors.append(standard_error)
        patient_effects.append(effects)
        objectives.append(objective)
    loo_contrasts = []
    for heldout_patient in range(len(patient_ids)):
        retained = patient_indices != heldout_patient
        remap_ids = patient_indices[retained]
        unique = sorted(set(remap_ids.tolist()))
        remap = {old: new for new, old in enumerate(unique)}
        remapped = np.asarray([remap[value] for value in remap_ids])
        fold = []
        for feature in range(values.shape[1]):
            contrast, _, _, _ = fit_niche(
                values[retained, feature], probabilities[retained], remapped, len(unique)
            )
            fold.append(contrast)
        loo_contrasts.append(fold)
    loo_contrasts = np.asarray(loo_contrasts)
    maximum_change = np.max(np.abs(loo_contrasts - np.asarray(contrasts)[None, :]), axis=0)
    result = {
        "format": "marklab.clone_phylogeography_and_niche_models",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(request_bytes),
        "tree": tree,
        "clone_labels": request["clone_labels"],
        "phylogeography": {
            "transition": "isotropic_brownian_diffusion_per_branch",
            "diffusion_posterior_mean": float(diffusion_draws.mean()),
            "diffusion_posterior_standard_deviation": float(diffusion_draws.std(ddof=1)),
            "diffusion_lower_95": float(np.quantile(diffusion_draws, 0.025)),
            "diffusion_upper_95": float(np.quantile(diffusion_draws, 0.975)),
            "diffusion_draws": diffusion_draws.tolist(),
            "posterior_shape_mean": float(np.mean(posterior_shapes)),
            "posterior_scale_mean": float(np.mean(posterior_scales)),
            "location_uncertainty_integrated": True,
            "direction_or_history_identified": False,
        },
        "niche_model": {
            "likelihood": (
                "clone_probability_weighted_two_component_gaussian_with_patient_intercepts"
            ),
            "patient_count": len(patient_ids),
            "cell_count": len(cells),
            "clone_a_minus_b_contrast": contrasts,
            "clone_a_minus_b_standard_error": standard_errors,
            "patient_feature_means": patient_effects,
            "objective": objectives,
            "leave_one_patient_out_contrasts": loo_contrasts.tolist(),
            "leave_one_patient_out_maximum_contrast_change": maximum_change.tolist(),
            "assignment_uncertainty_integrated": True,
        },
        "claim_status": "experimental_synthetic_clone_models_no_identified_history",
    }
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "clone model worker contract error", errors=CONTRACT_ERRORS)
