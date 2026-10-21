#!/usr/bin/env python3
"""Multiview Bayesian factor model of paired patient-level views with MOFA+ (mofapy2)."""

import json
import math
import sys
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parent))

import h5py
import mofapy2
import numpy as np
import scipy

from marklab_worker_support.mofa import train_mofa
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


def standardize_views(rows, modalities, train_indices):
    """Standardize each view with means and SDs of its observed training entries.

    Returns per-view means, SDs, training matrices (NaN where unobserved), and the
    standardized matrices for all rows.
    """
    means = []
    scales = []
    training_views = []
    standardized_complete = []
    for view, modality in enumerate(modalities):
        feature_count = len(modality["feature_names"])
        values = np.asarray([row["views"][view]["values"] for row in rows], dtype=float)
        observed = np.asarray([row["views"][view]["observed"] for row in rows], dtype=bool)
        view_means = np.empty(feature_count)
        view_scales = np.empty(feature_count)
        for feature in range(feature_count):
            training_values = values[train_indices, feature][observed[train_indices, feature]]
            if len(training_values) < 3:
                raise ContractError(
                    "every MOFA training feature requires at least three observed values"
                )
            view_means[feature] = training_values.mean()
            view_scales[feature] = training_values.std(ddof=0)
            if not math.isfinite(view_scales[feature]) or view_scales[feature] <= 0.0:
                raise ContractError("every MOFA training feature must vary")
        standardized = (values - view_means) / view_scales
        training = standardized[train_indices].copy()
        training[~observed[train_indices]] = np.nan
        means.append(view_means)
        scales.append(view_scales)
        training_views.append(training)
        standardized_complete.append(standardized)
    return means, scales, training_views, standardized_complete


def heldout_predictions(rows, modalities, test_indices, factor_count, fitted_views):
    """Infer held-out factor scores from observed entries and predict the masked entries.

    The score posterior mean under the fitted loadings ``W_v``, feature precisions
    ``tau_v``, and a standard-normal factor prior is ``(I + sum W' tau W)^-1 sum W' tau x``
    over the observed entries of every view.
    """
    weights, precisions, standardized_complete, means, scales = fitted_views
    missing_predictions = []
    heldout_errors = []
    heldout_scores = []
    for row_index in test_indices:
        precision = np.eye(factor_count)
        information = np.zeros(factor_count)
        for view in range(len(modalities)):
            observed = np.asarray(rows[row_index]["views"][view]["observed"], dtype=bool)
            if observed.any():
                selected_weights = weights[view][observed]
                selected_precision = precisions[view][observed]
                selected_values = standardized_complete[view][row_index, observed]
                precision += selected_weights.T @ (selected_weights * selected_precision[:, None])
                information += (selected_values * selected_precision) @ selected_weights
        score = information @ np.linalg.inv(precision)
        heldout_scores.append(score.tolist())
        for view, modality in enumerate(modalities):
            observed = rows[row_index]["views"][view]["observed"]
            for feature, is_observed in enumerate(observed):
                if is_observed:
                    continue
                prediction_standardized = float(weights[view][feature] @ score)
                prediction = prediction_standardized * scales[view][feature] + means[view][feature]
                target = rows[row_index]["views"][view]["values"][feature]
                heldout_errors.append((prediction - target) ** 2)
                missing_predictions.append(
                    {
                        "entity_id": rows[row_index]["entity_id"],
                        "modality_id": modality["id"],
                        "feature_name": modality["feature_names"][feature],
                        "posterior_mean": prediction,
                        "evaluation_target": target,
                        "evaluation_role": "masked_before_fit_and_factor_inference",
                    }
                )
    return missing_predictions, heldout_errors, heldout_scores


def main() -> None:
    require_versions(
        mofapy2=mofapy2.__version__,
        h5py=h5py.__version__,
        numpy=np.__version__,
        scipy=scipy.__version__,
    )
    request_bytes = read_request(16 * MIB)
    request = json.loads(request_bytes)
    exact(request["format"], "marklab.mofapy2_multiview_request", "request.format")
    exact(request["version"], 1, "request.version")
    design = request["design"]
    rows = request["rows"]
    modalities = design["modalities"]
    if (
        design["entity_level"] != "patient"
        or design["missingness_assumption"] != "structurally_absent_or_mar"
        or len(modalities) < 2
        or any(modality["likelihood"] != "gaussian" for modality in modalities)
        or any(modality["measurement_status"] != "measured" for modality in modalities)
    ):
        raise ContractError(
            "MOFA design requires measured patient-level Gaussian views "
            "under structural/MAR missingness"
        )
    train_indices = [index for index, row in enumerate(rows) if row["split"] == "train"]
    test_indices = [index for index, row in enumerate(rows) if row["split"] == "test"]
    if len(train_indices) < 12 or not test_indices:
        raise ContractError("MOFA requires training and held-out rows")
    means, scales, training_views, standardized_complete = standardize_views(
        rows, modalities, train_indices
    )
    model, backend_log = train_mofa(
        training_views,
        view_names=[modality["id"] for modality in modalities],
        group_name="train",
        sample_names=[rows[index]["entity_id"] for index in train_indices],
        feature_names=[modality["feature_names"] for modality in modalities],
        factors=request["maximum_factors"],
        iterations=request["iterations"],
        convergence_mode=request["convergence_mode"],
        seed=request["seed"],
    )
    expectations = model.model.getExpectations()
    factor_scores = np.asarray(expectations["Z"]["E"], dtype=float)
    weights = [np.asarray(item["E"], dtype=float) for item in expectations["W"]]
    precisions = [
        np.nanmean(np.asarray(item["E"], dtype=float), axis=0) for item in expectations["Tau"]
    ]
    variance_explained = np.asarray(model.model.calculate_variance_explained()[0], dtype=float)
    total_variance = variance_explained.sum(axis=0)
    order = np.argsort(-total_variance, kind="stable")
    factor_scores = factor_scores[:, order]
    weights = [item[:, order] for item in weights]
    variance_explained = variance_explained[:, order]
    sign_flips = 0
    for factor in range(factor_scores.shape[1]):
        concatenated = np.concatenate([item[:, factor] for item in weights])
        pivot = int(np.argmax(np.abs(concatenated)))
        if concatenated[pivot] < 0.0:
            factor_scores[:, factor] *= -1.0
            for item in weights:
                item[:, factor] *= -1.0
            sign_flips += 1
    active = [index for index, value in enumerate(variance_explained.sum(axis=0)) if value > 1e-4]
    fitted_views = (weights, precisions, standardized_complete, means, scales)
    missing_predictions, heldout_errors, heldout_scores = heldout_predictions(
        rows, modalities, test_indices, factor_scores.shape[1], fitted_views
    )
    if not missing_predictions:
        raise ContractError("MOFA fixture requires held-out missing entries")
    training = model.model.getTrainingStats()
    elbo = np.asarray(training["elbo"], dtype=float)
    if not np.isfinite(elbo).all() or not np.isfinite(factor_scores).all():
        raise ContractError("MOFA produced non-finite fit artifacts")
    result = {
        "format": "marklab.multiview_factor_model",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(request_bytes),
        "design": {
            **design,
            "validation_status": "passed",
            "leakage_policy": "fit_train_observed_only",
        },
        "standardization": {
            "fit_split": "train_observed_only",
            "means_by_view": [value.tolist() for value in means],
            "scales_by_view": [value.tolist() for value in scales],
        },
        "training_row_count": len(train_indices),
        "heldout_row_count": len(test_indices),
        "factor_scores_train": factor_scores.tolist(),
        "factor_scores_heldout": heldout_scores,
        "loadings_by_view": [value.tolist() for value in weights],
        "variance_explained_by_view_factor": variance_explained.tolist(),
        "active_factor_indices": active,
        "active_factor_count": len(active),
        "factor_activity_per_modality": [
            {
                "modality_id": modality["id"],
                "variance_explained": variance_explained[index].tolist(),
            }
            for index, modality in enumerate(modalities)
        ],
        "alignment": {
            "factor_order": "descending_total_variance_explained",
            "sign": "global_max_loading_positive",
            "sign_flips": sign_flips,
        },
        "missing_predictions": missing_predictions,
        "heldout_missing_rmse": math.sqrt(sum(heldout_errors) / len(heldout_errors)),
        "diagnostics": {
            "elbo_first": float(elbo[0]),
            "elbo_last": float(elbo[-1]),
            "elbo_trace": elbo.tolist(),
            "iterations_completed": len(elbo),
            "trained": bool(model.model.trained),
            "captured_backend_log_lines": len(backend_log.splitlines()),
        },
        "missingness_assumption": design["missingness_assumption"],
        "claim_status": "experimental_synthetic_multiview_factor_model",
    }
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "MOFA worker contract error", errors=CONTRACT_ERRORS)
