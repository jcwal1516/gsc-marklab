#!/usr/bin/env python3
"""Prepare one complete CellViT patch for Marklab's bounded gridded LGCP."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import os
from pathlib import Path
from typing import Any, Iterable


SCHEMA_NAME = "marklab_cellvit_lgcp_input"
SCHEMA_VERSION = "1.0"


class AdapterError(ValueError):
    """The source or selected patch violates the closed LGCP input contract."""


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while block := stream.read(1024 * 1024):
            digest.update(block)
    return digest.hexdigest()


def write_csv(path: Path, fields: list[str], rows: Iterable[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    part = path.with_name(f".{path.name}.part")
    with part.open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=fields, lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)
    os.replace(part, path)


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    encoded = (
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
        + "\n"
    ).encode()
    part = path.with_name(f".{path.name}.part")
    part.write_bytes(encoded)
    os.replace(part, path)


def exact_positive_number(value: Any, name: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise AdapterError(f"{name} must be numeric")
    result = float(value)
    if not math.isfinite(result) or result <= 0.0:
        raise AdapterError(f"{name} must be finite and positive")
    return result


def exact_positive_integer(value: Any, name: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise AdapterError(f"{name} must be a positive integer")
    return value


def prepare_payload(
    payload: dict[str, Any],
    *,
    source_path: Path,
    source_sha256: str,
    patch_row: int,
    patch_column: int,
    grid_x: int,
    grid_y: int,
    events_path: Path,
    grid_path: Path,
    provenance_path: Path,
) -> dict[str, Any]:
    if set(payload) != {"wsi_metadata", "type_map", "cells"}:
        raise AdapterError("CellViT cell payload fields differ")
    metadata = payload["wsi_metadata"]
    cells = payload["cells"]
    if not isinstance(metadata, dict) or not isinstance(cells, list):
        raise AdapterError("CellViT metadata or cells differ")
    if len(source_sha256) != 64 or any(
        character not in "0123456789abcdef" for character in source_sha256
    ):
        raise AdapterError("source SHA-256 is invalid")
    if patch_row < 0 or patch_column < 0:
        raise AdapterError("patch coordinates must be nonnegative")
    if grid_x < 2 or grid_y < 2 or grid_x * grid_y > 36:
        raise AdapterError("LGCP grid must contain 4-36 cells with both dimensions at least two")

    patch_size = exact_positive_integer(metadata.get("patch_size"), "patch_size")
    downsampling = exact_positive_integer(metadata.get("downsampling"), "downsampling")
    overlap = metadata.get("patch_overlap")
    if isinstance(overlap, bool) or not isinstance(overlap, int) or not 0 <= overlap < patch_size:
        raise AdapterError("patch_overlap must be an integer in [0, patch_size)")
    mpp = exact_positive_number(metadata.get("target_patch_mpp"), "target_patch_mpp")
    selection = metadata.get("marklab_patch_selection")
    if not isinstance(selection, dict) or not isinstance(
        selection.get("selected_grid_coordinates"), list
    ):
        raise AdapterError("CellViT patch-selection metadata differs")
    selected = set()
    for raw in selection["selected_grid_coordinates"]:
        if (
            not isinstance(raw, list)
            or len(raw) < 2
            or any(isinstance(value, bool) or not isinstance(value, int) for value in raw[:2])
            or raw[0] < 0
            or raw[1] < 0
        ):
            raise AdapterError("recorded CellViT patch coordinates differ")
        selected.add((raw[0], raw[1]))
    patch = (patch_row, patch_column)
    if patch not in selected:
        raise AdapterError("requested patch is not in the recorded CellViT selection")

    y0_pixels = patch_row * patch_size * downsampling - (patch_row + 0.5) * overlap
    x0_pixels = patch_column * patch_size * downsampling - (patch_column + 0.5) * overlap
    xmin_um = x0_pixels * mpp
    ymin_um = y0_pixels * mpp
    xmax_um = (x0_pixels + patch_size) * mpp
    ymax_um = (y0_pixels + patch_size) * mpp
    width_um = xmax_um - xmin_um
    height_um = ymax_um - ymin_um

    event_rows = []
    counts = [[0 for _ in range(grid_x)] for _ in range(grid_y)]
    for source_row, cell in enumerate(cells):
        if not isinstance(cell, dict):
            raise AdapterError("CellViT cell row is not an object")
        coordinates = cell.get("patch_coordinates")
        if (
            not isinstance(coordinates, list)
            or len(coordinates) != 2
            or any(
                isinstance(value, bool) or not isinstance(value, int)
                for value in coordinates
            )
        ):
            raise AdapterError("CellViT cell patch coordinates differ")
        if coordinates != [patch_row, patch_column]:
            continue
        centroid = cell.get("centroid")
        if (
            not isinstance(centroid, list)
            or len(centroid) != 2
            or any(isinstance(value, bool) or not isinstance(value, (int, float)) for value in centroid)
        ):
            raise AdapterError("selected-patch cell centroid differs")
        x_um = float(centroid[0]) * mpp
        y_um = float(centroid[1]) * mpp
        if not math.isfinite(x_um) or not math.isfinite(y_um):
            raise AdapterError("selected-patch cell coordinate is non-finite")
        if not (xmin_um <= x_um < xmax_um and ymin_um <= y_um < ymax_um):
            raise AdapterError("selected-patch cell escapes its exact physical rectangle")
        ix = min(int((x_um - xmin_um) / width_um * grid_x), grid_x - 1)
        iy = min(int((y_um - ymin_um) / height_um * grid_y), grid_y - 1)
        covariate = 2.0 * ix / (grid_x - 1) - 1.0
        counts[iy][ix] += 1
        event_rows.append(
            {
                "event_id": f"cell:{source_row:09d}",
                "x_um": format(x_um, ".17g"),
                "y_um": format(y_um, ".17g"),
                "covariate": format(covariate, ".17g"),
                "offset": "0",
            }
        )
    if len(event_rows) < 5:
        raise AdapterError("selected patch has fewer than five complete CellViT events")

    grid_rows = []
    for iy in range(grid_y):
        for ix in range(grid_x):
            covariate = 2.0 * ix / (grid_x - 1) - 1.0
            grid_rows.append(
                {
                    "ix": ix,
                    "iy": iy,
                    "covariate": format(covariate, ".17g"),
                    "offset": "0",
                }
            )
    write_csv(
        events_path,
        ["event_id", "x_um", "y_um", "covariate", "offset"],
        event_rows,
    )
    write_csv(grid_path, ["ix", "iy", "covariate", "offset"], grid_rows)
    provenance = {
        "schema_name": SCHEMA_NAME,
        "schema_version": SCHEMA_VERSION,
        "source_path": str(source_path),
        "source_sha256": source_sha256,
        "patch_coordinates": [patch_row, patch_column],
        "window_um": [xmin_um, ymin_um, xmax_um, ymax_um],
        "physical_unit": "micrometre",
        "target_patch_mpp": mpp,
        "grid": [grid_x, grid_y],
        "grid_covariate": "x_cell_index_linearly_scaled_to_minus_one_plus_one",
        "offset": "zero; exact cell area is applied by the gridded LGCP owner",
        "cell_selection": "all_cells_with_exact_patch_coordinates",
        "event_count": len(event_rows),
        "cell_counts": counts,
    }
    write_json(provenance_path, provenance)
    return {
        "status": "prepared",
        "events": len(event_rows),
        "grid_cells": grid_x * grid_y,
        "window_um": provenance["window_um"],
    }


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--input", type=Path, required=True)
    result.add_argument("--expected-source-sha256", required=True)
    result.add_argument("--patch-row", type=int, required=True)
    result.add_argument("--patch-column", type=int, required=True)
    result.add_argument("--grid-x", type=int, required=True)
    result.add_argument("--grid-y", type=int, required=True)
    result.add_argument("--events-out", type=Path, required=True)
    result.add_argument("--grid-out", type=Path, required=True)
    result.add_argument("--provenance-out", type=Path, required=True)
    return result


def main() -> int:
    arguments = parser().parse_args()
    if arguments.input.is_symlink() or not arguments.input.is_file():
        raise AdapterError("input must be one regular non-symlink file")
    source = arguments.input.resolve()
    observed_sha256 = sha256(source)
    if observed_sha256 != arguments.expected_source_sha256:
        raise AdapterError("CellViT source digest differs")
    try:
        import snappy

        payload = json.loads(snappy.decompress(source.read_bytes()))
    except Exception as error:
        raise AdapterError(f"cannot decode CellViT source: {error}") from error
    if not isinstance(payload, dict):
        raise AdapterError("CellViT source is not an object")
    result = prepare_payload(
        payload,
        source_path=source,
        source_sha256=observed_sha256,
        patch_row=arguments.patch_row,
        patch_column=arguments.patch_column,
        grid_x=arguments.grid_x,
        grid_y=arguments.grid_y,
        events_path=arguments.events_out.resolve(),
        grid_path=arguments.grid_out.resolve(),
        provenance_path=arguments.provenance_out.resolve(),
    )
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except AdapterError as error:
        print(f"CellViT LGCP adapter failed: {error}", file=os.sys.stderr)
        raise SystemExit(2) from error
