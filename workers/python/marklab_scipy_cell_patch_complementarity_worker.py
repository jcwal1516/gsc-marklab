#!/usr/bin/env python3
"""Pinned SciPy worker for nested patient-held-out cell-patch complementarity."""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path
from typing import Any

sys.path.append(str(Path(__file__).resolve().parent))

import numpy as np
import scipy
from scipy import linalg

from marklab_worker_support import versions
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
    exact,
    obj,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    validate_backend,
    worker_digests,
    write_result,
)

# Rust re-serializes one CSV file of at most 16 MiB with at most 10,000 patient rows. Each CSV
# byte becomes at most 6 JSON bytes (\u00XX escapes; numbers grow less), and each row adds at
# most 256 bytes of keys: 96 MiB + 2.5 MiB + fixed fields, rounded up.
MAXIMUM_REQUEST_BYTES = 100 * MIB

FEATURE_GROUPS = [
    "technical",
    "clinical",
    "compartment",
    "acquisition",
    "cell",
    "patch",
    "neighbor",
    "measured",
]
BASELINE_GROUPS = ["technical", "clinical", "compartment", "acquisition"]
MODEL_GROUPS = [
    ("baseline", BASELINE_GROUPS),
    ("baseline_plus_cell", [*BASELINE_GROUPS, "cell"]),
    ("baseline_plus_patch", [*BASELINE_GROUPS, "patch"]),
    ("baseline_plus_cell_patch", [*BASELINE_GROUPS, "cell", "patch"]),
    ("baseline_plus_cell_patch_neighbor", [*BASELINE_GROUPS, "cell", "patch", "neighbor"]),
    (
        "baseline_plus_cell_patch_neighbor_measured",
        [*BASELINE_GROUPS, "cell", "patch", "neighbor", "measured"],
    ),
]


def validate_request(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "patients",
            "feature_names",
            "outer_folds",
            "inner_folds",
            "ridge_alphas",
            "resources",
        },
        "request",
    )
    exact(request["format"], "marklab.scipy_cell_patch_complementarity_request", "request.format")
    exact(request["version"], 1, "request.version")
    validate_backend(
        request["backend"],
        "request.backend",
        name="scipy",
        version=versions.SCIPY,
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )
    names = obj(request["feature_names"], FEATURE_GROUPS, "request.feature_names")
    for group in FEATURE_GROUPS:
        if not isinstance(names[group], list) or not names[group]:
            raise ContractError(f"feature group {group} is invalid")
    patients = request["patients"]
    if not isinstance(patients, list) or not 24 <= len(patients) <= 10_000:
        raise ContractError("patient count is invalid")
    patient_keys = {"patient_id", "outer_fold", "inner_fold", "target", *FEATURE_GROUPS}
    for index, raw in enumerate(patients):
        patient = obj(raw, patient_keys, f"request.patients[{index}]")
        if not isinstance(patient["patient_id"], str) or not patient["patient_id"]:
            raise ContractError("patient identity is invalid")
        for group in FEATURE_GROUPS:
            if not isinstance(patient[group], list) or len(patient[group]) != len(names[group]):
                raise ContractError(f"patient feature group {group} differs")
            if any(not math.isfinite(float(value)) for value in patient[group]):
                raise ContractError("patient feature is non-finite")
        if not math.isfinite(float(patient["target"])):
            raise ContractError("patient target is non-finite")
    if not 2 <= request["outer_folds"] <= 10 or not 2 <= request["inner_folds"] <= 10:
        raise ContractError("fold count is invalid")
    alphas = request["ridge_alphas"]
    if (
        not isinstance(alphas, list)
        or not 2 <= len(alphas) <= 32
        or any(not math.isfinite(float(value)) or value < 0 for value in alphas)
    ):
        raise ContractError("ridge alpha grid is invalid")
    obj(
        request["resources"],
        {
            "maximum_patients",
            "maximum_features",
            "maximum_work_units",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "request.resources",
    )
    return request


def model_columns(names: dict[str, list[str]]) -> list[tuple[str, list[int], list[str]]]:
    """Return each nested model's identifier, design-matrix column indices, and feature names."""
    offsets: dict[str, list[int]] = {}
    all_names: list[str] = []
    for group in FEATURE_GROUPS:
        offsets[group] = list(range(len(all_names), len(all_names) + len(names[group])))
        all_names.extend(names[group])
    return [
        (
            model_id,
            [index for group in selected for index in offsets[group]],
            [name for group in selected for name in names[group]],
        )
        for model_id, selected in MODEL_GROUPS
    ]


def fit_predict(
    x_train: np.ndarray, y_train: np.ndarray, x_test: np.ndarray, alpha: float
) -> np.ndarray:
    """Fit ridge regression on training rows and predict the test rows.

    Features are standardized with training means and population SDs (constant features keep
    scale 1) and the outcome is centred; the coefficients minimize ``||y - X b||^2 + alpha *
    ||b||^2`` (ordinary least squares when ``alpha`` is zero). The intercept is the training mean.
    """
    center = np.mean(x_train, axis=0)
    scale = np.std(x_train, axis=0, ddof=0)
    scale = np.where(scale > 1e-14, scale, 1.0)
    train = (x_train - center) / scale
    test = (x_test - center) / scale
    y_center = float(np.mean(y_train))
    centered_y = y_train - y_center
    if alpha == 0.0:
        beta = linalg.lstsq(train, centered_y, check_finite=True)[0]
    else:
        beta = linalg.solve(
            train.T @ train + alpha * np.eye(train.shape[1]),
            train.T @ centered_y,
            assume_a="pos",
            check_finite=True,
        )
    return y_center + test @ beta


def evaluate_model(
    request: dict[str, Any],
    model_x: np.ndarray,
    y: np.ndarray,
    outer: np.ndarray,
    inner: np.ndarray,
) -> tuple[np.ndarray, list[dict[str, Any]]]:
    """Nested cross-validation: choose the ridge penalty by inner-fold RMSE in each outer fold.

    Ties in inner RMSE go to the smaller penalty. Returns the outer-fold predictions and the
    per-fold selections.
    """
    predictions = np.empty(len(y), dtype=np.float64)
    selections: list[dict[str, Any]] = []
    for outer_fold in range(request["outer_folds"]):
        outer_train = outer != outer_fold
        outer_test = ~outer_train
        scores: list[tuple[float, float]] = []
        for alpha in request["ridge_alphas"]:
            squared_error = 0.0
            validation_count = 0
            for inner_fold in range(request["inner_folds"]):
                inner_validation = outer_train & (inner == inner_fold)
                inner_training = outer_train & (inner != inner_fold)
                predicted = fit_predict(
                    model_x[inner_training],
                    y[inner_training],
                    model_x[inner_validation],
                    float(alpha),
                )
                squared_error += float(np.sum(np.square(predicted - y[inner_validation])))
                validation_count += int(np.count_nonzero(inner_validation))
            scores.append((math.sqrt(squared_error / validation_count), float(alpha)))
        inner_rmse, selected_alpha = min(scores, key=lambda item: (item[0], item[1]))
        predictions[outer_test] = fit_predict(
            model_x[outer_train], y[outer_train], model_x[outer_test], selected_alpha
        )
        selections.append(
            {"outer_fold": outer_fold, "ridge_alpha": selected_alpha, "inner_rmse": inner_rmse}
        )
    return predictions, selections


def run(request: dict[str, Any]) -> list[dict[str, Any]]:
    patients = request["patients"]
    x = np.asarray(
        [[value for group in FEATURE_GROUPS for value in patient[group]] for patient in patients],
        dtype=np.float64,
    )
    y = np.asarray([patient["target"] for patient in patients], dtype=np.float64)
    outer = np.asarray([patient["outer_fold"] for patient in patients], dtype=np.int64)
    inner = np.asarray([patient["inner_fold"] for patient in patients], dtype=np.int64)
    models: list[dict[str, Any]] = []
    for model_id, columns, feature_names in model_columns(request["feature_names"]):
        predictions, selections = evaluate_model(request, x[:, columns], y, outer, inner)
        residual = predictions - y
        design = np.column_stack([np.ones(len(predictions)), predictions])
        calibration = linalg.lstsq(design, y, check_finite=True)[0]
        models.append(
            {
                "model_id": model_id,
                "feature_names": feature_names,
                "fold_selections": selections,
                "rmse": math.sqrt(float(np.mean(np.square(residual)))),
                "mae": float(np.mean(np.abs(residual))),
                "calibration_intercept": float(calibration[0]),
                "calibration_slope": float(calibration[1]),
                "predictions": [
                    {
                        "patient_id": patient["patient_id"],
                        "outer_fold": patient["outer_fold"],
                        "observed": float(patient["target"]),
                        "predicted": float(prediction),
                    }
                    for patient, prediction in zip(patients, predictions, strict=True)
                ],
            }
        )
    return models


def main() -> None:
    require_versions(scipy=scipy.__version__)
    lock_digest, worker_digest = worker_digests(__file__)
    raw = read_request(MAXIMUM_REQUEST_BYTES)
    request = validate_request(json.loads(raw), lock_digest, worker_digest)
    result = {
        "format": "marklab.scipy_cell_patch_complementarity_worker_result",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(raw),
        "models": run(request),
    }
    write_result(result, sort_keys=False, newline=False)


if __name__ == "__main__":
    run_worker(main, "cell-patch complementarity worker failed")
