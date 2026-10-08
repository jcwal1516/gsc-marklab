#!/usr/bin/env python3

import hashlib
import json
import sys

import numpy as np
import scipy
from scipy.interpolate import RBFInterpolator


class ContractError(Exception):
    pass


def main():
    if np.__version__ != "2.4.6" or scipy.__version__ != "1.18.1" or sys.version_info[:2] != (3, 12):
        raise ContractError("deformation-biology backend version drift")
    request_bytes = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.scipy_deformation_biology_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    points = request["points"]
    pre_coordinates = np.asarray([point["pre_coordinates"] for point in points], dtype=float)
    post_coordinates = np.asarray([point["post_coordinates"] for point in points], dtype=float)
    pre_values = np.asarray([point["pre_value"] for point in points], dtype=float)
    post_values = np.asarray([point["post_value"] for point in points], dtype=float)
    negative_values = np.asarray([point["negative_control_post_value"] for point in points], dtype=float)
    domain = np.asarray([point["domain_indicator"] for point in points], dtype=float)
    independent = np.asarray([point["independent_change_measurement"] for point in points], dtype=float)
    deformation_draws = np.asarray(request["deformation_translation_draws"], dtype=float)
    interpolator = RBFInterpolator(pre_coordinates, pre_values, kernel="thin_plate_spline", smoothing=1e-10)
    design = np.column_stack([np.ones(len(points)), domain])
    coefficient_draws = []
    negative_coefficient_draws = []
    residual_draws = []
    interpolation_errors = []
    for translation in deformation_draws:
        corrected_coordinates = post_coordinates - translation
        baseline_prediction = interpolator(corrected_coordinates)
        interpolation_errors.append(float(np.sqrt(np.mean((baseline_prediction - pre_values) ** 2))))
        residual = post_values - baseline_prediction
        negative_residual = negative_values - baseline_prediction
        coefficient_draws.append(np.linalg.lstsq(design, residual, rcond=None)[0])
        negative_coefficient_draws.append(np.linalg.lstsq(design, negative_residual, rcond=None)[0])
        residual_draws.append(residual)
    coefficient_draws = np.asarray(coefficient_draws)
    negative_coefficient_draws = np.asarray(negative_coefficient_draws)
    residual_draws = np.asarray(residual_draws)
    posterior_change = residual_draws.mean(axis=0)
    correlation = float(np.corrcoef(posterior_change, independent)[0, 1])
    result = {
        "format": "marklab.deformation_biology_model",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "coordinate_frame": request["coordinate_frame"],
        "deformation": {
            "family": "translation_posterior_from_registration",
            "draw_count": len(deformation_draws),
            "translation_xy_mean": deformation_draws.mean(axis=0).tolist(),
            "translation_xy_standard_deviation": deformation_draws.std(axis=0, ddof=1).tolist(),
            "interpolation": "thin_plate_spline",
            "mean_baseline_interpolation_rmse": float(np.mean(interpolation_errors)),
        },
        "biological_change": {
            "model": "intercept_plus_prespecified_domain",
            "intercept_mean": float(coefficient_draws[:, 0].mean()),
            "domain_effect_mean": float(coefficient_draws[:, 1].mean()),
            "domain_effect_standard_deviation": float(coefficient_draws[:, 1].std(ddof=1)),
            "domain_effect_lower_95": float(np.quantile(coefficient_draws[:, 1], 0.025)),
            "domain_effect_upper_95": float(np.quantile(coefficient_draws[:, 1], 0.975)),
            "point_change_mean": posterior_change.tolist(),
            "point_change_standard_deviation": residual_draws.std(axis=0, ddof=1).tolist(),
        },
        "negative_control": {
            "domain_effect_mean": float(negative_coefficient_draws[:, 1].mean()),
            "absolute_domain_effect_mean": abs(float(negative_coefficient_draws[:, 1].mean())),
            "domain_effect_standard_deviation": float(negative_coefficient_draws[:, 1].std(ddof=1)),
        },
        "independent_measurement": {
            "correlation": correlation,
            "role": "validation_not_model_input",
        },
        "identifiability": {
            "deformation_posterior_supplied": True,
            "negative_control_supplied": True,
            "independent_change_measurement_supplied": True,
            "sampling_change_model": "not_included",
        },
        "claim_status": "experimental_synthetic_deformation_biology_separation",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError, np.linalg.LinAlgError) as error:
        print(f"deformation-biology worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
