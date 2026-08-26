#!/usr/bin/env python3

import hashlib
import json
import math
import sys

import numpy as np
import scipy
from scipy.optimize import minimize
from scipy.special import logsumexp
from scipy.stats import invgamma


class ContractError(Exception):
    pass


def fit_niche(values, probabilities, patient_indices, patient_count):
    patient_means = np.asarray([values[patient_indices == patient].mean() for patient in range(patient_count)])
    centered = values - patient_means[patient_indices]

    def objective(parameters):
        contrast = parameters[0]
        sigma = math.exp(parameters[1])
        means = np.column_stack([
            np.full(len(centered), contrast / 2.0),
            np.full(len(centered), -contrast / 2.0),
        ])
        log_components = (
            np.log(np.maximum(probabilities, 1e-300))
            - 0.5 * ((centered[:, None] - means) / sigma) ** 2
            - math.log(sigma)
            - 0.5 * math.log(2.0 * math.pi)
        )
        return float(-np.sum(logsumexp(log_components, axis=1)))

    initial = np.asarray([2.0, math.log(max(centered.std(ddof=0) * 0.2, 0.05))])
    fit = minimize(objective, initial, method="L-BFGS-B", bounds=[(-20.0, 20.0), (-6.0, 4.0)])
    if not fit.success:
        raise ContractError(f"clone niche fit did not converge: {fit.message}")
    step = 1e-4
    curvature = (objective(fit.x + [step, 0.0]) - 2.0 * objective(fit.x) + objective(fit.x - [step, 0.0])) / step**2
    standard_error = math.sqrt(1.0 / max(curvature, 1e-12))
    return float(fit.x[0]), standard_error, patient_means.tolist(), float(fit.fun)


def main():
    if np.__version__ != "2.4.6" or scipy.__version__ != "1.18.1" or sys.version_info[:2] != (3, 12):
        raise ContractError("clone model backend version drift")
    request_bytes = sys.stdin.buffer.read(32 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.scipy_clone_models_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    tree = request["tree"]
    location_by_clone = {location["clone_id"]: location for location in request["clone_locations"]}
    rng = np.random.default_rng(request["seed"])
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
            displacement = location_draws[branch["child"]][draw] - location_draws[branch["parent"]][draw]
            scaled_sum += float(displacement @ displacement) / branch["length"]
        shape = request["diffusion_prior"]["inverse_gamma_shape"] + len(tree["branches"]) * dimension / 2.0
        scale = request["diffusion_prior"]["inverse_gamma_scale"] + scaled_sum / 4.0
        posterior_shapes.append(shape)
        posterior_scales.append(scale)
        diffusion_draws.append(float(invgamma.rvs(shape, scale=scale, random_state=rng)))
    diffusion_draws = np.asarray(diffusion_draws)
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
        admitted = patient_indices != heldout_patient
        remap_ids = patient_indices[admitted]
        unique = sorted(set(remap_ids.tolist()))
        remap = {old: new for new, old in enumerate(unique)}
        remapped = np.asarray([remap[value] for value in remap_ids])
        fold = []
        for feature in range(values.shape[1]):
            contrast, _, _, _ = fit_niche(
                values[admitted, feature], probabilities[admitted], remapped, len(unique)
            )
            fold.append(contrast)
        loo_contrasts.append(fold)
    loo_contrasts = np.asarray(loo_contrasts)
    maximum_change = np.max(np.abs(loo_contrasts - np.asarray(contrasts)[None, :]), axis=0)
    result = {
        "format": "marklab.clone_phylogeography_and_niche_models",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
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
            "likelihood": "clone_probability_weighted_two_component_gaussian_with_patient_centering",
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
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError, np.linalg.LinAlgError) as error:
        print(f"clone model worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
