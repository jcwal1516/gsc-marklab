#!/usr/bin/env python3
"""Materialize one correspondence-complete CPTAC joint location/embedding table."""

from __future__ import annotations

import argparse
import csv
from collections import Counter, defaultdict
import hashlib
import json
import math
import os
from pathlib import Path
from typing import Any, Iterable

from marklab_cellvit_cptac_results_adapter import arbitrary_window_ipp_quadrature


FORMAT = "marklab.cellvit_joint_location_embedding_table"
VERSION = 1
DIMENSION = 16
MAXIMUM_SOURCE_SLIDES = 128
MAXIMUM_SOURCE_CELLS_PER_SLIDE = 2_000
MAXIMUM_PATIENTS_PER_GROUP = 8
LOCATION_FIELDS = [
    "pattern_id",
    "patient_id",
    "group",
    "cohort",
    "node_id",
    "type_id",
    "x_um",
    "y_um",
    "weight_um2",
    "window_area_um2",
    "covariate",
    "count",
    "window_sha256",
    "event_sha256",
]
EMBEDDING_FIELDS = [
    "pattern_id",
    "patient_id",
    "group",
    "point_id",
    "x_um",
    "y_um",
] + [f"embedding_{index}" for index in range(DIMENSION)]
SOURCE_FIELDS = ["cell_id", "x_um", "y_um"] + [
    f"cellvit_pc_{index:03d}" for index in range(DIMENSION)
]


class JointTableError(ValueError):
    """An admitted source or derived correspondence violates the fixed contract."""


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def stable_rank(namespace: str, *parts: str) -> bytes:
    digest = hashlib.sha256()
    digest.update(namespace.encode())
    digest.update(b"\0")
    for part in parts:
        digest.update(part.encode())
        digest.update(b"\0")
    return digest.digest()


def read_csv(path: Path) -> tuple[list[str], list[dict[str, str]]]:
    with path.open(encoding="utf-8", newline="") as stream:
        reader = csv.DictReader(stream)
        fields = list(reader.fieldnames or ())
        return fields, list(reader)


def exact_identity(value: object, label: str) -> str:
    if not isinstance(value, str) or not value or value.strip() != value:
        raise JointTableError(f"{label} must be exact and nonempty")
    return value


def finite(value: object, label: str) -> float:
    try:
        result = float(value)
    except (TypeError, ValueError) as error:
        raise JointTableError(f"{label} is not numeric") from error
    if not math.isfinite(result):
        raise JointTableError(f"{label} is nonfinite")
    return result


def source_path(prepared: Path, relative: str, label: str) -> Path:
    value = Path(exact_identity(relative, label))
    if value.is_absolute() or ".." in value.parts:
        raise JointTableError(f"{label} must be a contained relative path")
    result = prepared / value
    try:
        contained = result.resolve().is_relative_to(prepared)
    except OSError as error:
        raise JointTableError(f"{label} cannot be resolved") from error
    if not contained or not result.is_file() or result.is_symlink():
        raise JointTableError(f"{label} is not a regular source file")
    return result


def select_patterns(
    manifest: list[dict[str, str]], patients_per_group: int
) -> list[dict[str, str]]:
    if not 2 <= patients_per_group <= MAXIMUM_PATIENTS_PER_GROUP:
        raise JointTableError("patients_per_group is outside 2..8")
    slides_by_patient: dict[str, list[dict[str, str]]] = defaultdict(list)
    patient_group: dict[str, str] = {}
    seen_slides: set[str] = set()
    for row in manifest:
        patient = exact_identity(row.get("patient_id"), "patient_id")
        slide = exact_identity(row.get("slide_id"), "slide_id")
        group = exact_identity(row.get("group"), "group")
        if group not in {"MSI", "MSS"} or slide in seen_slides:
            raise JointTableError("manifest group or slide identity differs")
        if patient in patient_group and patient_group[patient] != group:
            raise JointTableError("patient group changes across slides")
        patient_group[patient] = group
        seen_slides.add(slide)
        slides_by_patient[patient].append(row)
    selected: list[dict[str, str]] = []
    for group in ("MSI", "MSS"):
        patients = [
            patient
            for patient, slides in slides_by_patient.items()
            if patient_group[patient] == group and len(slides) >= 2
        ]
        patients.sort(key=lambda patient: (stable_rank("joint-patient-v1", patient), patient))
        if len(patients) < patients_per_group:
            raise JointTableError(f"fewer than {patients_per_group} repeated-slide {group} patients")
        for patient in patients[:patients_per_group]:
            slides = sorted(
                slides_by_patient[patient],
                key=lambda row: (
                    stable_rank("joint-slide-v1", patient, row["slide_id"]),
                    row["slide_id"],
                ),
            )[:2]
            selected.extend(slides)
    return sorted(selected, key=lambda row: (row["patient_id"], row["slide_id"]))


def write_csv(path: Path, fields: list[str], rows: Iterable[dict[str, Any]]) -> None:
    part = path.with_name(f".{path.name}.part")
    with part.open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=fields, lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)
    os.replace(part, path)


def write_json(path: Path, value: Any) -> None:
    encoded = json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n"
    part = path.with_name(f".{path.name}.part")
    part.write_text(encoded, encoding="utf-8")
    os.replace(part, path)


def materialize(
    prepared: Path,
    output: Path,
    patients_per_group: int = 3,
    grid_size: int = 4,
) -> dict[str, Any]:
    from shapely.geometry import Point, shape

    prepared = prepared.resolve()
    manifest_path = prepared / "manifest.csv"
    design_path = prepared / "design.json"
    if not manifest_path.is_file() or not design_path.is_file():
        raise JointTableError("prepared manifest or design is unavailable")
    fields, manifest = read_csv(manifest_path)
    required = {
        "patient_id",
        "group",
        "slide_id",
        "cell_count",
        "dimension",
        "input",
        "input_sha256",
        "window",
        "window_sha256",
    }
    if not required <= set(fields) or not manifest or len(manifest) > MAXIMUM_SOURCE_SLIDES:
        raise JointTableError("prepared manifest schema or slide bound differs")
    try:
        design = json.loads(design_path.read_text(encoding="utf-8"))
    except Exception as error:
        raise JointTableError("prepared design is not valid JSON") from error
    projection = exact_identity(design.get("projected_index_sha256"), "projection identity")
    if (
        len(projection) != 64
        or any(character not in "0123456789abcdef" for character in projection)
        or design.get("population_unit") != "patient"
        or design.get("specimen_unit") != "slide_nested_within_patient"
    ):
        raise JointTableError("prepared design identity differs")
    selected = select_patterns(manifest, patients_per_group)
    location_rows: list[dict[str, Any]] = []
    embedding_rows: list[dict[str, Any]] = []
    pattern_manifest: list[dict[str, Any]] = []
    for row in selected:
        patient = row["patient_id"]
        group = row["group"]
        slide = row["slide_id"]
        input_path = source_path(prepared, row["input"], "input")
        window_path = source_path(prepared, row["window"], "window")
        if sha256(input_path) != row["input_sha256"] or sha256(window_path) != row["window_sha256"]:
            raise JointTableError(f"{slide} source digest differs")
        source_fields, cells = read_csv(input_path)
        if source_fields != SOURCE_FIELDS or not cells or len(cells) > MAXIMUM_SOURCE_CELLS_PER_SLIDE:
            raise JointTableError(f"{slide} projected-cell schema or bound differs")
        if int(row["cell_count"]) != len(cells) or int(row["dimension"]) != DIMENSION:
            raise JointTableError(f"{slide} projected-cell count or dimension differs")
        try:
            geometry = shape(json.loads(window_path.read_text(encoding="utf-8")))
        except Exception as error:
            raise JointTableError(f"{slide} exact window cannot be decoded") from error
        if geometry.is_empty or not geometry.is_valid or not math.isfinite(geometry.area) or geometry.area <= 0.0:
            raise JointTableError(f"{slide} exact window is invalid")
        nodes = arbitrary_window_ipp_quadrature(geometry, grid_size)
        node_by_id = {str(node["node_id"]): node for node in nodes}
        xmin, ymin, xmax, ymax = (float(value) for value in geometry.bounds)
        counts: Counter[str] = Counter()
        event_digest = hashlib.sha256()
        seen_cells: set[str] = set()
        for cell in cells:
            cell_id = exact_identity(cell.get("cell_id"), "cell_id")
            if cell_id in seen_cells or not cell_id.startswith(f"{slide}:"):
                raise JointTableError(f"{slide} cell identity differs")
            seen_cells.add(cell_id)
            x_raw = exact_identity(cell.get("x_um"), "x_um")
            y_raw = exact_identity(cell.get("y_um"), "y_um")
            x_um = finite(x_raw, "x_um")
            y_um = finite(y_raw, "y_um")
            if not geometry.covers(Point(x_um, y_um)):
                raise JointTableError(f"{slide} cell is outside exact window")
            ix = min(int((x_um - xmin) * grid_size / (xmax - xmin)), grid_size - 1)
            iy = min(int((y_um - ymin) * grid_size / (ymax - ymin)), grid_size - 1)
            node_id = f"q-{iy:03d}-{ix:03d}"
            if node_id not in node_by_id:
                raise JointTableError(f"{slide} cell maps outside positive quadrature")
            counts[node_id] += 1
            event_digest.update(cell_id.encode())
            event_digest.update(b"\0")
            event_digest.update(x_raw.encode())
            event_digest.update(b"\0")
            event_digest.update(y_raw.encode())
            event_digest.update(b"\n")
            embedding = {
                "pattern_id": slide,
                "patient_id": patient,
                "group": group,
                "point_id": cell_id,
                "x_um": x_raw,
                "y_um": y_raw,
            }
            for index in range(DIMENSION):
                value = exact_identity(cell.get(f"cellvit_pc_{index:03d}"), "embedding")
                finite(value, "embedding")
                embedding[f"embedding_{index}"] = value
            embedding_rows.append(embedding)
        event_sha = event_digest.hexdigest()
        window_area = format(float(geometry.area), ".17g")
        for node in nodes:
            node_id = str(node["node_id"])
            location_rows.append(
                {
                    "pattern_id": slide,
                    "patient_id": patient,
                    "group": group,
                    "cohort": "CPTAC-COAD",
                    "node_id": node_id,
                    "type_id": "selected_cell",
                    "x_um": node["x_um"],
                    "y_um": node["y_um"],
                    "weight_um2": node["weight_um2"],
                    "window_area_um2": window_area,
                    "covariate": node["covariate"],
                    "count": counts[node_id],
                    "window_sha256": row["window_sha256"],
                    "event_sha256": event_sha,
                }
            )
        pattern_manifest.append(
            {
                "patient_id": patient,
                "group": group,
                "pattern_id": slide,
                "role": "pending_lexicographic_assignment",
                "cell_count": len(cells),
                "quadrature_node_count": len(nodes),
                "window_area_um2": float(geometry.area),
                "source_input": row["input"],
                "source_input_sha256": row["input_sha256"],
                "source_window": row["window"],
                "source_window_sha256": row["window_sha256"],
                "event_sha256": event_sha,
            }
        )
    by_patient: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in pattern_manifest:
        by_patient[str(row["patient_id"])].append(row)
    for patient, patterns in by_patient.items():
        if len(patterns) != 2:
            raise JointTableError(f"patient {patient} does not have exactly two selected patterns")
        patterns.sort(key=lambda row: str(row["pattern_id"]))
        patterns[0]["role"] = "embedding_train"
        patterns[1]["role"] = "embedding_heldout"
    if sum(int(row["count"]) for row in location_rows) != len(embedding_rows):
        raise JointTableError("location and embedding selected-cell counts differ")
    if output.exists() and (not output.is_dir() or any(output.iterdir())):
        raise JointTableError("output must be absent or an empty directory")
    output.mkdir(parents=True, exist_ok=True)
    location_path = output / "location.csv"
    embedding_path = output / "embedding.csv"
    write_csv(location_path, LOCATION_FIELDS, location_rows)
    write_csv(embedding_path, EMBEDDING_FIELDS, embedding_rows)
    group_counts = Counter(patterns[0]["group"] for patterns in by_patient.values())
    result = {
        "format": FORMAT,
        "version": VERSION,
        "population_unit": "patient",
        "specimen_unit": "slide_nested_within_patient",
        "cohort": "CPTAC-COAD",
        "selection_policy": "balanced_group_patient_sha256_then_two_slide_sha256_label_blind_within_group",
        "holdout_policy": "lexicographically_first_selected_pattern_trains_second_evaluates",
        "location_process": "bounded_label_blind_projected_cell_selection_not_whole_slide_cell_intensity",
        "embedding_projection_identity": projection,
        "patients_per_group": patients_per_group,
        "patient_count": len(by_patient),
        "group_patient_counts": dict(sorted(group_counts.items())),
        "pattern_count": len(pattern_manifest),
        "embedding_point_count": len(embedding_rows),
        "embedding_dimension": DIMENSION,
        "quadrature_grid_size": grid_size,
        "quadrature_node_count": len(location_rows),
        "source_manifest": str(manifest_path),
        "source_manifest_sha256": sha256(manifest_path),
        "source_design": str(design_path),
        "source_design_sha256": sha256(design_path),
        "location": "location.csv",
        "location_sha256": sha256(location_path),
        "embedding": "embedding.csv",
        "embedding_sha256": sha256(embedding_path),
        "patterns": sorted(pattern_manifest, key=lambda row: (row["patient_id"], row["pattern_id"])),
        "claim_limit": "patient_replicated_joint_model_input_not_biological_communication_or_causality",
    }
    write_json(output / "manifest.json", result)
    checksum_lines = [
        f"{result['embedding_sha256']}  embedding.csv",
        f"{result['location_sha256']}  location.csv",
        f"{sha256(output / 'manifest.json')}  manifest.json",
    ]
    (output / "SHA256SUMS").write_text("\n".join(checksum_lines) + "\n", encoding="utf-8")
    return result


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--prepared", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--patients-per-group", type=int, default=3)
    parser.add_argument("--grid-size", type=int, default=4)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    result = materialize(args.prepared, args.out, args.patients_per_group, args.grid_size)
    print(json.dumps({key: result[key] for key in ("patient_count", "pattern_count", "embedding_point_count", "quadrature_node_count")}, sort_keys=True))


if __name__ == "__main__":
    main()
