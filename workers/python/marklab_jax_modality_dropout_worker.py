#!/usr/bin/env python3
"""Modality-dropout-robust linear encoder/decoder model of paired standardized views."""

import json
import math
import sys
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parent))

import jax
import jax.numpy as jnp
import numpy as np
import scipy

from marklab_worker_support.factor_models import jax_objective, minimize_lbfgs
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


def main() -> None:
    require_versions(jax=jax.__version__, numpy=np.__version__, scipy=scipy.__version__)
    jax.config.update("jax_enable_x64", True)
    request_bytes = read_request(16 * MIB)
    request = json.loads(request_bytes)
    exact(request["format"], "marklab.jax_modality_dropout_request", "request.format")
    exact(request["version"], 1, "request.version")
    rows = request["rows"]
    train_indices = [index for index, row in enumerate(rows) if row["split"] == "train"]
    test_indices = [index for index, row in enumerate(rows) if row["split"] == "test"]
    modality_ids = [modality["id"] for modality in request["design"]["modalities"]]
    raw_views = [
        np.asarray([row["views"][modality]["values"] for row in rows], dtype=float)
        for modality in range(len(modality_ids))
    ]
    means = [view[train_indices].mean(axis=0) for view in raw_views]
    scales = [view[train_indices].std(axis=0, ddof=0) for view in raw_views]
    if any(np.any(scale <= 0.0) or not np.isfinite(scale).all() for scale in scales):
        raise ContractError("every dropout-training feature must vary")
    views = [
        (view - mean) / scale for view, mean, scale in zip(raw_views, means, scales, strict=True)
    ]
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
        for encoder_shape, decoder_shape in zip(encoder_shapes, decoder_shapes, strict=True):
            encoder_size = int(np.prod(encoder_shape))
            decoder_size = int(np.prod(decoder_shape))
            encoders.append(parameters[offset : offset + encoder_size].reshape(encoder_shape))
            offset += encoder_size
            decoders.append(parameters[offset : offset + decoder_size].reshape(decoder_shape))
            offset += decoder_size
        return encoders, decoders

    def latent_for(retained, current_views, encoders):
        return sum(current_views[index] @ encoders[index] for index in retained) / len(retained)

    def objective(parameters):
        """Dropout-weighted reconstruction loss with full/partial latent consistency.

        Each view ``v`` has a linear encoder ``E_v`` and decoder ``D_v``. A retained subset
        ``S`` encodes as the mean of ``X_v E_v`` over ``v in S``; for every declared dropout
        pattern (weighted by its probability) the loss adds the mean squared reconstruction
        error of all views from that latent plus ``consistency_weight`` times its mean squared
        distance to the all-view latent. A ridge penalty ``parameter_precision / 2`` per
        training row applies to all encoder and decoder entries.
        """
        encoders, decoders = unpack(parameters)
        full_latent = latent_for(range(len(training_views)), training_views, encoders)
        loss = 0.0
        for pattern, retained in zip(request["dropout_patterns"], pattern_indices, strict=True):
            partial_latent = latent_for(retained, training_views, encoders)
            reconstruction = sum(
                jnp.mean((partial_latent @ decoder - target) ** 2)
                for decoder, target in zip(decoders, training_views, strict=True)
            )
            consistency = jnp.mean((partial_latent - full_latent) ** 2)
            loss += pattern["probability"] * (
                reconstruction + request["consistency_weight"] * consistency
            )
        penalty = sum(jnp.sum(value * value) for value in [*encoders, *decoders])
        return loss + 0.5 * request["parameter_precision"] * penalty / len(train_indices)

    initial_objective, fit = minimize_lbfgs(
        jax_objective(objective),
        initial,
        maximum_iterations=request["maximum_iterations"],
        ftol=1e-12,
        gtol=1e-7,
        failure="modality-dropout optimization did not converge",
    )
    encoders, decoders = unpack(np.asarray(fit.x))
    test_views = [view[test_indices] for view in views]
    full_test_latent = latent_for(range(len(test_views)), test_views, encoders)
    pattern_validation = []
    for pattern, retained in zip(request["dropout_patterns"], pattern_indices, strict=True):
        partial_latent = latent_for(retained, test_views, encoders)
        missing = [index for index in range(len(modality_ids)) if index not in retained]
        squared_errors = []
        predictions = []
        for modality in missing:
            standardized_prediction = partial_latent @ decoders[modality]
            prediction = standardized_prediction * scales[modality] + means[modality]
            target = raw_views[modality][test_indices]
            squared_errors.extend(((prediction - target) ** 2).ravel().tolist())
            predictions.append(
                {
                    "modality_id": modality_ids[modality],
                    "entity_ids": [rows[index]["entity_id"] for index in test_indices],
                    "posterior_means": prediction.tolist(),
                    "evaluation_targets": target.tolist(),
                }
            )
        consistency_rmse = math.sqrt(float(np.mean((partial_latent - full_test_latent) ** 2)))
        pattern_validation.append(
            {
                "pattern_id": pattern["pattern_id"],
                "retained_modalities": pattern["retained_modalities"],
                "missing_modalities": [modality_ids[index] for index in missing],
                "probability": pattern["probability"],
                "heldout_patient_count": len(test_indices),
                "heldout_missing_view_rmse": math.sqrt(sum(squared_errors) / len(squared_errors)),
                "heldout_latent_consistency_rmse": consistency_rmse,
                "predictions": predictions,
            }
        )
    result = {
        "format": "marklab.modality_robust_inference_model",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(request_bytes),
        "design": request["design"],
        "latent_dimensions": latent_dimensions,
        "required_anchor_modalities": request["required_anchor_modalities"],
        "dropout_patterns": request["dropout_patterns"],
        "standardization": [
            {
                "modality_id": modality_id,
                "means": mean.tolist(),
                "scales": scale.tolist(),
                "fit_split": "train",
            }
            for modality_id, mean, scale in zip(modality_ids, means, scales, strict=True)
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
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "modality dropout worker contract error", errors=CONTRACT_ERRORS)
