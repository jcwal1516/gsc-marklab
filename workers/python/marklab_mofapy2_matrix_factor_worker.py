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
        raise ContractError("matrix factor backend version drift")
    request_bytes = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.mofapy2_matrix_factor_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    rows = request["rows"]
    values = np.asarray([row["values"] for row in rows], dtype=float)
    observed = np.asarray([row["observed"] for row in rows], dtype=bool)
    means = np.empty(values.shape[1])
    scales = np.empty(values.shape[1])
    for feature in range(values.shape[1]):
        admitted = values[:, feature][observed[:, feature]]
        if len(admitted) < 8:
            raise ContractError("every matrix feature requires eight observed entries")
        means[feature] = admitted.mean()
        scales[feature] = admitted.std(ddof=0)
        if scales[feature] <= 0.0 or not math.isfinite(scales[feature]):
            raise ContractError("every matrix feature must vary")
    standardized = (values - means) / scales
    training = standardized.copy()
    training[~observed] = np.nan
    captured = io.StringIO()
    with contextlib.redirect_stdout(captured):
        model = entry_point()
        model.set_data_options(scale_views=False, scale_groups=False, center_groups=False, use_float32=False)
        model.set_data_matrix(
            [[training]],
            likelihoods=["gaussian"],
            views_names=[request["matrix_id"]],
            groups_names=["all"],
            samples_names=[[row["entity_id"] for row in rows]],
            features_names=[request["feature_names"]],
        )
        model.set_model_options(
            factors=request["factors"],
            spikeslab_factors=False,
            spikeslab_weights=False,
            ard_factors=True,
            ard_weights=True,
        )
        model.set_train_options(
            iter=request["iterations"], startELBO=1, freqELBO=1,
            convergence_mode=request["convergence_mode"], startDrop=1, freqDrop=1,
            dropR2=None, nostop=False, quiet=True, seed=request["seed"], gpu_mode=False,
        )
        model.build()
        model.run()
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
    sign_flips = 0
    for factor in range(scores.shape[1]):
        pivot = int(np.argmax(np.abs(weights[:, factor])))
        if weights[pivot, factor] < 0.0:
            weights[:, factor] *= -1.0
            scores[:, factor] *= -1.0
            sign_flips += 1
    masked_predictions = []
    errors = []
    for row in range(values.shape[0]):
        for feature in range(values.shape[1]):
            if observed[row, feature]:
                continue
            prediction_standardized = float(scores[row] @ weights[feature])
            prediction = prediction_standardized * scales[feature] + means[feature]
            latent_variance = float(
                np.sum(scores_second[row] * weights_second[feature] - (scores[row] * weights[feature]) ** 2)
            )
            predictive_variance = max(latent_variance + 1.0 / max(precision[feature], 1e-12), 0.0) * scales[feature] ** 2
            target = values[row, feature]
            errors.append((prediction - target) ** 2)
            masked_predictions.append({
                "entity_id": rows[row]["entity_id"],
                "feature_name": request["feature_names"][feature],
                "posterior_mean": prediction,
                "posterior_standard_deviation": math.sqrt(predictive_variance),
                "evaluation_target": target,
                "evaluation_role": "masked_before_fit",
            })
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
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "matrix_id": request["matrix_id"],
        "entity_level": request["entity_level"],
        "likelihood": "gaussian",
        "standardization": {"fit_entries": "observed_only", "means": means.tolist(), "scales": scales.tolist()},
        "entity_factor_means": scores.tolist(),
        "entity_factor_second_moments": scores_second.tolist(),
        "feature_loading_means": weights.tolist(),
        "feature_loading_second_moments": weights_second.tolist(),
        "variance_explained_by_factor": variance_explained.tolist(),
        "active_factor_indices": active,
        "active_factor_count": len(active),
        "alignment": {"order": "descending_variance_explained", "sign": "max_loading_positive", "sign_flips": sign_flips},
        "masked_predictions": masked_predictions,
        "masked_rmse": math.sqrt(sum(errors) / len(errors)),
        "diagnostics": {"elbo_first": float(elbo[0]), "elbo_last": float(elbo[-1]), "elbo_trace": elbo.tolist(), "trained": bool(model.model.trained), "captured_backend_log_lines": len(captured.getvalue().splitlines())},
        "claim_status": "experimental_synthetic_bayesian_matrix_factorization",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError, np.linalg.LinAlgError) as error:
        print(f"matrix factor worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
