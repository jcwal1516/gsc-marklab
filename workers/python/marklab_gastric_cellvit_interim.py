#!/usr/bin/env python3
"""Prepare a bounded interim full-tissue gastric CellViT analysis."""

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
import sys
from typing import Any, Iterable, Sequence


SEED = 20260830
FIELD_COUNT = 4
MAXIMUM_FIELD_CELLS = 2_000
MAXIMUM_TOPOLOGY_CELLS = 512
TOPOLOGY_LANDMARK_COUNT = 32
GRAPH_RADII_UM = (45.0, 50.0, 55.0)
GRAPH_TIMES = (0.025, 0.05, 0.1, 0.2)
TOPOLOGY_SCALES_UM = (180.0, 200.0, 220.0)
CELL_SUBSAMPLE_FRACTION = 0.8
COORDINATE_JITTER_UM = 1.0
TOPOLOGY_PERTURBATIONS = 4


class InterimError(ValueError):
    """An interim gastric input violates its declared analysis contract."""


def refuse_output(path: Path) -> None:
    if path.exists() or path.is_symlink():
        raise InterimError(f"output already exists: {path}")


def _finite(value: object, field: str) -> float:
    try:
        parsed = float(value)
    except (TypeError, ValueError) as error:
        raise InterimError(f"{field} is not numeric") from error
    if not math.isfinite(parsed):
        raise InterimError(f"{field} is not finite")
    return parsed


def _rank(namespace: str, seed: int, *parts: object) -> bytes:
    digest = hashlib.sha256()
    for value in (namespace, seed, *parts):
        digest.update(str(value).encode("utf-8"))
        digest.update(b"\0")
    return digest.digest()


def _ranks(values: list[float]) -> list[float]:
    order = sorted(range(len(values)), key=lambda index: (values[index], index))
    result = [0.0] * len(values)
    position = 0
    while position < len(order):
        end = position + 1
        while end < len(order) and values[order[end]] == values[order[position]]:
            end += 1
        rank = (position + 1 + end) / 2.0
        for index in order[position:end]:
            result[index] = rank
        position = end
    return result


def _spearman(left: list[float], right: list[float]) -> float:
    if len(left) != len(right) or len(left) < 3:
        raise InterimError("field-rank stability requires at least three aligned fields")
    left_rank, right_rank = _ranks(left), _ranks(right)
    left_mean = math.fsum(left_rank) / len(left_rank)
    right_mean = math.fsum(right_rank) / len(right_rank)
    numerator = math.fsum(
        (a - left_mean) * (b - right_mean) for a, b in zip(left_rank, right_rank)
    )
    left_scale = math.fsum((value - left_mean) ** 2 for value in left_rank)
    right_scale = math.fsum((value - right_mean) ** 2 for value in right_rank)
    if left_scale == 0.0 and right_scale == 0.0:
        return 1.0 if left == right else 0.0
    if left_scale == 0.0 or right_scale == 0.0:
        return 0.0
    return numerator / math.sqrt(left_scale * right_scale)


def _percentile(values: list[float], probability: float) -> float:
    if not values or not 0.0 <= probability <= 1.0:
        raise InterimError("percentile input differs")
    ordered = sorted(values)
    position = probability * (len(ordered) - 1)
    lower, upper = math.floor(position), math.ceil(position)
    weight = position - lower
    return ordered[lower] * (1.0 - weight) + ordered[upper] * weight


def continuous_field_stability(
    baseline: dict[str, dict[str, float]],
    alternatives: list[dict[str, dict[str, float]]],
) -> dict[str, object]:
    """Report continuous feature-rank repeatability without an interim promotion gate."""
    fields = sorted(baseline)
    if len(fields) != FIELD_COUNT or not alternatives:
        raise InterimError("stability requires four fields and at least one alternative")
    features = sorted(baseline[fields[0]])
    if (
        not features
        or any(sorted(baseline[field]) != features for field in fields)
        or any(set(alternative) != set(fields) for alternative in alternatives)
        or any(
            sorted(alternative[field]) != features
            for alternative in alternatives
            for field in fields
        )
    ):
        raise InterimError("stability field or feature identities differ")
    correlations = [
        _spearman(
            [float(baseline[field][feature]) for field in fields],
            [float(alternative[field][feature]) for field in fields],
        )
        for alternative in alternatives
        for feature in features
    ]
    if not all(math.isfinite(value) for value in correlations):
        raise InterimError("stability correlation is nonfinite")
    return {
        "field_count": len(fields),
        "feature_count": len(features),
        "comparison_count": len(alternatives),
        "correlation_count": len(correlations),
        "minimum_spearman": min(correlations),
        "q10_spearman": _percentile(correlations, 0.1),
        "median_spearman": _percentile(correlations, 0.5),
        "promotion_gate": "not_applied_in_interim_analysis",
    }


def _cosine(left: Sequence[float], right: Sequence[float]) -> float | None:
    if len(left) != len(right) or not left:
        raise InterimError("cosine vectors differ")
    numerator = math.fsum(a * b for a, b in zip(left, right))
    left_norm = math.sqrt(math.fsum(value * value for value in left))
    right_norm = math.sqrt(math.fsum(value * value for value in right))
    if not all(math.isfinite(value) for value in (numerator, left_norm, right_norm)):
        raise InterimError("cosine input is nonfinite")
    return None if left_norm == 0.0 or right_norm == 0.0 else numerator / (left_norm * right_norm)


def _load_crc_summary_module():
    path = Path(__file__).with_name("marklab_crc_graph_topology_summary.py")
    spec = importlib.util.spec_from_file_location("marklab_crc_graph_topology_summary", path)
    if spec is None or spec.loader is None:
        raise InterimError("graph/topology summary owner is unavailable")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _embedding_summary(values: Any) -> tuple[list[float], list[float], int, int]:
    import numpy as np

    shape = getattr(values, "shape", None)
    if shape is None:
        values = np.asarray(values, dtype=np.float64)
        shape = values.shape
    if len(shape) != 2 or int(shape[0]) < 1 or int(shape[1]) < 1:
        raise InterimError("embedding matrix must be nonempty and two-dimensional")
    count, width = int(shape[0]), int(shape[1])
    mean = np.zeros(width, dtype=np.float64)
    m2 = np.zeros(width, dtype=np.float64)
    seen = 0
    for start in range(0, count, 4096):
        block = values[start : min(start + 4096, count)]
        if hasattr(block, "detach"):
            block = block.detach().cpu().numpy()
        block = np.asarray(block, dtype=np.float64)
        if block.shape[1:] != (width,) or not np.isfinite(block).all():
            raise InterimError("embedding matrix is ragged or nonfinite")
        block_count = block.shape[0]
        block_mean = block.mean(axis=0)
        block_m2 = ((block - block_mean) ** 2).sum(axis=0)
        if seen == 0:
            mean = block_mean
            m2 = block_m2
        else:
            delta = block_mean - mean
            total = seen + block_count
            mean += delta * (block_count / total)
            m2 += block_m2 + delta * delta * (seen * block_count / total)
        seen += block_count
    standard_deviation = np.sqrt(m2 / seen)
    if not np.isfinite(mean).all() or not np.isfinite(standard_deviation).all():
        raise InterimError("embedding summary is nonfinite")
    return mean.tolist(), standard_deviation.tolist(), count, width


def _anchors(patch_centres: Sequence[tuple[float, float]]) -> list[tuple[float, float]]:
    if len(patch_centres) < FIELD_COUNT:
        raise InterimError("at least four tissue patches are required")
    points = sorted(set((float(x), float(y)) for x, y in patch_centres))
    if len(points) < FIELD_COUNT or any(not all(map(math.isfinite, point)) for point in points):
        raise InterimError("tissue patch centres are invalid")
    centre = (
        math.fsum(point[0] for point in points) / len(points),
        math.fsum(point[1] for point in points) / len(points),
    )
    chosen = [min(points, key=lambda point: ((point[0] - centre[0]) ** 2 + (point[1] - centre[1]) ** 2, point))]
    while len(chosen) < FIELD_COUNT:
        chosen.append(
            max(
                (point for point in points if point not in chosen),
                key=lambda point: (
                    min((point[0] - anchor[0]) ** 2 + (point[1] - anchor[1]) ** 2 for anchor in chosen),
                    tuple(-coordinate for coordinate in point),
                ),
            )
        )
    return chosen


def prepare_slide(
    slide_id: str,
    rows: list[dict[str, object]],
    vectors: Any,
    patch_centres: Sequence[tuple[float, float]],
    *,
    maximum_field_cells: int = MAXIMUM_FIELD_CELLS,
    seed: int = SEED,
) -> dict[str, object]:
    """Summarize every cell and derive four deterministic bounded spatial fields."""
    if not slide_id or not rows or not 2 <= maximum_field_cells <= 20_000:
        raise InterimError("slide identity, cells, or field bound is invalid")
    mean, deviation, count, width = _embedding_summary(vectors)
    if count != len(rows):
        raise InterimError("embedding and cell row counts differ")
    anchors = _anchors(patch_centres)
    assignments: list[list[dict[str, object]]] = [[] for _ in anchors]
    composition: dict[int, int] = {}
    seen_indices: set[int] = set()
    for row in rows:
        index = int(row["index"])
        x_um = _finite(row.get("x_um"), "x_um")
        y_um = _finite(row.get("y_um"), "y_um")
        type_code = int(row["type_code"])
        if index in seen_indices or not 1 <= type_code <= 5:
            raise InterimError("cell identity or type is invalid")
        seen_indices.add(index)
        composition[type_code] = composition.get(type_code, 0) + 1
        field_index = min(
            range(len(anchors)),
            key=lambda candidate: (
                (x_um - anchors[candidate][0]) ** 2 + (y_um - anchors[candidate][1]) ** 2,
                candidate,
            ),
        )
        assignments[field_index].append(
            {"index": index, "x_um": x_um, "y_um": y_um, "type_code": type_code}
        )
    if seen_indices != set(range(count)):
        raise InterimError("cell indices must exactly cover embedding rows")
    fields = []
    for field_index, source_rows in enumerate(assignments):
        if len(source_rows) < 2:
            raise InterimError("a spatial field has fewer than two cells")
        sampled = sorted(
            sorted(
                source_rows,
                key=lambda row: (
                    _rank("gastric-interim-field-sample", seed, slide_id, field_index, row["index"]),
                    row["index"],
                ),
            )[:maximum_field_cells],
            key=lambda row: int(row["index"]),
        )
        indices = [int(row["index"]) for row in sampled]
        field_values = (
            [vectors[index] for index in indices]
            if isinstance(vectors, (list, tuple))
            else vectors[indices]
        )
        field_mean, field_deviation, _, _ = _embedding_summary(field_values)
        cells = [
            {
                "id": f"{slide_id}:field-{field_index + 1}:{int(row['index']):09d}",
                "source_index": int(row["index"]),
                "x_um": float(row["x_um"]),
                "y_um": float(row["y_um"]),
                "type_code": int(row["type_code"]),
            }
            for row in sampled
        ]
        fields.append(
            {
                "field_id": f"field-{field_index + 1}",
                "anchor_um": list(anchors[field_index]),
                "source_cell_count": len(source_rows),
                "sampled_cell_count": len(sampled),
                "embedding_mean": field_mean,
                "embedding_standard_deviation": field_deviation,
                "cells": cells,
            }
        )
    return {
        "slide_id": slide_id,
        "cell_count": count,
        "embedding_width": width,
        "embedding_mean": mean,
        "embedding_standard_deviation": deviation,
        "composition_counts": {str(key): value for key, value in sorted(composition.items())},
        "composition_proportions": {
            str(key): value / count for key, value in sorted(composition.items())
        },
        "field_partition": "nearest_of_four_label_blind_farthest_patch_anchors",
        "fields": fields,
    }


def _subsample(cells: list[dict[str, object]], seed: int) -> list[dict[str, object]]:
    count = max(2, math.floor(len(cells) * CELL_SUBSAMPLE_FRACTION))
    return sorted(
        sorted(cells, key=lambda row: (_rank("gastric-interim-cell-subsample", seed, row["id"]), row["id"]))[:count],
        key=lambda row: str(row["id"]),
    )


def _jitter(cells: list[dict[str, object]], seed: int) -> list[dict[str, object]]:
    result = []
    for row in cells:
        copy = dict(row)
        for coordinate, axis in (("x_um", "x"), ("y_um", "y")):
            raw = _rank("gastric-interim-coordinate-jitter", seed, row["id"], axis)
            unit = int.from_bytes(raw[:8], "big") / float((1 << 64) - 1)
            copy[coordinate] = float(row[coordinate]) + (2.0 * unit - 1.0) * COORDINATE_JITTER_UM
        result.append(copy)
    return result


def _graph_request(cells: list[dict[str, object]], radius_um: float) -> dict[str, object]:
    nodes = [
        {
            "id": str(row["id"]),
            "coordinates_um": [float(row["x_um"]), float(row["y_um"])],
            "signal": float(int(row["type_code"]) == 1),
        }
        for row in cells
    ]
    if len(nodes) < 2 or not 0 < sum(node["signal"] for node in nodes) < len(nodes):
        raise InterimError("field graph requires a varying neoplastic signal")
    pairs = len(nodes) * (len(nodes) - 1) // 2
    maximum_edges = min(pairs, 500_000)
    maximum_order = 64
    work = maximum_order * (len(nodes) + 2 * maximum_edges)
    heat_applications = 13
    return {
        "nodes": nodes,
        "radius_um": radius_um,
        "times": list(GRAPH_TIMES),
        "scattering_order": 2,
        "tolerance": 1e-6,
        "maximum_order": maximum_order,
        "maximum_nodes": len(nodes),
        "maximum_candidate_pairs": pairs,
        "maximum_edges": maximum_edges,
        "maximum_matrix_vector_work": work,
        "maximum_working_bytes": len(nodes) * 448 + maximum_edges * 64 + 520,
        "maximum_retained_bytes": 2 * 1024 * 1024,
        "maximum_total_candidate_pairs": pairs * heat_applications,
        "maximum_total_matrix_vector_work": work * heat_applications,
    }


def _witness_request(cells: list[dict[str, object]], scale_um: float) -> dict[str, object]:
    if len(cells) < 3:
        raise InterimError("field topology requires at least three cells")
    return {
        "points": [
            {"id": str(row["id"]), "coordinates_um": [float(row["x_um"]), float(row["y_um"])]}
            for row in cells
        ],
        "landmark_method": "farthest_point",
        "landmark_count": min(TOPOLOGY_LANDMARK_COUNT, len(cells) - 1),
        "maximum_dimension": 2,
        "nu": 0,
        "max_scale_um": scale_um,
        "coefficient_field": 2,
        "maximum_simplices": 500_000,
        "timeout_seconds": 180,
    }


def field_requests(cells: list[dict[str, object]], seed: int = SEED) -> dict[str, object]:
    subsampled = _subsample(cells, seed)
    topology_cells = sorted(
        sorted(
            cells,
            key=lambda row: (
                _rank("gastric-interim-topology-cap", seed, row["id"]),
                row["id"],
            ),
        )[:MAXIMUM_TOPOLOGY_CELLS],
        key=lambda row: str(row["id"]),
    )
    topology_subsampled = _subsample(topology_cells, seed)
    stability = _witness_request(topology_cells, 200.0)
    stability.update(
        {
            "perturbation_replicates": TOPOLOGY_PERTURBATIONS,
            "maximum_coordinate_jitter_um": COORDINATE_JITTER_UM,
            "seed": seed,
            "minimum_landmark_id_match_fraction": 0.9,
            "maximum_coverage_radius_change_um": 5.0,
            "maximum_simplex_count_l1_change": 100,
            "maximum_total_persistence_change_um_squared": 10_000.0,
            "maximum_backend_executions": TOPOLOGY_PERTURBATIONS + 1,
            "maximum_total_point_work": len(topology_cells) * (TOPOLOGY_PERTURBATIONS + 1),
            "maximum_total_simplex_budget": 500_000 * (TOPOLOGY_PERTURBATIONS + 1),
            "maximum_total_timeout_seconds": 180 * (TOPOLOGY_PERTURBATIONS + 1),
        }
    )
    return {
        "graph": {
            "baseline": _graph_request(cells, 50.0),
            "subsample": _graph_request(subsampled, 50.0),
            "jitter": _graph_request(_jitter(cells, seed), 50.0),
            "radius_45": _graph_request(cells, 45.0),
            "radius_55": _graph_request(cells, 55.0),
        },
        "topology": {
            "stability": stability,
            "subsample": _witness_request(topology_subsampled, 200.0),
            "scale_180": _witness_request(topology_cells, 180.0),
            "scale_220": _witness_request(topology_cells, 220.0),
        },
    }


def _read_csv(path: Path, *, delimiter: str = ",") -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as source:
        rows = list(csv.DictReader(source, delimiter=delimiter))
    if not rows:
        raise InterimError(f"CSV has no rows: {path}")
    return rows


def _write_json(path: Path, document: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(document, sort_keys=True, separators=(",", ":")) + "\n", encoding="utf-8")


def _write_csv(path: Path, rows: Iterable[dict[str, object]]) -> None:
    rows = list(rows)
    if not rows:
        raise InterimError("cannot write an empty manifest")
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as target:
        writer = csv.DictWriter(target, fieldnames=list(rows[0]), lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def _runner_hashes(path: Path, slide_id: str) -> dict[str, str]:
    result = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        digest, source = line.split(maxsplit=1)
        name = Path(source).name
        if len(digest) != 64 or not name.startswith(slide_id) or name in result:
            raise InterimError("runner output hash entry is invalid")
        result[name] = digest
    if len(result) != 3:
        raise InterimError("runner output hash count differs")
    return result


def _load_graph(path: Path, source_root: Path):
    import torch

    source = str(source_root.resolve())
    sys.path.insert(0, source)
    try:
        from cellvit.data.dataclass.cell_graph import CellGraphDataWSI

        with torch.serialization.safe_globals([CellGraphDataWSI]):
            graph = torch.load(path, map_location="cpu", mmap=True, weights_only=True)
    finally:
        if source in sys.path:
            sys.path.remove(source)
    if not isinstance(graph, CellGraphDataWSI):
        raise InterimError("CellViT graph type differs")
    return graph


def _read_cells(path: Path) -> dict[str, object]:
    import snappy

    try:
        document = json.loads(snappy.decompress(path.read_bytes()))
    except Exception as error:
        raise InterimError(f"cannot decode CellViT cells: {error}") from error
    if not isinstance(document, dict) or not isinstance(document.get("cells"), list):
        raise InterimError("CellViT cell document schema differs")
    return document


def prepare(arguments: argparse.Namespace) -> None:
    run_root = arguments.run_root.resolve()
    output = arguments.out.resolve()
    refuse_output(output)
    manifest_rows = _read_csv(run_root / "manifest.csv")
    manifest = {row["slide_id"]: row for row in manifest_rows}
    if len(manifest) != len(manifest_rows):
        raise InterimError("run manifest slide identities are duplicated")
    status = _read_csv(run_root / "status.tsv", delimiter="\t")
    complete = {row["slide_id"] for row in status if row["status"] == "COMPLETE"}
    selected = list(arguments.slide_id)
    if len(selected) < 1 or len(set(selected)) != len(selected) or any(slide not in complete for slide in selected):
        raise InterimError("selected slides must be distinct completed outputs")
    staging = output.with_name(f".{output.name}.{os.getpid()}.tmp")
    refuse_output(staging)
    staging.mkdir(parents=True)
    execution_rows = []
    slide_manifest = []
    try:
        for slide_id in selected:
            if slide_id not in manifest:
                raise InterimError(f"manifest lacks selected slide {slide_id}")
            slide_root = run_root / "inference" / slide_id
            graph_path = slide_root / f"{slide_id}_cells.pt"
            cells_path = slide_root / f"{slide_id}_cell_detection.json.snappy"
            if any(not path.is_file() or path.is_symlink() for path in (graph_path, cells_path)):
                raise InterimError(f"{slide_id} lacks regular completed artifacts")
            graph = _load_graph(graph_path, arguments.cellvit_source_root)
            cell_document = _read_cells(cells_path)
            cells = cell_document["cells"]
            if graph.x.ndim != 2 or graph.positions.shape != (len(cells), 2) or graph.x.shape[0] != len(cells):
                raise InterimError(f"{slide_id} graph and cell shapes differ")
            metadata = graph.metadata.get("wsi_metadata")
            selection = metadata.get("marklab_patch_selection") if isinstance(metadata, dict) else None
            if not isinstance(selection, dict) or selection.get("selected_patch_count") != selection.get("available_tissue_patch_count"):
                raise InterimError(f"{slide_id} is not a complete-tissue output")
            mpp = _finite(metadata.get("target_patch_mpp"), "target_patch_mpp")
            rows = []
            for index, cell in enumerate(cells):
                centroid = cell.get("centroid")
                if not isinstance(centroid, list) or len(centroid) != 2:
                    raise InterimError(f"{slide_id} cell centroid differs")
                expected = graph.positions[index].detach().cpu().tolist()
                if [float(value) for value in centroid] != [float(value) for value in expected]:
                    raise InterimError(f"{slide_id} graph/cell coordinate correspondence differs")
                rows.append(
                    {
                        "index": index,
                        "x_um": float(centroid[0]) * mpp,
                        "y_um": float(centroid[1]) * mpp,
                        "type_code": int(cell["type"]),
                    }
                )
            patch_size = int(metadata["patch_size"])
            downsampling = int(metadata["downsampling"])
            overlap = int(metadata["patch_overlap"])
            patch_centres = []
            for raw in selection["selected_grid_coordinates"]:
                row, column = int(raw[0]), int(raw[1])
                y0 = row * patch_size * downsampling - (row + 0.5) * overlap
                x0 = column * patch_size * downsampling - (column + 0.5) * overlap
                patch_centres.append(((x0 + patch_size / 2) * mpp, (y0 + patch_size / 2) * mpp))
            summary = prepare_slide(
                slide_id,
                rows,
                graph.x,
                patch_centres,
                maximum_field_cells=arguments.maximum_field_cells,
                seed=arguments.seed,
            )
            hashes = _runner_hashes(run_root / "provenance" / f"{slide_id}_outputs.sha256", slide_id)
            for field in summary["fields"]:
                field_id = str(field["field_id"])
                request_family = field_requests(field.pop("cells"), arguments.seed)
                for family, requests in request_family.items():
                    for variant, request in requests.items():
                        relative = Path("requests") / slide_id / field_id / family / f"{variant}.json"
                        _write_json(staging / relative, request)
                        execution_rows.append(
                            {
                                "slide_id": slide_id,
                                "patient_id": manifest[slide_id]["patient_lane_id"],
                                "field_id": field_id,
                                "family": family,
                                "variant": variant,
                                "request": relative.as_posix(),
                                "project": (Path("projects") / slide_id / field_id / family / variant).as_posix(),
                                "result": (Path("results") / slide_id / field_id / family / f"{variant}.json").as_posix(),
                            }
                        )
            summary.update(
                {
                    "patient_id": manifest[slide_id]["patient_lane_id"],
                    "treatment_state": manifest[slide_id]["treatment_state"],
                    "temporal_order": manifest[slide_id]["temporal_order"],
                    "target_mpp": mpp,
                    "selected_tissue_patch_count": selection["selected_patch_count"],
                    "source_graph": str(graph_path),
                    "source_graph_sha256": hashes[graph_path.name],
                    "source_cell_detection": str(cells_path),
                    "source_cell_detection_sha256": hashes[cells_path.name],
                    "source_digest_provenance": "runner_hash_before_atomic_part_directory_rename",
                }
            )
            relative_summary = Path("slides") / f"{slide_id}.json"
            _write_json(staging / relative_summary, summary)
            slide_manifest.append(
                {
                    "slide_id": slide_id,
                    "patient_id": manifest[slide_id]["patient_lane_id"],
                    "treatment_state": manifest[slide_id]["treatment_state"],
                    "temporal_order": manifest[slide_id]["temporal_order"],
                    "cell_count": summary["cell_count"],
                    "tissue_patch_count": selection["selected_patch_count"],
                    "summary": relative_summary.as_posix(),
                }
            )
        _write_csv(staging / "slides.csv", slide_manifest)
        _write_csv(staging / "execution_manifest.csv", execution_rows)
        provenance_script = staging / "provenance" / Path(__file__).name
        provenance_script.parent.mkdir(parents=True)
        shutil.copy2(Path(__file__).resolve(), provenance_script)
        _write_json(
            staging / "design.json",
            {
                "schema_name": "marklab_gastric_cellvit_interim",
                "schema_version": "1.0",
                "status": "provisional_four_of_seven_completed_slides",
                "population_unit": "patient",
                "slide_unit": "slide_nested_within_patient",
                "spatial_field_unit": "label_blind_diagnostic_nested_within_slide",
                "selected_slide_ids": selected,
                "patient_count": len({manifest[slide]["patient_lane_id"] for slide in selected}),
                "field_count_per_slide": FIELD_COUNT,
                "maximum_field_cells": arguments.maximum_field_cells,
                "maximum_topology_cells": MAXIMUM_TOPOLOGY_CELLS,
                "topology_landmark_count": TOPOLOGY_LANDMARK_COUNT,
                "graph_radii_um": list(GRAPH_RADII_UM),
                "topology_scales_um": list(TOPOLOGY_SCALES_UM),
                "cell_subsample_fraction": CELL_SUBSAMPLE_FRACTION,
                "coordinate_jitter_um": COORDINATE_JITTER_UM,
                "seed": arguments.seed,
                "molecular_or_outcome_labels_used": False,
                "inference_policy": "descriptive_only_until_all_seven_slides_complete",
                "source_manifest_sha256": sha256(run_root / "manifest.csv"),
                "source_status_sha256": sha256(run_root / "status.tsv"),
                "source_run_metadata_sha256": sha256(run_root / "provenance" / "run_metadata.txt"),
                "adapter_sha256": sha256(provenance_script),
            },
        )
        os.replace(staging, output)
    except Exception:
        # Leave staging evidence for diagnosis; never replace a requested output after failure.
        raise


def execute_manifest(
    prepared: Path,
    marklab_binary: Path,
    *,
    maximum_processes: int,
    replay: bool,
    family_filter: str | None = None,
) -> dict[str, object]:
    """Run every declared field request through its existing durable project command."""
    prepared = prepared.resolve()
    marklab_binary = marklab_binary.resolve()
    if not 1 <= maximum_processes <= 8 or not marklab_binary.is_file():
        raise InterimError("executor process bound or Marklab binary is invalid")
    rows = _read_csv(prepared / "execution_manifest.csv")
    required = {
        "slide_id",
        "patient_id",
        "field_id",
        "family",
        "variant",
        "request",
        "project",
        "result",
    }
    if set(rows[0]) != required:
        raise InterimError("execution manifest schema differs")
    if family_filter not in {None, "graph", "topology"}:
        raise InterimError("execution family filter differs")
    if family_filter is not None:
        rows = [row for row in rows if row["family"] == family_filter]
    if not rows:
        raise InterimError("execution manifest selection is empty")

    def run(row: dict[str, str]) -> dict[str, object]:
        family = row["family"]
        variant = row["variant"]
        if family == "graph":
            command = "sparse-radius-scattering"
        elif family == "topology" and variant == "stability":
            command = "witness-persistence-stability"
        elif family == "topology":
            command = "witness-persistence"
        else:
            raise InterimError("execution family or variant differs")
        request = prepared / row["request"]
        project = prepared / row["project"]
        miss_result = prepared / row["result"]
        result = prepared / "replay" / row["result"] if replay else miss_result
        if not request.is_file() or request.is_symlink():
            raise InterimError(f"request is unavailable: {request}")
        if result.exists() or result.is_symlink():
            raise InterimError(f"result already exists: {result}")
        result.parent.mkdir(parents=True, exist_ok=True)
        environment = os.environ.copy()
        environment["PYTHONDONTWRITEBYTECODE"] = "1"
        if replay:
            environment["MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION"] = "1"
        process = subprocess.run(
            [
                str(marklab_binary),
                "project",
                command,
                "--project",
                str(project),
                "--input",
                str(request),
                "--out",
                str(result),
            ],
            capture_output=True,
            text=True,
            env=environment,
            timeout=1_200,
        )
        if process.returncode != 0:
            raise InterimError(
                f"{row['slide_id']}/{row['field_id']}/{family}/{variant} failed: "
                f"{process.stderr.strip()}"
            )
        expected_status = "hit" if replay else "miss"
        if f"cache_status={expected_status}" not in process.stderr:
            raise InterimError("durable cache status differs")
        ledger = project / "executions.jsonl"
        if not ledger.is_file():
            raise InterimError("durable execution ledger is absent")
        ledger_count = len(ledger.read_text(encoding="utf-8").splitlines())
        bytes_equal = not replay or (miss_result.is_file() and miss_result.read_bytes() == result.read_bytes())
        return {
            "slide_id": row["slide_id"],
            "field_id": row["field_id"],
            "family": family,
            "variant": variant,
            "cache_status": expected_status,
            "result": result.relative_to(prepared).as_posix(),
            "result_sha256": sha256(result),
            "ledger_execution_count": ledger_count,
            "replay_bytes_equal": bytes_equal,
        }

    records = []
    with ThreadPoolExecutor(max_workers=maximum_processes) as executor:
        futures = [executor.submit(run, row) for row in rows]
        for future in as_completed(futures):
            records.append(future.result())
    records.sort(key=lambda row: (row["slide_id"], row["field_id"], row["family"], row["variant"]))
    summary = {
        "schema_name": "marklab_gastric_cellvit_interim_execution",
        "schema_version": "1.0",
        "mode": "backend_disabled_replay" if replay else "durable_miss",
        "execution_count": len(records),
        "miss_count": sum(row["cache_status"] == "miss" for row in records),
        "hit_count": sum(row["cache_status"] == "hit" for row in records),
        "maximum_processes": maximum_processes,
        "family_filter": family_filter,
        "backend_execution_disabled": replay,
        "all_replay_bytes_equal": all(row["replay_bytes_equal"] for row in records),
        "all_ledgers_one_execution": all(row["ledger_execution_count"] == 1 for row in records),
        "records": records,
    }
    return summary


def _mean_features(rows: Iterable[dict[str, float]]) -> dict[str, float]:
    rows = list(rows)
    if not rows:
        raise InterimError("feature mean has no rows")
    names = sorted(rows[0])
    if not names or any(sorted(row) != names for row in rows):
        raise InterimError("feature mean identities differ")
    return {
        name: math.fsum(float(row[name]) for row in rows) / len(rows) for name in names
    }


def _vector(features: dict[str, float]) -> list[float]:
    return [float(features[name]) for name in sorted(features)]


def _similarity_rows(vectors: dict[str, list[float]]) -> list[dict[str, object]]:
    result = []
    for left in sorted(vectors):
        for right in sorted(vectors):
            result.append(
                {
                    "left_id": left,
                    "right_id": right,
                    "cosine_similarity": _cosine(vectors[left], vectors[right]),
                }
            )
    return result


def summarize(graph_root: Path, topology_root: Path, output: Path) -> None:
    """Seal descriptive four-slide summaries from the valid durable result families."""
    graph_root, topology_root, output = graph_root.resolve(), topology_root.resolve(), output.resolve()
    refuse_output(output)
    graph_design = json.loads((graph_root / "design.json").read_text(encoding="utf-8"))
    topology_design = json.loads((topology_root / "design.json").read_text(encoding="utf-8"))
    slides = _read_csv(graph_root / "slides.csv")
    if (
        graph_design.get("schema_name") != "marklab_gastric_cellvit_interim"
        or topology_design.get("schema_name") != "marklab_gastric_cellvit_interim"
        or graph_design.get("selected_slide_ids") != topology_design.get("selected_slide_ids")
        or [row["slide_id"] for row in slides] != graph_design.get("selected_slide_ids")
    ):
        raise InterimError("graph/topology interim design identities differ")
    graph_replay = json.loads((graph_root / "replay.json").read_text(encoding="utf-8"))
    topology_execution = json.loads((topology_root / "execution.json").read_text(encoding="utf-8"))
    topology_replay = json.loads((topology_root / "replay.json").read_text(encoding="utf-8"))
    if not (
        graph_replay.get("execution_count") == 80
        and graph_replay.get("hit_count") == 80
        and graph_replay.get("backend_execution_disabled")
        and graph_replay.get("all_replay_bytes_equal")
        and graph_replay.get("all_ledgers_one_execution")
        and topology_execution.get("execution_count") == 64
        and topology_execution.get("miss_count") == 64
        and topology_execution.get("all_ledgers_one_execution")
        and topology_replay.get("hit_count") == 64
        and topology_replay.get("backend_execution_disabled")
        and topology_replay.get("all_replay_bytes_equal")
        and topology_replay.get("all_ledgers_one_execution")
    ):
        raise InterimError("durable execution/replay evidence differs")
    owner = _load_crc_summary_module()
    graph_stability: dict[str, dict[str, dict[str, object]]] = {}
    topology_stability: dict[str, dict[str, object]] = {}
    slide_blocks: dict[str, dict[str, list[float]]] = {
        name: {} for name in ("composition", "embedding", "graph", "topology")
    }
    slide_rows = []
    source_paths = [
        graph_root / "design.json",
        graph_root / "slides.csv",
        graph_root / "replay.json",
        topology_root / "design.json",
        topology_root / "execution.json",
        topology_root / "replay.json",
    ]
    slide_metadata = {}
    for row in slides:
        slide_id = row["slide_id"]
        slide_document = json.loads((graph_root / row["summary"]).read_text(encoding="utf-8"))
        slide_metadata[slide_id] = slide_document
        fields = [str(field["field_id"]) for field in slide_document["fields"]]
        if fields != [f"field-{index}" for index in range(1, FIELD_COUNT + 1)]:
            raise InterimError("interim field identities differ")
        graph_variants = {
            name: {} for name in ("baseline", "subsample", "jitter", "radius_45", "radius_55")
        }
        topology_variants = {
            name: {} for name in ("baseline", "subsample", "scale_180", "scale_220")
        }
        coordinate_documents = []
        for field_id in fields:
            for variant in graph_variants:
                path = graph_root / "results" / slide_id / field_id / "graph" / f"{variant}.json"
                graph_variants[variant][field_id] = owner.graph_features(json.loads(path.read_text(encoding="utf-8")))
                source_paths.append(path)
            stability_path = topology_root / "results" / slide_id / field_id / "topology" / "stability.json"
            stability_document = json.loads(stability_path.read_text(encoding="utf-8"))
            topology_variants["baseline"][field_id] = owner.topology_features(stability_document["baseline"])
            coordinate_documents.append(stability_document)
            source_paths.append(stability_path)
            for variant in ("subsample", "scale_180", "scale_220"):
                path = topology_root / "results" / slide_id / field_id / "topology" / f"{variant}.json"
                topology_variants[variant][field_id] = owner.topology_features(json.loads(path.read_text(encoding="utf-8")))
                source_paths.append(path)
        graph_stability[slide_id] = {
            "cell_subsample": continuous_field_stability(graph_variants["baseline"], [graph_variants["subsample"]]),
            "coordinate_perturbation": continuous_field_stability(graph_variants["baseline"], [graph_variants["jitter"]]),
            "nearby_scale": continuous_field_stability(
                graph_variants["baseline"], [graph_variants["radius_45"], graph_variants["radius_55"]]
            ),
        }
        topology_stability[slide_id] = {
            "cell_subsample": continuous_field_stability(topology_variants["baseline"], [topology_variants["subsample"]]),
            "nearby_scale": continuous_field_stability(
                topology_variants["baseline"], [topology_variants["scale_180"], topology_variants["scale_220"]]
            ),
            "coordinate_perturbation": {
                "field_count": FIELD_COUNT,
                "stable_field_count_under_legacy_diagnostic_thresholds": sum(
                    bool(document["stable_under_declared_thresholds"]) for document in coordinate_documents
                ),
                "minimum_landmark_id_match_fraction": min(
                    float(document["minimum_landmark_id_match_fraction"]) for document in coordinate_documents
                ),
                "maximum_coverage_radius_change_um": max(
                    float(document["maximum_coverage_radius_change_um"]) for document in coordinate_documents
                ),
                "maximum_simplex_count_l1_change": max(
                    int(document["maximum_simplex_count_l1_change"]) for document in coordinate_documents
                ),
                "maximum_total_persistence_change_um_squared": max(
                    float(document["maximum_total_persistence_change_um_squared"])
                    for document in coordinate_documents
                ),
                "promotion_gate": "not_applied_in_interim_analysis",
            },
        }
        graph_features = _mean_features(graph_variants["baseline"].values())
        topology_features = _mean_features(topology_variants["baseline"].values())
        composition = [
            float(slide_document["composition_proportions"].get(str(code), 0.0))
            for code in range(1, 6)
        ]
        embedding = [
            *[float(value) for value in slide_document["embedding_mean"]],
            *[float(value) for value in slide_document["embedding_standard_deviation"]],
        ]
        slide_blocks["composition"][slide_id] = composition
        slide_blocks["embedding"][slide_id] = embedding
        slide_blocks["graph"][slide_id] = _vector(graph_features)
        slide_blocks["topology"][slide_id] = _vector(topology_features)
        sampled_total = sum(int(field["sampled_cell_count"]) for field in slide_document["fields"])
        sampled_embedding_mean = [
            math.fsum(
                float(field["embedding_mean"][index]) * int(field["sampled_cell_count"])
                for field in slide_document["fields"]
            )
            / sampled_total
            for index in range(int(slide_document["embedding_width"]))
        ]
        slide_rows.append(
            {
                "slide_id": slide_id,
                "patient_id": row["patient_id"],
                "treatment_state": row["treatment_state"],
                "cell_count": int(row["cell_count"]),
                "tissue_patch_count": int(row["tissue_patch_count"]),
                "spatial_sample_cell_count": sampled_total,
                "full_vs_spatial_sample_embedding_mean_cosine": _cosine(
                    [float(value) for value in slide_document["embedding_mean"]], sampled_embedding_mean
                ),
            }
        )
    by_patient: dict[str, list[str]] = {}
    for row in slides:
        by_patient.setdefault(row["patient_id"], []).append(row["slide_id"])
    patient_blocks: dict[str, dict[str, list[float]]] = {name: {} for name in slide_blocks}
    for block, vectors in slide_blocks.items():
        for patient, patient_slides in sorted(by_patient.items()):
            width = len(vectors[patient_slides[0]])
            patient_blocks[block][patient] = [
                math.fsum(vectors[slide][index] for slide in patient_slides) / len(patient_slides)
                for index in range(width)
            ]
    paired = []
    for patient, patient_slides in sorted(by_patient.items()):
        states = {slide_metadata[slide]["treatment_state"]: slide for slide in patient_slides}
        if set(states) == {"pre_treatment", "post_treatment"}:
            paired.append(
                {
                    "patient_id": patient,
                    "pre_slide_id": states["pre_treatment"],
                    "post_slide_id": states["post_treatment"],
                    "block_cosine_similarity": {
                        block: _cosine(vectors[states["pre_treatment"]], vectors[states["post_treatment"]])
                        for block, vectors in slide_blocks.items()
                    },
                    "composition_post_minus_pre": [
                        post - pre
                        for pre, post in zip(
                            slide_blocks["composition"][states["pre_treatment"]],
                            slide_blocks["composition"][states["post_treatment"]],
                        )
                    ],
                }
            )
    staging = output.with_name(f".{output.name}.{os.getpid()}.tmp")
    refuse_output(staging)
    staging.mkdir(parents=True)
    try:
        _write_csv(staging / "slide_qc.csv", slide_rows)
        for block, vectors in slide_blocks.items():
            _write_csv(staging / "similarity" / f"slide_{block}.csv", _similarity_rows(vectors))
        for block, vectors in patient_blocks.items():
            _write_csv(staging / "similarity" / f"patient_{block}.csv", _similarity_rows(vectors))
        source_digest = hashlib.sha256()
        for path in sorted(set(source_paths)):
            source_digest.update(sha256(path).encode("ascii"))
            source_digest.update(b"\n")
        _write_json(
            staging / "summary.json",
            {
                "schema_name": "marklab_gastric_cellvit_interim_summary",
                "schema_version": "1.0",
                "status": "provisional_four_of_seven_slides",
                "population_unit": "patient",
                "patient_count": len(by_patient),
                "slide_count": len(slides),
                "complete_prepost_pair_count": len(paired),
                "total_cell_count": sum(int(row["cell_count"]) for row in slide_rows),
                "total_tissue_patch_count": sum(int(row["tissue_patch_count"]) for row in slide_rows),
                "graph": {
                    "field_cell_ceiling": graph_design["maximum_field_cells"],
                    "stability": graph_stability,
                    "durable_misses_inferred_from_one_row_ledgers": 80,
                    "backend_disabled_byte_equal_hits": 80,
                },
                "topology": {
                    "witness_cell_ceiling": topology_design["maximum_topology_cells"],
                    "landmark_count": topology_design["topology_landmark_count"],
                    "stability": topology_stability,
                    "durable_misses": 64,
                    "backend_disabled_byte_equal_hits": 64,
                },
                "paired_descriptive_results": paired,
                "incremental_information": "unavailable_until_all_seven_slides_and_a_declared_patient_endpoint_are_admitted",
                "promotion_gate": "not_applied_in_interim_analysis",
                "failed_resource_attempts_retained": [
                    {
                        "root": str(graph_root),
                        "topology_cell_ceiling": 2000,
                        "completed_results": 53,
                        "failed_result_size_requests": 11,
                    },
                    {
                        "root": str(topology_root.with_name("gastric-he-cellvit-interim-v2")),
                        "topology_cell_ceiling": 512,
                        "landmark_count": 64,
                        "completed_results": 55,
                        "failed_result_size_requests": 9,
                    },
                ],
                "claim_limitations": [
                    "four of seven gastric slides are complete",
                    "three patient lanes and one complete pre/post pair are descriptive only",
                    "four spatial fields per slide are nested diagnostics, not population replicates",
                    "CellViT classes and embeddings are model outputs",
                    "no molecular, outcome, or treatment label was used for selection, fitting, or stability",
                    "no significance, clinical, causal, prospective, or transportability claim",
                ],
                "source_result_sha256": source_digest.hexdigest(),
            },
        )
        provenance_script = staging / "provenance" / Path(__file__).name
        provenance_script.parent.mkdir(parents=True)
        shutil.copy2(Path(__file__).resolve(), provenance_script)
        artifacts = []
        for path in sorted(item for item in staging.rglob("*") if item.is_file()):
            artifacts.append(
                {"path": path.relative_to(staging).as_posix(), "sha256": sha256(path)}
            )
        _write_json(
            staging / "manifest.json",
            {
                "schema_name": "marklab_gastric_cellvit_interim_bundle",
                "schema_version": "1.0",
                "artifact_count": len(artifacts),
                "artifacts": artifacts,
            },
        )
        os.replace(staging, output)
    except Exception:
        raise


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    prepare_parser = commands.add_parser("prepare")
    prepare_parser.add_argument("--run-root", required=True, type=Path)
    prepare_parser.add_argument("--cellvit-source-root", required=True, type=Path)
    prepare_parser.add_argument("--slide-id", action="append", required=True)
    prepare_parser.add_argument("--maximum-field-cells", type=int, default=MAXIMUM_FIELD_CELLS)
    prepare_parser.add_argument("--seed", type=int, default=SEED)
    prepare_parser.add_argument("--out", required=True, type=Path)
    execute_parser = commands.add_parser("execute")
    execute_parser.add_argument("--prepared", required=True, type=Path)
    execute_parser.add_argument("--marklab-binary", required=True, type=Path)
    execute_parser.add_argument("--maximum-processes", type=int, default=4)
    execute_parser.add_argument("--family", choices=("graph", "topology"))
    execute_parser.add_argument("--replay", action="store_true")
    summarize_parser = commands.add_parser("summarize")
    summarize_parser.add_argument("--graph-root", required=True, type=Path)
    summarize_parser.add_argument("--topology-root", required=True, type=Path)
    summarize_parser.add_argument("--out", required=True, type=Path)
    return parser.parse_args()


def main() -> None:
    arguments = parse_args()
    if arguments.command == "prepare":
        prepare(arguments)
    elif arguments.command == "execute":
        summary = execute_manifest(
            arguments.prepared,
            arguments.marklab_binary,
            maximum_processes=arguments.maximum_processes,
            replay=arguments.replay,
            family_filter=arguments.family,
        )
        destination = arguments.prepared / ("replay.json" if arguments.replay else "execution.json")
        if destination.exists() or destination.is_symlink():
            raise InterimError(f"execution summary already exists: {destination}")
        _write_json(destination, summary)
    elif arguments.command == "summarize":
        summarize(arguments.graph_root, arguments.topology_root, arguments.out)
    else:  # pragma: no cover
        raise AssertionError(f"unknown command: {arguments.command}")


if __name__ == "__main__":
    main()
