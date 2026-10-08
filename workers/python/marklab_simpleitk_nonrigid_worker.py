#!/usr/bin/env python3

import contextlib
import hashlib
import io
import json
import sys

import numpy as np
import SimpleITK as sitk


class ContractError(Exception):
    pass


def image_from_array(array, spacing):
    image = sitk.GetImageFromArray(np.asarray(array, dtype=np.float32))
    image.SetSpacing(tuple(spacing))
    return image


def main():
    if sitk.Version_VersionString() != "2.5.5" or np.__version__ != "2.4.6" or sys.version_info[:2] != (3, 12):
        raise ContractError("nonrigid backend version drift")
    sitk.ProcessObject_SetGlobalDefaultNumberOfThreads(1)
    request_bytes = sys.stdin.buffer.read(32 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.simpleitk_nonrigid_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    fixed = image_from_array(request["fixed"]["pixels"], request["fixed"]["spacing_um"])
    moving = image_from_array(request["moving"]["pixels"], request["moving"]["spacing_um"])
    fixed_mask = image_from_array(request["fixed_mask"], request["fixed"]["spacing_um"])
    moving_mask = image_from_array(request["moving_mask"], request["moving"]["spacing_um"])
    fixed_mask = sitk.Cast(fixed_mask, sitk.sitkUInt8)
    moving_mask = sitk.Cast(moving_mask, sitk.sitkUInt8)
    transform = sitk.BSplineTransformInitializer(fixed, request["mesh_size"], order=3)
    registration = sitk.ImageRegistrationMethod()
    registration.SetMetricAsMeanSquares()
    registration.SetMetricFixedMask(fixed_mask)
    registration.SetMetricMovingMask(moving_mask)
    registration.SetInterpolator(sitk.sitkLinear)
    registration.SetOptimizerAsLBFGSB(
        gradientConvergenceTolerance=1e-6,
        numberOfIterations=request["maximum_iterations"],
        maximumNumberOfCorrections=5,
        maximumNumberOfFunctionEvaluations=request["maximum_iterations"] * 10,
        costFunctionConvergenceFactor=1e7,
    )
    registration.SetShrinkFactorsPerLevel(request["shrink_factors"])
    registration.SetSmoothingSigmasPerLevel(request["smoothing_sigmas_um"])
    registration.SmoothingSigmasAreSpecifiedInPhysicalUnitsOn()
    registration.SetInitialTransform(transform, inPlace=True)
    objective_trace = []
    level_starts = []
    registration.AddCommand(sitk.sitkIterationEvent, lambda: objective_trace.append(float(registration.GetMetricValue())))
    registration.AddCommand(sitk.sitkMultiResolutionIterationEvent, lambda: level_starts.append(len(objective_trace)))
    captured = io.StringIO()
    with contextlib.redirect_stdout(captured), contextlib.redirect_stderr(captured):
        registration.Execute(fixed, moving)
    warped = sitk.Resample(moving, fixed, transform, sitk.sitkLinear, 0.0, sitk.sitkFloat32)
    displacement = sitk.TransformToDisplacementField(
        transform,
        sitk.sitkVectorFloat64,
        fixed.GetSize(),
        fixed.GetOrigin(),
        fixed.GetSpacing(),
        fixed.GetDirection(),
    )
    jacobian = sitk.DisplacementFieldJacobianDeterminant(displacement)
    fixed_array = sitk.GetArrayFromImage(fixed).astype(float)
    moving_array = sitk.GetArrayFromImage(moving).astype(float)
    warped_array = sitk.GetArrayFromImage(warped).astype(float)
    mask = np.asarray(request["fixed_mask"], dtype=bool)
    jacobian_array = sitk.GetArrayFromImage(jacobian).astype(float)
    displacement_array = sitk.GetArrayFromImage(displacement).astype(float)
    mse_before = float(np.mean((fixed_array[mask] - moving_array[mask]) ** 2))
    mse_after = float(np.mean((fixed_array[mask] - warped_array[mask]) ** 2))
    if not objective_trace or not np.isfinite(warped_array).all() or not np.isfinite(jacobian_array).all():
        raise ContractError("nonrigid registration returned incomplete or non-finite output")
    result = {
        "format": "marklab.multiresolution_nonrigid_registration",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "fixed_frame": request["fixed"]["frame"],
        "moving_frame": request["moving"]["frame"],
        "transform": {
            "type": "bspline",
            "direction": "fixed_physical_to_moving_physical",
            "parameters": list(transform.GetParameters()),
            "fixed_parameters": list(transform.GetFixedParameters()),
            "mesh_size": request["mesh_size"],
            "displacement_field_xy": displacement_array.tolist(),
        },
        "warped_moving_pixels": warped_array.tolist(),
        "quality": {
            "masked_mse_before": mse_before,
            "masked_mse_after": mse_after,
            "minimum_jacobian_determinant": float(jacobian_array.min()),
            "maximum_jacobian_determinant": float(jacobian_array.max()),
            "nonpositive_jacobian_fraction": float(np.mean(jacobian_array <= 0.0)),
            "inverse_consistency": "unavailable_bspline_transform_has_no_analytic_inverse",
        },
        "diagnostics": {
            "metric": "mean_squares_same_stain",
            "objective_trace": objective_trace,
            "pyramid_level_trace_offsets": level_starts,
            "stop_condition": registration.GetOptimizerStopConditionDescription(),
            "iterations": int(registration.GetOptimizerIteration()),
            "captured_backend_log_lines": len(captured.getvalue().splitlines()),
        },
        "fit_state": "complete" if mse_after < mse_before else "nonconverged",
        "claim_status": "experimental_synthetic_nonrigid_registration",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, RuntimeError, json.JSONDecodeError) as error:
        print(f"nonrigid registration worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
