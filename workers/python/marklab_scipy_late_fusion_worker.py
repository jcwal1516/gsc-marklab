#!/usr/bin/env python3
"""Pinned SciPy worker for calibrated patient-level late fusion."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

sys.path.append(str(Path(__file__).resolve().parent))

import numpy as np
import scipy
from scipy.special import expit

from marklab_worker_support import versions
from marklab_worker_support.logistic_calibration import (
    apply_platt,
    fit_penalized_logistic,
    fit_platt_to_probabilities,
)
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
# byte becomes at most 6 JSON bytes (\u00XX escapes; numbers and nulls grow less), and each row
# adds at most 256 bytes of keys: 96 MiB + 24.4 MiB + fixed fields, rounded up.
MAXIMUM_REQUEST_BYTES = 122 * MIB


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "base_prediction_source",
            "patients",
            "modalities",
            "l2_penalty",
            "resources",
        },
        "request",
    )
    exact(request["format"], "marklab.scipy_late_fusion_request", "request.format")
    exact(request["version"], 1, "request.version")
    exact(
        request["base_prediction_source"],
        "patient_level_out_of_fold",
        "request.base_prediction_source",
    )
    validate_backend(
        request["backend"],
        "request.backend",
        name="scipy",
        version=versions.SCIPY,
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )
    if not isinstance(request["modalities"], list) or not 2 <= len(request["modalities"]) <= 16:
        raise ContractError("modalities differ")
    keys = {"patient_id", "split", "base_prediction_source", "label", "modality_probabilities"}
    for index, raw in enumerate(request["patients"]):
        row = obj(raw, keys, f"patients[{index}]")
        if (
            row["split"] not in {"meta_train", "calibration", "test"}
            or row["base_prediction_source"] != "patient_level_out_of_fold"
            or row["label"] not in {0, 1}
        ):
            raise ContractError("patient differs")
    obj(
        request["resources"],
        {"maximum_patients", "maximum_modalities", "maximum_output_bytes", "timeout_seconds"},
        "resources",
    )
    return request


def features(row: dict[str, Any]) -> np.ndarray:
    """Interleave each modality's probability (0.5 when missing) with its availability flag."""
    values = []
    for probability in row["modality_probabilities"]:
        values.append(0.5 if probability is None else probability)
        values.append(float(probability is not None))
    return np.asarray(values, dtype=np.float64)


def brier_score(probabilities: list[float], labels: list[int]) -> float:
    return float(
        np.mean(
            [
                (probability - label) ** 2
                for probability, label in zip(probabilities, labels, strict=True)
            ]
        )
    )


def run(request: dict[str, Any]) -> dict[str, Any]:
    """Stack modality probabilities with a penalized logistic meta-model, then Platt-calibrate.

    The meta-model is L2-penalized logistic regression on (probability, availability) pairs
    from the meta-training split; its probabilities are Platt-scaled on the calibration split.
    Test predictions are scored by Brier score, overall, per missing-modality pattern, and with
    each modality ablated (treated as missing).
    """
    patients = request["patients"]
    meta = [row for row in patients if row["split"] == "meta_train"]
    calibration = [row for row in patients if row["split"] == "calibration"]
    test = [row for row in patients if row["split"] == "test"]
    model = fit_penalized_logistic(
        np.asarray([features(row) for row in meta]),
        np.asarray([row["label"] for row in meta], dtype=np.float64),
        float(request["l2_penalty"]),
    )

    def raw_probability(row: dict[str, Any], ablate: int | None = None) -> float:
        values = features(row)
        if ablate is not None:
            values[2 * ablate] = 0.5
            values[2 * ablate + 1] = 0.0
        return float(expit(model[0] + np.dot(values, model[1:])))

    calibration_raw = np.asarray([raw_probability(row) for row in calibration])
    calibrator = fit_platt_to_probabilities(
        calibration_raw, np.asarray([row["label"] for row in calibration], dtype=np.float64)
    )

    predictions = []
    for row in test:
        raw_value = raw_probability(row)
        predictions.append(
            {
                "patient_id": row["patient_id"],
                "label": row["label"],
                "availability": [value is not None for value in row["modality_probabilities"]],
                "raw_probability": raw_value,
                "probability": apply_platt(calibrator, raw_value),
            }
        )
    full_brier = brier_score(
        [row["probability"] for row in predictions], [row["label"] for row in predictions]
    )
    scenarios: dict[str, list[dict[str, Any]]] = {}
    for row in predictions:
        missing = [
            name
            for name, available in zip(request["modalities"], row["availability"], strict=True)
            if not available
        ]
        name = "complete" if not missing else "missing_" + "_and_".join(missing)
        scenarios.setdefault(name, []).append(row)
    missing_scenarios = [
        {
            "scenario": name,
            "count": len(rows),
            "brier_score": brier_score(
                [row["probability"] for row in rows], [row["label"] for row in rows]
            ),
        }
        for name, rows in sorted(scenarios.items())
    ]
    ablations = []
    for index, modality in enumerate(request["modalities"]):
        probabilities = [apply_platt(calibrator, raw_probability(row, index)) for row in test]
        brier = brier_score(probabilities, [row["label"] for row in test])
        ablations.append(
            {
                "modality": modality,
                "brier_score": brier,
                "brier_difference_ablated_minus_full": brier - full_brier,
            }
        )
    names = [
        name
        for modality in request["modalities"]
        for name in (f"{modality}_probability", f"{modality}_available")
    ]
    return {
        "model": {
            "intercept": float(model[0]),
            "coefficients": model[1:].tolist(),
            "feature_names": names,
            "l2_penalty": request["l2_penalty"],
        },
        "calibrator": {"intercept": float(calibrator[0]), "slope": float(calibrator[1])},
        "predictions": predictions,
        "metrics": {"brier_score": full_brier},
        "missing_scenarios": missing_scenarios,
        "modality_ablations": ablations,
    }


def main() -> None:
    require_versions(scipy=scipy.__version__)
    lock_digest, worker_digest = worker_digests(__file__)
    raw = read_request(MAXIMUM_REQUEST_BYTES)
    request = validate(json.loads(raw), lock_digest, worker_digest)
    result = {
        "format": "marklab.scipy_late_fusion_worker_result",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(raw),
        **run(request),
    }
    write_result(result, sort_keys=False, newline=False)


if __name__ == "__main__":
    run_worker(main, "late-fusion worker failed")
