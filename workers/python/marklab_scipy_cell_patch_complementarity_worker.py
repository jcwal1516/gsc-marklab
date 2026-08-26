#!/usr/bin/env python3
"""Pinned SciPy worker for nested patient-held-out cell-patch complementarity."""

from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path
import sys
from typing import Any

import numpy as np
import scipy
from scipy import linalg


SCIPY_VERSION = "1.18.1"


class ContractError(ValueError):
    pass


def exact_object(value: Any, keys: set[str], path: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        raise ContractError(f"{path} fields differ")
    return value


def validate_request(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = exact_object(
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
    if request["format"] != "marklab.scipy_cell_patch_complementarity_request" or request["version"] != 1:
        raise ContractError("request identity differs")
    backend = exact_object(
        request["backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "request.backend",
    )
    if (
        backend["name"] != "scipy"
        or backend["version"] != SCIPY_VERSION
        or backend["python_version"] != "3.12"
        or backend["environment_lock_sha256"] != lock_digest
        or backend["worker_sha256"] != worker_digest
        or scipy.__version__ != SCIPY_VERSION
    ):
        raise ContractError("backend identity differs")
    groups = [
        "technical",
        "clinical",
        "compartment",
        "acquisition",
        "cell",
        "patch",
        "neighbor",
        "measured",
    ]
    names = exact_object(request["feature_names"], set(groups), "request.feature_names")
    for group in groups:
        if not isinstance(names[group], list) or not names[group]:
            raise ContractError(f"feature group {group} is invalid")
    patients = request["patients"]
    if not isinstance(patients, list) or not 24 <= len(patients) <= 10_000:
        raise ContractError("patient count is invalid")
    patient_keys = {"patient_id", "outer_fold", "inner_fold", "target", *groups}
    for index, raw in enumerate(patients):
        patient = exact_object(raw, patient_keys, f"request.patients[{index}]")
        if not isinstance(patient["patient_id"], str) or not patient["patient_id"]:
            raise ContractError("patient identity is invalid")
        for group in groups:
            if not isinstance(patient[group], list) or len(patient[group]) != len(names[group]):
                raise ContractError(f"patient feature group {group} differs")
            if any(not math.isfinite(float(value)) for value in patient[group]):
                raise ContractError("patient feature is non-finite")
        if not math.isfinite(float(patient["target"])):
            raise ContractError("patient target is non-finite")
    if not 2 <= request["outer_folds"] <= 10 or not 2 <= request["inner_folds"] <= 10:
        raise ContractError("fold count is invalid")
    if (
        not isinstance(request["ridge_alphas"], list)
        or not 2 <= len(request["ridge_alphas"]) <= 32
        or any(not math.isfinite(float(value)) or value < 0 for value in request["ridge_alphas"])
    ):
        raise ContractError("ridge alpha grid is invalid")
    exact_object(
        request["resources"],
        {"maximum_patients", "maximum_features", "maximum_work_units", "maximum_output_bytes", "timeout_seconds"},
        "request.resources",
    )
    return request


def model_columns(names: dict[str, list[str]]) -> list[tuple[str, list[int], list[str]]]:
    groups = ["technical", "clinical", "compartment", "acquisition", "cell", "patch", "neighbor", "measured"]
    offsets: dict[str, list[int]] = {}
    all_names: list[str] = []
    for group in groups:
        offsets[group] = list(range(len(all_names), len(all_names) + len(names[group])))
        all_names.extend(names[group])
    base_groups = ["technical", "clinical", "compartment", "acquisition"]
    definitions = [
        ("m0", base_groups),
        ("m1", [*base_groups, "cell"]),
        ("m2", [*base_groups, "patch"]),
        ("m3", [*base_groups, "cell", "patch"]),
        ("m4", [*base_groups, "cell", "patch", "neighbor"]),
        ("m5", [*base_groups, "cell", "patch", "neighbor", "measured"]),
    ]
    return [
        (
            model_id,
            [index for group in selected for index in offsets[group]],
            [name for group in selected for name in names[group]],
        )
        for model_id, selected in definitions
    ]


def fit_predict(x_train: np.ndarray, y_train: np.ndarray, x_test: np.ndarray, alpha: float) -> np.ndarray:
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


def run(raw: bytes) -> dict[str, Any]:
    worker_path = Path(__file__)
    request = validate_request(
        json.loads(raw),
        hashlib.sha256(worker_path.with_name("uv.lock").read_bytes()).hexdigest(),
        hashlib.sha256(worker_path.read_bytes()).hexdigest(),
    )
    group_order = ["technical", "clinical", "compartment", "acquisition", "cell", "patch", "neighbor", "measured"]
    patients = request["patients"]
    x = np.asarray([[value for group in group_order for value in patient[group]] for patient in patients], dtype=np.float64)
    y = np.asarray([patient["target"] for patient in patients], dtype=np.float64)
    outer = np.asarray([patient["outer_fold"] for patient in patients], dtype=np.int64)
    inner = np.asarray([patient["inner_fold"] for patient in patients], dtype=np.int64)
    models: list[dict[str, Any]] = []
    for model_id, columns, feature_names in model_columns(request["feature_names"]):
        model_x = x[:, columns]
        predictions = np.empty(len(patients), dtype=np.float64)
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
                    predicted = fit_predict(model_x[inner_training], y[inner_training], model_x[inner_validation], float(alpha))
                    squared_error += float(np.sum(np.square(predicted - y[inner_validation])))
                    validation_count += int(np.count_nonzero(inner_validation))
                scores.append((math.sqrt(squared_error / validation_count), float(alpha)))
            inner_rmse, selected_alpha = min(scores, key=lambda item: (item[0], item[1]))
            predictions[outer_test] = fit_predict(
                model_x[outer_train], y[outer_train], model_x[outer_test], selected_alpha
            )
            selections.append({"outer_fold": outer_fold, "ridge_alpha": selected_alpha, "inner_rmse": inner_rmse})
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
    return {
        "format": "marklab.scipy_cell_patch_complementarity_worker_result",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(raw).hexdigest(),
        "models": models,
    }


def main() -> None:
    raw = sys.stdin.buffer.read()
    try:
        output = json.dumps(run(raw), allow_nan=False, separators=(",", ":")).encode()
    except Exception as error:
        print(f"cell-patch complementarity worker failed: {error}", file=sys.stderr)
        raise SystemExit(2) from error
    sys.stdout.buffer.write(output)


if __name__ == "__main__":
    main()
