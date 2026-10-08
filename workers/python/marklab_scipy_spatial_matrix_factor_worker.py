#!/usr/bin/env python3

import hashlib
import json
import math
import sys

import numpy as np
import scipy
from scipy.optimize import minimize


class ContractError(Exception):
    pass


def main():
    if np.__version__ != "2.4.6" or scipy.__version__ != "1.18.1" or sys.version_info[:2] != (3, 12):
        raise ContractError("spatial matrix backend version drift")
    request_bytes = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.scipy_spatial_matrix_factor_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    rows = request["rows"]
    values = np.asarray([row["values"] for row in rows], dtype=float)
    observed = np.asarray([row["observed"] for row in rows], dtype=bool)
    n, p = values.shape
    k = request["factors"]
    means = np.asarray([values[:, feature][observed[:, feature]].mean() for feature in range(p)])
    scales = np.asarray([values[:, feature][observed[:, feature]].std(ddof=0) for feature in range(p)])
    if not np.isfinite(scales).all() or np.any(scales <= 0.0):
        raise ContractError("every spatial matrix feature must vary")
    standardized = (values - means) / scales
    training = np.where(observed, standardized, 0.0)
    entity_index = {row["entity_id"]: index for index, row in enumerate(rows)}
    adjacency = np.zeros((n, n), dtype=float)
    for edge in request["graph"]["edges"]:
        left = entity_index[edge["left"]]
        right = entity_index[edge["right"]]
        adjacency[left, right] += edge["weight"]
        adjacency[right, left] += edge["weight"]
    laplacian = np.diag(adjacency.sum(axis=1)) - adjacency
    precision = request["spatial_precision"] * (
        laplacian + request["diagonal_epsilon"] * np.eye(n)
    )
    noise_variance = request["noise_standard_deviation"] ** 2
    left, singular, right_t = np.linalg.svd(training, full_matrices=False)
    root = np.sqrt(np.maximum(singular[:k], 1e-6))
    initial_u = left[:, :k] * root
    initial_v = right_t[:k, :].T * root
    for _ in range(40):
        system_u = np.zeros((n * k, n * k), dtype=float)
        target_u = np.zeros(n * k, dtype=float)
        for row in range(n):
            admitted = observed[row]
            row_slice = slice(row * k, (row + 1) * k)
            system_u[row_slice, row_slice] += initial_v[admitted].T @ initial_v[admitted] / noise_variance
            target_u[row_slice] += initial_v[admitted].T @ standardized[row, admitted] / noise_variance
        for left_row in range(n):
            for right_row in range(n):
                if precision[left_row, right_row] == 0.0:
                    continue
                for factor in range(k):
                    system_u[left_row * k + factor, right_row * k + factor] += precision[left_row, right_row]
        updated_u = np.linalg.solve(system_u, target_u).reshape(n, k)
        updated_v = np.empty_like(initial_v)
        for feature in range(p):
            admitted = observed[:, feature]
            system_v = (
                updated_u[admitted].T @ updated_u[admitted] / noise_variance
                + request["loading_precision"] * np.eye(k)
            )
            target_v = updated_u[admitted].T @ standardized[admitted, feature] / noise_variance
            updated_v[feature] = np.linalg.solve(system_v, target_v)
        change = max(np.max(np.abs(updated_u - initial_u)), np.max(np.abs(updated_v - initial_v)))
        initial_u, initial_v = updated_u, updated_v
        if change < 1e-8:
            break
    initial = np.concatenate([initial_u.ravel(), initial_v.ravel()])

    def unpack(parameters):
        return parameters[: n * k].reshape(n, k), parameters[n * k :].reshape(p, k)

    def objective_and_gradient(parameters):
        entity_factors, loadings = unpack(parameters)
        residual = np.where(observed, entity_factors @ loadings.T - standardized, 0.0)
        objective = (
            0.5 * np.sum(residual * residual) / noise_variance
            + 0.5 * np.sum(entity_factors * (precision @ entity_factors))
            + 0.5 * request["loading_precision"] * np.sum(loadings * loadings)
        )
        gradient_u = residual @ loadings / noise_variance + precision @ entity_factors
        gradient_v = residual.T @ entity_factors / noise_variance + request["loading_precision"] * loadings
        return float(objective), np.concatenate([gradient_u.ravel(), gradient_v.ravel()])

    initial_objective = objective_and_gradient(initial)[0]
    fit = minimize(
        objective_and_gradient,
        initial,
        method="L-BFGS-B",
        jac=True,
        options={"maxiter": request["maximum_iterations"], "ftol": 1e-9, "gtol": 1e-5},
    )
    if not fit.success or not np.isfinite(fit.x).all():
        raise ContractError(f"spatial Laplace MAP did not converge: {fit.message}")
    entity_factors, loadings = unpack(fit.x)
    covariance = np.asarray(fit.hess_inv.todense(), dtype=float)
    if covariance.shape != (len(fit.x), len(fit.x)) or not np.isfinite(covariance).all():
        raise ContractError("L-BFGS posterior covariance approximation is unavailable")
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
            variance_standardized = max(float(gradient @ covariance @ gradient) + noise_variance, 1e-12)
            prediction = mean_standardized * scales[feature] + means[feature]
            target = values[row, feature]
            errors.append((prediction - target) ** 2)
            masked_predictions.append({
                "entity_id": rows[row]["entity_id"],
                "feature_name": request["feature_names"][feature],
                "posterior_mean": prediction,
                "posterior_standard_deviation": math.sqrt(variance_standardized) * scales[feature],
                "evaluation_target": target,
                "evaluation_role": "masked_before_fit",
            })
    factor_variance = np.var(entity_factors, axis=0) * np.sum(loadings * loadings, axis=0)
    order = np.argsort(-factor_variance, kind="stable")
    entity_factors = entity_factors[:, order]
    loadings = loadings[:, order]
    factor_variance = factor_variance[order]
    sign_flips = 0
    for factor in range(k):
        pivot = int(np.argmax(np.abs(loadings[:, factor])))
        if loadings[pivot, factor] < 0.0:
            entity_factors[:, factor] *= -1.0
            loadings[:, factor] *= -1.0
            sign_flips += 1
    result = {
        "format": "marklab.spatial_bayesian_matrix_factorization",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "matrix_id": request["matrix_id"],
        "entity_level": request["entity_level"],
        "likelihood": "gaussian",
        "graph": request["graph"],
        "spatial_prior": {
            "operator": "graph_laplacian_plus_diagonal",
            "spatial_precision": request["spatial_precision"],
            "diagonal_epsilon": request["diagonal_epsilon"],
            "loading_precision": request["loading_precision"],
        },
        "standardization": {"fit_entries": "observed_only", "means": means.tolist(), "scales": scales.tolist()},
        "entity_factor_map": entity_factors.tolist(),
        "feature_loading_map": loadings.tolist(),
        "factor_variance_scores": factor_variance.tolist(),
        "alignment": {"order": "descending_factor_variance", "sign": "max_loading_positive", "sign_flips": sign_flips},
        "masked_predictions": masked_predictions,
        "masked_rmse": math.sqrt(sum(errors) / len(errors)),
        "diagnostics": {
            "objective_initial": initial_objective,
            "objective_final": float(fit.fun),
            "iterations": int(fit.nit),
            "gradient_max_absolute": float(np.max(np.abs(fit.jac))),
            "covariance": "limited_memory_inverse_hessian",
        },
        "fit_state": "approximate_only",
        "claim_status": "experimental_synthetic_spatial_matrix_factorization",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError, np.linalg.LinAlgError) as error:
        print(f"spatial matrix factor worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
