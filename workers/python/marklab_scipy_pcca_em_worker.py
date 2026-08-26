#!/usr/bin/env python3

import hashlib
import json
import math
import sys

import numpy as np
import scipy
from scipy import linalg


class ContractError(Exception):
    pass


def log_likelihood(observations, loadings, noise):
    covariance = loadings @ loadings.T + np.diag(noise)
    factor = linalg.cho_factor(covariance, lower=True, check_finite=True)
    solved = linalg.cho_solve(factor, observations.T, check_finite=True).T
    log_determinant = 2.0 * np.log(np.diag(factor[0])).sum()
    return float(
        -0.5
        * (
            observations.shape[0]
            * (observations.shape[1] * math.log(2.0 * math.pi) + log_determinant)
            + np.sum(observations * solved)
        )
    )


def posterior_scores(observations, loadings, noise):
    precision_loadings = loadings / noise[:, None]
    middle = np.eye(loadings.shape[1]) + loadings.T @ precision_loadings
    return observations @ precision_loadings @ linalg.inv(middle)


def correlation_columns(left, right):
    output = []
    for column in range(left.shape[1]):
        value = float(np.corrcoef(left[:, column], right[:, column])[0, 1])
        output.append(value)
    return output


def main():
    if np.__version__ != "2.4.6" or scipy.__version__ != "1.18.1":
        raise ContractError("NumPy or SciPy version drift")
    if sys.version_info[:2] != (3, 12):
        raise ContractError("Python version drift")
    request_bytes = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.scipy_pcca_em_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    design = request["design"]
    if (
        design["entity_level"] != "patient"
        or design["modality_x"]["id"] == design["modality_y"]["id"]
        or design["modality_x"]["measurement_status"] != "measured"
        or design["modality_y"]["measurement_status"] != "measured"
        or design["modality_x"]["likelihood"] != "gaussian"
        or design["modality_y"]["likelihood"] != "gaussian"
        or design["missingness_assumption"] != "complete_paired_rows"
    ):
        raise ContractError("pCCA design is not paired measured patient-level Gaussian data")
    rows = request["rows"]
    x = np.asarray([row["x"] for row in rows], dtype=float)
    y = np.asarray([row["y"] for row in rows], dtype=float)
    train = np.asarray([row["split"] == "train" for row in rows], dtype=bool)
    test = np.asarray([row["split"] == "test" for row in rows], dtype=bool)
    if train.sum() < 8 or test.sum() < 1 or not np.isfinite(x).all() or not np.isfinite(y).all():
        raise ContractError("pCCA rows require finite train and held-out data")
    means_x = x[train].mean(axis=0)
    means_y = y[train].mean(axis=0)
    scales_x = x[train].std(axis=0, ddof=0)
    scales_y = y[train].std(axis=0, ddof=0)
    if np.any(scales_x <= 0.0) or np.any(scales_y <= 0.0):
        raise ContractError("training features must vary")
    standardized_x = (x - means_x) / scales_x
    standardized_y = (y - means_y) / scales_y
    observations = np.concatenate([standardized_x[train], standardized_y[train]], axis=1)
    dx = x.shape[1]
    latent = request["latent_dimensions"]
    cross = standardized_x[train].T @ standardized_y[train] / train.sum()
    left, _, right_t = linalg.svd(cross, full_matrices=False)
    loadings = 0.5 * np.vstack([left[:, :latent], right_t.T[:, :latent]])
    noise = np.full(observations.shape[1], 0.75, dtype=float)
    trace = []
    converged = False
    monotone_violations = 0
    for iteration in range(request["maximum_iterations"]):
        scores = posterior_scores(observations, loadings, noise)
        precision_loadings = loadings / noise[:, None]
        posterior_covariance = linalg.inv(np.eye(latent) + loadings.T @ precision_loadings)
        sum_cross = observations.T @ scores
        sum_second = train.sum() * posterior_covariance + scores.T @ scores
        updated_loadings = sum_cross @ linalg.inv(
            sum_second + request["regularization"] * np.eye(latent)
        )
        residual = (
            observations.T @ observations
            - updated_loadings @ sum_cross.T
            - sum_cross @ updated_loadings.T
            + updated_loadings @ sum_second @ updated_loadings.T
        ) / train.sum()
        updated_noise = np.maximum(np.diag(residual), request["noise_floor"])
        value = log_likelihood(observations, updated_loadings, updated_noise)
        if trace and value < trace[-1] - 1e-7:
            monotone_violations += 1
        trace.append(value)
        change = max(
            float(np.max(np.abs(updated_loadings - loadings))),
            float(np.max(np.abs(updated_noise - noise))),
        )
        loadings = updated_loadings
        noise = updated_noise
        if change <= request["convergence_tolerance"]:
            converged = True
            break
    for column in range(latent):
        pivot = int(np.argmax(np.abs(loadings[:dx, column])))
        if loadings[pivot, column] < 0.0:
            loadings[:, column] *= -1.0
    loadings_x = loadings[:dx]
    loadings_y = loadings[dx:]
    noise_x = noise[:dx]
    noise_y = noise[dx:]
    scores_x = posterior_scores(standardized_x, loadings_x, noise_x)
    scores_y = posterior_scores(standardized_y, loadings_y, noise_y)
    joint_scores = posterior_scores(
        np.concatenate([standardized_x, standardized_y], axis=1), loadings, noise
    )
    prediction_y_standardized = scores_x @ loadings_y.T
    prediction_y = prediction_y_standardized * scales_y + means_y
    heldout_rmse = float(np.sqrt(np.mean((prediction_y[test] - y[test]) ** 2)))
    result = {
        "format": "marklab.probabilistic_cca",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "design": {**design, "validation_status": "passed", "paired_row_count": len(rows)},
        "standardization": {
            "fit_split": "train_only",
            "fit_row_count": int(train.sum()),
            "mean_x": means_x.tolist(),
            "scale_x": scales_x.tolist(),
            "mean_y": means_y.tolist(),
            "scale_y": scales_y.tolist(),
        },
        "parameters": {
            "loadings_x": loadings_x.tolist(),
            "loadings_y": loadings_y.tolist(),
            "noise_diagonal_x": noise_x.tolist(),
            "noise_diagonal_y": noise_y.tolist(),
        },
        "posterior_scores": [
            {"entity_id": row["entity_id"], "split": row["split"], "mean": score.tolist()}
            for row, score in zip(rows, joint_scores)
        ],
        "canonical_correlations": correlation_columns(scores_x[train], scores_y[train]),
        "heldout_row_count": int(test.sum()),
        "heldout_cross_view_rmse_y_from_x": heldout_rmse,
        "diagnostics": {
            "converged": converged,
            "iterations": len(trace),
            "log_likelihood_trace": trace,
            "monotone_violations": monotone_violations,
            "noise_floor": request["noise_floor"],
            "regularization": request["regularization"],
        },
        "claim_status": "experimental_synthetic_paired_gaussian_pcca",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError, linalg.LinAlgError) as error:
        print(f"pCCA worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
