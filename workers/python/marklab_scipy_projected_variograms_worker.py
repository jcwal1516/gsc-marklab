#!/usr/bin/env python3
"""Pinned SciPy worker for split-safe projected embedding variograms."""

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


def integer(value: Any, lower: int, upper: int, path: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not lower <= value <= upper:
        raise ContractError(f"{path} must be an integer in [{lower}, {upper}]")
    return value


def number(value: Any, path: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ContractError(f"{path} must be numeric")
    result = float(value)
    if not math.isfinite(result):
        raise ContractError(f"{path} must be finite")
    return result


def exact_string(value: Any, expected: str, path: str) -> None:
    if value != expected:
        raise ContractError(f"{path} must equal {expected!r}")


def validate_request(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = exact_object(
        request,
        {
            "format",
            "version",
            "backend",
            "feature_names",
            "points",
            "bins",
            "components",
            "permutations",
            "seed",
            "resources",
        },
        "request",
    )
    exact_string(
        request["format"],
        "marklab.scipy_projected_embedding_variograms_request",
        "request.format",
    )
    integer(request["version"], 1, 1, "request.version")
    backend = exact_object(
        request["backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "request.backend",
    )
    exact_string(backend["name"], "scipy", "request.backend.name")
    exact_string(backend["version"], SCIPY_VERSION, "request.backend.version")
    exact_string(backend["python_version"], "3.12", "request.backend.python_version")
    exact_string(
        backend["environment_lock_sha256"], lock_digest, "request.backend.environment_lock_sha256"
    )
    exact_string(backend["worker_sha256"], worker_digest, "request.backend.worker_sha256")
    if scipy.__version__ != SCIPY_VERSION:
        raise ContractError(f"installed SciPy version is {scipy.__version__!r}")

    features = request["feature_names"]
    if (
        not isinstance(features, list)
        or not 2 <= len(features) <= 128
        or any(not isinstance(value, str) or not value.startswith("embedding_") for value in features)
        or len(set(features)) != len(features)
    ):
        raise ContractError("request.feature_names are invalid")
    points = request["points"]
    if not isinstance(points, list) or not 8 <= len(points) <= 10_000:
        raise ContractError("request.points count is invalid")
    seen_ids: set[str] = set()
    for index, raw in enumerate(points):
        point = exact_object(
            raw,
            {
                "object_id",
                "biological_unit",
                "split",
                "permutation_stratum",
                "x_um",
                "y_um",
                "embedding",
            },
            f"request.points[{index}]",
        )
        for key in ("object_id", "biological_unit", "permutation_stratum"):
            value = point[key]
            if not isinstance(value, str) or not value or value.strip() != value:
                raise ContractError(f"request.points[{index}].{key} is invalid")
        if point["object_id"] in seen_ids:
            raise ContractError("request.points object IDs are duplicated")
        seen_ids.add(point["object_id"])
        if point["split"] not in {"train", "validation", "test"}:
            raise ContractError(f"request.points[{index}].split is invalid")
        number(point["x_um"], f"request.points[{index}].x_um")
        number(point["y_um"], f"request.points[{index}].y_um")
        embedding = point["embedding"]
        if not isinstance(embedding, list) or len(embedding) != len(features):
            raise ContractError(f"request.points[{index}].embedding shape differs")
        for feature, value in enumerate(embedding):
            number(value, f"request.points[{index}].embedding[{feature}]")

    bins = request["bins"]
    if not isinstance(bins, list) or not 1 <= len(bins) <= 256:
        raise ContractError("request.bins count is invalid")
    previous_upper: float | None = None
    for index, raw in enumerate(bins):
        distance_bin = exact_object(
            raw, {"bin_id", "lower_um", "upper_um", "upper_inclusive"}, f"request.bins[{index}]"
        )
        lower = number(distance_bin["lower_um"], f"request.bins[{index}].lower_um")
        upper = number(distance_bin["upper_um"], f"request.bins[{index}].upper_um")
        if (
            not isinstance(distance_bin["bin_id"], str)
            or not distance_bin["bin_id"]
            or lower < 0.0
            or lower >= upper
            or (previous_upper is not None and lower != previous_upper)
            or distance_bin["upper_inclusive"] is not (index == len(bins) - 1)
        ):
            raise ContractError(f"request.bins[{index}] is invalid")
        previous_upper = upper

    components = integer(request["components"], 1, min(16, len(features)), "request.components")
    permutations = integer(request["permutations"], 20, 10_000, "request.permutations")
    integer(request["seed"], 0, 2**64 - 1, "request.seed")
    resources = exact_object(
        request["resources"],
        {
            "maximum_objects",
            "maximum_embedding_dimension",
            "maximum_components",
            "maximum_pair_visits",
            "maximum_work_units",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "request.resources",
    )
    integer(resources["maximum_objects"], 10_000, 10_000, "request.resources.maximum_objects")
    integer(
        resources["maximum_embedding_dimension"],
        128,
        128,
        "request.resources.maximum_embedding_dimension",
    )
    integer(resources["maximum_components"], 16, 16, "request.resources.maximum_components")
    integer(resources["maximum_pair_visits"], 1, 2**64 - 1, "request.resources.maximum_pair_visits")
    integer(resources["maximum_work_units"], 250_000_000, 250_000_000, "request.resources.maximum_work_units")
    integer(resources["maximum_output_bytes"], 16 * 1024 * 1024, 16 * 1024 * 1024, "request.resources.maximum_output_bytes")
    integer(resources["timeout_seconds"], 1, 3_600, "request.resources.timeout_seconds")
    if components >= sum(point["split"] == "train" for point in points):
        raise ContractError("training row count must exceed component count")
    if permutations * components > resources["maximum_work_units"]:
        raise ContractError("request work controls are inconsistent")
    return request


def assign_bin(distance: float, bins: list[dict[str, Any]]) -> int | None:
    for index, distance_bin in enumerate(bins):
        if distance >= distance_bin["lower_um"] and (
            distance < distance_bin["upper_um"]
            or (distance_bin["upper_inclusive"] and distance <= distance_bin["upper_um"])
        ):
            return index
    return None


def pair_plan(coordinates: np.ndarray, bins: list[dict[str, Any]]) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    left: list[int] = []
    right: list[int] = []
    bin_indices: list[int] = []
    for left_index in range(coordinates.shape[0]):
        for right_index in range(left_index + 1, coordinates.shape[0]):
            distance = float(np.linalg.norm(coordinates[left_index] - coordinates[right_index]))
            bin_index = assign_bin(distance, bins)
            if bin_index is not None:
                left.append(left_index)
                right.append(right_index)
                bin_indices.append(bin_index)
    return np.asarray(left, dtype=np.int64), np.asarray(right, dtype=np.int64), np.asarray(bin_indices, dtype=np.int64)


def semivariograms(
    values: np.ndarray,
    left: np.ndarray,
    right: np.ndarray,
    pair_bins: np.ndarray,
    bin_count: int,
) -> tuple[np.ndarray, np.ndarray]:
    component_count = values.shape[1]
    result = np.full((component_count, bin_count), np.nan, dtype=np.float64)
    counts = np.bincount(pair_bins, minlength=bin_count).astype(np.int64)
    for component in range(component_count):
        contributions = 0.5 * np.square(values[left, component] - values[right, component])
        for bin_index in range(bin_count):
            selected = pair_bins == bin_index
            if np.any(selected):
                result[component, bin_index] = math.fsum(float(value) for value in contributions[selected]) / int(counts[bin_index])
    return result, counts


def fit_projection(
    request: dict[str, Any],
) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, list[str]]:
    points = request["points"]
    matrix = np.asarray([point["embedding"] for point in points], dtype=np.float64)
    training_indices = [index for index, point in enumerate(points) if point["split"] == "train"]
    training = matrix[training_indices]
    center = np.mean(training, axis=0, dtype=np.float64)
    centered = training - center
    covariance = centered.T @ centered / (training.shape[0] - 1)
    eigenvalues, eigenvectors = linalg.eigh(covariance, check_finite=True)
    order = np.argsort(eigenvalues, kind="stable")[::-1][: request["components"]]
    selected_values = eigenvalues[order]
    components = eigenvectors[:, order].T.copy()
    tolerance = np.finfo(np.float64).eps * max(covariance.shape) * max(float(np.max(np.abs(eigenvalues))), 1.0)
    if np.any(selected_values <= tolerance):
        raise ContractError("training covariance lacks the requested positive-variance components")
    for component in components:
        pivot = int(np.argmax(np.abs(component)))
        if component[pivot] < 0.0:
            component *= -1.0
    units = sorted({points[index]["biological_unit"] for index in training_indices})
    return matrix - center, center, selected_values, components, units


def split_curve(
    request: dict[str, Any],
    centered: np.ndarray,
    components: np.ndarray,
    split: str,
) -> dict[str, Any]:
    points = request["points"]
    indices = [index for index, point in enumerate(points) if point["split"] == split]
    coordinates = np.asarray([[points[index]["x_um"], points[index]["y_um"]] for index in indices])
    projected = centered[indices] @ components.T
    left, right, pair_bins = pair_plan(coordinates, request["bins"])
    observed, counts = semivariograms(projected, left, right, pair_bins, len(request["bins"]))
    null = np.full(
        (request["permutations"], request["components"], len(request["bins"])),
        np.nan,
        dtype=np.float64,
    )
    local_strata: dict[str, list[int]] = {}
    for local_index, point_index in enumerate(indices):
        local_strata.setdefault(points[point_index]["permutation_stratum"], []).append(local_index)
    rng = np.random.Generator(np.random.PCG64(request["seed"]))
    for permutation in range(request["permutations"]):
        permuted = projected.copy()
        for stratum_indices in local_strata.values():
            selected = np.asarray(stratum_indices, dtype=np.int64)
            permuted[selected] = projected[rng.permutation(selected)]
        null[permutation], _ = semivariograms(
            permuted, left, right, pair_bins, len(request["bins"])
        )
    means = np.nanmean(null, axis=0)
    standard_deviations = np.nanstd(null, axis=0, ddof=1)
    valid = np.isfinite(observed) & np.isfinite(standard_deviations) & (standard_deviations > 1e-14)
    studentized_observed = np.zeros_like(observed)
    studentized_observed[valid] = np.abs((observed[valid] - means[valid]) / standard_deviations[valid])
    maximum_statistics = np.zeros(request["permutations"], dtype=np.float64)
    if np.any(valid):
        studentized_null = np.zeros_like(null)
        studentized_null[:, valid] = np.abs((null[:, valid] - means[valid]) / standard_deviations[valid])
        maximum_statistics = np.max(studentized_null[:, valid], axis=1)

    rows: list[dict[str, Any]] = []
    for component in range(request["components"]):
        for bin_index, distance_bin in enumerate(request["bins"]):
            count = int(counts[bin_index])
            adjusted = None
            if valid[component, bin_index]:
                adjusted = float(
                    (1 + np.count_nonzero(maximum_statistics >= studentized_observed[component, bin_index]))
                    / (request["permutations"] + 1)
                )
            rows.append(
                {
                    "component": component,
                    "bin_id": distance_bin["bin_id"],
                    "lower_um": distance_bin["lower_um"],
                    "upper_um": distance_bin["upper_um"],
                    "upper_inclusive": distance_bin["upper_inclusive"],
                    "pair_count": count,
                    "semivariance": None if count == 0 else float(observed[component, bin_index]),
                    "permutation_mean": None if count == 0 else float(means[component, bin_index]),
                    "permutation_sd": None if not valid[component, bin_index] else float(standard_deviations[component, bin_index]),
                    "max_t_adjusted_p": adjusted,
                }
            )
    family_size = int(request["components"] * np.count_nonzero(counts))
    return {
        "split": split,
        "row_count": len(indices),
        "pair_visits": len(indices) * (len(indices) - 1) // 2,
        "family_size": family_size,
        "rows": rows,
    }


def run(raw_request: bytes) -> dict[str, Any]:
    worker_path = Path(__file__)
    lock_path = worker_path.with_name("uv.lock")
    request = validate_request(
        json.loads(raw_request),
        hashlib.sha256(lock_path.read_bytes()).hexdigest(),
        hashlib.sha256(worker_path.read_bytes()).hexdigest(),
    )
    centered, center, eigenvalues, components, units = fit_projection(request)
    curves = [
        split_curve(request, centered, components, split)
        for split in ("train", "validation", "test")
        if any(point["split"] == split for point in request["points"])
    ]
    return {
        "format": "marklab.scipy_projected_embedding_variograms_worker_result",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(raw_request).hexdigest(),
        "projection_artifact": {
            "fit_split": "train",
            "training_biological_units": units,
            "center": center.tolist(),
            "eigenvalues": eigenvalues.tolist(),
            "components": components.tolist(),
            "method": "training_only_centered_pca_scipy_eigh",
            "sign_orientation": "largest_absolute_loading_positive",
        },
        "multiplicity_control": "single_step_max_t",
        "permutations": request["permutations"],
        "seed": request["seed"],
        "curves": curves,
    }


def main() -> None:
    raw_request = sys.stdin.buffer.read()
    try:
        result = run(raw_request)
        encoded = json.dumps(result, allow_nan=False, separators=(",", ":")).encode()
    except Exception as error:
        print(f"projected variogram worker failed: {error}", file=sys.stderr)
        raise SystemExit(2) from error
    sys.stdout.buffer.write(encoded)


if __name__ == "__main__":
    main()
