#!/usr/bin/env python3
"""SciPy worker for Gaussian-process landmark deformation uncertainty and cell correspondence."""

import json
import math
import sys
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parent))

import numpy as np
import scipy

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


def squared_exponential(left, right, amplitude, length_scale):
    differences = left[:, None, :] - right[None, :, :]
    return amplitude**2 * np.exp(
        -np.sum(differences * differences, axis=2) / (2.0 * length_scale**2)
    )


def deformation_posterior(
    source_landmarks, landmark_displacements, query, amplitude, length_scale, noise_variance
):
    """Gaussian-process posterior of the deformation at the query points.

    Each displacement coordinate is an independent zero-mean GP with squared-exponential kernel
    ``amplitude^2 exp(-|x - x'|^2 / (2 length_scale^2))``, observed at the source landmarks as
    target-minus-source displacements with Gaussian noise of variance ``noise_variance``.
    Returns the query posterior mean (n x 2), the shared query posterior covariance
    (symmetrized, with 1e-10 jitter), and the posterior mean at the landmarks.
    """
    landmark_kernel = squared_exponential(
        source_landmarks, source_landmarks, amplitude, length_scale
    )
    regularized_kernel = landmark_kernel + noise_variance * np.eye(len(source_landmarks))
    cross_kernel = squared_exponential(query, source_landmarks, amplitude, length_scale)
    query_kernel = squared_exponential(query, query, amplitude, length_scale)
    solved_displacement = np.linalg.solve(regularized_kernel, landmark_displacements)
    posterior_mean = cross_kernel @ solved_displacement
    explained = cross_kernel @ np.linalg.solve(regularized_kernel, cross_kernel.T)
    posterior_covariance = query_kernel - explained
    posterior_covariance = 0.5 * (posterior_covariance + posterior_covariance.T)
    posterior_covariance += 1e-10 * np.eye(len(query))
    landmark_posterior_mean = landmark_kernel @ solved_displacement
    return posterior_mean, posterior_covariance, landmark_posterior_mean


def main():
    require_versions(numpy=np.__version__, scipy=scipy.__version__)
    request_bytes = read_request(16 * MIB)
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
    noise_variance = request["landmark_noise_standard_deviation_um"] ** 2
    posterior_mean, posterior_covariance, landmark_posterior_mean = deformation_posterior(
        source_landmarks,
        landmark_displacements,
        query,
        prior["amplitude_um"],
        prior["length_scale_um"],
        noise_variance,
    )
    landmark_residual = source_landmarks + landmark_posterior_mean - target_landmarks
    rng = np.random.default_rng(request["seed"])
    cholesky = np.linalg.cholesky(posterior_covariance)
    standard = rng.normal(size=(request["posterior_draws"], 2, len(query)))
    displacement_draws = np.empty((request["posterior_draws"], len(query), 2), dtype=float)
    for coordinate in range(2):
        displacement_draws[:, :, coordinate] = (
            posterior_mean[:, coordinate] + standard[:, coordinate] @ cholesky.T
        )
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
        correspondences.append(
            {
                "source_id": source_cell["cell_id"],
                "constraint": "many_to_one_with_source_dustbin",
                "probabilities": [
                    {"target_id": target_cell["cell_id"], "probability": float(probability)}
                    for target_cell, probability in zip(
                        target_cells, probabilities[:-1], strict=True
                    )
                ]
                + [{"target_id": "__dustbin__", "probability": float(probabilities[-1])}],
                "entropy": entropy,
            }
        )
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
        "request_sha256": sha256_hex(request_bytes),
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
            "landmark_posterior_mean_rmse_um": float(
                np.sqrt(np.mean(np.sum(landmark_residual * landmark_residual, axis=1)))
            ),
            "minimum_posterior_covariance_eigenvalue": float(
                np.linalg.eigvalsh(posterior_covariance).min()
            ),
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
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "landmark uncertainty worker contract error", errors=CONTRACT_ERRORS)
