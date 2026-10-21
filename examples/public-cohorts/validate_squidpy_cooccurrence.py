#!/usr/bin/env python3
"""Validate categorical co-occurrence against Squidpy on one Schürch CODEX ROI."""

from __future__ import annotations

import argparse
from collections import Counter
import csv
from importlib.metadata import version
import json
import math
from pathlib import Path
import subprocess

import numpy as np


REPOSITORY = Path(__file__).resolve().parents[2]
SOURCE_FILE = "CRC_clusters_neighborhoods_markers.csv"
SOURCE_BYTES = 222_999_273
SOURCE_DOI = "10.17632/mpjzbtfgfr.1"
SOURCE_LICENSE = "CC BY 4.0"
ROI = "reg001_A"
SCALE_UM_PER_PIXEL = 0.377442
IMAGE_SIZE_PX = (1920, 1440)
RADII_UM = [10.0, 25.0, 50.0, 100.0]
LEVEL_COUNTS = {
    "B cells": 117,
    "CD11b+CD68+ macrophages": 5,
    "CD11c+ DCs": 3,
    "CD4+ T cells": 28,
    "CD4+ T cells CD45RO+": 134,
    "CD68+CD163+ macrophages": 246,
    "CD8+ T cells": 83,
    "Tregs": 8,
    "adipocytes": 19,
    "granulocytes": 80,
    "immune cells": 1,
    "immune cells / vasculature": 1,
    "plasma cells": 32,
    "smooth muscle": 5,
    "stroma": 180,
    "tumor cells": 48,
    "tumor cells / immune cells": 1,
    "undefined": 12,
    "vasculature": 104,
}
LEVELS = sorted(LEVEL_COUNTS)
EXPECTED_DIRECTED_PAIR_COUNTS = [1_806, 11_666, 42_836, 141_924]
NATIVE_ABSOLUTE_TOLERANCE = 1e-12
SQUIDPY_ULPS = 16


def new_external_directory(path: Path) -> Path:
    path = path.resolve()
    if path == REPOSITORY or REPOSITORY in path.parents:
        raise ValueError("validation output must remain outside the checkout")
    if path.exists():
        raise ValueError("validation output must be a new directory")
    path.mkdir(parents=True)
    return path


def validate_source(directory: Path) -> Path:
    directory = directory.resolve()
    if directory == REPOSITORY or REPOSITORY in directory.parents:
        raise ValueError("source data must remain outside the checkout")
    source_file = directory / SOURCE_FILE
    if not source_file.is_file() or source_file.stat().st_size != SOURCE_BYTES:
        raise ValueError("preserved Schürch cell table differs from its reviewed size")
    try:
        metadata = json.loads((directory / "source.json").read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read preserved source metadata: {error}") from error
    if (
        metadata.get("doi") != SOURCE_DOI
        or metadata.get("license") != SOURCE_LICENSE
        or metadata.get("filename") != SOURCE_FILE
        or metadata.get("expected_bytes") != SOURCE_BYTES
    ):
        raise ValueError("preserved Schürch metadata differs from the reviewed source")
    return source_file


def read_fixture(source_file: Path) -> tuple[list[dict], np.ndarray, np.ndarray]:
    cells = []
    labels = []
    coordinates = []
    seen = set()
    with source_file.open(newline="", encoding="utf-8") as stream:
        for row in csv.DictReader(stream):
            if row["File Name"] != ROI or row["ClusterName"] == "dirt":
                continue
            source_id = row["CellID"]
            if not source_id or source_id in seen:
                raise ValueError("selected Schürch CellID is empty or duplicated")
            seen.add(source_id)
            label = row["ClusterName"]
            x_px = float(row["X:X"])
            y_px = float(row["Y:Y"])
            if (
                label not in LEVEL_COUNTS
                or not math.isfinite(x_px)
                or not math.isfinite(y_px)
                or not 0 <= x_px < IMAGE_SIZE_PX[0]
                or not 0 <= y_px < IMAGE_SIZE_PX[1]
            ):
                raise ValueError("selected Schürch row differs from the reviewed ROI profile")
            x_um = x_px * SCALE_UM_PER_PIXEL
            y_um = y_px * SCALE_UM_PER_PIXEL
            cell_id = f"{ROI}:{source_id}"
            cells.append(
                {"id": cell_id, "x_um": x_um, "y_um": y_um, "class_id": label}
            )
            labels.append(LEVELS.index(label))
            coordinates.append([x_um, y_um])
    if len(cells) != 1_107 or Counter(cell["class_id"] for cell in cells) != LEVEL_COUNTS:
        raise ValueError("selected Schürch ROI differs from reviewed cell/class counts")
    return cells, np.asarray(coordinates, dtype=np.float64), np.asarray(labels, dtype=np.int32)


def request_document(cells: list[dict]) -> dict:
    half_pixel = 0.5 * SCALE_UM_PER_PIXEL
    right = (IMAGE_SIZE_PX[0] - 0.5) * SCALE_UM_PER_PIXEL
    top = (IMAGE_SIZE_PX[1] - 0.5) * SCALE_UM_PER_PIXEL
    return {
        "format": "marklab.categorical_cooccurrence",
        "version": 1,
        "coordinate_frame_id": "schurch-reg001-a-local-um",
        "coordinate_unit": "micrometer",
        "window": {
            "type": "MultiPolygon",
            "coordinates": [
                [
                    [
                        [-half_pixel, -half_pixel],
                        [right, -half_pixel],
                        [right, top],
                        [-half_pixel, top],
                        [-half_pixel, -half_pixel],
                    ]
                ]
            ],
        },
        "channel": {
            "id": "cell_type",
            "label": "Schürch CODEX cell type",
            "kind": "categorical",
            "unit": "categorical",
            "measurement_status": "classified",
            "provenance": (
                "Schürch et al. 2020 published ClusterName; " + SOURCE_DOI
            ),
            "classes": LEVELS,
        },
        "cells": cells,
        "radii_um": RADII_UM,
        "limits": {
            "maximum_input_bytes": 2 * 1024 * 1024,
            "maximum_points": 2_000,
            "maximum_classes": 32,
            "maximum_radii": 8,
            "maximum_output_matrix_bytes": 2 * 1024 * 1024,
            "maximum_directed_pair_visits": 200_000,
            "maximum_retained_bytes": 64 * 1024 * 1024,
        },
    }


def squared_distances(coordinates: np.ndarray) -> np.ndarray:
    dx2 = np.subtract.outer(coordinates[:, 0], coordinates[:, 0])
    np.square(dx2, out=dx2)
    dy2 = np.subtract.outer(coordinates[:, 1], coordinates[:, 1])
    np.square(dy2, out=dy2)
    dx2 += dy2
    np.fill_diagonal(dx2, np.inf)
    return dx2


def pair_counts(
    distances2: np.ndarray, labels: np.ndarray, radii: np.ndarray
) -> list[np.ndarray]:
    class_count = len(LEVELS)
    pair_codes = labels[:, None] * class_count + labels[None, :]
    counts = []
    for radius in radii:
        selected = distances2 <= radius * radius
        count = np.bincount(
            pair_codes[selected], minlength=class_count * class_count
        ).reshape(class_count, class_count)
        counts.append(count.astype(np.uint64, copy=False))
    return counts


def expected_matrices(counts: list[np.ndarray]) -> list[dict]:
    matrices = []
    for radius, count in zip(RADII_UM, counts, strict=True):
        incidences = count.sum(axis=1, dtype=np.uint64)
        total = int(incidences.sum(dtype=np.uint64))
        entries = []
        for source, source_id in enumerate(LEVELS):
            for target, target_id in enumerate(LEVELS):
                pair_count = int(count[source, target])
                source_incidence = int(incidences[source])
                target_incidence = int(incidences[target])
                if source_incidence == 0 or target_incidence == 0:
                    status = "unavailable_zero_edge_incidence"
                    score = None
                else:
                    status = "available"
                    score = (
                        float(pair_count)
                        * float(total)
                        / (float(source_incidence) * float(target_incidence))
                    )
                entries.append(
                    {
                        "source_class_id": source_id,
                        "target_class_id": target_id,
                        "directed_pair_count": pair_count,
                        "source_edge_incidence": source_incidence,
                        "target_edge_incidence": target_incidence,
                        "status": status,
                        "score": score,
                    }
                )
        matrices.append(
            {
                "radius_um": radius,
                "total_directed_pairs": total,
                "entries": entries,
            }
        )
    return matrices


def squidpy_reference(
    cells: list[dict], coordinates: np.ndarray, expected: list[dict]
) -> tuple[dict, float, float]:
    import anndata as ad
    import pandas as pd
    import squidpy as sq

    observations = pd.DataFrame(
        {
            "cell_type": pd.Categorical(
                [cell["class_id"] for cell in cells], categories=LEVELS
            )
        },
        index=[cell["id"] for cell in cells],
    )
    adata = ad.AnnData(obs=observations)
    adata.obsm["spatial"] = coordinates
    interval_input = np.asarray([0.0, *RADII_UM], dtype=np.float64)
    actual, returned_interval = sq.gr.co_occurrence(
        adata, cluster_key="cell_type", interval=interval_input, copy=True
    )
    if (
        actual.dtype != np.float64
        or returned_interval.dtype != np.float32
        or actual.shape != (len(LEVELS), len(LEVELS), len(RADII_UM))
        or not np.array_equal(returned_interval, interval_input.astype(np.float32))
    ):
        raise ValueError("Squidpy returned an unexpected co-occurrence layout")
    maximum_error = 0.0
    maximum_bound = 0.0
    for radius_index, matrix in enumerate(expected):
        for source in range(len(LEVELS)):
            for target in range(len(LEVELS)):
                entry = matrix["entries"][source * len(LEVELS) + target]
                observed = float(actual[target, source, radius_index])
                if entry["status"] != "available":
                    if observed != 0.0:
                        raise ValueError("Squidpy unavailable cell does not contain zero")
                    continue
                expected_value = entry["score"]
                bound = SQUIDPY_ULPS * np.finfo(np.float64).eps * (
                    1.0 + abs(expected_value)
                )
                error = abs(observed - expected_value)
                maximum_error = max(maximum_error, error)
                maximum_bound = max(maximum_bound, bound)
                if error > bound:
                    raise ValueError("Squidpy differs from its independent count formula")
    return (
        {
            "package": "squidpy",
            "version": version("squidpy"),
            "numpy_version": np.__version__,
            "coordinate_input_dtype": str(coordinates.dtype),
            "coordinate_computation_dtype": "float32",
            "returned_interval": returned_interval.tolist(),
            "returned_interval_dtype": str(returned_interval.dtype),
            "output_dtype": str(actual.dtype),
            "axis_order": "target, source, radius",
            "maximum_absolute_formula_error": maximum_error,
            "maximum_acceptance_bound": maximum_bound,
        },
        maximum_error,
        maximum_bound,
    )


def compare_native(native: dict, expected: list[dict]) -> float:
    if (
        native.get("format") != "marklab.categorical_cooccurrence_result"
        or native.get("version") != 1
        or native.get("coordinate_frame_id") != "schurch-reg001-a-local-um"
        or native.get("channel_id") != "cell_type"
        or native.get("classes") != LEVELS
        or native.get("point_count") != 1_107
        or native.get("class_count") != len(LEVELS)
        or native.get("maximum_radius_directed_pair_visits")
        != EXPECTED_DIRECTED_PAIR_COUNTS[-1]
        or len(native.get("matrices", [])) != len(expected)
    ):
        raise ValueError("native co-occurrence result has an unexpected profile")
    maximum_error = 0.0
    for observed_matrix, expected_matrix in zip(
        native["matrices"], expected, strict=True
    ):
        if (
            observed_matrix.get("radius_um") != expected_matrix["radius_um"]
            or observed_matrix.get("total_directed_pairs")
            != expected_matrix["total_directed_pairs"]
            or len(observed_matrix.get("entries", []))
            != len(expected_matrix["entries"])
        ):
            raise ValueError("native radius matrix differs from the independent oracle")
        for observed, reference in zip(
            observed_matrix["entries"], expected_matrix["entries"], strict=True
        ):
            for field in [
                "source_class_id",
                "target_class_id",
                "directed_pair_count",
                "source_edge_incidence",
                "target_edge_incidence",
                "status",
            ]:
                if observed.get(field) != reference[field]:
                    raise ValueError(f"native co-occurrence entry differs at {field}")
            if reference["status"] == "available":
                score = observed.get("score")
                if not isinstance(score, (int, float)) or not math.isfinite(score):
                    raise ValueError("native available co-occurrence value is not finite")
                error = abs(float(score) - reference["score"])
                maximum_error = max(maximum_error, error)
                if error > NATIVE_ABSOLUTE_TOLERANCE:
                    raise ValueError("native co-occurrence value differs from the f64 oracle")
            elif observed.get("score") is not None:
                raise ValueError("native unavailable co-occurrence value must be null")
    return maximum_error


def validate(
    source_directory: Path,
    output_directory: Path,
    binary: Path,
    *,
    reference_only: bool,
) -> None:
    source_file = validate_source(source_directory)
    output = new_external_directory(output_directory)
    cells, coordinates64, labels = read_fixture(source_file)
    request = request_document(cells)
    request_path = output / "request.json"
    request_path.write_text(json.dumps(request, indent=2) + "\n", encoding="utf-8")

    distances64 = squared_distances(coordinates64)
    radii64 = np.asarray(RADII_UM, dtype=np.float64)
    counts64 = pair_counts(distances64, labels, radii64)
    coordinates32 = coordinates64.astype(np.float32)
    distances32 = squared_distances(coordinates32)
    radii32 = radii64.astype(np.float32)
    counts32 = pair_counts(distances32, labels, radii32)
    if any(not np.array_equal(left, right) for left, right in zip(counts64, counts32)):
        raise ValueError("fixture has different float32 and float64 pair memberships")
    directed_pair_counts = [int(count.sum(dtype=np.uint64)) for count in counts64]
    if directed_pair_counts != EXPECTED_DIRECTED_PAIR_COUNTS:
        raise ValueError("Schürch fixture directed-pair counts changed")
    expected = expected_matrices(counts64)
    (output / "independent-reference.json").write_text(
        json.dumps(
            {
                "classes": LEVELS,
                "coordinate_dtype": "float64",
                "radii_um": RADII_UM,
                "matrices": expected,
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    squidpy, squidpy_error, squidpy_bound = squidpy_reference(
        cells, coordinates64, expected
    )

    thresholds2 = radii64 * radii64
    margins = [
        float(np.min(np.abs(distances64[np.isfinite(distances64)] - threshold)))
        for threshold in thresholds2
    ]
    distance_quantization_error = float(
        np.max(
            np.abs(
                distances32.astype(np.float64)[np.isfinite(distances32)]
                - distances64[np.isfinite(distances64)]
            )
        )
    )
    coordinate_quantization_error = float(
        np.max(np.abs(coordinates32.astype(np.float64) - coordinates64))
    )

    native_summary = {"status": "not_run_reference_only"}
    if not reference_only:
        binary = binary.resolve()
        if not binary.is_file():
            raise ValueError(f"Marklab binary does not exist: {binary}")
        native_path = output / "native-result.json"
        run = subprocess.run(
            [
                str(binary),
                "numerics",
                "categorical-co-occurrence",
                "--input",
                str(request_path),
                "--out",
                str(native_path),
            ],
            cwd=REPOSITORY,
            capture_output=True,
            text=True,
        )
        if run.returncode != 0:
            raise ValueError(f"native co-occurrence failed: {run.stderr.strip()}")
        try:
            native = json.loads(native_path.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, json.JSONDecodeError) as error:
            raise ValueError(f"cannot read native result: {error}") from error
        native_summary = {
            "status": "passed",
            "maximum_absolute_formula_error": compare_native(native, expected),
            "acceptance": f"absolute error <= {NATIVE_ABSOLUTE_TOLERANCE}",
        }

    summary = {
        "source": {
            "doi": SOURCE_DOI,
            "license": SOURCE_LICENSE,
            "roi": ROI,
            "calibration_um_per_pixel": SCALE_UM_PER_PIXEL,
            "cells": len(cells),
            "classes": len(LEVELS),
        },
        "pair_membership": {
            "requested_radii_um": RADII_UM,
            "directed_pair_counts": directed_pair_counts,
            "float32_matches_float64": True,
            "nearest_f64_squared_distance_margin_um2": margins,
            "maximum_squared_distance_quantization_error_um2": distance_quantization_error,
            "maximum_coordinate_quantization_error_um": coordinate_quantization_error,
        },
        "squidpy": {
            **squidpy,
            "acceptance": "abs(actual-expected) <= 16*eps64*(1+abs(expected))",
            "maximum_absolute_formula_error": squidpy_error,
            "maximum_acceptance_bound": squidpy_bound,
        },
        "native": native_summary,
    }
    (output / "validation.json").write_text(
        json.dumps(summary, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps(summary, indent=2))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source_directory", type=Path)
    parser.add_argument("output_directory", type=Path)
    parser.add_argument("--binary", type=Path, default=REPOSITORY / "target/debug/marklab")
    parser.add_argument(
        "--reference-only",
        action="store_true",
        help="validate only the independent and Squidpy references",
    )
    args = parser.parse_args()
    validate(
        args.source_directory,
        args.output_directory,
        args.binary,
        reference_only=args.reference_only,
    )


if __name__ == "__main__":
    main()
