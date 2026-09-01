#!/usr/bin/env python3
"""Run projected CellViT local fields and reduce slides at the patient unit."""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
import csv
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import shutil
import subprocess
from typing import Any, Iterable


RADIUS_UM = 200.0
LOCAL_PERMUTATIONS = 99
POPULATION_PERMUTATIONS = 999
ALPHA = 0.05
DIMENSION = 16
STABILITY_VARIANTS = (
    "cell_subsample_80",
    "coordinate_jitter_1um",
    "nearby_radius_180um",
    "nearby_radius_220um",
)


class LocalFieldError(ValueError):
    """A local-field source, result, or patient design violates the frozen contract."""


def _read_csv(path: Path) -> list[dict[str, str]]:
    with path.open(encoding="utf-8", newline="") as stream:
        return list(csv.DictReader(stream))


def _write_csv(path: Path, fields: list[str], rows: Iterable[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=fields, lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def _read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except Exception as error:
        raise LocalFieldError(f"cannot decode JSON {path}: {error}") from error
    if not isinstance(value, dict):
        raise LocalFieldError(f"JSON document is not an object: {path}")
    return value


def _write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n",
        encoding="utf-8",
    )


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _stable_digest(namespace: str, *parts: object) -> bytes:
    digest = hashlib.sha256()
    digest.update(namespace.encode("utf-8"))
    digest.update(b"\0")
    for part in parts:
        digest.update(str(part).encode("utf-8"))
        digest.update(b"\0")
    return digest.digest()


def _exact_identity(value: object, label: str) -> str:
    if not isinstance(value, str) or not value or value.strip() != value:
        raise LocalFieldError(f"{label} must be exact and nonempty")
    return value


def _finite(value: object, label: str) -> float:
    try:
        parsed = float(value)
    except (TypeError, ValueError) as error:
        raise LocalFieldError(f"{label} is not numeric") from error
    if not math.isfinite(parsed):
        raise LocalFieldError(f"{label} is non-finite")
    return parsed


def admit_repeated_slides(
    projected_rows: list[dict[str, str]], labels: dict[str, str]
) -> list[dict[str, str]]:
    """Admit every labeled patient with at least two distinct projected slides."""
    by_patient: dict[str, set[str]] = {}
    for row in projected_rows:
        patient = _exact_identity(row.get("biological_unit"), "biological_unit")
        slide = _exact_identity(row.get("permutation_stratum"), "permutation_stratum")
        by_patient.setdefault(patient, set()).add(slide)
    admitted = []
    for patient in sorted(by_patient):
        if patient not in labels or len(by_patient[patient]) < 2:
            continue
        group = labels[patient]
        if group not in {"MSI", "MSS"}:
            raise LocalFieldError(f"patient {patient} has an unsupported molecular group")
        admitted.extend(
            {"patient_id": patient, "group": group, "slide_id": slide}
            for slide in sorted(by_patient[patient])
        )
    return admitted


def _load_adapter():
    path = Path(__file__).with_name("marklab_cellvit_cptac_results_adapter.py")
    spec = importlib.util.spec_from_file_location("marklab_cellvit_adapter", path)
    if spec is None or spec.loader is None:
        raise LocalFieldError("cannot load the canonical CellViT adapter")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _load_sibling_module(filename: str, module_name: str):
    path = Path(__file__).with_name(filename)
    spec = importlib.util.spec_from_file_location(module_name, path)
    if spec is None or spec.loader is None:
        raise LocalFieldError(f"cannot load canonical analysis owner: {filename}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _labels(path: Path) -> dict[str, str]:
    rows = _read_csv(path)
    if not rows or set(rows[0]) != {"patient_id", "class_name"}:
        raise LocalFieldError("molecular-label schema differs")
    result = {}
    for row in rows:
        patient = _exact_identity(row["patient_id"], "patient_id")
        group = row["class_name"]
        if group not in {"MSI", "MSS"} or patient in result:
            raise LocalFieldError("molecular labels are invalid or duplicated")
        result[patient] = group
    return result


def drop_radius_isolates(
    rows: list[dict[str, str]], radius_um: float
) -> tuple[list[dict[str, str]], list[str]]:
    """Return rows having another row within radius, using coordinates only."""
    if not math.isfinite(radius_um) or radius_um <= 0.0:
        raise LocalFieldError("isolate radius must be finite and positive")
    coordinates = [
        (_finite(row.get("x_um"), "x_um"), _finite(row.get("y_um"), "y_um"))
        for row in rows
    ]
    cells: dict[tuple[int, int], list[int]] = {}
    for index, (x_um, y_um) in enumerate(coordinates):
        key = (math.floor(x_um / radius_um), math.floor(y_um / radius_um))
        cells.setdefault(key, []).append(index)
    retained = []
    removed = []
    squared_radius = radius_um * radius_um
    for index, (x_um, y_um) in enumerate(coordinates):
        column = math.floor(x_um / radius_um)
        row = math.floor(y_um / radius_um)
        has_neighbor = any(
            other != index
            and (x_um - coordinates[other][0]) ** 2
            + (y_um - coordinates[other][1]) ** 2
            <= squared_radius
            for dx in (-1, 0, 1)
            for dy in (-1, 0, 1)
            for other in cells.get((column + dx, row + dy), ())
        )
        if has_neighbor:
            retained.append(rows[index])
        else:
            removed.append(_exact_identity(rows[index].get("cell_id"), "cell_id"))
    return retained, removed


def _selected_rows(
    path: Path, maximum_cells: int, seed: int
) -> tuple[list[dict[str, str]], int, int]:
    rows = _read_csv(path)
    fields = ["cell_id", "x_um", "y_um"] + [f"cellvit_pc_{index:03d}" for index in range(DIMENSION)]
    if not rows or list(rows[0]) != fields:
        raise LocalFieldError(f"canonical projected-cell schema differs: {path}")
    if len(rows) < 64:
        raise LocalFieldError(f"slide has fewer than 64 projected cells: {path}")
    for row in rows:
        _exact_identity(row["cell_id"], "cell_id")
        for field in fields[1:]:
            _finite(row[field], field)
    radius_eligible, source_isolates = drop_radius_isolates(rows, RADIUS_UM)
    selected = sorted(
        sorted(
            radius_eligible,
            key=lambda row: (
                _stable_digest("cellvit-patient-local-field-subsample", seed, row["cell_id"]),
                row["cell_id"],
            ),
        )[: min(maximum_cells, len(rows))],
        key=lambda row: row["cell_id"],
    )
    selected, selected_isolates = drop_radius_isolates(selected, RADIUS_UM)
    if len(selected) < 64:
        raise LocalFieldError(f"fewer than 64 radius-eligible selected cells remain: {path}")
    return selected, len(source_isolates), len(selected_isolates)


def prepare(
    projected_index: Path,
    labels_path: Path,
    cells_root: Path,
    inference_root: Path,
    output: Path,
    maximum_cells: int,
    seed: int,
) -> dict[str, Any]:
    """Create exact per-slide projected inputs and patch-union windows."""
    for path in (projected_index, labels_path, cells_root, inference_root):
        if not path.exists() or path.is_symlink():
            raise LocalFieldError(f"required source is absent or symlinked: {path}")
    if output.exists() or output.is_symlink():
        raise LocalFieldError(f"output already exists: {output}")
    if not 64 <= maximum_cells <= 2_000:
        raise LocalFieldError("maximum_cells must be between 64 and 2000")
    admitted = admit_repeated_slides(_read_csv(projected_index), _labels(labels_path))
    group_counts = {
        group: len({row["patient_id"] for row in admitted if row["group"] == group})
        for group in ("MSI", "MSS")
    }
    if min(group_counts.values(), default=0) < 2:
        raise LocalFieldError("admission requires at least two repeated-slide patients per group")
    adapter = _load_adapter()
    from shapely.geometry import Point

    staging = output.with_name(f".{output.name}.{os.getpid()}.tmp")
    staging.mkdir(parents=True)
    manifest = []
    try:
        for row in admitted:
            slide = row["slide_id"]
            cell_source = cells_root / f"{slide}.csv"
            selected, source_isolates, selected_isolates = _selected_rows(
                cell_source, maximum_cells, seed
            )
            slide_root = inference_root / slide
            inference_manifest_path = slide_root / "inference_manifest.json"
            inference_manifest = _read_json(inference_manifest_path)
            adapter.manifest_outputs_match(slide_root, inference_manifest)
            cell_payload = adapter.read_cells(adapter.one_file(slide_root, "*_cells.json.snappy"))
            metadata = cell_payload.get("wsi_metadata")
            if not isinstance(metadata, dict):
                raise LocalFieldError(f"slide {slide} lacks CellViT WSI metadata")
            geometry, window = adapter.patch_window(metadata)
            if any(
                not window.covers(Point(float(cell["x_um"]), float(cell["y_um"])))
                for cell in selected
            ):
                raise LocalFieldError(f"slide {slide} projected cell escapes its patch-union window")
            input_path = staging / "inputs" / f"{slide}.csv"
            window_path = staging / "windows" / f"{slide}.geojson"
            _write_csv(input_path, list(selected[0]), selected)
            _write_json(window_path, geometry)
            slide_seed = int.from_bytes(
                _stable_digest("cellvit-patient-local-field-seed", seed, slide)[:8], "big"
            )
            count = len(selected)
            maximum_edges = count * (count - 1)
            manifest.append(
                {
                    **row,
                    "cell_count": count,
                    "source_radius_isolates_excluded": source_isolates,
                    "selected_radius_isolates_excluded": selected_isolates,
                    "dimension": DIMENSION,
                    "radius_um": RADIUS_UM,
                    "permutations": LOCAL_PERMUTATIONS,
                    "seed": slide_seed,
                    "maximum_directed_edges": maximum_edges,
                    "maximum_permutation_edge_evaluations": maximum_edges
                    * LOCAL_PERMUTATIONS,
                    "input": input_path.relative_to(staging).as_posix(),
                    "input_sha256": _sha256(input_path),
                    "window": window_path.relative_to(staging).as_posix(),
                    "window_sha256": _sha256(window_path),
                    "source_cells": str(cell_source.resolve()),
                    "source_cells_sha256": _sha256(cell_source),
                    "source_inference_manifest": str(inference_manifest_path.resolve()),
                    "source_inference_manifest_sha256": _sha256(inference_manifest_path),
                }
            )
        _write_csv(staging / "manifest.csv", list(manifest[0]), manifest)
        design = {
            "schema_name": "marklab_cellvit_patient_local_field_design",
            "schema_version": "1.0",
            "population_unit": "patient",
            "specimen_unit": "slide_nested_within_patient",
            "selection": "all_molecularly_labeled_patients_with_at_least_two_projected_slides",
            "cell_selection": "label_blind_stable_sha256_without_replacement",
            "patient_count": len({row["patient_id"] for row in manifest}),
            "slide_count": len(manifest),
            "group_patient_counts": group_counts,
            "radius_um": RADIUS_UM,
            "local_permutations": LOCAL_PERMUTATIONS,
            "maximum_cells_per_slide": maximum_cells,
            "seed": seed,
            "projected_index": str(projected_index.resolve()),
            "projected_index_sha256": _sha256(projected_index),
            "molecular_labels": str(labels_path.resolve()),
            "molecular_labels_sha256": _sha256(labels_path),
            "claim_limit": "patient_level_exploratory_spatial_field_evidence",
        }
        _write_json(staging / "design.json", design)
        os.rename(staging, output)
        return design
    except Exception:
        if staging.exists():
            shutil.rmtree(staging)
        raise


def prepare_stability(prepared: Path, output: Path, seed: int) -> dict[str, Any]:
    """Create four coordinate-only, scale, and cell-subsample stability variants."""
    if output.exists() or output.is_symlink():
        raise LocalFieldError(f"output already exists: {output}")
    design = _read_json(prepared / "design.json")
    manifest = _read_csv(prepared / "manifest.csv")
    if design.get("schema_name") != "marklab_cellvit_patient_local_field_design":
        raise LocalFieldError("base local-field design differs")
    from shapely.geometry import Point, shape

    staging = output.with_name(f".{output.name}.{os.getpid()}.tmp")
    staging.mkdir(parents=True)
    variant_rows = []
    try:
        for source in manifest:
            slide = source["slide_id"]
            base_rows = _read_csv(prepared / source["input"])
            window_source = prepared / source["window"]
            window = shape(_read_json(window_source))
            window_path = staging / "windows" / f"{slide}.geojson"
            window_path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(window_source, window_path)
            variants: dict[str, tuple[list[dict[str, str]], float, int]] = {}

            subsample_count = max(64, math.floor(len(base_rows) * 0.8))
            subsample = sorted(
                sorted(
                    base_rows,
                    key=lambda row: (
                        _stable_digest(
                            "cellvit-patient-local-field-stability-subsample",
                            seed,
                            row["cell_id"],
                        ),
                        row["cell_id"],
                    ),
                )[:subsample_count],
                key=lambda row: row["cell_id"],
            )
            subsample, removed = drop_radius_isolates(subsample, RADIUS_UM)
            variants["cell_subsample_80"] = (subsample, RADIUS_UM, len(removed))

            jittered = []
            outside = 0
            for row in base_rows:
                copy = dict(row)
                offsets = []
                for axis in ("x", "y"):
                    raw = _stable_digest(
                        "cellvit-patient-local-field-coordinate-jitter",
                        seed,
                        row["cell_id"],
                        axis,
                    )
                    unit = int.from_bytes(raw[:8], "big") / float((1 << 64) - 1)
                    offsets.append(2.0 * unit - 1.0)
                x_um = float(row["x_um"]) + offsets[0]
                y_um = float(row["y_um"]) + offsets[1]
                if not window.covers(Point(x_um, y_um)):
                    outside += 1
                    continue
                copy["x_um"] = format(x_um, ".17g")
                copy["y_um"] = format(y_um, ".17g")
                jittered.append(copy)
            jittered, removed = drop_radius_isolates(jittered, RADIUS_UM)
            variants["coordinate_jitter_1um"] = (
                jittered,
                RADIUS_UM,
                outside + len(removed),
            )

            low_radius, removed = drop_radius_isolates(base_rows, 180.0)
            variants["nearby_radius_180um"] = (low_radius, 180.0, len(removed))
            variants["nearby_radius_220um"] = (base_rows, 220.0, 0)

            for variant in STABILITY_VARIANTS:
                rows, radius_um, excluded = variants[variant]
                if len(rows) < 64:
                    raise LocalFieldError(
                        f"variant {variant}/{slide} retains fewer than 64 eligible cells"
                    )
                input_path = staging / "inputs" / variant / f"{slide}.csv"
                _write_csv(input_path, list(rows[0]), rows)
                count = len(rows)
                maximum_edges = count * (count - 1)
                variant_seed = int.from_bytes(
                    _stable_digest(
                        "cellvit-patient-local-field-variant-seed", seed, variant, slide
                    )[:8],
                    "big",
                )
                variant_rows.append(
                    {
                        "patient_id": source["patient_id"],
                        "group": source["group"],
                        "slide_id": slide,
                        "variant": variant,
                        "cell_count": count,
                        "variant_cells_excluded": excluded,
                        "dimension": DIMENSION,
                        "radius_um": radius_um,
                        "permutations": LOCAL_PERMUTATIONS,
                        "seed": variant_seed,
                        "maximum_directed_edges": maximum_edges,
                        "maximum_permutation_edge_evaluations": maximum_edges
                        * LOCAL_PERMUTATIONS,
                        "input": input_path.relative_to(staging).as_posix(),
                        "input_sha256": _sha256(input_path),
                        "window": window_path.relative_to(staging).as_posix(),
                        "window_sha256": _sha256(window_path),
                    }
                )
        _write_csv(staging / "manifest.csv", list(variant_rows[0]), variant_rows)
        result = {
            "schema_name": "marklab_cellvit_patient_local_field_stability_design",
            "schema_version": "1.0",
            "population_unit": "patient",
            "specimen_unit": "slide_nested_within_patient",
            "variants": list(STABILITY_VARIANTS),
            "patient_count": design["patient_count"],
            "slide_count": design["slide_count"],
            "job_count": len(variant_rows),
            "seed": seed,
            "base_design_sha256": _sha256(prepared / "design.json"),
        }
        _write_json(staging / "design.json", result)
        os.rename(staging, output)
        return result
    except Exception:
        if staging.exists():
            shutil.rmtree(staging)
        raise


def _run_slide(marklab: Path, prepared: Path, output: Path, row: dict[str, str]) -> dict[str, Any]:
    slide = row["slide_id"]
    variant = row.get("variant", "baseline")
    project = output / "projects" / variant / slide
    miss = output / "miss" / variant / f"{slide}.json"
    hit = output / "hit" / variant / f"{slide}.json"
    result = output / "results" / variant / f"{slide}.json"
    ledger = project / "executions.jsonl"
    if all(path.is_file() and not path.is_symlink() for path in (miss, hit, result)):
        if (
            miss.read_bytes() != hit.read_bytes()
            or miss.read_bytes() != result.read_bytes()
            or not ledger.is_file()
            or len(ledger.read_text(encoding="utf-8").splitlines()) != 1
        ):
            raise LocalFieldError(f"slide {variant}/{slide} resumed durable state differs")
        return {
            "slide_id": slide,
            "variant": variant,
            "result_sha256": _sha256(result),
            "ledger_rows": 1,
            "resumed": True,
        }
    if any(path.exists() or path.is_symlink() for path in (miss, hit, result)):
        raise LocalFieldError(f"slide {variant}/{slide} has an incomplete published result set")
    if ledger.is_file() and ledger.read_text(encoding="utf-8").splitlines():
        raise LocalFieldError(f"slide {variant}/{slide} has a committed ledger without outputs")
    for path in (miss, hit, result):
        path.parent.mkdir(parents=True, exist_ok=True)
    command = [
        str(marklab),
        "project",
        "local-multivariate-moran",
        "--project",
        str(project),
        "--input",
        str(prepared / row["input"]),
        "--window",
        str(prepared / row["window"]),
        "--radius-um",
        row["radius_um"],
        "--permutations",
        row["permutations"],
        "--seed",
        row["seed"],
        "--maximum-points",
        row["cell_count"],
        "--maximum-dimension",
        row["dimension"],
        "--maximum-directed-edges",
        row["maximum_directed_edges"],
        "--maximum-permutation-edge-evaluations",
        row["maximum_permutation_edge_evaluations"],
        "--memory-budget-mib",
        "64",
    ]
    first = subprocess.run(
        command + ["--out", str(miss)], capture_output=True, text=True, check=False
    )
    if first.returncode != 0 or "cache_status=miss" not in first.stderr:
        raise LocalFieldError(f"slide {slide} durable miss failed: {first.stderr.strip()}")
    environment = dict(os.environ)
    environment["MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION"] = "1"
    second = subprocess.run(
        command + ["--out", str(hit)],
        capture_output=True,
        text=True,
        check=False,
        env=environment,
    )
    if second.returncode != 0 or "cache_status=hit" not in second.stderr:
        raise LocalFieldError(f"slide {slide} durable hit failed: {second.stderr.strip()}")
    if miss.read_bytes() != hit.read_bytes():
        raise LocalFieldError(f"slide {slide} durable replay bytes differ")
    if len(ledger.read_text(encoding="utf-8").splitlines()) != 1:
        raise LocalFieldError(f"slide {slide} durable ledger does not contain one execution")
    shutil.copyfile(miss, result)
    return {
        "slide_id": slide,
        "variant": variant,
        "result_sha256": _sha256(result),
        "ledger_rows": 1,
        "resumed": False,
    }


def execute(
    prepared: Path, marklab: Path, output: Path, workers: int, resume: bool = False
) -> dict[str, Any]:
    """Run bounded durable per-slide local fields and prove fresh-process replay."""
    if (output.exists() or output.is_symlink()) and not resume:
        raise LocalFieldError(f"output already exists: {output}")
    if resume and (not output.is_dir() or output.is_symlink()):
        raise LocalFieldError(f"resume output is not one existing directory: {output}")
    if not marklab.is_file() or marklab.is_symlink():
        raise LocalFieldError("marklab must be one regular executable")
    if not 1 <= workers <= 8:
        raise LocalFieldError("workers must be between 1 and 8")
    design = _read_json(prepared / "design.json")
    manifest = _read_csv(prepared / "manifest.csv")
    if design.get("schema_name") not in {
        "marklab_cellvit_patient_local_field_design",
        "marklab_cellvit_patient_local_field_stability_design",
    }:
        raise LocalFieldError("prepared local-field design differs")
    output.mkdir(parents=True, exist_ok=resume)
    completed = []
    try:
        with ThreadPoolExecutor(max_workers=workers) as pool:
            futures = {
                pool.submit(_run_slide, marklab.resolve(), prepared.resolve(), output.resolve(), row): row
                for row in manifest
            }
            for future in as_completed(futures):
                completed.append(future.result())
        completed.sort(key=lambda row: (row["variant"], row["slide_id"]))
        result = {
            "schema_name": "marklab_cellvit_patient_local_field_execution",
            "schema_version": "1.0",
            "job_count": len(completed),
            "cache_status_counts": {"miss": len(completed), "hit": len(completed)},
            "all_replay_bytes_equal": True,
            "all_ledgers_one_execution": True,
            "workers": workers,
            "resumed_job_count": sum(row["resumed"] for row in completed),
            "new_job_count": sum(not row["resumed"] for row in completed),
            "results": completed,
        }
        _write_json(output / "execution_manifest.json", result)
        return result
    except Exception:
        raise


def local_result_features(
    document: dict[str, Any], expected_points: int, expected_radius_um: float
) -> dict[str, float]:
    """Extract fixed sign-invariant slide endpoints from one typed local result."""
    locations = document.get("locations")
    if (
        document.get("format") != "marklab.local_multivariate_moran"
        or document.get("version") != 1
        or document.get("population_claim") != "within_specimen_field_diagnostic_only"
        or document.get("point_count") != expected_points
        or not isinstance(document.get("dimension"), int)
        or document["dimension"] < 2
        or _finite(document.get("radius_um"), "radius_um") != expected_radius_um
        or not isinstance(locations, list)
        or len(locations) != expected_points
    ):
        raise LocalFieldError("typed local multivariate result identity differs")
    statistics = []
    significant = 0
    for row in locations:
        if not isinstance(row, dict):
            raise LocalFieldError("local result location row is malformed")
        statistic = abs(_finite(row.get("statistic"), "local statistic"))
        adjusted = _finite(row.get("adjusted_p_value"), "adjusted p-value")
        if not 0.0 <= adjusted <= 1.0:
            raise LocalFieldError("adjusted p-value is outside [0,1]")
        statistics.append(statistic)
        significant += adjusted <= ALPHA
    global_maximum = _finite(
        document.get("global_max_abs_statistic"), "global maximum statistic"
    )
    global_p = _finite(document.get("global_max_abs_p_value"), "global p-value")
    if not 0.0 <= global_p <= 1.0 or global_maximum != max(statistics):
        raise LocalFieldError("global local-field statistic or p-value differs")
    return {
        "local_global_max_abs": global_maximum,
        "local_mean_abs": math.fsum(statistics) / len(statistics),
        "local_adjusted_significant_fraction": significant / len(statistics),
    }


def adaptive_result_features(
    document: dict[str, Any], expected_window_sha256: str, expected_regions: int
) -> dict[str, float]:
    """Extract fixed sign-invariant endpoints from one fitted adaptive SPDE result."""
    window = document.get("window")
    mesh = document.get("mesh")
    factor = document.get("spatial_factor")
    if (
        document.get("format") != "marklab.adaptive_window_spde"
        or document.get("version") != 1
        or document.get("claim_status") != "fitted_arbitrary_window_spde_diagnostic"
        or not isinstance(window, dict)
        or window.get("canonical_sha256") != expected_window_sha256
        or not isinstance(mesh, dict)
        or not isinstance(factor, dict)
        or factor.get("factor_count") != 1
    ):
        raise LocalFieldError("typed adaptive-SPDE result identity differs")
    projected = factor.get("projected_region_factor")
    loadings = factor.get("loadings")
    if (
        not isinstance(projected, list)
        or len(projected) != expected_regions
        or not isinstance(loadings, list)
        or not loadings
    ):
        raise LocalFieldError("adaptive-SPDE fitted field shape differs")
    projected = [_finite(value, "projected factor") for value in projected]
    loadings = [_finite(value, "factor loading") for value in loadings]
    center = math.fsum(projected) / len(projected)
    factor_sd = math.sqrt(
        math.fsum((value - center) ** 2 for value in projected) / len(projected)
    )
    reconstruction_rmse = _finite(
        factor.get("reconstruction_rmse"), "SPDE reconstruction RMSE"
    )
    area_error = _finite(mesh.get("relative_area_error"), "mesh relative area error")
    gradient = _finite(factor.get("gradient_maximum"), "SPDE gradient maximum")
    iterations = factor.get("iterations")
    if (
        factor_sd < 0.0
        or reconstruction_rmse < 0.0
        or not 0.0 <= area_error <= 1.0
        or gradient < 0.0
        or not isinstance(iterations, int)
        or iterations < 1
    ):
        raise LocalFieldError("adaptive-SPDE diagnostics are invalid")
    return {
        "spde_factor_sd": factor_sd,
        "spde_loading_l2": math.sqrt(math.fsum(value * value for value in loadings)),
        "spde_reconstruction_rmse": reconstruction_rmse,
    }


def _median(values: list[float]) -> float:
    if not values:
        raise LocalFieldError("median requires finite values")
    ordered = sorted(_finite(value, "median value") for value in values)
    middle = len(ordered) // 2
    return (
        ordered[middle]
        if len(ordered) % 2
        else (ordered[middle - 1] + ordered[middle]) / 2.0
    )


def _ranks(values: list[float]) -> list[float]:
    order = sorted(range(len(values)), key=lambda index: (values[index], index))
    result = [0.0] * len(values)
    start = 0
    while start < len(order):
        end = start + 1
        while end < len(order) and values[order[start]] == values[order[end]]:
            end += 1
        rank = (start + end - 1) / 2.0 + 1.0
        for index in order[start:end]:
            result[index] = rank
        start = end
    return result


def _spearman(left: list[float], right: list[float]) -> float:
    if len(left) != len(right) or len(left) < 2:
        raise LocalFieldError("rank correlation requires aligned nontrivial vectors")
    left_ranks = _ranks(left)
    right_ranks = _ranks(right)
    left_mean = math.fsum(left_ranks) / len(left_ranks)
    right_mean = math.fsum(right_ranks) / len(right_ranks)
    covariance = math.fsum(
        (left_value - left_mean) * (right_value - right_mean)
        for left_value, right_value in zip(left_ranks, right_ranks)
    )
    left_ss = math.fsum((value - left_mean) ** 2 for value in left_ranks)
    right_ss = math.fsum((value - right_mean) ** 2 for value in right_ranks)
    if left_ss <= 0.0 or right_ss <= 0.0:
        raise LocalFieldError("rank correlation is undefined for a constant vector")
    return covariance / math.sqrt(left_ss * right_ss)


def variant_stability(
    baseline: dict[str, dict[str, float]],
    variants: dict[str, dict[str, dict[str, float]]],
) -> dict[str, dict[str, float | int]]:
    """Compare prespecified slide variants with exact baseline slide identities."""
    if not baseline or not variants:
        raise LocalFieldError("variant stability requires baseline and variant fields")
    slide_ids = set(baseline)
    result = {}
    for variant, rows in sorted(variants.items()):
        if set(rows) != slide_ids:
            raise LocalFieldError(f"variant {variant} does not contain the exact baseline slides")
        correlations = []
        relative_differences = []
        for slide in sorted(slide_ids):
            endpoints = sorted(baseline[slide])
            if endpoints != sorted(rows[slide]) or len(endpoints) < 2:
                raise LocalFieldError(f"variant {variant}/{slide} endpoint identity differs")
            left = [_finite(baseline[slide][endpoint], endpoint) for endpoint in endpoints]
            right = [_finite(rows[slide][endpoint], endpoint) for endpoint in endpoints]
            correlations.append(_spearman(left, right))
            rms = math.sqrt(
                math.fsum((a - b) ** 2 for a, b in zip(left, right)) / len(left)
            )
            baseline_rms = math.sqrt(math.fsum(value * value for value in left) / len(left))
            if baseline_rms <= 0.0:
                raise LocalFieldError(f"baseline {slide} has zero endpoint magnitude")
            relative_differences.append(rms / baseline_rms)
        result[variant] = {
            "comparison_count": len(correlations),
            "median_rank_spearman": _median(correlations),
            "minimum_rank_spearman": min(correlations),
            "median_relative_rms_difference": _median(relative_differences),
            "maximum_relative_rms_difference": max(relative_differences),
        }
    return result


def patient_endpoint_rows(
    manifest: list[dict[str, str]], results: Path, radius_um: float
) -> list[dict[str, Any]]:
    rows = []
    for source in manifest:
        path = results / f"{source['slide_id']}.json"
        if not path.is_file() or path.is_symlink():
            raise LocalFieldError(f"missing local result: {path}")
        features = local_result_features(
            _read_json(path), int(source["cell_count"]), radius_um
        )
        rows.extend(
            {
                "patient_id": source["patient_id"],
                "specimen_id": source["slide_id"],
                "group": source["group"],
                "endpoint": endpoint,
                "value": value,
            }
            for endpoint, value in sorted(features.items())
        )
    return rows


def _feature_map(
    manifest: list[dict[str, str]], results: Path, nested_variant_paths: bool
) -> dict[str, dict[str, dict[str, float]]]:
    mapped: dict[str, dict[str, dict[str, float]]] = {}
    for row in manifest:
        variant = row.get("variant", "baseline")
        path = (
            results / variant / f"{row['slide_id']}.json"
            if nested_variant_paths
            else results / f"{row['slide_id']}.json"
        )
        if not path.is_file() or path.is_symlink():
            raise LocalFieldError(f"missing local result: {path}")
        features = local_result_features(
            _read_json(path), int(row["cell_count"]), float(row["radius_um"])
        )
        if row["slide_id"] in mapped.setdefault(variant, {}):
            raise LocalFieldError(f"duplicate variant/slide result: {variant}/{row['slide_id']}")
        mapped[variant][row["slide_id"]] = features
    return mapped


def _patient_means(
    slide_features: dict[str, dict[str, float]], manifest: list[dict[str, str]]
) -> dict[str, dict[str, float]]:
    ownership = {row["slide_id"]: row["patient_id"] for row in manifest}
    grouped: dict[str, list[dict[str, float]]] = {}
    for slide, features in slide_features.items():
        if slide not in ownership:
            raise LocalFieldError(f"feature slide is absent from the patient manifest: {slide}")
        grouped.setdefault(ownership[slide], []).append(features)
    result = {}
    for patient, rows in grouped.items():
        endpoints = sorted(rows[0])
        if any(sorted(row) != endpoints for row in rows):
            raise LocalFieldError(f"patient {patient} has incomplete variant endpoints")
        result[patient] = {
            endpoint: math.fsum(row[endpoint] for row in rows) / len(rows)
            for endpoint in endpoints
        }
    return result


def patient_model_blocks(
    slide_rows: list[dict[str, Any]],
    technical: dict[str, list[float]],
    local_field: dict[str, list[float]],
) -> dict[str, dict[str, list[float]]]:
    """Average slide summaries inside patients and bind complete model blocks."""
    grouped: dict[str, list[dict[str, Any]]] = {}
    for row in slide_rows:
        patient = _exact_identity(row.get("patient_id"), "patient_id")
        grouped.setdefault(patient, []).append(row)
    patients = set(grouped)
    if patients != set(technical) or patients != set(local_field):
        raise LocalFieldError("technical, field, and slide patient identities differ")
    if any(len(rows) < 2 for rows in grouped.values()):
        raise LocalFieldError("incremental blocks require at least two slides per patient")

    def patient_mean(name: str) -> dict[str, list[float]]:
        result = {}
        for patient, rows in grouped.items():
            vectors = [row.get(name) for row in rows]
            if (
                not vectors
                or not isinstance(vectors[0], list)
                or not vectors[0]
                or any(not isinstance(vector, list) or len(vector) != len(vectors[0]) for vector in vectors)
            ):
                raise LocalFieldError(f"patient {patient} has invalid {name} slide vectors")
            parsed = [[_finite(value, name) for value in vector] for vector in vectors]
            result[patient] = [
                math.fsum(vector[column] for vector in parsed) / len(parsed)
                for column in range(len(parsed[0]))
            ]
        return result

    parsed_technical = {
        patient: [_finite(value, "technical") for value in technical[patient]]
        for patient in sorted(patients)
    }
    parsed_field = {
        patient: [_finite(value, "local field") for value in local_field[patient]]
        for patient in sorted(patients)
    }
    if any(not values for values in parsed_technical.values()) or any(
        not values for values in parsed_field.values()
    ):
        raise LocalFieldError("technical and local-field blocks must be nonempty")
    return {
        "technical": parsed_technical,
        "composition": patient_mean("composition"),
        "nonspatial_embedding": patient_mean("embedding"),
        "local_field": parsed_field,
    }


def incremental_information(
    prepared: Path,
    inference_root: Path,
    clinical_path: Path,
    patient_summary_path: Path,
    output: Path,
    seed: int,
) -> dict[str, Any]:
    """Test local fields beyond technical, composition, and raw CellViT summaries."""
    if output.exists() or output.is_symlink():
        raise LocalFieldError(f"output already exists: {output}")
    manifest = _read_csv(prepared / "manifest.csv")
    labels: dict[str, str] = {}
    for row in manifest:
        if labels.setdefault(row["patient_id"], row["group"]) != row["group"]:
            raise LocalFieldError("patient molecular group differs across slides")
    if min(sum(group == label for group in labels.values()) for label in ("MSI", "MSS")) < 2:
        raise LocalFieldError("incremental evaluation requires at least two patients per group")
    patient_summary = _read_json(patient_summary_path)
    patient_result = patient_summary.get("patient_result")
    if not isinstance(patient_result, dict) or patient_result.get("format") != "marklab.cohort_patient_nested_fields":
        raise LocalFieldError("patient local-field result identity differs")
    endpoints = patient_result.get("endpoints")
    patient_rows = patient_result.get("patient_endpoints")
    if not isinstance(endpoints, list) or not isinstance(patient_rows, list):
        raise LocalFieldError("patient local-field endpoint table is absent")
    local_field = {
        row["patient_id"]: [_finite(value, "local field") for value in row["values"]]
        for row in patient_rows
    }
    if set(local_field) != set(labels):
        raise LocalFieldError("local-field and admitted patient identities differ")

    adapter = _load_adapter()
    lane = _load_sibling_module(
        "marklab_crc_graph_topology_final.py", "marklab_crc_graph_topology_final"
    )
    summary_owner = _load_sibling_module(
        "marklab_crc_graph_topology_summary.py", "marklab_crc_graph_topology_summary"
    )
    technical = {}
    technical_exclusions = []
    for patient in sorted(labels):
        try:
            technical.update(summary_owner.clinical_features(clinical_path, {patient}))
        except Exception as error:
            technical_exclusions.append(
                {
                    "patient_id": patient,
                    "reason": f"incomplete_age_or_gender_technical_covariate: {error}",
                }
            )
    retained_patients = set(technical)
    labels = {patient: group for patient, group in labels.items() if patient in retained_patients}
    local_field = {
        patient: values for patient, values in local_field.items() if patient in retained_patients
    }
    manifest = [row for row in manifest if row["patient_id"] in retained_patients]
    if min(sum(group == label for group in labels.values()) for label in ("MSI", "MSS")) < 2:
        raise LocalFieldError(
            "complete technical-covariate subset lacks two patients in each molecular group"
        )
    type_names = ("Neoplastic", "Inflammatory", "Connective", "Dead", "Epithelial")
    slide_rows = []
    source_graphs = []
    for row in manifest:
        slide = row["slide_id"]
        selected = _read_csv(prepared / row["input"])
        slide_root = inference_root / slide
        graph_path = adapter.one_file(slide_root, "*_cells.pt")
        cells_path = adapter.one_file(slide_root, "*_cells.json.snappy")
        graph = adapter.read_graph(graph_path)
        payload = adapter.read_cells(cells_path)
        if graph.x.ndim != 2 or graph.x.shape[1] != 1280 or not bool(graph.x.isfinite().all()):
            raise LocalFieldError(f"slide {slide} raw CellViT tensor differs")
        cells = payload.get("cells")
        type_map = payload.get("type_map")
        if (
            not isinstance(cells, list)
            or not isinstance(type_map, dict)
            or tuple(type_map[str(index)] for index in range(1, 6)) != type_names
        ):
            raise LocalFieldError(f"slide {slide} CellViT annotation vocabulary differs")
        indices = []
        counts = {name: 0 for name in type_names}
        for selected_row in selected:
            prefix, separator, suffix = selected_row["cell_id"].rpartition(":")
            if prefix != slide or separator != ":" or len(suffix) != 9 or not suffix.isdigit():
                raise LocalFieldError(f"slide {slide} selected CellId differs")
            index = int(suffix)
            if index >= graph.x.shape[0] or index >= len(cells):
                raise LocalFieldError(f"slide {slide} selected row exceeds CellViT tensors")
            type_id = cells[index].get("type")
            if not isinstance(type_id, int) or str(type_id) not in type_map:
                raise LocalFieldError(f"slide {slide} selected type differs")
            counts[type_map[str(type_id)]] += 1
            indices.append(index)
        vectors = graph.x[indices].detach().cpu().to(dtype=__import__("torch").float64).tolist()
        embedding = lane.embedding_summary(vectors)
        slide_rows.append(
            {
                "patient_id": row["patient_id"],
                "slide_id": slide,
                "embedding": embedding["mean"] + embedding["standard_deviation"],
                "composition": [counts[name] / len(indices) for name in type_names],
            }
        )
        source_graphs.append(
            {
                "slide_id": slide,
                "graph": str(graph_path.resolve()),
                "graph_sha256": _sha256(graph_path),
                "cells": str(cells_path.resolve()),
                "cells_sha256": _sha256(cells_path),
                "selected_cell_count": len(indices),
            }
        )
    blocks = patient_model_blocks(slide_rows, technical, local_field)
    model_blocks = {
        "m0_technical_composition": {
            "technical": blocks["technical"],
            "composition": blocks["composition"],
        },
        "m3_nonspatial_cellvit": {
            "technical": blocks["technical"],
            "composition": blocks["composition"],
            "nonspatial_embedding": blocks["nonspatial_embedding"],
        },
        "local_field_only": {"local_field": blocks["local_field"]},
        "m3_plus_local_field": {
            "technical": blocks["technical"],
            "composition": blocks["composition"],
            "nonspatial_embedding": blocks["nonspatial_embedding"],
            "local_field": blocks["local_field"],
        },
    }
    models = {}
    for index, (name, selected_blocks) in enumerate(model_blocks.items()):
        model = lane.heldout_model(labels, selected_blocks, 3)
        model["uncertainty"] = summary_owner.model_uncertainty(model, seed + index)
        model["whole_patient_permutation"] = summary_owner.exact_label_permutation(
            labels, selected_blocks, lane
        )
        models[name] = model
    increment = summary_owner.incremental_summary(
        models["m3_nonspatial_cellvit"],
        models["m3_plus_local_field"],
        seed + 100,
        lane,
    )
    interval = increment["whole_patient_bootstrap_interval_95"]
    promoted = increment["balanced_accuracy_increment"] > 0.0 and interval[0] > 0.0
    result = {
        "schema_name": "marklab_cellvit_patient_local_field_incremental_summary",
        "schema_version": "1.0",
        "population_unit": "patient",
        "specimen_unit": "slide_nested_within_patient",
        "patient_count": len(labels),
        "slide_count": len(manifest),
        "groups": {
            group: sum(value == group for value in labels.values())
            for group in ("MSI", "MSS")
        },
        "technical_complete_case_exclusions": technical_exclusions,
        "blocks": {
            "technical": ["age", "male_indicator"],
            "composition": list(type_names),
            "nonspatial_embedding": "raw_1280_component_slide_mean_and_population_sd",
            "local_field": endpoints,
        },
        "models": models,
        "incremental_information": increment,
        "promotion_status": (
            "positive_increment_candidate_not_automatically_fused"
            if promoted
            else "failed_positive_increment_and_uncertainty_gate_not_fused"
        ),
        "leakage_checks": {
            "population_split_unit": "whole_patient",
            "standardization_and_pca": "fit_inside_each_patient_held_out_training_fold",
            "slides_nested_inside_patients": True,
            "molecular_labels_used_for_embedding_or_field_construction": False,
            "coordinates_used_for_nonspatial_baseline": False,
        },
        "source_graphs": source_graphs,
        "claim_limitations": [
            f"{sum(value == 'MSI' for value in labels.values())} MSI and {sum(value == 'MSS' for value in labels.values())} MSS patients yield imprecise group assessment",
            "raw CellViT embeddings are morphology-model outputs",
            "negative or null incremental evidence is retained without tuning",
            "no causal clinical prospective or external-transportability claim",
        ],
    }
    output.mkdir(parents=True)
    _write_json(output / "summary.json", result)
    _write_json(
        output / "manifest.json",
        {
            "schema_name": "marklab_cellvit_patient_local_field_incremental_bundle",
            "schema_version": "1.0",
            "summary_sha256": _sha256(output / "summary.json"),
            "patient_result_sha256": _sha256(patient_summary_path),
            "clinical_sha256": _sha256(clinical_path),
            "result_format_compatibility": "0.3_preserved",
        },
    )
    return result


def summarize_stability(
    base_prepared: Path,
    base_execution: Path,
    stability_prepared: Path,
    stability_execution: Path,
    output: Path,
) -> dict[str, Any]:
    """Summarize slide and patient sensitivity to four prespecified perturbations."""
    if output.exists() or output.is_symlink():
        raise LocalFieldError(f"output already exists: {output}")
    base_manifest = _read_csv(base_prepared / "manifest.csv")
    stability_manifest = _read_csv(stability_prepared / "manifest.csv")
    execution_manifest = _read_json(stability_execution / "execution_manifest.json")
    if (
        execution_manifest.get("job_count") != len(stability_manifest)
        or execution_manifest.get("all_replay_bytes_equal") is not True
        or execution_manifest.get("all_ledgers_one_execution") is not True
    ):
        raise LocalFieldError("stability durable execution proof differs")
    baseline = _feature_map(base_manifest, base_execution / "results", False)["baseline"]
    variants = _feature_map(
        stability_manifest, stability_execution / "results", True
    )
    if tuple(sorted(variants)) != tuple(sorted(STABILITY_VARIANTS)):
        raise LocalFieldError("stability variant identity differs")
    slide_stability = variant_stability(baseline, variants)
    baseline_patients = _patient_means(baseline, base_manifest)
    variant_patients = {
        variant: _patient_means(features, base_manifest)
        for variant, features in variants.items()
    }
    patient_stability = variant_stability(baseline_patients, variant_patients)
    result = {
        "schema_name": "marklab_cellvit_patient_local_field_stability_summary",
        "schema_version": "1.0",
        "population_unit": "patient",
        "specimen_unit": "slide_nested_within_patient",
        "variants": list(STABILITY_VARIANTS),
        "patient_count": len(baseline_patients),
        "slide_count": len(baseline),
        "slide_stability": slide_stability,
        "patient_nested_stability": patient_stability,
        "durable_replay": {
            "miss_count": len(stability_manifest),
            "backend_disabled_hit_count": len(stability_manifest),
            "all_bytes_equal": True,
            "all_ledgers_one_execution": True,
        },
        "claim_limitations": [
            "stability is diagnostic and not evidence of molecular discrimination",
            "three sign-invariant local-field endpoints provide a small rank vector",
            "slides remain nested inside patients",
        ],
    }
    output.mkdir(parents=True)
    _write_json(output / "summary.json", result)
    _write_json(
        output / "manifest.json",
        {
            "schema_name": "marklab_cellvit_patient_local_field_stability_bundle",
            "schema_version": "1.0",
            "summary_sha256": _sha256(output / "summary.json"),
            "result_format_compatibility": "0.3_preserved",
        },
    )
    return result


def prepare_adaptive_spde(
    prepared: Path,
    base_execution: Path,
    output: Path,
    seed: int,
    maximum_regions: int,
    maximum_iterations: int,
) -> dict[str, Any]:
    """Prepare one fixed adaptive-SPDE request per admitted slide."""
    if output.exists() or output.is_symlink():
        raise LocalFieldError(f"output already exists: {output}")
    if not 64 <= maximum_regions <= 256:
        raise LocalFieldError("maximum_regions must be between 64 and 256")
    if not 10 <= maximum_iterations <= 10_000:
        raise LocalFieldError("maximum_iterations must be between 10 and 10000")
    manifest = _read_csv(prepared / "manifest.csv")
    staging = output.with_name(f".{output.name}.{os.getpid()}.tmp")
    staging.mkdir(parents=True)
    requests = []
    try:
        for row in manifest:
            slide = row["slide_id"]
            source_rows = _read_csv(prepared / row["input"])
            selected = sorted(
                sorted(
                    source_rows,
                    key=lambda source: (
                        _stable_digest(
                            "cellvit-patient-adaptive-spde-regions",
                            seed,
                            source["cell_id"],
                        ),
                        source["cell_id"],
                    ),
                )[: min(maximum_regions, len(source_rows))],
                key=lambda source: source["cell_id"],
            )
            if len(selected) < 64:
                raise LocalFieldError(f"slide {slide} has fewer than 64 SPDE regions")
            window = _read_json(prepared / row["window"])
            local_result = _read_json(base_execution / "results" / f"{slide}.json")
            window_sha256 = local_result.get("window_sha256")
            if not isinstance(window_sha256, str) or len(window_sha256) != 64:
                raise LocalFieldError(f"slide {slide} lacks canonical window identity")
            request = {
                "window_frame": f"CPTAC_COAD:{slide}:micrometer",
                "window": window,
                "base_resolution": 9,
                "boundary_refinement_levels": 1,
                "maximum_relative_area_error": 0.02,
                "alpha": 2,
                "kappa": 0.01,
                "tau": 1.0,
                "factor_observations": [
                    {
                        "region_id": source["cell_id"],
                        "coordinates": [float(source["x_um"]), float(source["y_um"])],
                        "values": [
                            float(source[f"cellvit_pc_{index:03d}"])
                            for index in range(DIMENSION)
                        ],
                    }
                    for source in selected
                ],
                "factor_count": 1,
                "factor_noise_sd": 0.1,
                "maximum_iterations": maximum_iterations,
                "maximum_vertices": 512,
                "maximum_triangles": 1024,
                "maximum_boundary_segment_triangle_checks": 2_000_000,
                "maximum_projection_triangle_visits": 2_000_000,
                "memory_budget_mib": 128,
                "maximum_result_bytes": 1_000_000,
                "timeout_seconds": 300,
            }
            request_path = staging / "inputs" / f"{slide}.json"
            _write_json(request_path, request)
            requests.append(
                {
                    "patient_id": row["patient_id"],
                    "group": row["group"],
                    "slide_id": slide,
                    "region_count": len(selected),
                    "window_sha256": window_sha256,
                    "request": request_path.relative_to(staging).as_posix(),
                    "request_sha256": _sha256(request_path),
                }
            )
        _write_csv(staging / "manifest.csv", list(requests[0]), requests)
        result = {
            "schema_name": "marklab_cellvit_patient_adaptive_spde_design",
            "schema_version": "1.0",
            "population_unit": "patient",
            "specimen_unit": "slide_nested_within_patient",
            "patient_count": len({row["patient_id"] for row in requests}),
            "slide_count": len(requests),
            "maximum_regions": maximum_regions,
            "maximum_iterations": maximum_iterations,
            "selection": "label_blind_stable_sha256_without_replacement",
            "hyperparameters": {
                "kappa": 0.01,
                "tau": 1.0,
                "factor_noise_sd": 0.1,
                "factor_count": 1,
            },
            "seed": seed,
        }
        _write_json(staging / "design.json", result)
        os.rename(staging, output)
        return result
    except Exception:
        if staging.exists():
            shutil.rmtree(staging)
        raise


def _run_adaptive_slide(
    marklab: Path, prepared: Path, output: Path, row: dict[str, str]
) -> dict[str, Any]:
    slide = row["slide_id"]
    project = output / "projects" / slide
    miss = output / "miss" / f"{slide}.json"
    hit = output / "hit" / f"{slide}.json"
    result = output / "results" / f"{slide}.json"
    ledger = project / "executions.jsonl"
    if all(path.is_file() and not path.is_symlink() for path in (miss, hit, result)):
        if (
            miss.read_bytes() != hit.read_bytes()
            or miss.read_bytes() != result.read_bytes()
            or not ledger.is_file()
            or len(ledger.read_text(encoding="utf-8").splitlines()) != 1
        ):
            raise LocalFieldError(f"adaptive slide {slide} resumed durable state differs")
        return {
            "slide_id": slide,
            "status": "complete",
            "result_sha256": _sha256(result),
            "ledger_rows": 1,
            "resumed": True,
        }
    if any(path.exists() or path.is_symlink() for path in (miss, hit, result)):
        raise LocalFieldError(f"adaptive slide {slide} has an incomplete published result set")
    if ledger.is_file() and ledger.read_text(encoding="utf-8").splitlines():
        raise LocalFieldError(f"adaptive slide {slide} has a committed ledger without outputs")
    for path in (miss, hit, result):
        path.parent.mkdir(parents=True, exist_ok=True)
    command = [
        str(marklab),
        "project",
        "adaptive-window-spde",
        "--project",
        str(project),
        "--input",
        str(prepared / row["request"]),
    ]
    first = subprocess.run(
        command + ["--out", str(miss)], capture_output=True, text=True, check=False
    )
    if first.returncode != 0 or "cache_status=miss" not in first.stderr:
        raise LocalFieldError(f"adaptive slide {slide} miss failed: {first.stderr.strip()}")
    environment = dict(os.environ)
    environment["MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION"] = "1"
    second = subprocess.run(
        command + ["--out", str(hit)],
        capture_output=True,
        text=True,
        check=False,
        env=environment,
    )
    if second.returncode != 0 or "cache_status=hit" not in second.stderr:
        raise LocalFieldError(f"adaptive slide {slide} hit failed: {second.stderr.strip()}")
    if miss.read_bytes() != hit.read_bytes():
        raise LocalFieldError(f"adaptive slide {slide} replay bytes differ")
    if len(ledger.read_text(encoding="utf-8").splitlines()) != 1:
        raise LocalFieldError(f"adaptive slide {slide} ledger differs")
    shutil.copyfile(miss, result)
    return {
        "slide_id": slide,
        "status": "complete",
        "result_sha256": _sha256(result),
        "ledger_rows": 1,
        "resumed": False,
    }


def execute_adaptive_spde(
    prepared: Path, marklab: Path, output: Path, workers: int, resume: bool = False
) -> dict[str, Any]:
    """Execute independent fitted SPDEs while retaining per-slide failures."""
    if (output.exists() or output.is_symlink()) and not resume:
        raise LocalFieldError(f"output already exists: {output}")
    if resume and (not output.is_dir() or output.is_symlink()):
        raise LocalFieldError(f"resume output is not one existing directory: {output}")
    if not 1 <= workers <= 6:
        raise LocalFieldError("adaptive workers must be between 1 and 6")
    manifest = _read_csv(prepared / "manifest.csv")
    output.mkdir(parents=True, exist_ok=resume)
    completed = []
    failures = []
    with ThreadPoolExecutor(max_workers=workers) as pool:
        futures = {
            pool.submit(_run_adaptive_slide, marklab, prepared, output, row): row
            for row in manifest
        }
        for future in as_completed(futures):
            row = futures[future]
            try:
                completed.append(future.result())
            except Exception as error:
                failures.append(
                    {
                        "patient_id": row["patient_id"],
                        "group": row["group"],
                        "slide_id": row["slide_id"],
                        "status": "unavailable",
                        "blocker": str(error),
                    }
                )
    completed.sort(key=lambda row: row["slide_id"])
    failures.sort(key=lambda row: row["slide_id"])
    result = {
        "schema_name": "marklab_cellvit_patient_adaptive_spde_execution",
        "schema_version": "1.0",
        "attempted": len(manifest),
        "completed": len(completed),
        "failed": len(failures),
        "workers": workers,
        "resumed_job_count": sum(row["resumed"] for row in completed),
        "new_job_count": sum(not row["resumed"] for row in completed),
        "all_completed_replay_bytes_equal": True,
        "all_completed_ledgers_one_execution": True,
        "results": completed,
        "failures": failures,
    }
    _write_json(output / "execution_manifest.json", result)
    return result


def summarize_adaptive_spde(
    prepared: Path,
    execution: Path,
    marklab: Path,
    output: Path,
    seed: int,
) -> dict[str, Any]:
    """Reduce successful fitted fields inside patients and retain exact blockers."""
    if output.exists() or output.is_symlink():
        raise LocalFieldError(f"output already exists: {output}")
    manifest = _read_csv(prepared / "manifest.csv")
    execution_manifest = _read_json(execution / "execution_manifest.json")
    completed = {row["slide_id"] for row in execution_manifest.get("results", [])}
    feature_rows = []
    by_patient: dict[str, list[str]] = {}
    for row in manifest:
        if row["slide_id"] not in completed:
            continue
        features = adaptive_result_features(
            _read_json(execution / "results" / f"{row['slide_id']}.json"),
            row["window_sha256"],
            int(row["region_count"]),
        )
        by_patient.setdefault(row["patient_id"], []).append(row["slide_id"])
        feature_rows.extend(
            {
                "patient_id": row["patient_id"],
                "specimen_id": row["slide_id"],
                "group": row["group"],
                "endpoint": endpoint,
                "value": value,
            }
            for endpoint, value in sorted(features.items())
        )
    admitted_patients = {patient for patient, slides in by_patient.items() if len(slides) >= 2}
    feature_rows = [row for row in feature_rows if row["patient_id"] in admitted_patients]
    groups = {
        group: len(
            {
                row["patient_id"]
                for row in feature_rows
                if row["group"] == group
            }
        )
        for group in ("MSI", "MSS")
    }
    if min(groups.values(), default=0) < 2:
        raise LocalFieldError(
            "successful adaptive-SPDE subset lacks two patients in each molecular group"
        )
    output.mkdir(parents=True)
    input_path = output / "patient_nested_adaptive_spde.csv"
    _write_csv(input_path, list(feature_rows[0]), feature_rows)
    endpoint_count = len({row["endpoint"] for row in feature_rows})
    specimen_count = len({row["specimen_id"] for row in feature_rows})
    work = len(admitted_patients) * endpoint_count * (POPULATION_PERMUTATIONS + 1)
    common = [
        "--input",
        str(input_path),
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
        "--maximum-patients",
        str(len(admitted_patients)),
        "--maximum-specimens",
        str(specimen_count),
        "--maximum-endpoints",
        str(endpoint_count),
        "--maximum-permutation-endpoint-evaluations",
        str(work),
        "--memory-budget-mib",
        "64",
    ]
    project = output / "project"
    miss = output / "miss.json"
    hit = output / "hit.json"
    first = subprocess.run(
        [
            str(marklab),
            "project",
            "patient-nested-fields",
            "--project",
            str(project),
            *common,
            "--out",
            str(miss),
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if first.returncode != 0 or "cache_status=miss" not in first.stderr:
        raise LocalFieldError(f"adaptive patient reduction miss failed: {first.stderr.strip()}")
    environment = dict(os.environ)
    environment["MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION"] = "1"
    second = subprocess.run(
        [
            str(marklab),
            "project",
            "patient-nested-fields",
            "--project",
            str(project),
            *common,
            "--out",
            str(hit),
        ],
        capture_output=True,
        text=True,
        check=False,
        env=environment,
    )
    if second.returncode != 0 or "cache_status=hit" not in second.stderr:
        raise LocalFieldError(f"adaptive patient reduction hit failed: {second.stderr.strip()}")
    if miss.read_bytes() != hit.read_bytes():
        raise LocalFieldError("adaptive patient reduction replay bytes differ")
    result = {
        "schema_name": "marklab_cellvit_patient_adaptive_spde_summary",
        "schema_version": "1.0",
        "population_unit": "patient",
        "specimen_unit": "slide_nested_within_patient",
        "admitted_patient_count": len(admitted_patients),
        "admitted_specimen_count": specimen_count,
        "groups": groups,
        "patient_result": _read_json(hit),
        "unavailable_slides": execution_manifest.get("failures", []),
        "unavailable_patients": [
            {
                "patient_id": patient,
                "reason": "fewer_than_two_successful_adaptive_spde_slides",
                "successful_slide_count": len(slides),
            }
            for patient, slides in sorted(by_patient.items())
            if patient not in admitted_patients
        ],
        "durable_replay": {
            "slide_backend_disabled_hits": execution_manifest.get("completed", 0),
            "patient_backend_disabled_hits": 1,
            "all_bytes_equal": True,
            "all_ledgers_one_execution": True,
        },
        "claim_limitations": [
            "fixed kappa tau and one factor are sensitivity choices rather than a complete field model",
            "slides remain nested inside patients",
            "nonconverged or failed slide fits are retained as unavailable",
            "no communication causal clinical or transportability claim",
        ],
    }
    _write_json(output / "summary.json", result)
    _write_json(
        output / "manifest.json",
        {
            "schema_name": "marklab_cellvit_patient_adaptive_spde_bundle",
            "schema_version": "1.0",
            "summary_sha256": _sha256(output / "summary.json"),
            "result_sha256": _sha256(hit),
            "result_format_compatibility": "0.3_preserved",
        },
    )
    return result


def summarize(
    prepared: Path, execution: Path, marklab: Path, output: Path, seed: int
) -> dict[str, Any]:
    """Run the canonical patient reducer over verified per-slide durable results."""
    if output.exists() or output.is_symlink():
        raise LocalFieldError(f"output already exists: {output}")
    manifest = _read_csv(prepared / "manifest.csv")
    execution_manifest = _read_json(execution / "execution_manifest.json")
    if (
        execution_manifest.get("schema_name")
        != "marklab_cellvit_patient_local_field_execution"
        or execution_manifest.get("job_count") != len(manifest)
        or execution_manifest.get("all_replay_bytes_equal") is not True
        or execution_manifest.get("all_ledgers_one_execution") is not True
    ):
        raise LocalFieldError("per-slide durable execution proof differs")
    endpoint_rows = patient_endpoint_rows(manifest, execution / "results", RADIUS_UM)
    output.mkdir(parents=True)
    input_path = output / "patient_nested_fields.csv"
    _write_csv(input_path, list(endpoint_rows[0]), endpoint_rows)
    patient_count = len({row["patient_id"] for row in manifest})
    endpoint_count = len({row["endpoint"] for row in endpoint_rows})
    work = patient_count * endpoint_count * (POPULATION_PERMUTATIONS + 1)
    common = [
        "--input",
        str(input_path),
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
        "--maximum-patients",
        str(patient_count),
        "--maximum-specimens",
        str(len(manifest)),
        "--maximum-endpoints",
        str(endpoint_count),
        "--maximum-permutation-endpoint-evaluations",
        str(work),
        "--memory-budget-mib",
        "64",
    ]
    direct = output / "direct.json"
    miss = output / "miss.json"
    hit = output / "hit.json"
    direct_run = subprocess.run(
        [str(marklab), "cohort", "patient-nested-fields", *common, "--out", str(direct)],
        capture_output=True,
        text=True,
        check=False,
    )
    if direct_run.returncode != 0:
        raise LocalFieldError(f"direct patient reduction failed: {direct_run.stderr.strip()}")
    project = output / "project"
    miss_run = subprocess.run(
        [
            str(marklab),
            "project",
            "patient-nested-fields",
            "--project",
            str(project),
            *common,
            "--out",
            str(miss),
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if miss_run.returncode != 0 or "cache_status=miss" not in miss_run.stderr:
        raise LocalFieldError(f"durable patient reduction miss failed: {miss_run.stderr.strip()}")
    environment = dict(os.environ)
    environment["MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION"] = "1"
    hit_run = subprocess.run(
        [
            str(marklab),
            "project",
            "patient-nested-fields",
            "--project",
            str(project),
            *common,
            "--out",
            str(hit),
        ],
        capture_output=True,
        text=True,
        check=False,
        env=environment,
    )
    if hit_run.returncode != 0 or "cache_status=hit" not in hit_run.stderr:
        raise LocalFieldError(f"durable patient reduction hit failed: {hit_run.stderr.strip()}")
    if direct.read_bytes() != miss.read_bytes() or direct.read_bytes() != hit.read_bytes():
        raise LocalFieldError("direct, durable miss, and durable hit bytes differ")
    if len((project / "executions.jsonl").read_text(encoding="utf-8").splitlines()) != 1:
        raise LocalFieldError("patient reduction ledger does not contain one execution")
    result = _read_json(hit)
    summary = {
        "schema_name": "marklab_cellvit_patient_local_field_summary",
        "schema_version": "1.0",
        "population_unit": "patient",
        "specimen_unit": "slide_nested_within_patient",
        "patient_count": patient_count,
        "slide_count": len(manifest),
        "endpoint_count": endpoint_count,
        "patient_result_sha256": _sha256(hit),
        "durable_replay": {
            "per_slide_miss_count": len(manifest),
            "per_slide_backend_disabled_hit_count": len(manifest),
            "patient_miss_count": 1,
            "patient_backend_disabled_hit_count": 1,
            "all_bytes_equal": True,
            "all_ledgers_one_execution": True,
        },
        "patient_result": result,
        "claim_limitations": [
            "slides are nested measurements and never population replicates",
            "projected CellViT embeddings are morphology-model outputs",
            "this block does not establish incremental information beyond nonspatial embeddings",
            "no causal clinical prospective or transportability claim",
        ],
    }
    _write_json(output / "summary.json", summary)
    artifacts = sorted(path for path in output.rglob("*") if path.is_file())
    _write_json(
        output / "manifest.json",
        {
            "schema_name": "marklab_cellvit_patient_local_field_bundle",
            "schema_version": "1.0",
            "artifact_sha256": {
                path.relative_to(output).as_posix(): _sha256(path) for path in artifacts
            },
            "result_format_compatibility": "0.3_preserved",
        },
    )
    return summary


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    prepare_parser = commands.add_parser("prepare")
    prepare_parser.add_argument("--projected-index", type=Path, required=True)
    prepare_parser.add_argument("--labels", type=Path, required=True)
    prepare_parser.add_argument("--cells-root", type=Path, required=True)
    prepare_parser.add_argument("--inference-root", type=Path, required=True)
    prepare_parser.add_argument("--maximum-cells", type=int, default=512)
    prepare_parser.add_argument("--seed", type=int, required=True)
    prepare_parser.add_argument("--out", type=Path, required=True)
    execute_parser = commands.add_parser("execute")
    execute_parser.add_argument("--prepared", type=Path, required=True)
    execute_parser.add_argument("--marklab", type=Path, required=True)
    execute_parser.add_argument("--workers", type=int, default=6)
    execute_parser.add_argument("--resume", action="store_true")
    execute_parser.add_argument("--out", type=Path, required=True)
    summarize_parser = commands.add_parser("summarize")
    summarize_parser.add_argument("--prepared", type=Path, required=True)
    summarize_parser.add_argument("--execution", type=Path, required=True)
    summarize_parser.add_argument("--marklab", type=Path, required=True)
    summarize_parser.add_argument("--seed", type=int, required=True)
    summarize_parser.add_argument("--out", type=Path, required=True)
    stability_prepare = commands.add_parser("prepare-stability")
    stability_prepare.add_argument("--prepared", type=Path, required=True)
    stability_prepare.add_argument("--seed", type=int, required=True)
    stability_prepare.add_argument("--out", type=Path, required=True)
    stability_execute = commands.add_parser("execute-stability")
    stability_execute.add_argument("--prepared", type=Path, required=True)
    stability_execute.add_argument("--marklab", type=Path, required=True)
    stability_execute.add_argument("--workers", type=int, default=6)
    stability_execute.add_argument("--resume", action="store_true")
    stability_execute.add_argument("--out", type=Path, required=True)
    stability_summary = commands.add_parser("summarize-stability")
    stability_summary.add_argument("--base-prepared", type=Path, required=True)
    stability_summary.add_argument("--base-execution", type=Path, required=True)
    stability_summary.add_argument("--stability-prepared", type=Path, required=True)
    stability_summary.add_argument("--stability-execution", type=Path, required=True)
    stability_summary.add_argument("--out", type=Path, required=True)
    incremental = commands.add_parser("incremental")
    incremental.add_argument("--prepared", type=Path, required=True)
    incremental.add_argument("--inference-root", type=Path, required=True)
    incremental.add_argument("--clinical", type=Path, required=True)
    incremental.add_argument("--patient-summary", type=Path, required=True)
    incremental.add_argument("--seed", type=int, required=True)
    incremental.add_argument("--out", type=Path, required=True)
    adaptive_prepare = commands.add_parser("prepare-adaptive-spde")
    adaptive_prepare.add_argument("--prepared", type=Path, required=True)
    adaptive_prepare.add_argument("--base-execution", type=Path, required=True)
    adaptive_prepare.add_argument("--maximum-regions", type=int, default=128)
    adaptive_prepare.add_argument("--maximum-iterations", type=int, default=5000)
    adaptive_prepare.add_argument("--seed", type=int, required=True)
    adaptive_prepare.add_argument("--out", type=Path, required=True)
    adaptive_execute = commands.add_parser("execute-adaptive-spde")
    adaptive_execute.add_argument("--prepared", type=Path, required=True)
    adaptive_execute.add_argument("--marklab", type=Path, required=True)
    adaptive_execute.add_argument("--workers", type=int, default=4)
    adaptive_execute.add_argument("--resume", action="store_true")
    adaptive_execute.add_argument("--out", type=Path, required=True)
    adaptive_summary = commands.add_parser("summarize-adaptive-spde")
    adaptive_summary.add_argument("--prepared", type=Path, required=True)
    adaptive_summary.add_argument("--execution", type=Path, required=True)
    adaptive_summary.add_argument("--marklab", type=Path, required=True)
    adaptive_summary.add_argument("--seed", type=int, required=True)
    adaptive_summary.add_argument("--out", type=Path, required=True)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.command == "prepare":
        result = prepare(
            args.projected_index,
            args.labels,
            args.cells_root,
            args.inference_root,
            args.out,
            args.maximum_cells,
            args.seed,
        )
    elif args.command in {"execute", "execute-stability"}:
        result = execute(args.prepared, args.marklab, args.out, args.workers, args.resume)
    elif args.command == "summarize":
        result = summarize(args.prepared, args.execution, args.marklab, args.out, args.seed)
    elif args.command == "prepare-stability":
        result = prepare_stability(args.prepared, args.out, args.seed)
    elif args.command == "summarize-stability":
        result = summarize_stability(
            args.base_prepared,
            args.base_execution,
            args.stability_prepared,
            args.stability_execution,
            args.out,
        )
    elif args.command == "incremental":
        result = incremental_information(
            args.prepared,
            args.inference_root,
            args.clinical,
            args.patient_summary,
            args.out,
            args.seed,
        )
    elif args.command == "prepare-adaptive-spde":
        result = prepare_adaptive_spde(
            args.prepared,
            args.base_execution,
            args.out,
            args.seed,
            args.maximum_regions,
            args.maximum_iterations,
        )
    elif args.command == "execute-adaptive-spde":
        result = execute_adaptive_spde(
            args.prepared, args.marklab, args.out, args.workers, args.resume
        )
    else:
        result = summarize_adaptive_spde(
            args.prepared, args.execution, args.marklab, args.out, args.seed
        )
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
