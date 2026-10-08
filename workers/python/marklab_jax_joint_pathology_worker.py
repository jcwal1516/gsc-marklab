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


def standardize(matrix):
    means = matrix.mean(axis=0)
    scales = matrix.std(axis=0, ddof=0)
    if np.any(scales <= 0.0) or not np.isfinite(scales).all():
        raise ContractError("every joint Gaussian feature must vary")
    return (matrix - means) / scales, means, scales


def main():
    if (
        jax.__version__ != "0.11.1"
        or np.__version__ != "2.4.6"
        or scipy.__version__ != "1.18.1"
        or sys.version_info[:2] != (3, 12)
    ):
        raise ContractError("joint pathology backend version drift")
    jax.config.update("jax_enable_x64", True)
    request_bytes = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.jax_joint_pathology_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    patient_ids = request["patients"]
    patient_index = {patient_id: index for index, patient_id in enumerate(patient_ids)}
    region_ids = [region["region_id"] for region in request["regions"]]
    region_index = {region_id: index for index, region_id in enumerate(region_ids)}
    parent = np.asarray([patient_index[region["patient_id"]] for region in request["regions"]], dtype=int)
    ordered_observations = sorted(
        request["region_observations"], key=lambda observation: region_index[observation["region_id"]]
    )
    morphology_raw = np.asarray([observation["morphology"] for observation in ordered_observations], dtype=float)
    ihc_raw = np.asarray([observation["ihc"] for observation in ordered_observations], dtype=float)
    counts = np.asarray([observation["omics_counts"] for observation in ordered_observations], dtype=float)
    library_size = np.asarray([observation["library_size"] for observation in ordered_observations], dtype=float)
    clone = np.asarray([observation["clone_label"] for observation in ordered_observations], dtype=float)
    morphology, morphology_means, morphology_scales = standardize(morphology_raw)
    ihc, ihc_means, ihc_scales = standardize(ihc_raw)
    outcomes = sorted(request["patient_outcomes"], key=lambda outcome: patient_index[outcome["patient_id"]])
    outcome_values = np.asarray([outcome["value"] for outcome in outcomes], dtype=float)
    outcome_observed = np.asarray([outcome["observed"] for outcome in outcomes], dtype=bool)
    outcome_mean = float(outcome_values[outcome_observed].mean())
    outcome_scale = float(outcome_values[outcome_observed].std(ddof=0))
    if outcome_scale <= 0.0 or not math.isfinite(outcome_scale):
        raise ContractError("observed clinical outcomes must vary")
    outcome_standardized = (outcome_values - outcome_mean) / outcome_scale
    initialization_matrix = np.concatenate(
        [morphology, ihc, np.log1p(counts), clone[:, None]], axis=1
    )
    region_latent = np.linalg.svd(initialization_matrix - initialization_matrix.mean(axis=0), full_matrices=False)[0][:, 0]
    patient_latent = np.asarray(
        [region_latent[parent == patient].mean() for patient in range(len(patient_ids))], dtype=float
    )
    observed_correlation = np.corrcoef(patient_latent[outcome_observed], outcome_standardized[outcome_observed])[0, 1]
    if observed_correlation < 0.0:
        patient_latent *= -1.0
        region_latent *= -1.0

    def gaussian_regression(latent, target):
        design = np.column_stack([np.ones(len(latent)), latent])
        coefficients = np.linalg.lstsq(design, target, rcond=None)[0]
        return coefficients[0], coefficients[1]

    morphology_bias, morphology_loading = gaussian_regression(region_latent, morphology)
    ihc_bias, ihc_loading = gaussian_regression(region_latent, ihc)
    count_target = np.log((counts + 0.5) / library_size[:, None])
    omics_bias, omics_loading = gaussian_regression(region_latent, count_target)
    clone_target = np.where(clone > 0.5, math.log(5.0), -math.log(5.0))
    clone_bias_array, clone_loading_array = gaussian_regression(region_latent, clone_target[:, None])
    outcome_bias_array, outcome_loading_array = gaussian_regression(
        patient_latent[outcome_observed], outcome_standardized[outcome_observed, None]
    )
    initial_parts = [
        patient_latent,
        region_latent,
        morphology_bias,
        morphology_loading,
        ihc_bias,
        ihc_loading,
        omics_bias,
        omics_loading,
        clone_bias_array,
        clone_loading_array,
        outcome_bias_array,
        outcome_loading_array,
    ]
    shapes = [part.shape for part in initial_parts]
    initial = np.concatenate([part.ravel() for part in initial_parts])

    def unpack(parameters):
        offset = 0
        parts = []
        for shape in shapes:
            size = int(np.prod(shape))
            parts.append(parameters[offset : offset + size].reshape(shape))
            offset += size
        return parts

    morphology_jax = jnp.asarray(morphology)
    ihc_jax = jnp.asarray(ihc)
    counts_jax = jnp.asarray(counts)
    library_jax = jnp.asarray(library_size)
    clone_jax = jnp.asarray(clone)
    outcome_jax = jnp.asarray(outcome_standardized)
    outcome_observed_jax = jnp.asarray(outcome_observed)
    parent_jax = jnp.asarray(parent)

    def objective(parameters):
        (
            z_patient,
            z_region,
            bias_m,
            loading_m,
            bias_i,
            loading_i,
            bias_o,
            loading_o,
            bias_c,
            loading_c,
            bias_y,
            loading_y,
        ) = unpack(parameters)
        eta_m = bias_m[None, :] + z_region[:, None] * loading_m[None, :]
        eta_i = bias_i[None, :] + z_region[:, None] * loading_i[None, :]
        eta_o = jnp.log(library_jax)[:, None] + bias_o[None, :] + z_region[:, None] * loading_o[None, :]
        eta_c = bias_c[0] + z_region * loading_c[0]
        eta_y = bias_y[0] + z_patient * loading_y[0]
        gaussian_scale = request["gaussian_noise_standard_deviation"]
        loss = 0.5 * jnp.sum(z_patient * z_patient)
        loss += 0.5 * jnp.sum((z_region - z_patient[parent_jax]) ** 2) / request["region_latent_standard_deviation"] ** 2
        loss += 0.5 * jnp.sum((morphology_jax - eta_m) ** 2) / gaussian_scale ** 2
        loss += 0.5 * jnp.sum((ihc_jax - eta_i) ** 2) / gaussian_scale ** 2
        loss += jnp.sum(jnp.exp(eta_o) - counts_jax * eta_o)
        loss += jnp.sum(jax.nn.softplus(eta_c) - clone_jax * eta_c)
        loss += 0.5 * jnp.sum(jnp.where(outcome_observed_jax, (outcome_jax - eta_y) ** 2, 0.0)) / gaussian_scale ** 2
        non_latent = parameters[len(patient_ids) + len(region_ids) :]
        loss += 0.5 * request["parameter_precision"] * jnp.sum(non_latent * non_latent)
        return loss

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
        raise ContractError(f"joint pathology Laplace MAP did not converge: {fit.message}")
    covariance = np.asarray(fit.hess_inv.todense(), dtype=float)
    if covariance.shape != (len(fit.x), len(fit.x)) or not np.isfinite(covariance).all():
        raise ContractError("joint inverse-Hessian approximation is unavailable")
    (
        z_patient,
        z_region,
        bias_m,
        loading_m,
        bias_i,
        loading_i,
        bias_o,
        loading_o,
        bias_c,
        loading_c,
        bias_y,
        loading_y,
    ) = [np.asarray(part, dtype=float).copy() for part in unpack(fit.x)]
    heldout_predictions = []
    heldout_errors = []
    for patient in np.flatnonzero(~outcome_observed):
        mean_standardized = bias_y[0] + loading_y[0] * z_patient[patient]
        gradient = np.zeros(len(fit.x), dtype=float)
        gradient[patient] = loading_y[0]
        outcome_bias_offset = sum(int(np.prod(shape)) for shape in shapes[:-2])
        gradient[outcome_bias_offset] = 1.0
        gradient[outcome_bias_offset + 1] = z_patient[patient]
        variance_standardized = max(
            float(gradient @ covariance @ gradient)
            + request["gaussian_noise_standard_deviation"] ** 2,
            1e-12,
        )
        prediction = mean_standardized * outcome_scale + outcome_mean
        target = outcome_values[patient]
        heldout_errors.append((prediction - target) ** 2)
        heldout_predictions.append({
            "patient_id": patient_ids[patient],
            "posterior_mean": float(prediction),
            "posterior_standard_deviation": math.sqrt(variance_standardized) * outcome_scale,
            "evaluation_target": float(target),
            "evaluation_role": "masked_before_fit",
        })
    if loading_y[0] < 0.0:
        z_patient *= -1.0
        z_region *= -1.0
        for loading in (loading_m, loading_i, loading_o, loading_c, loading_y):
            loading *= -1.0
        sign_flipped = True
    else:
        sign_flipped = False
    morphology_prediction = (bias_m[None, :] + z_region[:, None] * loading_m[None, :]) * morphology_scales + morphology_means
    ihc_prediction = (bias_i[None, :] + z_region[:, None] * loading_i[None, :]) * ihc_scales + ihc_means
    count_prediction = library_size[:, None] * np.exp(bias_o[None, :] + z_region[:, None] * loading_o[None, :])
    clone_probability = 1.0 / (1.0 + np.exp(-(bias_c[0] + z_region * loading_c[0])))
    observed_outcome_prediction = (bias_y[0] + z_patient * loading_y[0]) * outcome_scale + outcome_mean
    result = {
        "format": "marklab.joint_pathology_model_fit",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "project_id": request["project_id"],
        "model_ir": {
            "format": "marklab.joint_pathology_model_ir",
            "version": 1,
            "hierarchy": {"patient_count": len(patient_ids), "region_count": len(region_ids), "region_parent_patient": parent.tolist()},
            "latent_structure": {"patient": "standard_normal", "region": "conditional_gaussian"},
            "observation_blocks": [
                {"id": block_id, **request["model_spec"][block_id]}
                for block_id in ("morphology", "ihc", "omics", "clone", "clinical")
            ],
            "claim_limits": ["synthetic_only", "laplace_approximation", "no_causal_interpretation", "measured_not_predicted"],
        },
        "posterior_map": {
            "patient_factor": z_patient.tolist(),
            "region_factor": z_region.tolist(),
            "morphology_loadings": loading_m.tolist(),
            "ihc_loadings": loading_i.tolist(),
            "omics_loadings": loading_o.tolist(),
            "clone_loading": float(loading_c[0]),
            "clinical_loading": float(loading_y[0]),
        },
        "alignment": {"sign": "clinical_loading_positive", "sign_flipped": sign_flipped},
        "heldout_outcome_predictions": heldout_predictions,
        "heldout_outcome_rmse": math.sqrt(sum(heldout_errors) / len(heldout_errors)),
        "modality_checks": {
            "morphology_rmse": float(np.sqrt(np.mean((morphology_prediction - morphology_raw) ** 2))),
            "ihc_rmse": float(np.sqrt(np.mean((ihc_prediction - ihc_raw) ** 2))),
            "omics_count_rmse": float(np.sqrt(np.mean((count_prediction - counts) ** 2))),
            "clone_accuracy": float(np.mean((clone_probability >= 0.5) == (clone >= 0.5))),
            "observed_clinical_rmse": float(np.sqrt(np.mean((observed_outcome_prediction[outcome_observed] - outcome_values[outcome_observed]) ** 2))),
        },
        "diagnostics": {
            "inference": "laplace_map_inverse_hessian",
            "objective_initial": initial_objective,
            "objective_final": float(fit.fun),
            "iterations": int(fit.nit),
            "gradient_max_absolute": float(np.max(np.abs(fit.jac))),
        },
        "fit_state": "approximate_only",
        "claim_status": "experimental_synthetic_joint_pathology_model",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError, np.linalg.LinAlgError) as error:
        print(f"joint pathology worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
