#!/usr/bin/env python3
"""Prepare bounded real CPTAC CellViT inputs for RESULTS-CELLVIT-2DAY-01."""

from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import csv
import hashlib
import heapq
import json
import math
import os
from pathlib import Path
import statistics
from typing import Any, Iterable


OBJECTIVE = "RESULTS-CELLVIT-2DAY-01"
SCHEMA_VERSION = "1.0"
SEED = 20_260_826
MAXIMUM_COORDINATE_CELLS = 2_000
MAXIMUM_RAW_VECTOR_CELLS = 512
PROJECTED_PATIENTS = 30
PROJECTED_CELLS_PER_PATIENT = 100
HIGH_CONFIDENCE_THRESHOLD = 0.75


class AdapterError(ValueError):
    """A source or derived input violates the concrete result contract."""


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while block := stream.read(1024 * 1024):
            digest.update(block)
    return digest.hexdigest()


def stable_rank(*parts: object) -> int:
    framed = "\0".join(str(part) for part in (SEED, *parts)).encode()
    return int.from_bytes(hashlib.sha256(framed).digest(), "big")


def bounded_indices(count: int, maximum: int, identity: str) -> list[int]:
    if count < 0 or maximum < 1:
        raise AdapterError("invalid deterministic sample bounds")
    if count <= maximum:
        return list(range(count))
    return sorted(
        heapq.nsmallest(maximum, range(count), key=lambda row: stable_rank(identity, row))
    )


def write_json(path: Path, value: Any) -> None:
    encoded = (
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
        + "\n"
    ).encode()
    part = path.with_name(f".{path.name}.part")
    part.write_bytes(encoded)
    os.replace(part, path)


def write_csv(path: Path, fields: list[str], rows: Iterable[dict[str, Any]]) -> None:
    part = path.with_name(f".{path.name}.part")
    with part.open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=fields, lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)
    os.replace(part, path)


def exact_csv(path: Path) -> list[dict[str, str]]:
    with path.open(encoding="utf-8", newline="") as stream:
        return list(csv.DictReader(stream))


def one_file(root: Path, pattern: str) -> Path:
    matches = list(root.glob(pattern))
    if len(matches) != 1 or not matches[0].is_file() or matches[0].is_symlink():
        raise AdapterError(f"{root} must contain exactly one regular {pattern} file")
    return matches[0]


def read_graph(path: Path):
    import torch
    from cellvit.data.dataclass.cell_graph import CellGraphDataWSI

    with torch.serialization.safe_globals([CellGraphDataWSI]):
        graph = torch.load(path, map_location="cpu", weights_only=True)
    if not isinstance(graph, CellGraphDataWSI):
        raise AdapterError(f"{path} is not the allowlisted CellViT graph type")
    return graph


def read_cells(path: Path) -> dict[str, Any]:
    import snappy

    try:
        value = json.loads(snappy.decompress(path.read_bytes()))
    except Exception as error:
        raise AdapterError(f"cannot decode {path}: {error}") from error
    if not isinstance(value, dict):
        raise AdapterError(f"{path} is not a CellViT cell object")
    return value


def load_case_map(path: Path) -> dict[str, dict[str, str]]:
    rows = exact_csv(path)
    required = {
        "case_id",
        "submitter_id",
        "project_id",
        "file_id",
        "file_name",
        "file_size",
        "md5sum",
        "source_relative_path",
        "source_sha256",
    }
    if not rows or set(rows[0]) != required:
        raise AdapterError("case map schema differs")
    result: dict[str, dict[str, str]] = {}
    for row in rows:
        file_id = row["file_id"]
        if (
            file_id in result
            or not file_id
            or Path(file_id).name != file_id
            or row["project_id"] != "CPTAC-COAD"
            or len(row["source_sha256"]) != 64
        ):
            raise AdapterError("case map identity is invalid or duplicated")
        result[file_id] = row
    return result


def load_labels(path: Path) -> dict[str, str]:
    rows = exact_csv(path)
    if not rows or set(rows[0]) != {"patient_id", "class_name"}:
        raise AdapterError("molecular label schema differs")
    labels: dict[str, str] = {}
    for row in rows:
        if row["class_name"] not in {"MSI", "MSS"} or row["patient_id"] in labels:
            raise AdapterError("molecular labels are invalid or duplicated")
        labels[row["patient_id"]] = row["class_name"]
    return labels


def load_clinical_age(path: Path) -> dict[str, float]:
    with path.open(encoding="utf-8", newline="") as stream:
        rows = list(csv.reader(stream, delimiter="\t"))
    if len(rows) < 2 or not rows[0] or rows[0][0] != "attrib_name":
        raise AdapterError("clinical table schema differs")
    patients = rows[0][1:]
    age_rows = [row for row in rows[1:] if row and row[0] == "Age"]
    if len(age_rows) != 1 or len(age_rows[0]) != len(patients) + 1:
        raise AdapterError("clinical table must contain exactly one complete Age row")
    ages: dict[str, float] = {}
    for patient, raw in zip(patients, age_rows[0][1:]):
        try:
            age = float(raw)
        except ValueError:
            continue
        if not math.isfinite(age) or age <= 0.0 or patient in ages:
            raise AdapterError("clinical age is invalid or duplicated")
        ages[patient] = age
    if not ages:
        raise AdapterError("clinical table contains no finite ages")
    return ages


def manifest_outputs_match(slide_root: Path, manifest: dict[str, Any]) -> None:
    outputs = manifest.get("output_sha256")
    if not isinstance(outputs, dict) or len(outputs) != 3:
        raise AdapterError(f"{slide_root} output digest map differs")
    for name, expected in outputs.items():
        if (
            not isinstance(name, str)
            or Path(name).name != name
            or not isinstance(expected, str)
            or len(expected) != 64
            or any(character not in "0123456789abcdef" for character in expected)
        ):
            raise AdapterError(f"{slide_root} output digest entry is invalid")
        path = slide_root / name
        if not path.is_file() or path.is_symlink() or sha256(path) != expected:
            raise AdapterError(f"{slide_root} output digest mismatch")


def patch_window(metadata: dict[str, Any]):
    from shapely.geometry import box, mapping
    from shapely.ops import unary_union

    selection = metadata.get("marklab_patch_selection")
    if not isinstance(selection, dict) or selection.get("maximum_patch_count") != 12:
        raise AdapterError("representative slide patch-selection metadata differs")
    patch_size = int(metadata["patch_size"])
    downsampling = int(metadata["downsampling"])
    overlap = int(metadata["patch_overlap"])
    mpp = float(metadata["target_patch_mpp"])
    rectangles = []
    for raw in selection["selected_grid_coordinates"]:
        row, column = int(raw[0]), int(raw[1])
        y0 = row * patch_size * downsampling - (row + 0.5) * overlap
        x0 = column * patch_size * downsampling - (column + 0.5) * overlap
        rectangles.append(
            box(x0 * mpp, y0 * mpp, (x0 + patch_size) * mpp, (y0 + patch_size) * mpp)
        )
    window = unary_union(rectangles)
    if window.is_empty or not window.is_valid or window.area <= 0.0:
        raise AdapterError("representative slide patch union is invalid")
    return mapping(window), window


def projected_row_index(cell_id: str, roi_id: str) -> int:
    prefix = f"{roi_id}:"
    if not cell_id.startswith(prefix):
        raise AdapterError("projected cell ID does not bind its ROI")
    suffix = cell_id[len(prefix) :]
    if len(suffix) != 9 or not suffix.isdigit():
        raise AdapterError("projected cell ID has an invalid source-row suffix")
    return int(suffix)


def median(values: list[float]) -> float:
    if not values or not all(math.isfinite(value) for value in values):
        raise AdapterError("patient endpoint has no finite values")
    return float(statistics.median(values))


def prepare(arguments: argparse.Namespace) -> dict[str, Any]:
    import numpy as np
    import torch
    from shapely.geometry import Point

    inference_root = arguments.inference_root.resolve()
    projected_root = arguments.projected_root.resolve()
    spatial_root = arguments.spatial_root.resolve()
    output = arguments.output.resolve()
    stage = output.with_name(f".{output.name}.part")
    if output.exists() or stage.exists():
        raise AdapterError("output and staging paths must both be absent")
    for path in (
        inference_root,
        projected_root,
        spatial_root,
        arguments.case_map,
        arguments.labels,
        arguments.clinical,
        arguments.inference_verification,
        arguments.spatial_verification,
        arguments.transform,
    ):
        if not path.exists():
            raise AdapterError(f"required source is absent: {path}")

    case_map = load_case_map(arguments.case_map)
    labels = load_labels(arguments.labels)
    clinical_age = load_clinical_age(arguments.clinical)
    inference_verification = json.loads(arguments.inference_verification.read_text())
    spatial_verification = json.loads(arguments.spatial_verification.read_text())
    if (
        inference_verification.get("status") != "verified"
        or inference_verification.get("slide_count") != len(case_map)
        or inference_verification.get("failed_slide_count") != 0
        or inference_verification.get("partial_directory_count") != 0
        or spatial_verification.get("status") != "verified"
    ):
        raise AdapterError("existing inference or spatial verification is not admissible")

    stage.mkdir(parents=True)
    inputs = stage / "inputs"
    inputs.mkdir()

    raw_counts = Counter()
    patient_type_counts: dict[str, Counter[int]] = defaultdict(Counter)
    patient_slide_counts = Counter()
    widths: set[int] = set()
    type_maps: set[tuple[tuple[int, str], ...]] = set()
    mpp_pairs: set[tuple[float, float]] = set()
    representative: tuple[int, str, dict[str, str], Any, dict[str, Any]] | None = None

    for position, (file_id, case) in enumerate(sorted(case_map.items()), 1):
        slide_root = inference_root / file_id
        manifest_path = slide_root / "inference_manifest.json"
        manifest = json.loads(manifest_path.read_text())
        if (
            manifest.get("file_id") != file_id
            or manifest.get("case_id") != case["case_id"]
            or manifest.get("source_sha256") != case["source_sha256"]
            or manifest.get("molecular_labels_used") is not False
        ):
            raise AdapterError("slide inference manifest disagrees with the case map")
        manifest_outputs_match(slide_root, manifest)
        graph = read_graph(one_file(slide_root, "*_cells.pt"))
        payload = read_cells(one_file(slide_root, "*_cells.json.snappy"))
        detection = read_cells(one_file(slide_root, "*_cell_detection.json.snappy"))
        cells = payload.get("cells")
        detected = detection.get("cells")
        if not isinstance(cells, list) or not isinstance(detected, list):
            raise AdapterError("CellViT row containers differ")
        n = len(cells)
        if (
            len(detected) != n
            or tuple(graph.x.shape[:1]) != (n,)
            or tuple(graph.positions.shape) != (n, 2)
            or graph.x.ndim != 2
            or not bool(torch.isfinite(graph.x).all())
            or not bool(torch.isfinite(graph.positions).all())
        ):
            raise AdapterError("CellViT tensor shape or finiteness differs")
        widths.add(int(graph.x.shape[1]))
        metadata = payload.get("wsi_metadata")
        graph_metadata = graph.metadata.get("wsi_metadata")
        if metadata != detection.get("wsi_metadata") or metadata != graph_metadata:
            raise AdapterError("CellViT WSI metadata differs across artifacts")
        base_mpp = float(metadata["base_mpp"])
        target_mpp = float(metadata["target_patch_mpp"])
        if not math.isfinite(base_mpp) or not math.isfinite(target_mpp) or min(base_mpp, target_mpp) <= 0:
            raise AdapterError("CellViT physical scale is invalid")
        mpp_pairs.add((base_mpp, target_mpp))
        positions = graph.positions.detach().cpu().numpy().astype(np.float64)
        centroids = np.asarray([cell["centroid"] for cell in cells], dtype=np.float64).reshape(n, 2)
        if not np.array_equal(positions * target_mpp, centroids * base_mpp):
            raise AdapterError("CellViT graph/annotation physical coordinates differ")
        type_map = tuple(sorted((int(key), str(value)) for key, value in payload["type_map"].items()))
        type_maps.add(type_map)
        valid_types = dict(type_map)
        probabilities = np.asarray([cell["type_prob"] for cell in cells], dtype=float)
        types = [int(cell["type"]) for cell in cells]
        if (
            not np.isfinite(probabilities).all()
            or np.any(probabilities < 0)
            or np.any(probabilities > 1)
            or any(cell_type not in valid_types for cell_type in types)
            or any(
                left["centroid"] != right["centroid"] or left["type"] != right["type"]
                for left, right in zip(cells, detected)
            )
        ):
            raise AdapterError("CellViT annotation values or row alignment differ")
        selected = {
            tuple(raw[:2])
            for raw in metadata["marklab_patch_selection"]["selected_grid_coordinates"]
        }
        occupied = {tuple(cell["patch_coordinates"]) for cell in cells}
        if not occupied <= selected:
            raise AdapterError("CellViT cell-to-patch link escapes the selected patches")

        patient = case["case_id"]
        patient_type_counts[patient].update(types)
        patient_slide_counts[patient] += 1
        raw_counts.update(
            slides=1,
            cells=n,
            selected_patches=len(selected),
            occupied_patches=len(occupied),
        )
        raw_counts.update({f"type_{key}": value for key, value in Counter(types).items()})
        if n >= MAXIMUM_COORDINATE_CELLS:
            rank = stable_rank("representative-slide", file_id)
            if representative is None or rank < representative[0]:
                representative = (rank, file_id, case, graph, payload)
        if position % 50 == 0:
            print(json.dumps({"audit_slides": position, "cells": raw_counts["cells"]}), flush=True)

    if widths != {1280} or len(type_maps) != 1 or len(mpp_pairs) != 1 or representative is None:
        raise AdapterError("cohort-wide CellViT width, annotation, scale, or representative admission differs")

    _, representative_id, representative_case, representative_graph, representative_payload = representative
    representative_cells = representative_payload["cells"]
    representative_mpp = float(representative_payload["wsi_metadata"]["target_patch_mpp"])
    coordinate_indices = bounded_indices(
        len(representative_cells), MAXIMUM_COORDINATE_CELLS, representative_id
    )
    vector_indices = bounded_indices(
        len(representative_cells), MAXIMUM_RAW_VECTOR_CELLS, representative_id
    )
    geometry, shapely_window = patch_window(representative_payload["wsi_metadata"])
    coordinate_rows = []
    for row in coordinate_indices:
        cell = representative_cells[row]
        x_um = float(representative_graph.positions[row, 0]) * representative_mpp
        y_um = float(representative_graph.positions[row, 1]) * representative_mpp
        if not shapely_window.covers(Point(x_um, y_um)):
            raise AdapterError("representative point escapes the exact selected-patch union")
        probability = float(cell["type_prob"])
        coordinate_rows.append(
            {
                "x_um": format(x_um, ".17g"),
                "y_um": format(y_um, ".17g"),
                "mark": int(probability >= HIGH_CONFIDENCE_THRESHOLD),
                "case_id": representative_case["case_id"],
                "timepoint": "baseline",
                "protein": "cellvit_predicted_class_confidence",
                "valid_tumor": "true",
                "valid_ihc": "true",
                "slide_id": representative_id,
                "histologic_compartment": dict(next(iter(type_maps)))[int(cell["type"])],
                "mark_probability": format(probability, ".9g"),
            }
        )
    coordinate_fields = [
        "x_um",
        "y_um",
        "mark",
        "case_id",
        "timepoint",
        "protein",
        "valid_tumor",
        "valid_ihc",
        "slide_id",
        "histologic_compartment",
        "mark_probability",
    ]
    write_csv(inputs / "coordinate_scalar_cells.csv", coordinate_fields, coordinate_rows)
    write_json(inputs / "coordinate_window.geojson", geometry)

    vector_fields = ["object_id", "x_um", "y_um"] + [
        f"embedding_{index}" for index in range(1280)
    ]
    vector_rows = []
    for row in vector_indices:
        values = representative_graph.x[row].detach().cpu().numpy()
        record = {
            "object_id": f"{representative_id}:{row:09d}",
            "x_um": format(float(representative_graph.positions[row, 0]) * representative_mpp, ".17g"),
            "y_um": format(float(representative_graph.positions[row, 1]) * representative_mpp, ".17g"),
        }
        record.update(
            {f"embedding_{index}": format(float(value), ".9g") for index, value in enumerate(values)}
        )
        vector_rows.append(record)
    write_csv(inputs / "raw_vector_semivariogram.csv", vector_fields, vector_rows)
    write_csv(
        inputs / "distance_bins.csv",
        ["bin_id", "lower_um", "upper_um"],
        [
            {"bin_id": "0_25", "lower_um": 0, "upper_um": 25},
            {"bin_id": "25_50", "lower_um": 25, "upper_um": 50},
            {"bin_id": "50_100", "lower_um": 50, "upper_um": 100},
            {"bin_id": "100_200", "lower_um": 100, "upper_um": 200},
        ],
    )

    phenotype = exact_csv(projected_root / "phenotype_manifest.csv")
    if not phenotype or set(phenotype[0]) != {"patient_id", "roi_id", "cells"}:
        raise AdapterError("projected phenotype manifest schema differs")
    projected_patients = sorted({row["patient_id"] for row in phenotype}, key=lambda value: stable_rank("projected-patient", value))
    selected_patients = projected_patients[:PROJECTED_PATIENTS]
    if len(selected_patients) != PROJECTED_PATIENTS:
        raise AdapterError("not enough projected patients for the bounded split")
    split_by_patient = {
        patient: "train" if index < 18 else "validation" if index < 24 else "test"
        for index, patient in enumerate(selected_patients)
    }
    sample_heaps: dict[str, list[tuple[int, str, dict[str, str]]]] = defaultdict(list)
    patient_pc_sum: dict[str, np.ndarray] = defaultdict(lambda: np.zeros(16, dtype=np.float64))
    patient_pc_count = Counter()
    patch_pc_sum: dict[tuple[str, str, int, int], np.ndarray] = defaultdict(
        lambda: np.zeros(16, dtype=np.float64)
    )
    patch_pc_count = Counter()
    feature_names = [f"cellvit_pc_{index:03d}" for index in range(16)]

    for manifest_row in phenotype:
        patient = manifest_row["patient_id"]
        roi_id = manifest_row["roi_id"]
        if (
            roi_id not in case_map
            or case_map[roi_id]["case_id"] != patient
            or Path(roi_id).name != roi_id
        ):
            raise AdapterError("projected phenotype identity disagrees with the case map")
        projected_candidate = projected_root / manifest_row["cells"]
        projected_path = projected_candidate.resolve()
        if (
            not projected_path.is_relative_to(projected_root)
            or projected_candidate.is_symlink()
            or not projected_path.is_file()
        ):
            raise AdapterError("projected phenotype path escapes its source root")
        rows = exact_csv(projected_path)
        slide_payload = read_cells(one_file(inference_root / roi_id, "*_cells.json.snappy"))
        source_cells = slide_payload["cells"]
        for record in rows:
            vector = np.asarray([float(record[name]) for name in feature_names], dtype=np.float64)
            if vector.shape != (16,) or not np.isfinite(vector).all():
                raise AdapterError("projected CellViT row is non-finite")
            source_row = projected_row_index(record["cell_id"], roi_id)
            if source_row >= len(source_cells) or int(source_cells[source_row]["type"]) != 1:
                raise AdapterError("projected CellViT row does not link to its Neoplastic source cell")
            patient_pc_sum[patient] += vector
            patient_pc_count[patient] += 1
            patch_row, patch_column = map(int, source_cells[source_row]["patch_coordinates"])
            patch_key = (patient, roi_id, patch_row, patch_column)
            patch_pc_sum[patch_key] += vector
            patch_pc_count[patch_key] += 1
            if patient in split_by_patient:
                candidate = {
                    "object_id": record["cell_id"],
                    "biological_unit": patient,
                    "split": split_by_patient[patient],
                    "permutation_stratum": roi_id,
                    "x_um": record["x_um"],
                    "y_um": record["y_um"],
                    **{f"embedding_{index}": record[name] for index, name in enumerate(feature_names)},
                }
                rank = stable_rank("projected-cell", record["cell_id"])
                heap = sample_heaps[patient]
                item = (-rank, record["cell_id"], candidate)
                if len(heap) < PROJECTED_CELLS_PER_PATIENT:
                    heapq.heappush(heap, item)
                elif item > heap[0]:
                    heapq.heapreplace(heap, item)

    projected_rows = []
    roi_offsets = {
        roi_id: index * 10_000_000.0
        for index, roi_id in enumerate(sorted({row["roi_id"] for row in phenotype}))
    }
    patient_offsets = {
        patient: index * 10_000_000_000.0
        for index, patient in enumerate(selected_patients)
    }
    for patient in selected_patients:
        selected = sorted(
            (record for _, _, record in sample_heaps[patient]),
            key=lambda row: row["object_id"],
        )
        if len(selected) < PROJECTED_CELLS_PER_PATIENT:
            raise AdapterError("selected projected patient has too few CellViT rows")
        for record in selected:
            shift = patient_offsets[patient] + roi_offsets[record["permutation_stratum"]]
            record["x_um"] = format(float(record["x_um"]) + shift, ".17g")
            projected_rows.append(record)
    projected_fields = [
        "object_id",
        "biological_unit",
        "split",
        "permutation_stratum",
        "x_um",
        "y_um",
    ] + [f"embedding_{index}" for index in range(16)]
    write_csv(inputs / "projected_embedding_variograms.csv", projected_fields, projected_rows)

    signatures = json.loads((spatial_root / "patient_signatures.json").read_text())
    roi_results = json.loads((spatial_root / "roi_results.json").read_text())
    signature_by_patient = {row["patient_id"]: row for row in signatures}
    annuli: dict[str, dict[str, list[float]]] = defaultdict(lambda: defaultdict(list))
    for row in roi_results:
        patient = row["patient_id"]
        annuli[patient]["primary_0_100um"].append(float(row["primary"]["cosine_excess"]))
        for annulus in row["annuli"]:
            if annulus["available"]:
                key = f"annulus_{int(annulus['lower_um'])}_{int(annulus['upper_um'])}um"
                annuli[patient][key].append(float(annulus["cosine_excess"]))

    endpoint_rows = []
    for patient in sorted(set(labels) & set(signature_by_patient)):
        for endpoint in ("primary_0_100um", "annulus_0_25um", "annulus_25_50um", "annulus_50_100um"):
            endpoint_rows.append(
                {
                    "patient_id": patient,
                    "group": labels[patient],
                    "endpoint": endpoint,
                    "value": format(median(annuli[patient][endpoint]), ".17g"),
                }
            )
    write_csv(inputs / "patient_endpoints.csv", ["patient_id", "group", "endpoint", "value"], endpoint_rows)

    pymc_values = [float(row["median_roi_cosine_excess"]) for row in signatures]
    if len(pymc_values) < 24 or not all(math.isfinite(value) for value in pymc_values):
        raise AdapterError("PyMC patient observations are not finite and independent")
    write_csv(
        inputs / "pymc_observations.csv",
        ["observation"],
        ({"observation": format(value, ".17g")} for value in pymc_values),
    )

    patches_by_patient: dict[str, list[np.ndarray]] = defaultdict(list)
    for key, total in patch_pc_sum.items():
        patches_by_patient[key[0]].append(total / patch_pc_count[key])
    labeled = sorted(
        set(labels) & set(clinical_age) & set(signature_by_patient) & set(patient_pc_count)
    )
    by_class: dict[str, list[str]] = defaultdict(list)
    for patient in labeled:
        by_class[labels[patient]].append(patient)
    for group in by_class:
        by_class[group].sort(key=lambda patient: stable_rank("fold", patient))

    complementarity_rows = []
    for group in ("MSI", "MSS"):
        for class_index, patient in enumerate(by_class[group]):
            cell_mean = patient_pc_sum[patient] / patient_pc_count[patient]
            patch_matrix = np.vstack(patches_by_patient[patient])
            patch_sd = np.std(patch_matrix, axis=0, ddof=1) if len(patch_matrix) > 1 else np.zeros(16)
            total_types = sum(patient_type_counts[patient].values())
            record: dict[str, Any] = {
                "patient_id": patient,
                "outer_fold": class_index % 5,
                "inner_fold": (class_index // 5) % 4,
                "target": 1 if group == "MSI" else 0,
                "technical_0": format(math.log1p(patient_pc_count[patient]), ".17g"),
                "clinical_0": format(clinical_age[patient], ".17g"),
                "compartment_0": format(patient_type_counts[patient][1] / total_types, ".17g"),
                "acquisition_0": format(total_types / patient_slide_counts[patient], ".17g"),
                "neighbor_0": format(float(signature_by_patient[patient]["median_roi_cosine_excess"]), ".17g"),
                "measured_0": len(annuli[patient]["primary_0_100um"]),
            }
            record.update({f"cell_{index}": format(float(cell_mean[index]), ".17g") for index in range(4)})
            record.update({f"patch_{index}": format(float(patch_sd[index]), ".17g") for index in range(4)})
            complementarity_rows.append(record)
    complementarity_fields = [
        "patient_id",
        "outer_fold",
        "inner_fold",
        "target",
        "technical_0",
        "clinical_0",
        "compartment_0",
        "acquisition_0",
        *[f"cell_{index}" for index in range(4)],
        *[f"patch_{index}" for index in range(4)],
        "neighbor_0",
        "measured_0",
    ]
    write_csv(inputs / "cell_patch_complementarity.csv", complementarity_fields, complementarity_rows)

    multiscale_rows = []
    for group in ("MSI", "MSS"):
        patients = by_class[group]
        cell_weighted = sum((patient_pc_sum[p] for p in patients), np.zeros(16)) / sum(patient_pc_count[p] for p in patients)
        group_patches = [patch for patient in patients for patch in patches_by_patient[patient]]
        patch_weighted = np.mean(np.vstack(group_patches), axis=0)
        patient_weighted = np.mean(
            np.vstack([patient_pc_sum[p] / patient_pc_count[p] for p in patients]), axis=0
        )
        for scale, vector in ((10, cell_weighted), (256, patch_weighted), (1000, patient_weighted)):
            multiscale_rows.append(
                {"sample_id": group, "scale_um": scale, **{f"embedding_{index}": format(float(value), ".17g") for index, value in enumerate(vector)}}
            )
    write_csv(
        inputs / "multiscale_group_embeddings.csv",
        ["sample_id", "scale_um"] + [f"embedding_{index}" for index in range(16)],
        multiscale_rows,
    )
    write_csv(
        inputs / "multiscale_weights.csv",
        ["scale_um", "weight"],
        [
            {"scale_um": 10, "weight": "0.3333333333333333"},
            {"scale_um": 256, "weight": "0.3333333333333333"},
            {"scale_um": 1000, "weight": "0.3333333333333334"},
        ],
    )

    scalar_config = f'''[analysis]\nmark_label = "cellvit_high_predicted_class_confidence"\nuse_probabilistic_marks = true\nanalyze_components = "pooled"\n\n[validation]\nn_min = 200\nn_marked_min = 25\nn_unmarked_min = 25\np_min = 0.01\np_max = 0.99\narea_min_um2 = 100000.0\nk_shell_min = 5\nlargest_interpretable_scale_fraction = 0.33\nvalid_mask_fraction_min = 0.5\n\n[spectrum]\nk_shells = 32\nlow_k_shells = 3\nfit_low_k_alpha = true\nanisotropy_low_k_shells = 5\n\n[periodogram]\nenabled = false\n\n[multiscale_residual]\nenabled = false\nterritory_detection = false\nmin_territory_z = 2.5\n\n[permutation]\nb = 99\nseed = {SEED}\nstratified = false\nstrata_fields = []\n\n[inference]\nfamily_wise_alpha = 0.05\n\n[performance]\nthreads = 1\nmemory_budget_mib = 1024\nk_chunk_modes = 64\nstrict_repro = true\nsave_intermediates = false\n\n[output]\nwrite_parquet_curves = false\nwrite_geojson_territories = false\nwrite_figures = false\nwrite_run_manifest = true\n'''
    (inputs / "scalar_analysis.toml").write_text(scalar_config, encoding="utf-8")

    metadata_hashes = {
        str(path.resolve()): sha256(path.resolve())
        for path in (
            arguments.case_map,
            arguments.labels,
            arguments.clinical,
            arguments.inference_verification,
            arguments.spatial_verification,
            arguments.transform,
            projected_root / "cellvit_model.json",
            projected_root / "phenotype_manifest.csv",
            spatial_root / "run_manifest.json",
            spatial_root / "patient_signatures.json",
            spatial_root / "roi_results.json",
        )
    }
    write_json(
        stage / "admission.json",
        {
            "schema_name": "marklab_cellvit_results_admission",
            "schema_version": SCHEMA_VERSION,
            "objective": OBJECTIVE,
            "status": "admitted_with_named_lane_limit",
            "source_counts": {
                "slides": raw_counts["slides"],
                "patients": len(patient_type_counts),
                "cells": raw_counts["cells"],
                "embedding_width": 1280,
                "annotation_classes": len(next(iter(type_maps))),
                "selected_patches": raw_counts["selected_patches"],
                "occupied_patches": raw_counts["occupied_patches"],
                "projected_patients": len(projected_patients),
                "projected_cells": sum(patient_pc_count.values()),
                "molecular_label_patients": len(labels),
                "spatial_patients": len(signatures),
            },
            "checks": {
                "inference_tree_verified": True,
                "all_output_digests_rechecked": True,
                "all_raw_embeddings_finite": True,
                "all_coordinates_finite": True,
                "all_annotations_finite": True,
                "raw_embedding_annotation_rows_exact": True,
                "projected_source_rows_exact": True,
                "cell_patch_links_exact": True,
                "patient_links_exact": True,
                "physical_scale_exact": True,
            },
            "lanes": {
                "coordinate_only": "available",
                "scalar_mark": "available",
                "vector_embedding": "available_raw_1280_and_projected_16",
                "annotation_combination": "available",
                "patch_multiscale": "available_cell_aggregated_patch_embeddings",
                "patient_level": "available",
                "pymc_bayesian": "available",
                "raw_patch_embedding": "unavailable: CellViT artifacts contain cell-token embeddings and exact patch links but no independent patch-vector tensor",
            },
        },
    )
    write_json(
        stage / "provenance.json",
        {
            "schema_name": "marklab_cellvit_results_provenance",
            "schema_version": SCHEMA_VERSION,
            "objective": OBJECTIVE,
            "seed": SEED,
            "source_sha256": metadata_hashes,
            "adapter_sha256": sha256(Path(__file__).resolve()),
            "selection": {
                "representative_slide": representative_id,
                "coordinate_cells": len(coordinate_rows),
                "raw_vector_cells": len(vector_rows),
                "projected_patients": PROJECTED_PATIENTS,
                "projected_cells_per_patient": PROJECTED_CELLS_PER_PATIENT,
                "projected_split_patients": {"train": 18, "validation": 6, "test": 6},
                "high_confidence_threshold": HIGH_CONFIDENCE_THRESHOLD,
            },
            "claim_scope": "exploratory real-data workflow evidence; not clinical, causal, calibration, or performance evidence",
        },
    )
    group_counts = Counter(labels[patient] for patient in labeled)
    write_json(
        stage / "diagnostics.json",
        {
            "schema_name": "marklab_cellvit_results_diagnostics",
            "schema_version": SCHEMA_VERSION,
            "objective": OBJECTIVE,
            "preparation": {
                "coordinate_cells": len(coordinate_rows),
                "coordinate_marked": sum(int(row["mark"]) for row in coordinate_rows),
                "raw_vector_rows": len(vector_rows),
                "raw_vector_pair_visits": len(vector_rows) * (len(vector_rows) - 1) // 2,
                "projected_rows": len(projected_rows),
                "patient_endpoint_rows": len(endpoint_rows),
                "complementarity_patients": len(complementarity_rows),
                "complementarity_groups": dict(sorted(group_counts.items())),
                "multiscale_rows": len(multiscale_rows),
                "pymc_observations": len(pymc_values),
                "pymc_known_sigma": statistics.stdev(pymc_values),
            },
            "executions": {},
        },
    )
    os.replace(stage, output)
    return {
        "status": "prepared",
        "output": str(output),
        "slides": raw_counts["slides"],
        "patients": len(patient_type_counts),
        "cells": raw_counts["cells"],
    }


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--inference-root", type=Path, required=True)
    result.add_argument("--projected-root", type=Path, required=True)
    result.add_argument("--spatial-root", type=Path, required=True)
    result.add_argument("--case-map", type=Path, required=True)
    result.add_argument("--labels", type=Path, required=True)
    result.add_argument("--clinical", type=Path, required=True)
    result.add_argument("--inference-verification", type=Path, required=True)
    result.add_argument("--spatial-verification", type=Path, required=True)
    result.add_argument("--transform", type=Path, required=True)
    result.add_argument("--output", type=Path, required=True)
    return result


def main() -> int:
    result = prepare(parser().parse_args())
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except AdapterError as error:
        print(f"CPTAC CellViT results adapter failed: {error}", file=os.sys.stderr)
        raise SystemExit(2) from error
