#!/usr/bin/env python3
"""Probabilistic CCA of two paired Gaussian views by penalized expectation-maximization."""

import json
import math
import sys
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parent))

import numpy as np
import scipy
from scipy import linalg

from marklab_worker_support.factor_models import standardize_paired_rows
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


def log_likelihood(observations, loadings, noise):
    """Gaussian log-likelihood of the rows under covariance ``W W' + diag(noise)``."""
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
    """Posterior mean of the latent scores given loadings and diagonal noise variances."""
    precision_loadings = loadings / noise[:, None]
    middle = np.eye(loadings.shape[1]) + loadings.T @ precision_loadings
    return observations @ precision_loadings @ linalg.inv(middle)


def canonical_correlations(left, right):
    centered_left = left - left.mean(axis=0)
    centered_right = right - right.mean(axis=0)
    denominator = left.shape[0] - 1
    covariance_left = centered_left.T @ centered_left / denominator
    covariance_right = centered_right.T @ centered_right / denominator
    cross_covariance = centered_left.T @ centered_right / denominator

    def inverse_square_root(covariance):
        eigenvalues, eigenvectors = linalg.eigh(covariance, check_finite=True)
        spectral_scale = max(float(np.max(eigenvalues)), 0.0)
        tolerance = np.finfo(np.float64).eps * max(covariance.shape) * spectral_scale
        retained = eigenvalues > tolerance
        inverse_roots = np.zeros_like(eigenvalues)
        inverse_roots[retained] = 1.0 / np.sqrt(eigenvalues[retained])
        return (eigenvectors * inverse_roots) @ eigenvectors.T

    whitened = (
        inverse_square_root(covariance_left)
        @ cross_covariance
        @ inverse_square_root(covariance_right)
    )
    return np.clip(linalg.svdvals(whitened, check_finite=True), 0.0, 1.0).tolist()


def fit_pcca_em(observations, loadings, noise, request):
    """Fit probabilistic CCA (a two-block factor analysis) by expectation-maximization.

    Model for a standardized paired row ``x = [x_view_1, x_view_2]``: ``z ~ Normal(0, I_k)``
    and ``x | z ~ Normal(W z, diag(noise))``, so ``x ~ Normal(0, W W' + diag(noise))``. The
    M-step adds ``regularization * I`` to the latent second moment (a ridge penalty on ``W``)
    and floors each noise variance at ``noise_floor``. Iterates until the largest parameter
    change is at most ``convergence_tolerance`` or ``maximum_iterations`` is reached.
    """
    latent = loadings.shape[1]
    row_count = observations.shape[0]
    trace = []
    converged = False
    monotone_violations = 0
    for _ in range(request["maximum_iterations"]):
        scores = posterior_scores(observations, loadings, noise)
        precision_loadings = loadings / noise[:, None]
        posterior_covariance = linalg.inv(np.eye(latent) + loadings.T @ precision_loadings)
        sum_cross = observations.T @ scores
        sum_second = row_count * posterior_covariance + scores.T @ scores
        updated_loadings = sum_cross @ linalg.inv(
            sum_second + request["regularization"] * np.eye(latent)
        )
        residual = (
            observations.T @ observations
            - updated_loadings @ sum_cross.T
            - sum_cross @ updated_loadings.T
            + updated_loadings @ sum_second @ updated_loadings.T
        ) / row_count
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
    return loadings, noise, trace, converged, monotone_violations


def main() -> None:
    require_versions(numpy=np.__version__, scipy=scipy.__version__)
    request_bytes = read_request(16 * MIB)
    request = json.loads(request_bytes)
    exact(request["format"], "marklab.scipy_pcca_em_request", "request.format")
    exact(request["version"], 1, "request.version")
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
    paired = standardize_paired_rows(rows)
    train = paired.train
    test = paired.test
    standardized_x = paired.standardized_x
    standardized_y = paired.standardized_y
    observations = np.concatenate([standardized_x[train], standardized_y[train]], axis=1)
    dx = paired.x.shape[1]
    latent = request["latent_dimensions"]

    # Initialize the loadings from the leading singular vectors of the training
    # cross-covariance and every noise variance at 0.75.
    cross = standardized_x[train].T @ standardized_y[train] / train.sum()
    left, _, right_t = linalg.svd(cross, full_matrices=False)
    initial_loadings = 0.5 * np.vstack([left[:, :latent], right_t.T[:, :latent]])
    initial_noise = np.full(observations.shape[1], 0.75, dtype=float)
    loadings, noise, trace, converged, monotone_violations = fit_pcca_em(
        observations, initial_loadings, initial_noise, request
    )

    # Make the largest-magnitude first-view loading of every factor positive.
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
    prediction_y = prediction_y_standardized * paired.scales_y + paired.means_y
    heldout_rmse = float(np.sqrt(np.mean((prediction_y[test] - paired.y[test]) ** 2)))
    result = {
        "format": "marklab.probabilistic_cca",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(request_bytes),
        "design": {**design, "validation_status": "passed", "paired_row_count": len(rows)},
        "standardization": paired.standardization_record(),
        "parameters": {
            "loadings_x": loadings_x.tolist(),
            "loadings_y": loadings_y.tolist(),
            "noise_diagonal_x": noise_x.tolist(),
            "noise_diagonal_y": noise_y.tolist(),
        },
        "posterior_scores": [
            {"entity_id": row["entity_id"], "split": row["split"], "mean": score.tolist()}
            for row, score in zip(rows, joint_scores, strict=True)
        ],
        "canonical_correlations": canonical_correlations(scores_x[train], scores_y[train]),
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
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "pCCA worker contract error", errors=CONTRACT_ERRORS)
