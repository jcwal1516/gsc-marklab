#!/usr/bin/env python3
"""SciPy worker for split-safe projected embedding variograms with max-t permutation tests."""

from __future__ import annotations

import json
import os
import struct
import subprocess
import sys
from collections import Counter
from pathlib import Path
from typing import Any

import numpy as np
import scipy
from scipy import linalg

sys.path.append(str(Path(__file__).resolve().parent))

from marklab_worker_support import versions
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
    exact,
    integer,
    number,
    obj,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    validate_backend,
    worker_digests,
    write_result,
)

# Rust re-serializes a point CSV file (at most 10,000 rows) and a bin file of at most 16 MiB
# each. Each input byte becomes at most 6 JSON bytes (\u00XX escapes; numbers grow less), and
# each point row adds fewer than 128 bytes of keys: 192 MiB + 1.25 MiB + fixed fields, rounded up.
MAXIMUM_REQUEST_BYTES = 194 * MIB
SPLITS = ("train", "validation", "test")


def validate_request(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
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
    exact(
        request["format"],
        "marklab.scipy_projected_embedding_variograms_request",
        "request.format",
    )
    integer(request["version"], "request.version", 1, 1)
    validate_backend(
        request["backend"],
        "request.backend",
        name="scipy",
        version=versions.SCIPY,
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )

    features = request["feature_names"]
    if (
        not isinstance(features, list)
        or not 2 <= len(features) <= 128
        or any(
            not isinstance(value, str) or not value.startswith("embedding_") for value in features
        )
        or len(set(features)) != len(features)
    ):
        raise ContractError("request.feature_names are invalid")
    points = request["points"]
    if not isinstance(points, list) or not 8 <= len(points) <= 10_000:
        raise ContractError("request.points count is invalid")
    seen_ids: set[str] = set()
    for index, raw in enumerate(points):
        point = obj(
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
        distance_bin = obj(
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

    components = integer(request["components"], "request.components", 1, min(16, len(features)))
    permutations = integer(request["permutations"], "request.permutations", 20, 10_000)
    integer(request["seed"], "request.seed", 0, 2**64 - 1)
    resources = obj(
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
    integer(resources["maximum_objects"], "request.resources.maximum_objects", 10_000, 10_000)
    integer(
        resources["maximum_embedding_dimension"],
        "request.resources.maximum_embedding_dimension",
        128,
        128,
    )
    integer(resources["maximum_components"], "request.resources.maximum_components", 16, 16)
    integer(resources["maximum_pair_visits"], "request.resources.maximum_pair_visits", 1, 2**64 - 1)
    integer(
        resources["maximum_work_units"],
        "request.resources.maximum_work_units",
        250_000_000,
        250_000_000,
    )
    integer(
        resources["maximum_output_bytes"],
        "request.resources.maximum_output_bytes",
        16 * MIB,
        16 * MIB,
    )
    integer(resources["timeout_seconds"], "request.resources.timeout_seconds", 1, 3_600)
    if components >= sum(point["split"] == "train" for point in points):
        raise ContractError("training row count must exceed component count")
    if permutations * components > resources["maximum_work_units"]:
        raise ContractError("request work controls are inconsistent")
    return request


def native_semivariograms(
    request: dict[str, Any],
    coordinates: np.ndarray,
    projected: np.ndarray,
    local_strata: dict[tuple[str, str], list[int]],
    biological_units: list[str],
) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """Accumulate in the running Marklab executable, retaining NumPy's exact PCG64 stream.

    The private little-endian pipe carries a dimension header, biological-unit codes, bin edges,
    coordinates and projected rows, then at most 32 row permutations at a time. Rust stores pairs
    within biological units; Python retains the null array used by the existing max-t reduction.
    The outer bounded
    worker runner owns the deadline; pipe closure stops the helper after its current permutation.
    """
    executable = os.environ.get("MARKLAB_PROJECTED_VARIOGRAM_EXECUTABLE")
    if not executable or not Path(executable).is_absolute():
        raise ContractError("projected variogram accumulation requires the Marklab executable")
    rows, component_count = projected.shape
    bin_count = len(request["bins"])
    cell_count = component_count * bin_count
    null = np.empty((request["permutations"], component_count, bin_count), dtype=np.float64)
    strata = [np.asarray(indices, dtype=np.int64) for indices in local_strata.values()]
    unit_codes = {unit: code for code, unit in enumerate(dict.fromkeys(biological_units))}
    rng = np.random.Generator(np.random.PCG64(request["seed"]))
    with subprocess.Popen(
        [executable, "--internal-projected-variogram-kernel"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    ) as kernel:

        def read_exact(count: int) -> bytes:
            value = kernel.stdout.read(count)
            if len(value) != count:
                detail = kernel.stderr.read(4096).decode(errors="replace").strip()
                raise ContractError(f"projected variogram kernel ended early: {detail}")
            return value

        try:
            kernel.stdin.write(
                struct.pack("<IIII", rows, component_count, bin_count, request["permutations"])
            )
            kernel.stdin.write(
                np.asarray([unit_codes[unit] for unit in biological_units], dtype="<u2").tobytes()
            )
            edges = [request["bins"][0]["lower_um"]] + [
                item["upper_um"] for item in request["bins"]
            ]
            for values in (edges, coordinates, projected):
                kernel.stdin.write(np.asarray(values, dtype="<f8").tobytes())
            kernel.stdin.flush()
            counts = np.frombuffer(read_exact(bin_count * 8), dtype="<u8")
            observed = np.frombuffer(read_exact(cell_count * 8), dtype="<f8").reshape(
                component_count, bin_count
            )
            for start in range(0, request["permutations"], 32):
                count = min(32, request["permutations"] - start)
                indices = np.empty((count, rows), dtype="<u2")
                for permutation in indices:
                    for selected in strata:
                        permutation[selected] = rng.permutation(selected)
                kernel.stdin.write(indices.tobytes())
                kernel.stdin.flush()
                null[start : start + count] = np.frombuffer(
                    read_exact(count * cell_count * 8), dtype="<f8"
                ).reshape(count, component_count, bin_count)
            kernel.stdin.close()
            if kernel.wait() != 0:
                detail = kernel.stderr.read(4096).decode(errors="replace").strip()
                raise ContractError(f"projected variogram kernel failed: {detail}")
        except BaseException:
            kernel.kill()
            kernel.wait()
            raise
    return observed, counts, null


def fit_projection(
    request: dict[str, Any],
) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, list[str]]:
    """Fit PCA on the training split only and center every point with the training mean.

    Returns the centered embeddings, the center, the leading eigenvalues, the sign-oriented
    components (largest absolute loading positive), and the training biological units.
    """
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
    largest = max(float(np.max(np.abs(eigenvalues))), 1.0)
    tolerance = np.finfo(np.float64).eps * max(covariance.shape) * largest
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
    """Projected semivariograms of one split and their stratified permutation max-t tests.

    The null permutes projected values within permutation strata; each cell's statistic is the
    absolute studentized semivariance, and adjusted p-values use the per-permutation maximum.
    """
    points = request["points"]
    indices = [index for index, point in enumerate(points) if point["split"] == split]
    coordinates = np.asarray([[points[index]["x_um"], points[index]["y_um"]] for index in indices])
    projected = centered[indices] @ components.T
    local_strata: dict[tuple[str, str], list[int]] = {}
    biological_units = [points[index]["biological_unit"] for index in indices]
    for local_index, point_index in enumerate(indices):
        key = (points[point_index]["biological_unit"], points[point_index]["permutation_stratum"])
        local_strata.setdefault(key, []).append(local_index)
    observed, counts, null = native_semivariograms(
        request, coordinates, projected, local_strata, biological_units
    )
    means = np.nanmean(null, axis=0)
    standard_deviations = np.nanstd(null, axis=0, ddof=1)
    valid = np.isfinite(observed) & np.isfinite(standard_deviations) & (standard_deviations > 1e-14)
    studentized_observed = np.zeros_like(observed)
    studentized_observed[valid] = np.abs(
        (observed[valid] - means[valid]) / standard_deviations[valid]
    )
    maximum_statistics = np.zeros(request["permutations"], dtype=np.float64)
    if np.any(valid):
        studentized_null = np.zeros_like(null)
        studentized_null[:, valid] = np.abs(
            (null[:, valid] - means[valid]) / standard_deviations[valid]
        )
        maximum_statistics = np.max(studentized_null[:, valid], axis=1)

    rows: list[dict[str, Any]] = []
    for component in range(request["components"]):
        for bin_index, distance_bin in enumerate(request["bins"]):
            count = int(counts[bin_index])
            cell_valid = bool(valid[component, bin_index])
            adjusted = None
            if cell_valid:
                exceedances = np.count_nonzero(
                    maximum_statistics >= studentized_observed[component, bin_index]
                )
                adjusted = float((1 + exceedances) / (request["permutations"] + 1))
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
                    "permutation_sd": (
                        float(standard_deviations[component, bin_index]) if cell_valid else None
                    ),
                    "max_t_adjusted_p": adjusted,
                }
            )
    family_size = int(request["components"] * np.count_nonzero(counts))
    return {
        "split": split,
        "row_count": len(indices),
        "pair_visits": sum(
            count * (count - 1) // 2 for count in Counter(biological_units).values()
        ),
        "family_size": family_size,
        "rows": rows,
    }


def run(raw_request: bytes, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = validate_request(json.loads(raw_request), lock_digest, worker_digest)
    centered, center, eigenvalues, components, units = fit_projection(request)
    curves = [
        split_curve(request, centered, components, split)
        for split in SPLITS
        if any(point["split"] == split for point in request["points"])
    ]
    return {
        "format": "marklab.scipy_projected_embedding_variograms_worker_result",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(raw_request),
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
    require_versions(scipy=scipy.__version__)
    lock_digest, worker_digest = worker_digests(__file__)
    raw_request = read_request(MAXIMUM_REQUEST_BYTES)
    result = run(raw_request, lock_digest, worker_digest)
    write_result(result, sort_keys=False, newline=False)


if __name__ == "__main__":
    run_worker(main, "projected variogram worker failed")
