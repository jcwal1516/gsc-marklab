#!/usr/bin/env python3

import contextlib
import hashlib
import io
import json
import math
import sys

import h5py
import mofapy2
import numpy as np
import scipy
from mofapy2.run.entry_point import entry_point


class ContractError(Exception):
    pass


def main():
    if (
        mofapy2.__version__ != "0.7.4"
        or h5py.__version__ != "3.16.0"
        or np.__version__ != "2.4.6"
        or scipy.__version__ != "1.18.1"
        or sys.version_info[:2] != (3, 12)
    ):
        raise ContractError("mofapy2/h5py/NumPy/SciPy/Python version drift")
    request_bytes = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.mofapy2_multiview_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
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
        raise ContractError("MOFA design requires measured patient-level Gaussian views under structural/MAR missingness")
    train_indices = [index for index, row in enumerate(rows) if row["split"] == "train"]
    test_indices = [index for index, row in enumerate(rows) if row["split"] == "test"]
    if len(train_indices) < 12 or not test_indices:
        raise ContractError("MOFA requires training and held-out rows")
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
            admitted = values[train_indices, feature][observed[train_indices, feature]]
            if len(admitted) < 3:
                raise ContractError("every MOFA training feature requires at least three observed values")
            view_means[feature] = admitted.mean()
            view_scales[feature] = admitted.std(ddof=0)
            if not math.isfinite(view_scales[feature]) or view_scales[feature] <= 0.0:
                raise ContractError("every MOFA training feature must vary")
        standardized = (values - view_means) / view_scales
        training = standardized[train_indices].copy()
        training[~observed[train_indices]] = np.nan
        means.append(view_means)
        scales.append(view_scales)
        training_views.append(training)
        standardized_complete.append(standardized)
    captured = io.StringIO()
    with contextlib.redirect_stdout(captured):
        model = entry_point()
        model.set_data_options(
            scale_views=False,
            scale_groups=False,
            center_groups=False,
            use_float32=False,
        )
        model.set_data_matrix(
            [[view] for view in training_views],
            likelihoods=["gaussian"] * len(modalities),
            views_names=[modality["id"] for modality in modalities],
            groups_names=["train"],
            samples_names=[[rows[index]["entity_id"] for index in train_indices]],
            features_names=[modality["feature_names"] for modality in modalities],
        )
        model.set_model_options(
            factors=request["maximum_factors"],
            spikeslab_factors=False,
            spikeslab_weights=False,
            ard_factors=True,
            ard_weights=True,
        )
        model.set_train_options(
            iter=request["iterations"],
            startELBO=1,
            freqELBO=1,
            convergence_mode=request["convergence_mode"],
            startDrop=1,
            freqDrop=1,
            dropR2=None,
            nostop=False,
            quiet=True,
            seed=request["seed"],
            gpu_mode=False,
        )
        model.build()
        model.run()
    expectations = model.model.getExpectations()
    factor_scores = np.asarray(expectations["Z"]["E"], dtype=float)
    weights = [np.asarray(item["E"], dtype=float) for item in expectations["W"]]
    precisions = [
        np.nanmean(np.asarray(item["E"], dtype=float), axis=0)
        for item in expectations["Tau"]
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
    missing_predictions = []
    heldout_errors = []
    heldout_scores = []
    for row_index in test_indices:
        precision = np.eye(factor_scores.shape[1])
        information = np.zeros(factor_scores.shape[1])
        for view, modality in enumerate(modalities):
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
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "design": {**design, "validation_status": "passed", "leakage_policy": "fit_train_observed_only"},
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
            {"modality_id": modality["id"], "variance_explained": variance_explained[index].tolist()}
            for index, modality in enumerate(modalities)
        ],
        "alignment": {"factor_order": "descending_total_variance_explained", "sign": "global_max_loading_positive", "sign_flips": sign_flips},
        "missing_predictions": missing_predictions,
        "heldout_missing_rmse": math.sqrt(sum(heldout_errors) / len(heldout_errors)),
        "diagnostics": {
            "elbo_first": float(elbo[0]),
            "elbo_last": float(elbo[-1]),
            "elbo_trace": elbo.tolist(),
            "iterations_completed": len(elbo),
            "trained": bool(model.model.trained),
            "captured_backend_log_lines": len(captured.getvalue().splitlines()),
        },
        "missingness_assumption": design["missingness_assumption"],
        "claim_status": "experimental_synthetic_multiview_factor_model",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError, np.linalg.LinAlgError) as error:
        print(f"MOFA worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
