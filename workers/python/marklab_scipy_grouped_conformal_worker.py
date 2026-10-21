#!/usr/bin/env python3
"""Pinned SciPy worker for patient-level split-conformal classification."""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path
from typing import Any

sys.path.append(str(Path(__file__).resolve().parent))

import numpy as np
import scipy
from scipy.special import expit

from marklab_worker_support import versions
from marklab_worker_support.logistic_calibration import fit_penalized_logistic
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

# Rust re-serializes one CSV file of at most 16 MiB with at most 100,000 patient rows. Each CSV
# byte becomes at most 6 JSON bytes (\u00XX escapes; numbers grow less), and each row adds at
# most 256 bytes of keys: 96 MiB + 24.4 MiB + fixed fields, rounded up.
MAXIMUM_REQUEST_BYTES = 122 * MIB


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "patients",
            "feature_names",
            "alpha",
            "l2_penalty",
            "resources",
        },
        "request",
    )
    exact(request["format"], "marklab.scipy_grouped_conformal_request", "request.format")
    exact(request["version"], 1, "request.version")
    validate_backend(
        request["backend"],
        "request.backend",
        name="scipy",
        version=versions.SCIPY,
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )
    patients = request["patients"]
    feature_names = request["feature_names"]
    if not isinstance(patients, list) or not 30 <= len(patients) <= 100_000:
        raise ContractError("patient count differs")
    if not isinstance(feature_names, list) or not 2 <= len(feature_names) <= 128:
        raise ContractError("feature count differs")
    keys = {"patient_id", "split", "site", "subgroup", "label", "features"}
    for index, raw in enumerate(patients):
        patient = obj(raw, keys, f"patients[{index}]")
        if (
            patient["split"] not in {"train", "calibration", "test"}
            or patient["label"] not in {0, 1}
            or len(patient["features"]) != len(feature_names)
            or any(not math.isfinite(float(value)) for value in patient["features"])
        ):
            raise ContractError("patient differs")
    obj(
        request["resources"],
        {"maximum_patients", "maximum_features", "maximum_output_bytes", "timeout_seconds"},
        "resources",
    )
    return request


def coverage(rows: list[dict[str, Any]], key: str | None) -> list[dict[str, Any]]:
    groups = ["all"] if key is None else sorted({row[key] for row in rows})
    output = []
    for group in groups:
        selected = rows if key is None else [row for row in rows if row[key] == group]
        covered = sum(bool(row["covered"]) for row in selected)
        output.append(
            {
                "group": group,
                "count": len(selected),
                "covered": covered,
                "coverage": covered / len(selected),
            }
        )
    return output


def run(request: dict[str, Any]) -> dict[str, Any]:
    """Split-conformal binary prediction sets from a penalized logistic model.

    The classifier is L2-penalized logistic regression on training features standardized with
    training means and population SDs. The nonconformity score is one minus the predicted
    probability of the true label; the threshold is the ``ceil((n + 1)(1 - alpha))``-th smallest
    calibration score, and each test prediction set holds every label whose score is at most it.
    """
    patients = request["patients"]
    train = [row for row in patients if row["split"] == "train"]
    calibration = [row for row in patients if row["split"] == "calibration"]
    test = [row for row in patients if row["split"] == "test"]
    train_x = np.asarray([row["features"] for row in train], dtype=np.float64)
    mean = np.mean(train_x, axis=0)
    sd = np.std(train_x, axis=0, ddof=0)
    if np.any(sd <= 1e-14):
        raise ContractError("every training feature must vary")
    standardized = (train_x - mean) / sd
    parameters = fit_penalized_logistic(
        standardized,
        np.asarray([row["label"] for row in train], dtype=np.float64),
        float(request["l2_penalty"]),
    )

    def probability(row: dict[str, Any]) -> float:
        values = (np.asarray(row["features"], dtype=np.float64) - mean) / sd
        return float(expit(parameters[0] + np.dot(values, parameters[1:])))

    calibration_scores = sorted(
        1.0 - probability(row) if row["label"] == 1 else probability(row) for row in calibration
    )
    rank = math.ceil((len(calibration) + 1) * (1.0 - float(request["alpha"])))
    threshold = calibration_scores[rank - 1]
    predictions = []
    for row in test:
        probability_one = probability(row)
        prediction_set = []
        if probability_one <= threshold:
            prediction_set.append(0)
        if 1.0 - probability_one <= threshold:
            prediction_set.append(1)
        predictions.append(
            {
                "patient_id": row["patient_id"],
                "site": row["site"],
                "subgroup": row["subgroup"],
                "label": row["label"],
                "probability_one": probability_one,
                "prediction_set": prediction_set,
                "covered": row["label"] in prediction_set,
            }
        )
    return {
        "model": {
            "training_mean": mean.tolist(),
            "training_population_sd": sd.tolist(),
            "intercept": float(parameters[0]),
            "coefficients": parameters[1:].tolist(),
            "l2_penalty": request["l2_penalty"],
        },
        "calibration_count": len(calibration),
        "corrected_rank": rank,
        "nonconformity_threshold": threshold,
        "predictions": predictions,
        "coverage": {
            "overall": coverage(predictions, None)[0],
            "by_site": coverage(predictions, "site"),
            "by_subgroup": coverage(predictions, "subgroup"),
        },
    }


def main() -> None:
    require_versions(scipy=scipy.__version__)
    lock_digest, worker_digest = worker_digests(__file__)
    raw = read_request(MAXIMUM_REQUEST_BYTES)
    request = validate(json.loads(raw), lock_digest, worker_digest)
    result = {
        "format": "marklab.scipy_grouped_conformal_worker_result",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(raw),
        **run(request),
    }
    write_result(result, sort_keys=False, newline=False)


if __name__ == "__main__":
    run_worker(main, "grouped conformal worker failed")
