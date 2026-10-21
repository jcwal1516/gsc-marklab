#!/usr/bin/env python3
"""Joint latent-factor model of region morphology, IHC, omics, clone, and patient outcome."""

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
    inverse_hessian_covariance,
    jax_objective,
    minimize_lbfgs,
)
from marklab_worker_support.posterior import sigmoid
from marklab_worker_support.protocol import (
    CONTRACT_ERRORS,
    MIB,
    ContractError,
    exact,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    write_result,
)


def standardize(matrix):
    means = matrix.mean(axis=0)
    scales = matrix.std(axis=0, ddof=0)
    if np.any(scales <= 0.0) or not np.isfinite(scales).all():
        raise ContractError("every joint Gaussian feature must vary")
    return (matrix - means) / scales, means, scales


def unpack(parameters, shapes):
    offset = 0
    parts = []
    for shape in shapes:
        size = int(np.prod(shape))
        parts.append(parameters[offset : offset + size].reshape(shape))
        offset += size
    return parts


def gaussian_regression(latent, target):
    """Least-squares intercept and slope of ``target`` (columns) on ``latent``."""
    design = np.column_stack([np.ones(len(latent)), latent])
    coefficients = np.linalg.lstsq(design, target, rcond=None)[0]
    return coefficients[0], coefficients[1]


def observation_blocks(request, patient_index, region_index):
    """Region observation blocks ordered by region and outcomes ordered by patient."""
    observations = sorted(
        request["region_observations"],
        key=lambda observation: region_index[observation["region_id"]],
    )

    def block(name):
        return np.asarray([observation[name] for observation in observations], dtype=float)

    outcomes = sorted(
        request["patient_outcomes"], key=lambda outcome: patient_index[outcome["patient_id"]]
    )
    return {
        "morphology": block("morphology"),
        "ihc": block("ihc"),
        "counts": block("omics_counts"),
        "library_size": block("library_size"),
        "clone": block("clone_label"),
        "outcome_values": np.asarray([outcome["value"] for outcome in outcomes], dtype=float),
        "outcome_observed": np.asarray([outcome["observed"] for outcome in outcomes], dtype=bool),
    }


def initial_parameters(data, parent, patient_count):
    """Initialize factors from the leading principal component and blocks by regression.

    The factor sign is chosen so that patient factors correlate positively with the
    observed standardized outcome.
    """
    morphology = data["morphology"]
    ihc = data["ihc"]
    counts = data["counts"]
    clone = data["clone"]
    outcome_observed = data["outcome_observed"]
    outcome_standardized = data["outcome_standardized"]
    initialization_matrix = np.concatenate(
        [morphology, ihc, np.log1p(counts), clone[:, None]], axis=1
    )
    centered = initialization_matrix - initialization_matrix.mean(axis=0)
    region_latent = np.linalg.svd(centered, full_matrices=False)[0][:, 0]
    patient_latent = np.asarray(
        [region_latent[parent == patient].mean() for patient in range(patient_count)],
        dtype=float,
    )
    observed_correlation = np.corrcoef(
        patient_latent[outcome_observed], outcome_standardized[outcome_observed]
    )[0, 1]
    if observed_correlation < 0.0:
        patient_latent *= -1.0
        region_latent *= -1.0

    morphology_bias, morphology_loading = gaussian_regression(region_latent, morphology)
    ihc_bias, ihc_loading = gaussian_regression(region_latent, ihc)
    count_target = np.log((counts + 0.5) / data["library_size"][:, None])
    omics_bias, omics_loading = gaussian_regression(region_latent, count_target)
    clone_target = np.where(clone > 0.5, math.log(5.0), -math.log(5.0))
    clone_bias, clone_loading = gaussian_regression(region_latent, clone_target[:, None])
    outcome_bias, outcome_loading = gaussian_regression(
        patient_latent[outcome_observed], outcome_standardized[outcome_observed, None]
    )
    return [
        patient_latent,
        region_latent,
        morphology_bias,
        morphology_loading,
        ihc_bias,
        ihc_loading,
        omics_bias,
        omics_loading,
        clone_bias,
        clone_loading,
        outcome_bias,
        outcome_loading,
    ]


def joint_negative_log_posterior(data, parent, shapes, latent_count, request):
    """Return the negative log posterior (up to a constant) of the joint pathology model.

    Patient factor ``z_p ~ Normal(0, 1)``; region factor ``z_r ~ Normal(z_parent(r), s_r^2)``
    with ``s_r = region_latent_standard_deviation``. Given region ``r``:
    standardized morphology and IHC features ``~ Normal(b_j + z_r w_j, sigma^2)``; omics
    counts ``~ Poisson(library_r * exp(b_j + z_r w_j))``; the clone label
    ``~ Bernoulli(logistic(b + z_r w))``. The standardized clinical outcome of an observed
    patient is ``~ Normal(b + z_p w, sigma^2)`` with ``sigma = gaussian_noise_standard_deviation``.
    Every bias and loading has an independent ``Normal(0, 1 / parameter_precision)`` prior.
    """
    morphology_jax = jnp.asarray(data["morphology"])
    ihc_jax = jnp.asarray(data["ihc"])
    counts_jax = jnp.asarray(data["counts"])
    library_jax = jnp.asarray(data["library_size"])
    clone_jax = jnp.asarray(data["clone"])
    outcome_jax = jnp.asarray(data["outcome_standardized"])
    outcome_observed_jax = jnp.asarray(data["outcome_observed"])
    parent_jax = jnp.asarray(parent)
    gaussian_scale = request["gaussian_noise_standard_deviation"]
    region_scale = request["region_latent_standard_deviation"]

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
        ) = unpack(parameters, shapes)
        eta_m = bias_m[None, :] + z_region[:, None] * loading_m[None, :]
        eta_i = bias_i[None, :] + z_region[:, None] * loading_i[None, :]
        eta_o = (
            jnp.log(library_jax)[:, None] + bias_o[None, :] + z_region[:, None] * loading_o[None, :]
        )
        eta_c = bias_c[0] + z_region * loading_c[0]
        eta_y = bias_y[0] + z_patient * loading_y[0]
        loss = 0.5 * jnp.sum(z_patient * z_patient)
        loss += 0.5 * jnp.sum((z_region - z_patient[parent_jax]) ** 2) / region_scale**2
        loss += 0.5 * jnp.sum((morphology_jax - eta_m) ** 2) / gaussian_scale**2
        loss += 0.5 * jnp.sum((ihc_jax - eta_i) ** 2) / gaussian_scale**2
        loss += jnp.sum(jnp.exp(eta_o) - counts_jax * eta_o)
        loss += jnp.sum(jax.nn.softplus(eta_c) - clone_jax * eta_c)
        outcome_squared_error = jnp.where(outcome_observed_jax, (outcome_jax - eta_y) ** 2, 0.0)
        loss += 0.5 * jnp.sum(outcome_squared_error) / gaussian_scale**2
        non_latent = parameters[latent_count:]
        loss += 0.5 * request["parameter_precision"] * jnp.sum(non_latent * non_latent)
        return loss

    return objective


def main() -> None:
    require_versions(jax=jax.__version__, numpy=np.__version__, scipy=scipy.__version__)
    jax.config.update("jax_enable_x64", True)
    request_bytes = read_request(16 * MIB)
    request = json.loads(request_bytes)
    exact(request["format"], "marklab.jax_joint_pathology_request", "request.format")
    exact(request["version"], 1, "request.version")
    patient_ids = request["patients"]
    patient_index = {patient_id: index for index, patient_id in enumerate(patient_ids)}
    region_ids = [region["region_id"] for region in request["regions"]]
    region_index = {region_id: index for index, region_id in enumerate(region_ids)}
    parent = np.asarray(
        [patient_index[region["patient_id"]] for region in request["regions"]], dtype=int
    )
    data = observation_blocks(request, patient_index, region_index)
    morphology_raw = data["morphology"]
    ihc_raw = data["ihc"]
    counts = data["counts"]
    library_size = data["library_size"]
    clone = data["clone"]
    data["morphology"], morphology_means, morphology_scales = standardize(morphology_raw)
    data["ihc"], ihc_means, ihc_scales = standardize(ihc_raw)
    outcome_values = data["outcome_values"]
    outcome_observed = data["outcome_observed"]
    outcome_mean = float(outcome_values[outcome_observed].mean())
    outcome_scale = float(outcome_values[outcome_observed].std(ddof=0))
    if outcome_scale <= 0.0 or not math.isfinite(outcome_scale):
        raise ContractError("observed clinical outcomes must vary")
    data["outcome_standardized"] = (outcome_values - outcome_mean) / outcome_scale

    initial_parts = initial_parameters(data, parent, len(patient_ids))
    shapes = [part.shape for part in initial_parts]
    initial = np.concatenate([part.ravel() for part in initial_parts])
    objective = joint_negative_log_posterior(
        data, parent, shapes, len(patient_ids) + len(region_ids), request
    )
    objective_and_gradient = jax_objective(objective)
    initial_objective, fit = minimize_lbfgs(
        objective_and_gradient,
        initial,
        maximum_iterations=request["maximum_iterations"],
        ftol=1e-10,
        gtol=1e-6,
        failure="joint pathology Laplace MAP did not converge",
    )
    covariance = inverse_hessian_covariance(
        fit, "joint posterior curvature is unavailable", objective_and_gradient
    )
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
    ) = [np.asarray(part, dtype=float).copy() for part in unpack(fit.x, shapes)]

    # Held-out outcomes: delta-method variance of b_y + w_y z_p plus the observation noise.
    outcome_bias_offset = sum(int(np.prod(shape)) for shape in shapes[:-2])
    noise_variance = request["gaussian_noise_standard_deviation"] ** 2
    heldout_predictions = []
    heldout_errors = []
    for patient in np.flatnonzero(~outcome_observed):
        mean_standardized = bias_y[0] + loading_y[0] * z_patient[patient]
        gradient = np.zeros(len(fit.x), dtype=float)
        gradient[patient] = loading_y[0]
        gradient[outcome_bias_offset] = 1.0
        gradient[outcome_bias_offset + 1] = z_patient[patient]
        variance_standardized = delta_method_variance(gradient, covariance, noise_variance)
        prediction = mean_standardized * outcome_scale + outcome_mean
        target = outcome_values[patient]
        heldout_errors.append((prediction - target) ** 2)
        heldout_predictions.append(
            {
                "patient_id": patient_ids[patient],
                "posterior_mean": float(prediction),
                "posterior_standard_deviation": math.sqrt(variance_standardized) * outcome_scale,
                "evaluation_target": float(target),
                "evaluation_role": "masked_before_fit",
            }
        )

    sign_flipped = bool(loading_y[0] < 0.0)
    if sign_flipped:
        z_patient *= -1.0
        z_region *= -1.0
        for loading in (loading_m, loading_i, loading_o, loading_c, loading_y):
            loading *= -1.0
    region_factor = z_region[:, None]
    morphology_prediction = (
        bias_m[None, :] + region_factor * loading_m[None, :]
    ) * morphology_scales + morphology_means
    ihc_prediction = (bias_i[None, :] + region_factor * loading_i[None, :]) * ihc_scales + ihc_means
    count_prediction = library_size[:, None] * np.exp(
        bias_o[None, :] + region_factor * loading_o[None, :]
    )
    clone_probability = sigmoid(bias_c[0] + z_region * loading_c[0])
    observed_outcome_prediction = (
        bias_y[0] + z_patient * loading_y[0]
    ) * outcome_scale + outcome_mean
    observed_outcome_error = (
        observed_outcome_prediction[outcome_observed] - outcome_values[outcome_observed]
    )
    result = {
        "format": "marklab.joint_pathology_model_fit",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(request_bytes),
        "project_id": request["project_id"],
        "model_ir": {
            "format": "marklab.joint_pathology_model_ir",
            "version": 1,
            "hierarchy": {
                "patient_count": len(patient_ids),
                "region_count": len(region_ids),
                "region_parent_patient": parent.tolist(),
            },
            "latent_structure": {"patient": "standard_normal", "region": "conditional_gaussian"},
            "observation_blocks": [
                {"id": block_id, **request["model_spec"][block_id]}
                for block_id in ("morphology", "ihc", "omics", "clone", "clinical")
            ],
            "claim_limits": [
                "synthetic_only",
                "laplace_approximation",
                "no_causal_interpretation",
                "measured_not_predicted",
            ],
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
            "morphology_rmse": float(
                np.sqrt(np.mean((morphology_prediction - morphology_raw) ** 2))
            ),
            "ihc_rmse": float(np.sqrt(np.mean((ihc_prediction - ihc_raw) ** 2))),
            "omics_count_rmse": float(np.sqrt(np.mean((count_prediction - counts) ** 2))),
            "clone_accuracy": float(np.mean((clone_probability >= 0.5) == (clone >= 0.5))),
            "observed_clinical_rmse": float(np.sqrt(np.mean(observed_outcome_error**2))),
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
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "joint pathology worker contract error", errors=CONTRACT_ERRORS)
