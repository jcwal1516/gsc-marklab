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


def main():
    if (
        jax.__version__ != "0.11.1"
        or np.__version__ != "2.4.6"
        or scipy.__version__ != "1.18.1"
        or sys.version_info[:2] != (3, 12)
    ):
        raise ContractError("modality dropout backend version drift")
    jax.config.update("jax_enable_x64", True)
    request_bytes = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.jax_modality_dropout_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    rows = request["rows"]
    train_indices = [index for index, row in enumerate(rows) if row["split"] == "train"]
    test_indices = [index for index, row in enumerate(rows) if row["split"] == "test"]
    modality_ids = [modality["id"] for modality in request["design"]["modalities"]]
    feature_counts = [len(modality["feature_names"]) for modality in request["design"]["modalities"]]
    raw_views = [
        np.asarray([row["views"][modality]["values"] for row in rows], dtype=float)
        for modality in range(len(modality_ids))
    ]
    means = [view[train_indices].mean(axis=0) for view in raw_views]
    scales = [view[train_indices].std(axis=0, ddof=0) for view in raw_views]
    if any(np.any(scale <= 0.0) or not np.isfinite(scale).all() for scale in scales):
        raise ContractError("every dropout-training feature must vary")
    views = [(view - mean) / scale for view, mean, scale in zip(raw_views, means, scales)]
    training_concat = np.concatenate([view[train_indices] for view in views], axis=1)
    left, singular, _ = np.linalg.svd(training_concat, full_matrices=False)
    latent_dimensions = request["latent_dimensions"]
    target_scores = left[:, :latent_dimensions] * singular[:latent_dimensions]
    encoder_shapes = []
    decoder_shapes = []
    initial_parts = []
    for view in views:
        training_view = view[train_indices]
        encoder = np.linalg.lstsq(training_view, target_scores, rcond=None)[0]
        decoder = np.linalg.lstsq(target_scores, training_view, rcond=None)[0]
        encoder_shapes.append(encoder.shape)
        decoder_shapes.append(decoder.shape)
        initial_parts.extend([encoder.ravel(), decoder.ravel()])
    initial = np.concatenate(initial_parts)
    training_views = [jnp.asarray(view[train_indices]) for view in views]
    pattern_indices = [
        [modality_ids.index(modality) for modality in pattern["retained_modalities"]]
        for pattern in request["dropout_patterns"]
    ]

    def unpack(parameters):
        offset = 0
        encoders = []
        decoders = []
        for encoder_shape, decoder_shape in zip(encoder_shapes, decoder_shapes):
            encoder_size = int(np.prod(encoder_shape))
            decoder_size = int(np.prod(decoder_shape))
            encoders.append(parameters[offset : offset + encoder_size].reshape(encoder_shape))
            offset += encoder_size
            decoders.append(parameters[offset : offset + decoder_size].reshape(decoder_shape))
            offset += decoder_size
        return encoders, decoders

    def latent_for(retained, current_views, encoders):
        return sum((current_views[index] @ encoders[index] for index in retained)) / len(retained)

    def objective(parameters):
        encoders, decoders = unpack(parameters)
        full_latent = latent_for(range(len(training_views)), training_views, encoders)
        loss = 0.0
        for pattern, retained in zip(request["dropout_patterns"], pattern_indices):
            partial_latent = latent_for(retained, training_views, encoders)
            reconstruction = sum(
                jnp.mean((partial_latent @ decoder - target) ** 2)
                for decoder, target in zip(decoders, training_views)
            )
            consistency = jnp.mean((partial_latent - full_latent) ** 2)
            loss += pattern["probability"] * (
                reconstruction + request["consistency_weight"] * consistency
            )
        penalty = sum(jnp.sum(value * value) for value in [*encoders, *decoders])
        return loss + 0.5 * request["parameter_precision"] * penalty / len(train_indices)

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
        options={"maxiter": request["maximum_iterations"], "ftol": 1e-12, "gtol": 1e-7},
    )
    if not fit.success or not np.isfinite(fit.x).all():
        raise ContractError(f"modality-dropout optimization did not converge: {fit.message}")
    encoders, decoders = unpack(np.asarray(fit.x))
    test_views = [view[test_indices] for view in views]
    full_test_latent = latent_for(range(len(test_views)), test_views, encoders)
    pattern_validation = []
    for pattern, retained in zip(request["dropout_patterns"], pattern_indices):
        partial_latent = latent_for(retained, test_views, encoders)
        missing = [index for index in range(len(modality_ids)) if index not in retained]
        squared_errors = []
        predictions = []
        for modality in missing:
            standardized_prediction = partial_latent @ decoders[modality]
            prediction = standardized_prediction * scales[modality] + means[modality]
            target = raw_views[modality][test_indices]
            squared_errors.extend(((prediction - target) ** 2).ravel().tolist())
            predictions.append({
                "modality_id": modality_ids[modality],
                "entity_ids": [rows[index]["entity_id"] for index in test_indices],
                "posterior_means": prediction.tolist(),
                "evaluation_targets": target.tolist(),
            })
        consistency_rmse = math.sqrt(float(np.mean((partial_latent - full_test_latent) ** 2)))
        pattern_validation.append({
            "pattern_id": pattern["pattern_id"],
            "retained_modalities": pattern["retained_modalities"],
            "missing_modalities": [modality_ids[index] for index in missing],
            "probability": pattern["probability"],
            "heldout_patient_count": len(test_indices),
            "heldout_missing_view_rmse": math.sqrt(sum(squared_errors) / len(squared_errors)),
            "heldout_latent_consistency_rmse": consistency_rmse,
            "predictions": predictions,
        })
    result = {
        "format": "marklab.modality_robust_inference_model",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "design": request["design"],
        "latent_dimensions": latent_dimensions,
        "required_anchor_modalities": request["required_anchor_modalities"],
        "dropout_patterns": request["dropout_patterns"],
        "standardization": [
            {"modality_id": modality_id, "means": mean.tolist(), "scales": scale.tolist(), "fit_split": "train"}
            for modality_id, mean, scale in zip(modality_ids, means, scales)
        ],
        "encoder_maps": [np.asarray(value).tolist() for value in encoders],
        "decoder_maps": [np.asarray(value).tolist() for value in decoders],
        "pattern_validation": pattern_validation,
        "diagnostics": {
            "objective": "weighted_reconstruction_plus_full_partial_consistency",
            "objective_initial": initial_objective,
            "objective_final": float(fit.fun),
            "iterations": int(fit.nit),
            "gradient_max_absolute": float(np.max(np.abs(fit.jac))),
        },
        "fit_state": "approximate_only",
        "claim_status": "experimental_synthetic_modality_dropout_robustness",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError, np.linalg.LinAlgError) as error:
        print(f"modality dropout worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
