#!/usr/bin/env python3

import hashlib
import json
import math
import sys

import numpy as np
import scipy


class ContractError(Exception):
    pass


def squared_exponential(left, right, amplitude, length_scale):
    differences = left[:, None, :] - right[None, :, :]
    return amplitude**2 * np.exp(-np.sum(differences * differences, axis=2) / (2.0 * length_scale**2))


def main():
    if np.__version__ != "2.4.6" or scipy.__version__ != "1.18.1" or sys.version_info[:2] != (3, 12):
        raise ContractError("landmark uncertainty backend version drift")
    request_bytes = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.scipy_landmark_uncertainty_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    source_landmarks = np.asarray(request["source_landmarks"], dtype=float)
    target_landmarks = np.asarray(request["target_landmarks"], dtype=float)
    landmark_displacements = target_landmarks - source_landmarks
    source_cells = request["source_cells"]
    target_cells = request["target_cells"]
    query = np.asarray([cell["coordinates_um"] for cell in source_cells], dtype=float)
    target_coordinates = np.asarray([cell["coordinates_um"] for cell in target_cells], dtype=float)
    source_features = np.asarray([cell["features"] for cell in source_cells], dtype=float)
    target_features = np.asarray([cell["features"] for cell in target_cells], dtype=float)
    prior = request["deformation_prior"]
    amplitude = prior["amplitude_um"]
    length_scale = prior["length_scale_um"]
    noise_variance = request["landmark_noise_standard_deviation_um"] ** 2
    landmark_kernel = squared_exponential(source_landmarks, source_landmarks, amplitude, length_scale)
    admitted_kernel = landmark_kernel + noise_variance * np.eye(len(source_landmarks))
    cross_kernel = squared_exponential(query, source_landmarks, amplitude, length_scale)
    query_kernel = squared_exponential(query, query, amplitude, length_scale)
    solved_displacement = np.linalg.solve(admitted_kernel, landmark_displacements)
    posterior_mean = cross_kernel @ solved_displacement
    posterior_covariance = query_kernel - cross_kernel @ np.linalg.solve(admitted_kernel, cross_kernel.T)
    posterior_covariance = 0.5 * (posterior_covariance + posterior_covariance.T)
    posterior_covariance += 1e-10 * np.eye(len(query))
    landmark_posterior_mean = landmark_kernel @ solved_displacement
    landmark_residual = source_landmarks + landmark_posterior_mean - target_landmarks
    rng = np.random.default_rng(request["seed"])
    cholesky = np.linalg.cholesky(posterior_covariance)
    standard = rng.normal(size=(request["posterior_draws"], 2, len(query)))
    displacement_draws = np.empty((request["posterior_draws"], len(query), 2), dtype=float)
    for coordinate in range(2):
        displacement_draws[:, :, coordinate] = posterior_mean[:, coordinate] + standard[:, coordinate] @ cholesky.T
    transformed_draws = query[None, :, :] + displacement_draws
    localization = rng.normal(
        0.0,
        request["localization_standard_deviation_um"],
        size=transformed_draws.shape,
    )
    transformed_with_localization = transformed_draws + localization

    def endpoint(draws):
        centroids = draws.mean(axis=1)
        return np.sum(centroids * centroids, axis=1)

    transform_endpoints = endpoint(transformed_draws)
    combined_endpoints = endpoint(transformed_with_localization)
    mean_positions = query + posterior_mean
    centroid = mean_positions.mean(axis=0)
    gradient_per_coordinate = [
        np.full(len(query), 2.0 * centroid[coordinate] / len(query)) for coordinate in range(2)
    ]
    delta_variance = sum(
        float(gradient @ posterior_covariance @ gradient) for gradient in gradient_per_coordinate
    )
    monte_carlo_sd = float(transform_endpoints.std(ddof=1))
    delta_sd = math.sqrt(max(delta_variance, 0.0))
    relative_error = abs(delta_sd - monte_carlo_sd) / max(monte_carlo_sd, 1e-12)
    correspondences = []
    probability_matrix = np.empty((len(source_cells), len(target_cells)), dtype=float)
    for source_index, source_cell in enumerate(source_cells):
        spatial_squared = np.sum(
            (transformed_draws[:, source_index, None, :] - target_coordinates[None, :, :]) ** 2,
            axis=2,
        )
        expected_spatial_score = np.mean(
            np.exp(-0.5 * spatial_squared / request["spatial_cost_scale_um"] ** 2), axis=0
        )
        feature_squared = np.sum(
            (source_features[source_index, None, :] - target_features) ** 2, axis=1
        )
        scores = expected_spatial_score * np.exp(
            -0.5 * feature_squared / request["feature_cost_scale"] ** 2
        )
        dustbin_score = math.exp(-request["dustbin_cost"])
        probabilities = np.concatenate([scores, [dustbin_score]])
        probabilities /= probabilities.sum()
        probability_matrix[source_index] = probabilities[:-1]
        entropy = float(-np.sum(probabilities * np.log(np.maximum(probabilities, 1e-300))))
        correspondences.append({
            "source_id": source_cell["cell_id"],
            "constraint": "many_to_one_with_source_dustbin",
            "probabilities": [
                {"target_id": target_cell["cell_id"], "probability": float(probability)}
                for target_cell, probability in zip(target_cells, probabilities[:-1])
            ] + [{"target_id": "__dustbin__", "probability": float(probabilities[-1])}],
            "entropy": entropy,
        })
    target_unmatched = [
        {
            "target_id": target_cell["cell_id"],
            "unmatched_probability": float(np.prod(1.0 - probability_matrix[:, target_index])),
        }
        for target_index, target_cell in enumerate(target_cells)
    ]
    result = {
        "format": "marklab.landmark_uncertainty_and_correspondence",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "source_frame": request["source_frame"],
        "target_frame": request["target_frame"],
        "transform_posterior": {
            "family": "independent_coordinate_gaussian_process_deformation",
            "kernel": prior,
            "landmark_noise_standard_deviation_um": request["landmark_noise_standard_deviation_um"],
            "query_displacement_mean_um": posterior_mean.tolist(),
            "query_displacement_covariance_um2": posterior_covariance.tolist(),
            "draw_count": request["posterior_draws"],
            "query_displacement_draws_um": displacement_draws.tolist(),
        },
        "quality": {
            "landmark_posterior_mean_rmse_um": float(np.sqrt(np.mean(np.sum(landmark_residual * landmark_residual, axis=1)))),
            "minimum_posterior_covariance_eigenvalue": float(np.linalg.eigvalsh(posterior_covariance).min()),
        },
        "downstream_uncertainty": {
            "endpoint": "squared_norm_of_transformed_source_centroid",
            "monte_carlo_mean": float(transform_endpoints.mean()),
            "monte_carlo_standard_deviation": monte_carlo_sd,
            "with_localization_standard_deviation": float(combined_endpoints.std(ddof=1)),
            "delta_mean": float(np.sum(centroid * centroid)),
            "delta_standard_deviation": delta_sd,
            "delta_vs_monte_carlo_relative_error": relative_error,
            "transform_and_localization_uncertainty_separated": True,
            "biological_sampling_uncertainty": "not_included",
        },
        "correspondences": correspondences,
        "target_unmatched_probabilities": target_unmatched,
        "claim_status": "probabilistic_compatibility_not_cell_identity",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError, np.linalg.LinAlgError) as error:
        print(f"landmark uncertainty worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
