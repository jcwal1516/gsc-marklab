#!/usr/bin/env python3
"""Bayesian matrix factorization of one entity-by-feature matrix with MOFA+ (mofapy2)."""

import json
import math
import sys
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parent))

import h5py
import mofapy2
import numpy as np
import scipy

from marklab_worker_support.factor_models import align_factor_signs
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


def observed_standardization(values, observed):
    means = np.empty(values.shape[1])
    scales = np.empty(values.shape[1])
    for feature in range(values.shape[1]):
        observed_values = values[:, feature][observed[:, feature]]
        if len(observed_values) < 8:
            raise ContractError("every matrix feature requires eight observed entries")
        means[feature] = observed_values.mean()
        scales[feature] = observed_values.std(ddof=0)
        if scales[feature] <= 0.0 or not math.isfinite(scales[feature]):
            raise ContractError("every matrix feature must vary")
    return means, scales


def masked_entry_predictions(request, rows, values, observed, means, scales, moments):
    """Posterior mean and SD of every masked entry from the factor and loading moments."""
    scores, scores_second, weights, weights_second, precision = moments
    predictions = []
    errors = []
    for row in range(values.shape[0]):
        for feature in range(values.shape[1]):
            if observed[row, feature]:
                continue
            prediction_standardized = float(scores[row] @ weights[feature])
            prediction = prediction_standardized * scales[feature] + means[feature]
            second_moment = scores_second[row] * weights_second[feature]
            latent_variance = float(np.sum(second_moment - (scores[row] * weights[feature]) ** 2))
            noise_variance = 1.0 / max(precision[feature], 1e-12)
            predictive_variance = max(latent_variance + noise_variance, 0.0) * scales[feature] ** 2
            target = values[row, feature]
            errors.append((prediction - target) ** 2)
            predictions.append(
                {
                    "entity_id": rows[row]["entity_id"],
                    "feature_name": request["feature_names"][feature],
                    "posterior_mean": prediction,
                    "posterior_standard_deviation": math.sqrt(predictive_variance),
                    "evaluation_target": target,
                    "evaluation_role": "masked_before_fit",
                }
            )
    return predictions, errors


def main() -> None:
    require_versions(
        mofapy2=mofapy2.__version__,
        h5py=h5py.__version__,
        numpy=np.__version__,
        scipy=scipy.__version__,
    )
    request_bytes = read_request(16 * MIB)
    request = json.loads(request_bytes)
    exact(request["format"], "marklab.mofapy2_matrix_factor_request", "request.format")
    exact(request["version"], 1, "request.version")
    rows = request["rows"]
    values = np.asarray([row["values"] for row in rows], dtype=float)
    observed = np.asarray([row["observed"] for row in rows], dtype=bool)
    means, scales = observed_standardization(values, observed)
    standardized = (values - means) / scales
    training = standardized.copy()
    training[~observed] = np.nan
    model, backend_log = train_mofa(
        [training],
        view_names=[request["matrix_id"]],
        group_name="all",
        sample_names=[row["entity_id"] for row in rows],
        feature_names=[request["feature_names"]],
        factors=request["factors"],
        iterations=request["iterations"],
        convergence_mode=request["convergence_mode"],
        seed=request["seed"],
    )
    expectations = model.model.getExpectations()
    scores = np.asarray(expectations["Z"]["E"], dtype=float)
    scores_second = np.asarray(expectations["Z"]["E2"], dtype=float)
    weights = np.asarray(expectations["W"][0]["E"], dtype=float)
    weights_second = np.asarray(expectations["W"][0]["E2"], dtype=float)
    precision = np.nanmean(np.asarray(expectations["Tau"][0]["E"], dtype=float), axis=0)
    variance_explained = np.asarray(model.model.calculate_variance_explained()[0][0], dtype=float)
    order = np.argsort(-variance_explained, kind="stable")
    scores = scores[:, order]
    scores_second = scores_second[:, order]
    weights = weights[:, order]
    weights_second = weights_second[:, order]
    variance_explained = variance_explained[order]
    sign_flips = align_factor_signs(weights, scores)
    masked_predictions, errors = masked_entry_predictions(
        request,
        rows,
        values,
        observed,
        means,
        scales,
        (scores, scores_second, weights, weights_second, precision),
    )
    if not masked_predictions:
        raise ContractError("matrix factor workflow requires masked evaluation entries")
    elbo_raw = np.asarray(model.model.getTrainingStats()["elbo"], dtype=float)
    elbo = elbo_raw[np.isfinite(elbo_raw)]
    if len(elbo) < 2 or not np.isfinite(scores).all() or not np.isfinite(weights).all():
        raise ContractError("matrix factor fit is incomplete or non-finite")
    active = [index for index, value in enumerate(variance_explained) if value > 1e-4]
    result = {
        "format": "marklab.bayesian_matrix_factorization",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(request_bytes),
        "matrix_id": request["matrix_id"],
        "entity_level": request["entity_level"],
        "likelihood": "gaussian",
        "standardization": {
            "fit_entries": "observed_only",
            "means": means.tolist(),
            "scales": scales.tolist(),
        },
        "entity_factor_means": scores.tolist(),
        "entity_factor_second_moments": scores_second.tolist(),
        "feature_loading_means": weights.tolist(),
        "feature_loading_second_moments": weights_second.tolist(),
        "variance_explained_by_factor": variance_explained.tolist(),
        "active_factor_indices": active,
        "active_factor_count": len(active),
        "alignment": {
            "order": "descending_variance_explained",
            "sign": "max_loading_positive",
            "sign_flips": sign_flips,
        },
        "masked_predictions": masked_predictions,
        "masked_rmse": math.sqrt(sum(errors) / len(errors)),
        "diagnostics": {
            "elbo_first": float(elbo[0]),
            "elbo_last": float(elbo[-1]),
            "elbo_trace": elbo.tolist(),
            "trained": bool(model.model.trained),
            "captured_backend_log_lines": len(backend_log.splitlines()),
        },
        "claim_status": "experimental_synthetic_bayesian_matrix_factorization",
    }
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "matrix factor worker contract error", errors=CONTRACT_ERRORS)
