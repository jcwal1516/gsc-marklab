#!/usr/bin/env python3
"""Run hard CellViT categorical pair curves at the patient population unit."""

from __future__ import annotations

import argparse
import concurrent.futures
import csv
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import subprocess
from typing import Any


PAIR_FAMILIES = (
    ("Neoplastic", "Inflammatory"),
    ("Inflammatory", "Neoplastic"),
    ("Neoplastic", "Connective"),
    ("Connective", "Neoplastic"),
)
RADII_UM = (20.0, 50.0, 100.0, 200.0)
WITHIN_PATTERN_PERMUTATIONS = 99
POPULATION_PERMUTATIONS = 999
ALPHA = 0.05
MEMORY_BUDGET_MIB = 64
PCA_COMPONENTS = 3
MAXIMUM_PROCESSES = 6
MAXIMUM_TIMEOUT_SECONDS = 600
CATEGORICAL_PAIR_ANALYSIS = "categorical-pair"
CATEGORICAL_CROSS_G_ANALYSIS = "categorical-cross-pair-correlation"
TRANSLATION_CROSS_G_ANALYSIS = "translation-categorical-cross-pair-correlation"
ISOTROPIC_CROSS_G_ANALYSIS = "isotropic-categorical-cross-pair-correlation"
CROSS_G_ANALYSES = (
    CATEGORICAL_CROSS_G_ANALYSIS,
    TRANSLATION_CROSS_G_ANALYSIS,
    ISOTROPIC_CROSS_G_ANALYSIS,
)
CROSS_G_BANDWIDTH_UM = 10.0


class CategoricalPairPatientError(ValueError):
    """The frozen patient categorical-pair contract is invalid or incomplete."""


def _validate_analysis(analysis: str) -> None:
    if analysis not in {CATEGORICAL_PAIR_ANALYSIS, *CROSS_G_ANALYSES}:
        raise CategoricalPairPatientError(f"unsupported categorical analysis: {analysis}")


def _correction_name(analysis: str) -> str:
    return {
        CATEGORICAL_CROSS_G_ANALYSIS: "standard_border_radius_plus_bandwidth",
        TRANSLATION_CROSS_G_ANALYSIS: "translation",
        ISOTROPIC_CROSS_G_ANALYSIS: "isotropic",
    }[analysis]


def _analysis_token(analysis: str) -> str:
    return {
        CATEGORICAL_CROSS_G_ANALYSIS: "categorical_cross_g",
        TRANSLATION_CROSS_G_ANALYSIS: "translation_categorical_cross_g",
        ISOTROPIC_CROSS_G_ANALYSIS: "isotropic_categorical_cross_g",
    }[analysis]


def _load_module(path: Path, name: str):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise CategoricalPairPatientError(f"cannot load required owner: {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _load_adapter():
    return _load_module(
        Path(__file__).with_name("marklab_cellvit_cptac_results_adapter.py"),
        "marklab_cellvit_cptac_results_adapter",
    )


def _load_summary_owner():
    return _load_module(
        Path(__file__).with_name("marklab_crc_graph_topology_summary.py"),
        "marklab_crc_graph_topology_summary",
    )


def _read_csv(path: Path) -> list[dict[str, str]]:
    try:
        with path.open(newline="", encoding="utf-8") as source:
            rows = list(csv.DictReader(source))
    except OSError as error:
        raise CategoricalPairPatientError(f"cannot read CSV {path}: {error}") from error
    if not rows:
        raise CategoricalPairPatientError(f"CSV is empty: {path}")
    return rows


def _write_csv(path: Path, fieldnames: list[str], rows: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as target:
        writer = csv.DictWriter(target, fieldnames=fieldnames, lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def _read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise CategoricalPairPatientError(f"cannot read JSON {path}: {error}") from error
    if not isinstance(value, dict):
        raise CategoricalPairPatientError(f"JSON root must be an object: {path}")
    return value


def _window_segment_count(path: Path) -> int:
    document = _read_json(path)
    coordinates = document.get("coordinates")
    if document.get("type") != "MultiPolygon" or not isinstance(coordinates, list):
        raise CategoricalPairPatientError("prepared window must be a GeoJSON MultiPolygon")
    segments = 0
    for polygon in coordinates:
        if not isinstance(polygon, list):
            raise CategoricalPairPatientError("prepared window polygon is invalid")
        for ring in polygon:
            if not isinstance(ring, list) or len(ring) < 4 or ring[0] != ring[-1]:
                raise CategoricalPairPatientError("prepared window ring is invalid")
            segments += len(ring) - 1
    if not 1 <= segments <= 4096:
        raise CategoricalPairPatientError("prepared window segment count is invalid")
    return segments


def _write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False) + "\n",
        encoding="utf-8",
    )


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _pair_id(source: str, target: str) -> str:
    return f"{source.lower()}-to-{target.lower()}"


def _window_covers(window: Any, x_um: float, y_um: float) -> bool:
    try:
        return bool(window.covers((x_um, y_um)))
    except TypeError:
        from shapely.geometry import Point

        return bool(window.covers(Point(x_um, y_um)))


def _contour_area_um2(cell: dict[str, Any], base_mpp: float) -> float:
    """Return the CellViT contour shoelace area in square micrometers."""
    contour = cell.get("contour")
    if (
        not isinstance(contour, list)
        or not 3 <= len(contour) <= 4096
        or not math.isfinite(base_mpp)
        or base_mpp <= 0.0
    ):
        raise CategoricalPairPatientError("CellViT contour or base MPP is invalid")
    points: list[tuple[float, float]] = []
    for point in contour:
        if not isinstance(point, (list, tuple)) or len(point) != 2:
            raise CategoricalPairPatientError("CellViT contour point is invalid")
        try:
            x = float(point[0])
            y = float(point[1])
        except (TypeError, ValueError) as error:
            raise CategoricalPairPatientError("CellViT contour point is invalid") from error
        if not math.isfinite(x) or not math.isfinite(y):
            raise CategoricalPairPatientError("CellViT contour point is non-finite")
        points.append((x, y))
    cross_products: list[float] = []
    for index, (x, y) in enumerate(points):
        next_x, next_y = points[(index + 1) % len(points)]
        cross_product = x * next_y - next_x * y
        if not math.isfinite(cross_product):
            raise CategoricalPairPatientError("CellViT contour area overflows")
        cross_products.append(cross_product)
    area_um2 = abs(math.fsum(cross_products)) * 0.5 * base_mpp * base_mpp
    if not math.isfinite(area_um2) or area_um2 <= 0.0:
        raise CategoricalPairPatientError("CellViT contour area is not finite and positive")
    return area_um2


def _prepare(
    marks_path: Path,
    inference_root: Path,
    output: Path,
    *,
    adapter=None,
    expected_cells_per_pattern: int = 512,
    include_nucleus_area: bool,
) -> list[dict[str, Any]]:
    marks_path = marks_path.resolve()
    inference_root = inference_root.resolve()
    output = output.resolve()
    if output.exists() or output.is_symlink():
        raise CategoricalPairPatientError(f"output already exists: {output}")
    if not 4 <= expected_cells_per_pattern <= 2_000:
        raise CategoricalPairPatientError("expected cells per pattern must be in 4..=2000")
    adapter = adapter or _load_adapter()
    marks = _read_csv(marks_path)
    expected_header = {
        "pattern_id",
        "patient_id",
        "group",
        "point_id",
        "x_um",
        "y_um",
        "type_id",
    }
    if set(marks[0]) != expected_header:
        raise CategoricalPairPatientError("frozen marks schema differs")
    by_pattern: dict[str, list[dict[str, str]]] = {}
    pattern_identity: dict[str, tuple[str, str]] = {}
    for row in marks:
        pattern = row["pattern_id"]
        patient = row["patient_id"]
        group = row["group"]
        if (
            not pattern
            or Path(pattern).name != pattern
            or not patient
            or group not in {"MSI", "MSS"}
            or not row["point_id"].startswith(f"{pattern}:")
        ):
            raise CategoricalPairPatientError("mark identity or molecular group is invalid")
        identity = pattern_identity.setdefault(pattern, (patient, group))
        if identity != (patient, group):
            raise CategoricalPairPatientError("pattern has conflicting patient identity")
        by_pattern.setdefault(pattern, []).append(row)
    patient_patterns: dict[str, list[str]] = {}
    patient_groups: dict[str, str] = {}
    for pattern, (patient, group) in pattern_identity.items():
        patient_patterns.setdefault(patient, []).append(pattern)
        if patient_groups.setdefault(patient, group) != group:
            raise CategoricalPairPatientError("patient has conflicting molecular group")
    if (
        len(patient_patterns) != 8
        or any(len(patterns) != 2 for patterns in patient_patterns.values())
        or set(patient_groups.values()) != {"MSI", "MSS"}
        or sum(group == "MSI" for group in patient_groups.values()) != 4
    ):
        raise CategoricalPairPatientError(
            "categorical pair requires the frozen four-MSI/four-MSS, two-slide subset"
        )

    staging = output.with_name(f".{output.name}.{os.getpid()}.tmp")
    if staging.exists() or staging.is_symlink():
        raise CategoricalPairPatientError(f"staging path already exists: {staging}")
    staging.mkdir(parents=True)
    manifest: list[dict[str, Any]] = []
    source_digest = _sha256(marks_path)
    for pattern in sorted(by_pattern):
        rows = sorted(by_pattern[pattern], key=lambda row: row["point_id"])
        if len(rows) != expected_cells_per_pattern:
            raise CategoricalPairPatientError(
                f"pattern {pattern} has {len(rows)} rather than "
                f"{expected_cells_per_pattern} frozen cells"
            )
        payload_path = adapter.one_file(
            inference_root / pattern, "*_cells.json.snappy"
        ).resolve()
        try:
            payload_path.relative_to(inference_root)
        except ValueError as error:
            raise CategoricalPairPatientError("CellViT payload escapes inference root") from error
        payload = adapter.read_cells(payload_path)
        cells = payload.get("cells")
        metadata = payload.get("wsi_metadata")
        raw_type_map = payload.get("type_map")
        if (
            not isinstance(cells, list)
            or not isinstance(metadata, dict)
            or not isinstance(raw_type_map, dict)
        ):
            raise CategoricalPairPatientError("CellViT cells, metadata, or type map differ")
        type_map = {int(key): str(value) for key, value in raw_type_map.items()}
        if not {level for pair in PAIR_FAMILIES for level in pair} <= set(type_map.values()):
            raise CategoricalPairPatientError("CellViT type map lacks a declared pair level")
        geometry, window = adapter.patch_window(metadata)
        base_mpp = float(metadata["base_mpp"])
        if not math.isfinite(base_mpp) or base_mpp <= 0.0:
            raise CategoricalPairPatientError("CellViT base MPP is invalid")
        typed_rows: list[dict[str, Any]] = []
        seen: set[str] = set()
        for row in rows:
            point_id = row["point_id"]
            if point_id in seen:
                raise CategoricalPairPatientError("frozen CellId is duplicated")
            seen.add(point_id)
            source_row = adapter.projected_row_index(point_id, pattern)
            if not 0 <= source_row < len(cells):
                raise CategoricalPairPatientError("frozen CellId source row is out of range")
            cell = cells[source_row]
            try:
                x_um = float(cell["centroid"][0]) * base_mpp
                y_um = float(cell["centroid"][1]) * base_mpp
                source_type = type_map[int(cell["type"])]
                declared_x = float(row["x_um"])
                declared_y = float(row["y_um"])
            except (KeyError, TypeError, ValueError, IndexError) as error:
                raise CategoricalPairPatientError("CellViT source row is malformed") from error
            if (
                not all(math.isfinite(value) for value in (x_um, y_um, declared_x, declared_y))
                or x_um != declared_x
                or y_um != declared_y
                or source_type != row["type_id"]
                or not _window_covers(window, x_um, y_um)
            ):
                raise CategoricalPairPatientError(
                    "frozen mark coordinate, type, or exact-window correspondence differs"
                )
            patient, _ = pattern_identity[pattern]
            typed_row = {
                "cell_id": point_id,
                "x_um": format(x_um, ".17g"),
                "y_um": format(y_um, ".17g"),
                "mark": int(source_type == "Neoplastic"),
                "case_id": patient,
                "timepoint": "baseline",
                "protein": "cellvit_hard_neoplastic_indicator_unused_by_categorical_pair",
                "valid_tumor": "true",
                "valid_ihc": "true",
                "slide_id": pattern,
                "histologic_compartment": source_type,
            }
            if include_nucleus_area:
                typed_row["nucleus_area_um2"] = format(
                    _contour_area_um2(cell, base_mpp), ".17g"
                )
            typed_rows.append(typed_row)
        pattern_root = staging / "patterns" / pattern
        cells_path = pattern_root / "cells.csv"
        window_path = pattern_root / "window.geojson"
        _write_csv(cells_path, list(typed_rows[0]), typed_rows)
        _write_json(window_path, geometry)
        patient, group = pattern_identity[pattern]
        manifest_row = {
            "pattern_id": pattern,
            "patient_id": patient,
            "group": group,
            "cell_count": len(typed_rows),
            "cells": cells_path.relative_to(staging).as_posix(),
            "window": window_path.relative_to(staging).as_posix(),
            "source_payload_sha256": _sha256(payload_path),
            "prepared_cells_sha256": _sha256(cells_path),
            "prepared_window_sha256": _sha256(window_path),
        }
        if include_nucleus_area:
            manifest_row["base_mpp"] = format(base_mpp, ".17g")
        manifest.append(manifest_row)
    _write_csv(staging / "manifest.csv", list(manifest[0]), manifest)
    if include_nucleus_area:
        design = {
            "schema_name": "marklab_crc_scalar_variogram_input_design",
            "schema_version": "1.0",
            "population_unit": "patient",
            "pattern_unit": "slide_nested_within_patient",
            "patient_count": len(patient_patterns),
            "pattern_count": len(manifest),
            "cells_per_pattern": expected_cells_per_pattern,
            "mark": "nucleus_area_um2",
            "mark_unit": "square_micrometer",
            "mark_source": "cellvit_predicted_nucleus_contour",
            "mark_derivation": "absolute_shoelace_contour_area_pixels_squared_times_base_mpp_squared",
            "contour_coordinate_unit": "source_pixel",
            "base_mpp_unit": "micrometer_per_source_pixel",
            "selection_uses_molecular_label": False,
            "source_marks_sha256": source_digest,
            "finite_result_policy": "reject_non_finite_or_non_positive_contour_area",
        }
    else:
        design = {
            "schema_name": "marklab_crc_categorical_pair_patient_design",
            "schema_version": "1.0",
            "population_unit": "patient",
            "pattern_unit": "slide_nested_within_patient",
            "patient_count": len(patient_patterns),
            "pattern_count": len(manifest),
            "cells_per_pattern": expected_cells_per_pattern,
            "pair_families": [
                {"source_level": source, "target_level": target}
                for source, target in PAIR_FAMILIES
            ],
            "radii_um": list(RADII_UM),
            "within_pattern_null": "complete_hard_categorical_row_random_labeling_at_fixed_locations",
            "within_pattern_permutations": WITHIN_PATTERN_PERMUTATIONS,
            "population_null": "whole_patient_population_independence",
            "population_multiplicity": "step_down_max_t_complete_endpoint_family",
            "selection_uses_molecular_label": False,
            "source_marks_sha256": source_digest,
            "finite_result_policy": "reject_non_finite_input_or_output_and_retain_structurally_unavailable_endpoints",
        }
    _write_json(
        staging / "design.json",
        design,
    )
    os.rename(staging, output)
    return manifest


def prepare(
    marks_path: Path,
    inference_root: Path,
    output: Path,
    *,
    adapter=None,
    expected_cells_per_pattern: int = 512,
) -> list[dict[str, Any]]:
    """Bind frozen marks to raw CellViT rows and exact patch-union windows."""
    return _prepare(
        marks_path,
        inference_root,
        output,
        adapter=adapter,
        expected_cells_per_pattern=expected_cells_per_pattern,
        include_nucleus_area=False,
    )


def prepare_scalar_variogram(
    marks_path: Path,
    inference_root: Path,
    output: Path,
    *,
    adapter=None,
    expected_cells_per_pattern: int = 512,
) -> list[dict[str, Any]]:
    """Prepare CellViT contour area as a physical scalar mark for variograms."""
    return _prepare(
        marks_path,
        inference_root,
        output,
        adapter=adapter,
        expected_cells_per_pattern=expected_cells_per_pattern,
        include_nucleus_area=True,
    )


def execute(
    prepared: Path,
    marklab: Path,
    output: Path,
    maximum_processes: int,
    timeout_seconds: int,
    *,
    replay: bool,
    analysis: str = CATEGORICAL_PAIR_ANALYSIS,
) -> dict[str, Any]:
    """Run every declared pair durably, or replay hits with backend execution disabled."""
    _validate_analysis(analysis)
    prepared = prepared.resolve()
    marklab = marklab.resolve()
    output = output.resolve()
    if not marklab.is_file() or not os.access(marklab, os.X_OK):
        raise CategoricalPairPatientError(f"marklab executable is unavailable: {marklab}")
    if (
        not 1 <= maximum_processes <= MAXIMUM_PROCESSES
        or not 1 <= timeout_seconds <= MAXIMUM_TIMEOUT_SECONDS
    ):
        raise CategoricalPairPatientError("process or timeout bound is invalid")
    if replay:
        if not output.is_dir():
            raise CategoricalPairPatientError("replay requires a completed execution directory")
    elif output.exists() or output.is_symlink():
        raise CategoricalPairPatientError(f"execution output already exists: {output}")
    else:
        output.mkdir(parents=True)
    design = _read_json(prepared / "design.json")
    manifest = _read_csv(prepared / "manifest.csv")
    if (
        design.get("population_unit") != "patient"
        or design.get("pattern_count") != len(manifest)
        or design.get("pair_families")
        != [
            {"source_level": source, "target_level": target}
            for source, target in PAIR_FAMILIES
        ]
    ):
        raise CategoricalPairPatientError("prepared categorical-pair design differs")
    jobs = []
    for row in manifest:
        pattern = row["pattern_id"]
        if Path(pattern).name != pattern:
            raise CategoricalPairPatientError("pattern identity is not a path-safe basename")
        cells = (prepared / row["cells"]).resolve()
        window = (prepared / row["window"]).resolve()
        try:
            cells.relative_to(prepared)
            window.relative_to(prepared)
        except ValueError as error:
            raise CategoricalPairPatientError("prepared pair input escapes its root") from error
        if _sha256(cells) != row["prepared_cells_sha256"] or _sha256(window) != row[
            "prepared_window_sha256"
        ]:
            raise CategoricalPairPatientError("prepared pair input digest differs")
        cell_count = int(row["cell_count"])
        maximum_pairs = cell_count * (cell_count - 1)
        maximum_null_work = maximum_pairs * WITHIN_PATTERN_PERMUTATIONS
        window_segments = _window_segment_count(window)
        for source, target in PAIR_FAMILIES:
            pair = _pair_id(source, target)
            project = output / "projects" / pattern / pair
            result_root = "replay" if replay else "results"
            result = output / result_root / pair / f"{pattern}.json"
            baseline = output / "results" / pair / f"{pattern}.json"
            jobs.append(
                (
                    pattern,
                    source,
                    target,
                    cells,
                    window,
                    project,
                    result,
                    baseline,
                    maximum_pairs,
                    maximum_null_work,
                    window_segments,
                )
            )
    if len(jobs) != len(manifest) * len(PAIR_FAMILIES) or len(jobs) > 256:
        raise CategoricalPairPatientError("categorical-pair job count differs or exceeds 256")

    def run(job):
        (
            pattern,
            source,
            target,
            cells,
            window,
            project,
            result,
            baseline,
            maximum_pairs,
            maximum_null_work,
            window_segments,
        ) = job
        result.parent.mkdir(parents=True, exist_ok=True)
        environment = os.environ.copy()
        if replay:
            environment["MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION"] = "1"
        command = [
            str(marklab),
            "project",
            analysis,
            "--project",
            str(project),
            "--cells",
            str(cells),
            "--mask",
            str(window),
            "--out",
            str(result),
            "--source-level",
            source,
            "--target-level",
            target,
            "--radii-um",
            ",".join(format(radius, ".17g") for radius in RADII_UM),
        ]
        if analysis in CROSS_G_ANALYSES:
            command.extend(
                ["--bandwidth-um", format(CROSS_G_BANDWIDTH_UM, ".17g")]
            )
        command.extend(
            [
                "--permutations",
                str(WITHIN_PATTERN_PERMUTATIONS),
                "--seed",
                "20260829",
                "--alpha",
                str(ALPHA),
                "--memory-budget-mib",
                str(MEMORY_BUDGET_MIB),
                "--max-pair-visits",
                str(maximum_pairs),
                "--max-null-pair-evaluations",
                str(maximum_null_work),
            ]
        )
        if analysis == TRANSLATION_CROSS_G_ANALYSIS:
            command.extend(
                [
                    "--max-overlap-evaluations",
                    str(maximum_pairs),
                    "--max-overlap-candidate-work",
                    str(maximum_pairs * window_segments * window_segments),
                    "--max-overlap-output-vertices",
                    str(window_segments * window_segments + 2 * window_segments),
                ]
            )
        elif analysis == ISOTROPIC_CROSS_G_ANALYSIS:
            command.extend(
                [
                    "--max-visible-arc-evaluations",
                    str(maximum_pairs),
                    "--max-arc-segment-tests",
                    str(maximum_pairs * window_segments),
                    "--max-arc-membership-queries",
                    str(maximum_pairs * max(1, 2 * window_segments)),
                ]
            )
        try:
            completed = subprocess.run(
                command,
                check=False,
                capture_output=True,
                text=True,
                timeout=timeout_seconds,
                env=environment,
            )
        except subprocess.TimeoutExpired as error:
            raise CategoricalPairPatientError(
                f"{pattern}/{source}-to-{target} exceeded {timeout_seconds} seconds"
            ) from error
        expected = "hit" if replay else "miss"
        if completed.returncode != 0 or f"cache_status={expected}" not in completed.stderr:
            raise CategoricalPairPatientError(
                f"{pattern}/{source}-to-{target} failed or did not report {expected}: "
                f"{completed.stderr.strip()}"
            )
        bytes_equal = True
        if replay:
            bytes_equal = baseline.is_file() and baseline.read_bytes() == result.read_bytes()
            if not bytes_equal:
                raise CategoricalPairPatientError(
                    f"{pattern}/{source}-to-{target} replay bytes differ"
                )
        ledger = project / "executions.jsonl"
        ledger_count = (
            sum(bool(line.strip()) for line in ledger.read_text(encoding="utf-8").splitlines())
            if ledger.is_file()
            else 0
        )
        if ledger_count != 1:
            raise CategoricalPairPatientError(
                f"{pattern}/{source}-to-{target} ledger must contain one execution"
            )
        return {
            "pattern_id": pattern,
            "source_level": source,
            "target_level": target,
            "cache_status": expected,
            "result": result.relative_to(output).as_posix(),
            "result_sha256": _sha256(result),
            "replay_bytes_equal": bytes_equal,
            "ledger_execution_count": ledger_count,
        }

    records = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=maximum_processes) as executor:
        futures = [executor.submit(run, job) for job in jobs]
        for future in concurrent.futures.as_completed(futures):
            records.append(future.result())
    records.sort(
        key=lambda row: (row["pattern_id"], row["source_level"], row["target_level"])
    )
    statuses = {
        status: sum(row["cache_status"] == status for row in records)
        for status in sorted({row["cache_status"] for row in records})
    }
    result = {
        "schema_name": (
            "marklab_crc_categorical_pair_patient_execution"
            if analysis == CATEGORICAL_PAIR_ANALYSIS
            else f"marklab_crc_{_analysis_token(analysis)}_patient_execution"
        ),
        "schema_version": "1.0",
        "population_unit": "patient",
        "replay": replay,
        "job_count": len(records),
        "maximum_processes": maximum_processes,
        "timeout_seconds_per_process": timeout_seconds,
        "cache_status_counts": statuses,
        "all_replay_bytes_equal": all(row["replay_bytes_equal"] for row in records),
        "all_ledgers_one_execution": all(
            row["ledger_execution_count"] == 1 for row in records
        ),
        "binary_sha256": _sha256(marklab),
        "records": records,
    }
    _write_json(output / ("replay_manifest.json" if replay else "execution_manifest.json"), result)
    return result


def _baseline_features(path: Path) -> tuple[dict[str, str], dict[str, list[float]]]:
    rows = _read_csv(path)
    labels: dict[str, str] = {}
    values: dict[str, dict[str, float]] = {}
    for row in rows:
        patient = row.get("patient_id", "")
        group = row.get("group", "")
        feature = row.get("feature", "")
        value = float(row.get("value", "nan"))
        if (
            not patient
            or group not in {"MSI", "MSS"}
            or not feature
            or not math.isfinite(value)
            or labels.setdefault(patient, group) != group
            or feature in values.setdefault(patient, {})
        ):
            raise CategoricalPairPatientError("baseline patient feature row is invalid")
        values[patient][feature] = value
    features = sorted(next(iter(values.values())))
    if any(sorted(row) != features for row in values.values()):
        raise CategoricalPairPatientError("baseline patient features are incomplete")
    return labels, {
        patient: [row[feature] for feature in features]
        for patient, row in sorted(values.items())
    }


def _result_features(
    document: dict[str, Any], source: str, target: str
) -> tuple[dict[str, float], list[dict[str, str]]]:
    if (
        document.get("format") != "marklab.categorical-pair/1"
        or document.get("source_level") != source
        or document.get("target_level") != target
        or int(document.get("source_count", 0)) <= 0
        or int(document.get("target_count", 0)) <= 0
    ):
        raise CategoricalPairPatientError("typed categorical-pair result identity differs")
    inference = document.get("inference")
    if (
        not isinstance(inference, dict)
        or inference.get("permutations_requested") != WITHIN_PATTERN_PERMUTATIONS
        or inference.get("permutations_completed") != WITHIN_PATTERN_PERMUTATIONS
    ):
        raise CategoricalPairPatientError("categorical-pair random-labeling execution differs")
    expected = float(document.get("expected_random_label_connection", "nan"))
    curve = document.get("curve")
    if not math.isfinite(expected) or not isinstance(curve, list) or len(curve) != len(RADII_UM):
        raise CategoricalPairPatientError("categorical-pair curve shape or expectation differs")
    pair = _pair_id(source, target)
    features: dict[str, float] = {}
    unavailable: list[dict[str, str]] = []
    for radius, point in zip(RADII_UM, curve):
        if not isinstance(point, dict) or float(point.get("radius_um", "nan")) != radius:
            raise CategoricalPairPatientError("categorical-pair radius identity differs")
        connection = point.get("connection_probability")
        cross_k = point.get("cross_k")
        theoretical = float(point.get("theoretical_cross_k", "nan"))
        connection_name = f"{pair}.connection_excess.r{radius:g}um"
        cross_k_name = f"{pair}.cross_k_relative_excess.r{radius:g}um"
        if (
            point.get("connection_inference_eligible") is True
            and isinstance(connection, (int, float))
            and math.isfinite(connection)
        ):
            features[connection_name] = float(connection) - expected
        else:
            unavailable.append(
                {"endpoint": connection_name, "reason": "not_jointly_inference_eligible"}
            )
        if (
            point.get("cross_k_inference_eligible") is True
            and isinstance(cross_k, (int, float))
            and math.isfinite(cross_k)
            and math.isfinite(theoretical)
            and theoretical > 0.0
        ):
            features[cross_k_name] = float(cross_k) / theoretical - 1.0
        else:
            unavailable.append(
                {"endpoint": cross_k_name, "reason": "not_jointly_inference_eligible"}
            )
    if any(not math.isfinite(value) for value in features.values()):
        raise CategoricalPairPatientError("categorical-pair feature is non-finite")
    return features, unavailable


def _cross_g_result_features(
    document: dict[str, Any], source: str, target: str, correction: str
) -> tuple[dict[str, float], list[dict[str, str]]]:
    if (
        document.get("source_level") != source
        or document.get("target_level") != target
        or document.get("kernel") != "epanechnikov"
        or document.get("edge_correction") != correction
        or float(document.get("bandwidth_um", "nan")) != CROSS_G_BANDWIDTH_UM
        or int(document.get("source_count", 0)) <= 0
        or int(document.get("target_count", 0)) <= 0
    ):
        raise CategoricalPairPatientError("typed categorical cross-g result identity differs")
    inference = document.get("inference")
    if (
        not isinstance(inference, dict)
        or inference.get("null_model") != "random_labeling"
        or inference.get("permutation_unit") != "complete_categorical_row"
        or inference.get("permutations_completed") != WITHIN_PATTERN_PERMUTATIONS
    ):
        raise CategoricalPairPatientError("categorical cross-g random-labeling execution differs")
    curve = document.get("curve")
    if not isinstance(curve, list) or len(curve) != len(RADII_UM):
        raise CategoricalPairPatientError("categorical cross-g curve shape differs")
    pair = _pair_id(source, target)
    features: dict[str, float] = {}
    unavailable: list[dict[str, str]] = []
    for radius, point in zip(RADII_UM, curve):
        if not isinstance(point, dict) or float(point.get("radius_um", "nan")) != radius:
            raise CategoricalPairPatientError("categorical cross-g radius identity differs")
        cross_g = point.get("cross_g")
        theoretical = float(point.get("theoretical_cross_g", "nan"))
        endpoint = f"{pair}.cross_g_relative_excess.r{radius:g}um"
        if (
            point.get("inference_eligible") is True
            and isinstance(cross_g, (int, float))
            and not isinstance(cross_g, bool)
            and math.isfinite(cross_g)
            and math.isfinite(theoretical)
            and theoretical > 0.0
        ):
            features[endpoint] = float(cross_g) / theoretical - 1.0
        else:
            unavailable.append(
                {"endpoint": endpoint, "reason": "not_jointly_inference_eligible"}
            )
    if any(not math.isfinite(value) for value in features.values()):
        raise CategoricalPairPatientError("categorical cross-g feature is non-finite")
    return features, unavailable


def summarize(
    prepared: Path,
    execution: Path,
    baseline_path: Path,
    marklab: Path,
    output: Path,
    seed: int,
    *,
    analysis: str = CATEGORICAL_PAIR_ANALYSIS,
) -> dict[str, Any]:
    """Reduce slides inside patients and run held-out and Max-T population inference."""
    _validate_analysis(analysis)
    prepared = prepared.resolve()
    execution = execution.resolve()
    baseline_path = baseline_path.resolve()
    marklab = marklab.resolve()
    output = output.resolve()
    if output.exists() or output.is_symlink():
        raise CategoricalPairPatientError(f"output already exists: {output}")
    execution_manifest = _read_json(execution / "execution_manifest.json")
    replay_manifest = _read_json(execution / "replay_manifest.json")
    expected_execution_schema = (
        "marklab_crc_categorical_pair_patient_execution"
        if analysis == CATEGORICAL_PAIR_ANALYSIS
        else f"marklab_crc_{_analysis_token(analysis)}_patient_execution"
    )
    if (
        execution_manifest.get("schema_name") != expected_execution_schema
        or replay_manifest.get("schema_name") != expected_execution_schema
        or execution_manifest.get("cache_status_counts") != {"miss": 64}
        or replay_manifest.get("cache_status_counts") != {"hit": 64}
        or replay_manifest.get("all_replay_bytes_equal") is not True
        or replay_manifest.get("all_ledgers_one_execution") is not True
    ):
        raise CategoricalPairPatientError("categorical-pair durable miss/hit proof differs")
    manifest = _read_csv(prepared / "manifest.csv")
    labels: dict[str, str] = {}
    pattern_counts: dict[str, int] = {}
    for row in manifest:
        patient = row["patient_id"]
        group = row["group"]
        if labels.setdefault(patient, group) != group:
            raise CategoricalPairPatientError("patient molecular group is inconsistent")
        pattern_counts[patient] = pattern_counts.get(patient, 0) + 1
    if len(labels) != 8 or set(pattern_counts.values()) != {2}:
        raise CategoricalPairPatientError("summary requires eight patients and two slides each")
    baseline_labels, baseline = _baseline_features(baseline_path)
    if baseline_labels != labels:
        raise CategoricalPairPatientError("baseline and pair patient identities differ")

    specimen_features: dict[str, dict[str, float]] = {row["pattern_id"]: {} for row in manifest}
    unavailable_rows: list[dict[str, str]] = []
    result_paths: list[Path] = []
    for row in manifest:
        pattern = row["pattern_id"]
        for source, target in PAIR_FAMILIES:
            pair = _pair_id(source, target)
            path = execution / "results" / pair / f"{pattern}.json"
            if analysis == CATEGORICAL_PAIR_ANALYSIS:
                features, unavailable = _result_features(_read_json(path), source, target)
            else:
                features, unavailable = _cross_g_result_features(
                    _read_json(path), source, target, _correction_name(analysis)
                )
            overlap = set(specimen_features[pattern]) & set(features)
            if overlap:
                raise CategoricalPairPatientError("categorical-pair endpoint is duplicated")
            specimen_features[pattern].update(features)
            unavailable_rows.extend(
                {"pattern_id": pattern, **record} for record in unavailable
            )
            result_paths.append(path)
    if analysis == CATEGORICAL_PAIR_ANALYSIS:
        declared = {
            f"{_pair_id(source, target)}.{component}.r{radius:g}um"
            for source, target in PAIR_FAMILIES
            for component in ("connection_excess", "cross_k_relative_excess")
            for radius in RADII_UM
        }
        block_name = "categorical_pair"
        only_model_name = "categorical_pair_only"
        augmented_model_name = "m0_m3_categorical_pair"
        summary_schema = "marklab_crc_categorical_pair_patient_summary"
        bundle_schema = "marklab_crc_categorical_pair_patient_bundle"
    else:
        declared = {
            f"{_pair_id(source, target)}.cross_g_relative_excess.r{radius:g}um"
            for source, target in PAIR_FAMILIES
            for radius in RADII_UM
        }
        token = _analysis_token(analysis)
        block_name = token
        only_model_name = f"{token}_only"
        augmented_model_name = f"m0_m3_{token}"
        summary_schema = f"marklab_crc_{token}_patient_summary"
        bundle_schema = f"marklab_crc_{token}_patient_bundle"
    complete = set.intersection(*(set(row) for row in specimen_features.values()))
    incomplete = declared - complete
    unavailable_rows.extend(
        {
            "pattern_id": "all_patterns",
            "endpoint": endpoint,
            "reason": "not_available_in_every_nested_slide",
        }
        for endpoint in sorted(incomplete)
    )
    admitted = sorted(complete)
    if not admitted:
        raise CategoricalPairPatientError("no categorical-pair endpoint is complete")
    summary_owner = _load_summary_owner()
    lane = summary_owner.load_lane_module()
    specimen_features = {
        pattern: {endpoint: values[endpoint] for endpoint in admitted}
        for pattern, values in specimen_features.items()
    }
    patient_features = summary_owner.patient_means(specimen_features, manifest)
    constant = {
        endpoint
        for endpoint in admitted
        if len({patient_features[patient][endpoint] for patient in patient_features}) == 1
    }
    unavailable_rows.extend(
        {
            "pattern_id": "all_patients",
            "endpoint": endpoint,
            "reason": "zero_between_patient_variance",
        }
        for endpoint in sorted(constant)
    )
    admitted = [endpoint for endpoint in admitted if endpoint not in constant]
    if not admitted:
        raise CategoricalPairPatientError("no estimable patient categorical-pair endpoint remains")
    specimen_features = {
        pattern: {endpoint: values[endpoint] for endpoint in admitted}
        for pattern, values in specimen_features.items()
    }
    patient_features = summary_owner.patient_means(specimen_features, manifest)
    nested_slide_stability = lane.feature_stability(
        patient_features,
        summary_owner.leave_one_specimen_alternatives(specimen_features, manifest),
    )
    pair_block = {
        patient: [values[endpoint] for endpoint in admitted]
        for patient, values in patient_features.items()
    }
    model_blocks = {
        "m0_m3_nonspatial": {"baseline": baseline},
        only_model_name: {block_name: pair_block},
        augmented_model_name: {
            "baseline": baseline,
            block_name: pair_block,
        },
    }
    models: dict[str, Any] = {}
    for index, (name, blocks) in enumerate(model_blocks.items()):
        result = lane.heldout_model(labels, blocks, PCA_COMPONENTS)
        result["uncertainty"] = summary_owner.model_uncertainty(result, seed + index)
        result["whole_patient_permutation"] = summary_owner.exact_label_permutation(
            labels, blocks, lane
        )
        models[name] = result
    increment = summary_owner.incremental_summary(
        models["m0_m3_nonspatial"],
        models[augmented_model_name],
        seed + 100,
        lane,
    )

    staging = output.with_name(f".{output.name}.{os.getpid()}.tmp")
    if staging.exists() or staging.is_symlink():
        raise CategoricalPairPatientError(f"staging path already exists: {staging}")
    staging.mkdir(parents=True)
    max_t_rows = [
        {
            "patient_id": patient,
            "group": labels[patient],
            "endpoint": endpoint,
            "value": patient_features[patient][endpoint],
        }
        for patient in sorted(labels)
        for endpoint in admitted
    ]
    max_t_input = staging / "patient_endpoints.csv"
    max_t_output = staging / "population_max_t.json"
    _write_csv(max_t_input, list(max_t_rows[0]), max_t_rows)
    command = [
        str(marklab),
        "cohort",
        "max-t",
        "--input",
        str(max_t_input),
        "--group-a",
        "MSI",
        "--group-b",
        "MSS",
        "--permutations",
        str(POPULATION_PERMUTATIONS),
        "--seed",
        str(seed),
        "--alpha",
        str(ALPHA),
        "--step-down",
        "--out",
        str(max_t_output),
    ]
    completed = subprocess.run(command, capture_output=True, text=True, check=False)
    if completed.returncode != 0:
        raise CategoricalPairPatientError(
            f"patient Max-T failed: {completed.stderr.strip()}"
        )
    max_t = _read_json(max_t_output)
    if (
        max_t.get("format") != "marklab.cohort_max_t"
        or max_t.get("design", {}).get("randomization_unit") != "patient"
        or max_t.get("design", {}).get("correction") != "step_down_max_t"
        or max_t.get("permutations", {}).get("completed") != POPULATION_PERMUTATIONS
        or len(max_t.get("endpoints", [])) != len(admitted)
    ):
        raise CategoricalPairPatientError("patient Max-T result identity differs")

    specimen_rows = [
        {
            "cohort": "CPTAC_COAD_CellViT",
            "patient_id": row["patient_id"],
            "specimen_id": row["pattern_id"],
            "feature": endpoint,
            "value": specimen_features[row["pattern_id"]][endpoint],
        }
        for row in manifest
        for endpoint in admitted
    ]
    patient_rows = [
        {
            "cohort": "CPTAC_COAD_CellViT",
            "patient_id": patient,
            "group": labels[patient],
            "feature": endpoint,
            "value": patient_features[patient][endpoint],
        }
        for patient in sorted(labels)
        for endpoint in admitted
    ]
    heldout_rows = [
        {"model": name, **row}
        for name, result in models.items()
        for row in result["predictions"]
    ]
    _write_csv(
        staging / "specimen_fingerprints.csv", list(specimen_rows[0]), specimen_rows
    )
    _write_csv(staging / "patient_fingerprints.csv", list(patient_rows[0]), patient_rows)
    _write_csv(staging / "heldout_predictions.csv", list(heldout_rows[0]), heldout_rows)
    _write_json(staging / "unavailable_endpoints.json", {"endpoints": unavailable_rows})
    source_digest = hashlib.sha256()
    for path in sorted(result_paths + [prepared / "design.json", baseline_path]):
        source_digest.update(_sha256(path).encode("ascii"))
        source_digest.update(b"\n")
    summary = {
        "schema_name": summary_schema,
        "schema_version": "1.0",
        "population_unit": "patient",
        "pattern_unit": "slide_nested_within_patient",
        "patient_count": len(labels),
        "pattern_count": len(manifest),
        "pair_family_count": len(PAIR_FAMILIES),
        "declared_endpoint_count": len(declared),
        "admitted_endpoint_count": len(admitted),
        "nested_slide_stability": nested_slide_stability,
        "models": models,
        "incremental_information": increment,
        "population_max_t": max_t,
        "durable_replay": {
            "miss_count": execution_manifest["job_count"],
            "backend_disabled_hit_count": replay_manifest["job_count"],
            "all_result_bytes_equal": True,
            "all_ledgers_one_execution": True,
        },
        "leakage_checks": {
            "patient_held_out": True,
            "preprocessing_inside_each_training_fold": True,
            "slides_nested_inside_patients": True,
            "site_held_out": "unavailable_exact_blocker_no_acquisition_site_field_in_admitted_CPTAC_manifest",
        },
        "interpretation_policy": "null_confounded_and_unstable_results_retained_without_endpoint_scale_subset_or_threshold_optimization",
        "claim_limitations": [
            "eight-patient resource-feasible exploratory subset",
            "two slides per patient are nested diagnostics and never population replicates",
            "hard CellViT classes are morphology-model predictions",
            "structurally unavailable and zero-variance endpoints are excluded before inference",
            "no clinical, causal, prospective, equivalence, or transportability claim",
        ],
        "source_result_sha256": source_digest.hexdigest(),
    }
    if analysis in CROSS_G_ANALYSES:
        if increment["balanced_accuracy_increment"] <= 0.0:
            summary["promotion_status"] = "nonincremental_not_promoted"
            summary["fusion_status"] = (
                "not_added_without_positive_incremental_information"
            )
        else:
            summary["promotion_status"] = "positive_increment_candidate_not_promoted"
            summary["fusion_status"] = "not_added_without_prespecified_stability_review"
    _write_json(staging / "summary.json", summary)
    artifacts = sorted(path for path in staging.rglob("*") if path.is_file())
    _write_json(
        staging / "manifest.json",
        {
            "schema_name": bundle_schema,
            "schema_version": "1.0",
            "population_unit": "patient",
            "source_result_sha256": summary["source_result_sha256"],
            "artifact_sha256": {
                path.relative_to(staging).as_posix(): _sha256(path) for path in artifacts
            },
            "result_format_compatibility": "0.3_preserved",
        },
    )
    os.rename(staging, output)
    return summary


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    prepare_parser = commands.add_parser("prepare")
    prepare_parser.add_argument("--marks", required=True, type=Path)
    prepare_parser.add_argument("--inference-root", required=True, type=Path)
    prepare_parser.add_argument("--out", required=True, type=Path)
    scalar_prepare_parser = commands.add_parser("prepare-scalar-variogram")
    scalar_prepare_parser.add_argument("--marks", required=True, type=Path)
    scalar_prepare_parser.add_argument("--inference-root", required=True, type=Path)
    scalar_prepare_parser.add_argument("--out", required=True, type=Path)
    execute_parser = commands.add_parser("execute")
    execute_parser.add_argument("--prepared", required=True, type=Path)
    execute_parser.add_argument("--marklab", required=True, type=Path)
    execute_parser.add_argument("--out", required=True, type=Path)
    execute_parser.add_argument("--maximum-processes", type=int, default=MAXIMUM_PROCESSES)
    execute_parser.add_argument("--timeout-seconds", type=int, default=300)
    execute_parser.add_argument("--replay", action="store_true")
    execute_parser.add_argument(
        "--analysis",
        choices=(CATEGORICAL_PAIR_ANALYSIS, *CROSS_G_ANALYSES),
        default=CATEGORICAL_PAIR_ANALYSIS,
    )
    summary_parser = commands.add_parser("summarize")
    summary_parser.add_argument("--prepared", required=True, type=Path)
    summary_parser.add_argument("--execution", required=True, type=Path)
    summary_parser.add_argument("--baseline", required=True, type=Path)
    summary_parser.add_argument("--marklab", required=True, type=Path)
    summary_parser.add_argument("--out", required=True, type=Path)
    summary_parser.add_argument("--seed", type=int, default=20260829)
    summary_parser.add_argument(
        "--analysis",
        choices=(CATEGORICAL_PAIR_ANALYSIS, *CROSS_G_ANALYSES),
        default=CATEGORICAL_PAIR_ANALYSIS,
    )
    return parser.parse_args()


def main() -> None:
    arguments = parse_args()
    if arguments.command == "prepare":
        prepare(arguments.marks, arguments.inference_root, arguments.out)
    elif arguments.command == "prepare-scalar-variogram":
        prepare_scalar_variogram(
            arguments.marks, arguments.inference_root, arguments.out
        )
    elif arguments.command == "execute":
        execute(
            arguments.prepared,
            arguments.marklab,
            arguments.out,
            arguments.maximum_processes,
            arguments.timeout_seconds,
            replay=arguments.replay,
            analysis=arguments.analysis,
        )
    elif arguments.command == "summarize":
        summarize(
            arguments.prepared,
            arguments.execution,
            arguments.baseline,
            arguments.marklab,
            arguments.out,
            arguments.seed,
            analysis=arguments.analysis,
        )
    else:  # pragma: no cover
        raise AssertionError(f"unknown command: {arguments.command}")


if __name__ == "__main__":
    main()
