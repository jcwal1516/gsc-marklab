#!/usr/bin/env python3
"""SciPy worker for a graph-smoothed Bayesian matrix factorization (Laplace approximation)."""

import json
import math
import sys
from pathlib import Path
from typing import Any

import numpy as np
import scipy

sys.path.append(str(Path(__file__).resolve().parent))

from marklab_worker_support.factor_models import (
    align_factor_signs,
    delta_method_variance,
    inverse_hessian_covariance,
    minimize_lbfgs,
    observed_feature_standardization,
)
from marklab_worker_support.protocol import (
    CONTRACT_ERRORS,
    MIB,
    ContractError,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    write_result,
)

ALTERNATING_SWEEPS = 40
ALTERNATING_TOLERANCE = 1e-8


def graph_precision(request: dict[str, Any], rows: list[dict[str, Any]]) -> np.ndarray:
    """``spatial_precision * (L + epsilon * I)`` for the weighted graph Laplacian ``L``."""
    n = len(rows)
    entity_index = {row["entity_id"]: index for index, row in enumerate(rows)}
    adjacency = np.zeros((n, n), dtype=float)
    for edge in request["graph"]["edges"]:
        left = entity_index[edge["left"]]
        right = entity_index[edge["right"]]
        adjacency[left, right] += edge["weight"]
        adjacency[right, left] += edge["weight"]
    laplacian = np.diag(adjacency.sum(axis=1)) - adjacency
    return request["spatial_precision"] * (laplacian + request["diagonal_epsilon"] * np.eye(n))


def alternating_start(
    request: dict[str, Any],
    standardized: np.ndarray,
    observed: np.ndarray,
    precision: np.ndarray,
    noise_variance: float,
) -> np.ndarray:
    """Starting point for the MAP fit: a rank-``k`` SVD refined by alternating solves.

    Each sweep solves the entity factors jointly under the graph precision with the loadings
    fixed, then each feature's loadings under their ridge prior; it stops after 40 sweeps or
    once no parameter moves by more than 1e-8.
    """
    n, p = standardized.shape
    k = request["factors"]
    training = np.where(observed, standardized, 0.0)
    left, singular, right_t = np.linalg.svd(training, full_matrices=False)
    root = np.sqrt(np.maximum(singular[:k], 1e-6))
    factors = left[:, :k] * root
    loadings = right_t[:k, :].T * root
    for _ in range(ALTERNATING_SWEEPS):
        system_u = np.zeros((n * k, n * k), dtype=float)
        target_u = np.zeros(n * k, dtype=float)
        for row in range(n):
            features = observed[row]
            row_slice = slice(row * k, (row + 1) * k)
            system_u[row_slice, row_slice] += (
                loadings[features].T @ loadings[features] / noise_variance
            )
            target_u[row_slice] += (
                loadings[features].T @ standardized[row, features] / noise_variance
            )
        for left_row in range(n):
            for right_row in range(n):
                if precision[left_row, right_row] == 0.0:
                    continue
                for factor in range(k):
                    system_u[left_row * k + factor, right_row * k + factor] += precision[
                        left_row, right_row
                    ]
        updated_factors = np.linalg.solve(system_u, target_u).reshape(n, k)
        updated_loadings = np.empty_like(loadings)
        for feature in range(p):
            entities = observed[:, feature]
            system_v = updated_factors[entities].T @ updated_factors[
                entities
            ] / noise_variance + request["loading_precision"] * np.eye(k)
            target_v = (
                updated_factors[entities].T @ standardized[entities, feature] / noise_variance
            )
            updated_loadings[feature] = np.linalg.solve(system_v, target_v)
        change = max(
            np.max(np.abs(updated_factors - factors)),
            np.max(np.abs(updated_loadings - loadings)),
        )
        factors, loadings = updated_factors, updated_loadings
        if change < ALTERNATING_TOLERANCE:
            break
    return np.concatenate([factors.ravel(), loadings.ravel()])


def main() -> None:
    """Fit the spatial factor model and predict the masked entries.

    Model: observed standardized entries ``y_rf ~ Normal(u_r . v_f, noise_sd)`` with entity
    factors ``u`` under the Gaussian graph prior ``exp(-tr(U' Q U) / 2)`` (``Q`` from
    ``graph_precision``) and loadings ``v_f ~ Normal(0, I / loading_precision)``. The MAP is
    found by L-BFGS-B; curvature of the fitted log posterior supplies the Laplace covariance
    for delta-method predictive SDs on the identified subspace.
    """
    require_versions(numpy=np.__version__, scipy=scipy.__version__)
    request_bytes = read_request(16 * MIB)
    request = json.loads(request_bytes)
    if (
        request["format"] != "marklab.scipy_spatial_matrix_factor_request"
        or request["version"] != 1
    ):
        raise ContractError("request identity mismatch")
    rows = request["rows"]
    values = np.asarray([row["values"] for row in rows], dtype=float)
    observed = np.asarray([row["observed"] for row in rows], dtype=bool)
    n, p = values.shape
    k = request["factors"]
    means, scales = observed_feature_standardization(values, observed, "spatial matrix")
    standardized = (values - means) / scales
    precision = graph_precision(request, rows)
    noise_variance = request["noise_standard_deviation"] ** 2
    loading_precision = request["loading_precision"]
    initial = alternating_start(request, standardized, observed, precision, noise_variance)

    def unpack(parameters: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
        return parameters[: n * k].reshape(n, k), parameters[n * k :].reshape(p, k)

    def objective_and_gradient(parameters: np.ndarray) -> tuple[float, np.ndarray]:
        entity_factors, loadings = unpack(parameters)
        residual = np.where(observed, entity_factors @ loadings.T - standardized, 0.0)
        objective = (
            0.5 * np.sum(residual * residual) / noise_variance
            + 0.5 * np.sum(entity_factors * (precision @ entity_factors))
            + 0.5 * loading_precision * np.sum(loadings * loadings)
        )
        gradient_u = residual @ loadings / noise_variance + precision @ entity_factors
        gradient_v = residual.T @ entity_factors / noise_variance + loading_precision * loadings
        return float(objective), np.concatenate([gradient_u.ravel(), gradient_v.ravel()])

    initial_objective, fit = minimize_lbfgs(
        objective_and_gradient,
        initial,
        maximum_iterations=request["maximum_iterations"],
        ftol=1e-9,
        gtol=1e-5,
        failure="spatial Laplace MAP did not converge",
    )
    entity_factors, loadings = unpack(fit.x)
    rotations = []
    for left in range(k):
        for right in range(left + 1, k):
            generator = np.zeros((k, k))
            generator[left, right], generator[right, left] = 1.0, -1.0
            rotations.append(
                np.concatenate(
                    [(entity_factors @ generator).ravel(), (loadings @ generator).ravel()]
                )
            )
    covariance = inverse_hessian_covariance(
        fit,
        "posterior curvature is unavailable",
        objective_and_gradient,
        np.asarray(rotations).T if rotations else None,
    )
    masked_predictions = []
    errors = []
    for row in range(n):
        for feature in range(p):
            if observed[row, feature]:
                continue
            mean_standardized = float(entity_factors[row] @ loadings[feature])
            gradient = np.zeros(len(fit.x), dtype=float)
            gradient[row * k : (row + 1) * k] = loadings[feature]
            loading_start = n * k + feature * k
            gradient[loading_start : loading_start + k] = entity_factors[row]
            variance_standardized = delta_method_variance(gradient, covariance, noise_variance)
            prediction = mean_standardized * scales[feature] + means[feature]
            target = values[row, feature]
            errors.append((prediction - target) ** 2)
            masked_predictions.append(
                {
                    "entity_id": rows[row]["entity_id"],
                    "feature_name": request["feature_names"][feature],
                    "posterior_mean": prediction,
                    "posterior_standard_deviation": (
                        math.sqrt(variance_standardized) * scales[feature]
                    ),
                    "evaluation_target": target,
                    "evaluation_role": "masked_before_fit",
                }
            )
    factor_variance = np.var(entity_factors, axis=0) * np.sum(loadings * loadings, axis=0)
    order = np.argsort(-factor_variance, kind="stable")
    entity_factors = entity_factors[:, order]
    loadings = loadings[:, order]
    factor_variance = factor_variance[order]
    sign_flips = align_factor_signs(loadings, entity_factors)
    result = {
        "format": "marklab.spatial_bayesian_matrix_factorization",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(request_bytes),
        "matrix_id": request["matrix_id"],
        "entity_level": request["entity_level"],
        "likelihood": "gaussian",
        "graph": request["graph"],
        "spatial_prior": {
            "operator": "graph_laplacian_plus_diagonal",
            "spatial_precision": request["spatial_precision"],
            "diagonal_epsilon": request["diagonal_epsilon"],
            "loading_precision": loading_precision,
        },
        "standardization": {
            "fit_entries": "observed_only",
            "means": means.tolist(),
            "scales": scales.tolist(),
        },
        "entity_factor_map": entity_factors.tolist(),
        "feature_loading_map": loadings.tolist(),
        "factor_variance_scores": factor_variance.tolist(),
        "alignment": {
            "order": "descending_factor_variance",
            "sign": "max_loading_positive",
            "sign_flips": sign_flips,
        },
        "masked_predictions": masked_predictions,
        "masked_rmse": math.sqrt(sum(errors) / len(errors)),
        "diagnostics": {
            "objective_initial": initial_objective,
            "objective_final": float(fit.fun),
            "iterations": int(fit.nit),
            "gradient_max_absolute": float(np.max(np.abs(fit.jac))),
            "covariance": "posterior_curvature_identified_subspace",
        },
        "fit_state": "approximate_only",
        "claim_status": "experimental_synthetic_spatial_matrix_factorization",
    }
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "spatial matrix factor worker contract error", errors=CONTRACT_ERRORS)
