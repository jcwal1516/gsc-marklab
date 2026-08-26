#!/usr/bin/env python3

import hashlib
import json
import math
import sys

import numpy as np
import scipy


class ContractError(Exception):
    pass


def main():
    if np.__version__ != "2.4.6" or scipy.__version__ != "1.18.1" or sys.version_info[:2] != (3, 12):
        raise ContractError("serial stack backend version drift")
    request_bytes = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.scipy_serial_stack_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    sections = request["sections"]
    section_ids = [section["section_id"] for section in sections]
    reference_index = section_ids.index(request["reference_section_id"])
    landmarks = [np.asarray(section["observed_landmarks"], dtype=float) for section in sections]
    reference = landmarks[reference_index]
    translations = np.asarray([(reference - section).mean(axis=0) for section in landmarks])
    count = len(reference)
    posterior_sd = request["landmark_noise_standard_deviation_um"] * math.sqrt(2.0 / count)
    posterior_sd_by_section = np.full((len(sections), 2), posterior_sd)
    posterior_sd_by_section[reference_index] = 0.0
    transformed = [section + translation for section, translation in zip(landmarks, translations)]
    residuals = np.concatenate([value - reference for value in transformed], axis=0)
    pairwise = []
    adjacent_translations = []
    for index in range(len(sections) - 1):
        translation = (landmarks[index + 1] - landmarks[index]).mean(axis=0)
        adjacent_translations.append(translation)
        pairwise.append({
            "source_section_id": section_ids[index],
            "target_section_id": section_ids[index + 1],
            "translation_xy_um": translation.tolist(),
            "landmark_rmse_um": float(np.sqrt(np.mean(np.sum((landmarks[index] + translation - landmarks[index + 1]) ** 2, axis=1)))),
        })
    composed = np.zeros((len(sections), 2))
    for index in range(reference_index - 1, -1, -1):
        composed[index] = composed[index + 1] + adjacent_translations[index]
    for index in range(reference_index + 1, len(sections)):
        composed[index] = composed[index - 1] - adjacent_translations[index - 1]
    cycle_errors = np.linalg.norm(composed - translations, axis=1)
    rng = np.random.default_rng(request["seed"])
    draws = rng.normal(
        translations[None, :, :],
        posterior_sd_by_section[None, :, :],
        size=(request["posterior_draws"], len(sections), 2),
    )
    centroid_draws = []
    for draw in draws:
        stack_points = []
        for section, section_landmarks, translation in zip(sections, landmarks, draw):
            xy = section_landmarks + translation
            z = np.full((len(xy), 1), section["z_um"])
            stack_points.append(np.column_stack([xy, z]))
        centroid_draws.append(np.concatenate(stack_points, axis=0).mean(axis=0))
    centroid_draws = np.asarray(centroid_draws)
    translation_payload = [
        {
            "section_id": section["section_id"],
            "z_um": section["z_um"],
            "posterior_mean": translation.tolist(),
            "posterior_standard_deviation": standard_deviation.tolist(),
            "draws": section_draws.tolist(),
        }
        for section, translation, standard_deviation, section_draws in zip(
            sections, translations, posterior_sd_by_section, np.moveaxis(draws, 1, 0)
        )
    ]
    result = {
        "format": "marklab.bayesian_serial_section_stack",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "stack_id": request["stack_id"],
        "coordinate_unit": "um",
        "reference_section_id": request["reference_section_id"],
        "ordered_sections": [{"section_id": section["section_id"], "z_um": section["z_um"]} for section in sections],
        "pairwise_models": pairwise,
        "stack_posterior": {
            "family": "gaussian_translation_landmark_posterior",
            "draw_count": request["posterior_draws"],
            "section_translations_xy_um": translation_payload,
            "reference_constraint": "reference_translation_fixed_zero",
        },
        "transformed_landmarks_xyz_um": [
            np.column_stack([points, np.full((len(points), 1), section["z_um"])]).tolist()
            for section, points in zip(sections, transformed)
        ],
        "quality": {
            "landmark_rmse_um": float(np.sqrt(np.mean(np.sum(residuals * residuals, axis=1)))),
            "maximum_cycle_consistency_error_um": float(cycle_errors.max()),
            "section_gap_um": np.diff([section["z_um"] for section in sections]).tolist(),
            "missing_section_model": "explicit_z_gaps_only",
        },
        "downstream_uncertainty": {
            "analysis": "stack_landmark_centroid_xyz",
            "stack_centroid_mean_um": centroid_draws.mean(axis=0).tolist(),
            "stack_centroid_standard_deviation_um": centroid_draws.std(axis=0, ddof=1).tolist(),
            "transform_draw_outputs_um": centroid_draws.tolist(),
            "between_transform_uncertainty": "included",
            "biological_sampling_uncertainty": "not_included",
        },
        "claim_status": "experimental_synthetic_serial_stack_reconstruction",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError, np.linalg.LinAlgError) as error:
        print(f"serial stack worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
