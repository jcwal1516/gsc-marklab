#!/usr/bin/env python3
"""Prepare and summarize the bounded final CRC graph/topology patient lane."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
from pathlib import Path
from typing import Any, Iterable


SEED = 20260829
GRAPH_TIMES = [0.025, 0.05, 0.1, 0.2]
GRAPH_RADII_UM = [45.0, 50.0, 55.0]
CELL_SUBSAMPLE_FRACTION = 0.8
COORDINATE_JITTER_UM = 1.0
TOPOLOGY_SCALES_UM = [180.0, 200.0, 220.0]
TOPOLOGY_PERTURBATIONS = 4


class AnalysisError(ValueError):
    """An admitted science input violates the frozen analysis contract."""


def read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, document: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(document, sort_keys=True, separators=(",", ":")) + "\n",
        encoding="utf-8",
    )


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def stable_rank(namespace: str, *parts: object) -> bytes:
    digest = hashlib.sha256()
    digest.update(namespace.encode("utf-8"))
    digest.update(b"\0")
    for part in parts:
        digest.update(str(part).encode("utf-8"))
        digest.update(b"\0")
    return digest.digest()


def _finite(value: object, field: str) -> float:
    try:
        parsed = float(value)
    except (TypeError, ValueError) as error:
        raise AnalysisError(f"{field} is not numeric") from error
    if not math.isfinite(parsed):
        raise AnalysisError(f"{field} is not finite")
    return parsed


def _validate_marks(rows: list[dict[str, object]]) -> dict[str, list[dict[str, object]]]:
    if not rows:
        raise AnalysisError("mark table has no rows")
    by_pattern: dict[str, list[dict[str, object]]] = {}
    patient_group: dict[str, str] = {}
    pattern_owner: dict[str, str] = {}
    point_ids: set[str] = set()
    for raw in rows:
        values = {
            field: raw.get(field)
            for field in (
                "pattern_id",
                "patient_id",
                "group",
                "point_id",
                "x_um",
                "y_um",
                "type_id",
            )
        }
        if any(not isinstance(values[field], str) or not values[field] for field in values):
            raise AnalysisError("mark identity fields must be nonempty strings")
        pattern = str(values["pattern_id"])
        patient = str(values["patient_id"])
        group = str(values["group"])
        point = str(values["point_id"])
        type_id = str(values["type_id"])
        if group not in {"MSI", "MSS"}:
            raise AnalysisError("molecular group must be MSI or MSS")
        if type_id not in {"Neoplastic", "Inflammatory", "Connective"}:
            raise AnalysisError("mark escapes the frozen three-type vocabulary")
        if point in point_ids:
            raise AnalysisError("point identity is duplicated")
        point_ids.add(point)
        if patient in patient_group and patient_group[patient] != group:
            raise AnalysisError("patient molecular group is inconsistent")
        if pattern in pattern_owner and pattern_owner[pattern] != patient:
            raise AnalysisError("pattern is assigned to multiple patients")
        patient_group[patient] = group
        pattern_owner[pattern] = patient
        row = dict(raw)
        row["x_um"] = _finite(raw.get("x_um"), "x_um")
        row["y_um"] = _finite(raw.get("y_um"), "y_um")
        by_pattern.setdefault(pattern, []).append(row)
    counts: dict[str, int] = {}
    for pattern, pattern_rows in by_pattern.items():
        pattern_rows.sort(key=lambda row: str(row["point_id"]))
        patient = str(pattern_rows[0]["patient_id"])
        counts[patient] = counts.get(patient, 0) + 1
        if len(pattern_rows) < 6:
            raise AnalysisError(f"pattern {pattern} has fewer than six cells")
        support = {str(row["type_id"]) for row in pattern_rows}
        if support != {"Neoplastic", "Inflammatory", "Connective"}:
            raise AnalysisError(f"pattern {pattern} lacks the frozen mark vocabulary")
    if set(counts.values()) != {2}:
        raise AnalysisError("every admitted patient must contribute exactly two patterns")
    return by_pattern


def _graph_request(rows: list[dict[str, object]], radius_um: float) -> dict[str, object]:
    nodes = [
        {
            "id": str(row["point_id"]),
            "coordinates_um": [float(row["x_um"]), float(row["y_um"])],
            "signal": float(row["type_id"] == "Neoplastic"),
        }
        for row in rows
    ]
    if not 0 < sum(node["signal"] for node in nodes) < len(nodes):
        raise AnalysisError("graph signal must vary within every pattern")
    node_count = len(nodes)
    candidate_pairs = node_count * (node_count - 1) // 2
    maximum_edges = min(candidate_pairs, 500_000)
    maximum_order = 64
    maximum_matrix_vector_work = maximum_order * (node_count + 2 * maximum_edges)
    heat_applications = 13
    return {
        "nodes": nodes,
        "radius_um": radius_um,
        "times": GRAPH_TIMES,
        "scattering_order": 2,
        "tolerance": 1e-6,
        "maximum_order": maximum_order,
        "maximum_nodes": node_count,
        "maximum_candidate_pairs": candidate_pairs,
        "maximum_edges": maximum_edges,
        "maximum_matrix_vector_work": maximum_matrix_vector_work,
        "maximum_working_bytes": node_count * 448 + maximum_edges * 64 + 520,
        "maximum_retained_bytes": 2 * 1024 * 1024,
        "maximum_total_candidate_pairs": candidate_pairs * heat_applications,
        "maximum_total_matrix_vector_work": maximum_matrix_vector_work
        * heat_applications,
    }


def _subsample(rows: list[dict[str, object]], seed: int) -> list[dict[str, object]]:
    count = max(6, math.floor(len(rows) * CELL_SUBSAMPLE_FRACTION))
    return sorted(
        sorted(
            rows,
            key=lambda row: (
                stable_rank("crc-graph-cell-subsample", seed, row["point_id"]),
                str(row["point_id"]),
            ),
        )[:count],
        key=lambda row: str(row["point_id"]),
    )


def _jitter(rows: list[dict[str, object]], seed: int) -> list[dict[str, object]]:
    jittered = []
    for row in rows:
        copy = dict(row)
        values = []
        for axis in ("x", "y"):
            raw = stable_rank("crc-graph-coordinate-jitter", seed, row["point_id"], axis)
            unit = int.from_bytes(raw[:8], "big") / float((1 << 64) - 1)
            values.append((2.0 * unit - 1.0) * COORDINATE_JITTER_UM)
        copy["x_um"] = float(row["x_um"]) + values[0]
        copy["y_um"] = float(row["y_um"]) + values[1]
        jittered.append(copy)
    return jittered


def _witness_request(rows: list[dict[str, object]], max_scale_um: float) -> dict[str, object]:
    return {
        "points": [
            {
                "id": str(row["point_id"]),
                "coordinates_um": [float(row["x_um"]), float(row["y_um"])],
            }
            for row in rows
        ],
        "landmark_method": "farthest_point",
        "landmark_count": min(64, len(rows) - 1),
        "maximum_dimension": 2,
        "nu": 0,
        "max_scale_um": max_scale_um,
        "coefficient_field": 2,
        "maximum_simplices": 500_000,
        "timeout_seconds": 180,
    }


def _witness_stability_request(rows: list[dict[str, object]], seed: int) -> dict[str, object]:
    request = _witness_request(rows, 200.0)
    request.update(
        {
            "perturbation_replicates": TOPOLOGY_PERTURBATIONS,
            "maximum_coordinate_jitter_um": COORDINATE_JITTER_UM,
            "seed": seed,
            "minimum_landmark_id_match_fraction": 0.9,
            "maximum_coverage_radius_change_um": 5.0,
            "maximum_simplex_count_l1_change": 100,
            "maximum_total_persistence_change_um_squared": 10_000.0,
            "maximum_backend_executions": TOPOLOGY_PERTURBATIONS + 1,
            "maximum_total_point_work": len(rows) * (TOPOLOGY_PERTURBATIONS + 1),
            "maximum_total_simplex_budget": 500_000
            * (TOPOLOGY_PERTURBATIONS + 1),
            "maximum_total_timeout_seconds": 180 * (TOPOLOGY_PERTURBATIONS + 1),
        }
    )
    return request


def _write_manifest(path: Path, rows: Iterable[dict[str, object]]) -> None:
    rows = list(rows)
    with path.open("w", newline="", encoding="utf-8") as target:
        writer = csv.DictWriter(target, fieldnames=list(rows[0]), lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def embedding_summary(vectors: list[list[float]]) -> dict[str, object]:
    """Compute finite order-invariant per-dimension mean and population SD."""
    if not vectors or not vectors[0]:
        raise AnalysisError("embedding matrix must be nonempty")
    width = len(vectors[0])
    parsed = []
    for row in vectors:
        if len(row) != width:
            raise AnalysisError("embedding matrix is ragged")
        parsed_row = [_finite(value, "embedding") for value in row]
        parsed.append(parsed_row)
    means = [math.fsum(row[column] for row in parsed) / len(parsed) for column in range(width)]
    deviations = [
        math.sqrt(
            math.fsum((row[column] - means[column]) ** 2 for row in parsed)
            / len(parsed)
        )
        for column in range(width)
    ]
    if not all(math.isfinite(value) for value in (*means, *deviations)):
        raise AnalysisError("embedding summary is nonfinite")
    return {
        "cell_count": len(parsed),
        "embedding_width": width,
        "mean": means,
        "standard_deviation": deviations,
    }


def _fold_transform(
    training: list[list[float]], query: list[float], pca_components: int
) -> tuple[list[list[float]], list[float]]:
    try:
        import numpy as np
    except ImportError as error:
        raise AnalysisError("the pinned numerical environment is unavailable") from error
    matrix = np.asarray(training, dtype=np.float64)
    heldout = np.asarray(query, dtype=np.float64)
    if (
        matrix.ndim != 2
        or matrix.shape[0] < 2
        or matrix.shape[1] < 1
        or heldout.shape != (matrix.shape[1],)
        or not np.isfinite(matrix).all()
        or not np.isfinite(heldout).all()
        or pca_components < 1
    ):
        raise AnalysisError("held-out feature block is invalid")
    mean = matrix.mean(axis=0)
    scale = matrix.std(axis=0)
    scale[scale == 0.0] = 1.0
    transformed = (matrix - mean) / scale
    transformed_query = (heldout - mean) / scale
    if transformed.shape[1] > pca_components:
        _, _, right = np.linalg.svd(transformed, full_matrices=False)
        components = right[: min(pca_components, right.shape[0])]
        transformed = transformed @ components.T
        transformed_query = transformed_query @ components.T
    return transformed.tolist(), transformed_query.tolist()


def _distance(left: list[float], right: list[float]) -> float:
    if len(left) != len(right) or not left:
        raise AnalysisError("distance vectors differ")
    value = math.sqrt(math.fsum((a - b) ** 2 for a, b in zip(left, right)))
    if not math.isfinite(value):
        raise AnalysisError("distance is nonfinite")
    return value


def heldout_model(
    labels: dict[str, str],
    blocks: dict[str, dict[str, list[float]]],
    pca_components: int = 3,
) -> dict[str, object]:
    """Evaluate a frozen nearest-centroid and nearest-patient model by patient folds."""
    patients = sorted(labels)
    if (
        len(patients) < 4
        or set(labels.values()) != {"MSI", "MSS"}
        or any(set(block) != set(patients) for block in blocks.values())
        or not blocks
    ):
        raise AnalysisError("held-out model requires complete two-group patient blocks")
    predictions = []
    for heldout in patients:
        training_patients = [patient for patient in patients if patient != heldout]
        training_vectors = {patient: [] for patient in training_patients}
        query_vector: list[float] = []
        for name in sorted(blocks):
            block = blocks[name]
            transformed, transformed_query = _fold_transform(
                [block[patient] for patient in training_patients],
                block[heldout],
                pca_components,
            )
            for patient, values in zip(training_patients, transformed):
                training_vectors[patient].extend(values)
            query_vector.extend(transformed_query)
        centroids = {}
        for group in ("MSI", "MSS"):
            members = [training_vectors[patient] for patient in training_patients if labels[patient] == group]
            if not members:
                raise AnalysisError("held-out fold lacks a molecular group")
            centroids[group] = [
                math.fsum(row[column] for row in members) / len(members)
                for column in range(len(members[0]))
            ]
        centroid_distances = {
            group: _distance(query_vector, vector) for group, vector in centroids.items()
        }
        predicted = min(centroid_distances, key=lambda group: (centroid_distances[group], group))
        retrieval = min(
            training_patients,
            key=lambda patient: (_distance(query_vector, training_vectors[patient]), patient),
        )
        predictions.append(
            {
                "patient_id": heldout,
                "observed_group": labels[heldout],
                "predicted_group": predicted,
                "correct": predicted == labels[heldout],
                "nearest_patient_id": retrieval,
                "nearest_patient_group": labels[retrieval],
                "retrieval_group_correct": labels[retrieval] == labels[heldout],
                "centroid_distance_msi": centroid_distances["MSI"],
                "centroid_distance_mss": centroid_distances["MSS"],
            }
        )
    recalls = {}
    for group in ("MSI", "MSS"):
        group_rows = [row for row in predictions if row["observed_group"] == group]
        recalls[group] = math.fsum(float(row["correct"]) for row in group_rows) / len(group_rows)
    return {
        "population_unit": "patient",
        "patient_count": len(patients),
        "blocks": sorted(blocks),
        "pca_components_per_block_ceiling": pca_components,
        "preprocessing": "standardization_and_pca_fit_inside_each_patient_held_out_training_fold",
        "classifier": "nearest_training_group_centroid_euclidean",
        "retrieval": "nearest_training_patient_euclidean",
        "balanced_accuracy": (recalls["MSI"] + recalls["MSS"]) / 2.0,
        "recall_by_group": recalls,
        "retrieval_group_accuracy": math.fsum(
            float(row["retrieval_group_correct"]) for row in predictions
        )
        / len(predictions),
        "predictions": predictions,
    }


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
        raise AnalysisError("Spearman comparison requires aligned patient values")
    left_rank = _ranks(left)
    right_rank = _ranks(right)
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
        raise AnalysisError("percentile input is invalid")
    ordered = sorted(values)
    position = probability * (len(ordered) - 1)
    lower = math.floor(position)
    upper = math.ceil(position)
    weight = position - lower
    return ordered[lower] * (1.0 - weight) + ordered[upper] * weight


def feature_stability(
    reference: dict[str, dict[str, float]],
    alternatives: list[dict[str, dict[str, float]]],
) -> dict[str, object]:
    """Summarize feature-wise patient-rank stability across frozen perturbations."""
    patients = sorted(reference)
    if len(patients) < 3 or not alternatives:
        raise AnalysisError("feature stability requires patients and alternatives")
    features = sorted(reference[patients[0]])
    if (
        not features
        or any(sorted(reference[patient]) != features for patient in patients)
        or any(set(alternative) != set(patients) for alternative in alternatives)
        or any(
            sorted(alternative[patient]) != features
            for alternative in alternatives
            for patient in patients
        )
    ):
        raise AnalysisError("feature stability identities differ")
    correlations = []
    for alternative in alternatives:
        for feature in features:
            correlations.append(
                _spearman(
                    [float(reference[patient][feature]) for patient in patients],
                    [float(alternative[patient][feature]) for patient in patients],
                )
            )
    if not all(math.isfinite(value) for value in correlations):
        raise AnalysisError("feature stability is nonfinite")
    return {
        "population_unit": "patient",
        "patient_count": len(patients),
        "feature_count": len(features),
        "comparison_count": len(alternatives),
        "correlation_count": len(correlations),
        "minimum": min(correlations),
        "q10": _percentile(correlations, 0.1),
        "median": _percentile(correlations, 0.5),
    }


def extract_nonspatial_embeddings(
    rows: list[dict[str, object]], inference_root: Path, output: Path
) -> list[dict[str, object]]:
    """Extract exact selected CellViT rows without using coordinates or labels as features."""
    if output.exists():
        raise AnalysisError(f"output already exists: {output}")
    by_pattern = _validate_marks(rows)
    try:
        import torch
        from cellvit.data.dataclass.cell_graph import CellGraphDataWSI
    except ImportError as error:
        raise AnalysisError("the admitted CellViT Python environment is unavailable") from error
    output.mkdir(parents=True)
    manifest = []
    widths = set()
    for pattern, pattern_rows in sorted(by_pattern.items()):
        matches = list((inference_root / pattern).glob("*_cells.pt"))
        if len(matches) != 1 or not matches[0].is_file() or matches[0].is_symlink():
            raise AnalysisError(f"{pattern} lacks one regular CellViT graph")
        graph_path = matches[0]
        with torch.serialization.safe_globals([CellGraphDataWSI]):
            graph = torch.load(graph_path, map_location="cpu", weights_only=True)
        if not isinstance(graph, CellGraphDataWSI) or graph.x.ndim != 2:
            raise AnalysisError(f"{pattern} has an invalid allowlisted CellViT graph")
        if not bool(torch.isfinite(graph.x).all()) or not bool(torch.isfinite(graph.positions).all()):
            raise AnalysisError(f"{pattern} CellViT tensors are nonfinite")
        metadata = graph.metadata.get("wsi_metadata")
        if not isinstance(metadata, dict):
            raise AnalysisError(f"{pattern} lacks CellViT WSI metadata")
        target_mpp = _finite(metadata.get("target_patch_mpp"), "target_patch_mpp")
        indices = []
        for row in pattern_rows:
            prefix, separator, suffix = str(row["point_id"]).rpartition(":")
            if prefix != pattern or separator != ":" or not suffix.isdigit():
                raise AnalysisError(f"{pattern} point identity cannot recover its source row")
            index = int(suffix)
            if index >= graph.x.shape[0]:
                raise AnalysisError(f"{pattern} point identity exceeds its CellViT graph")
            coordinates = graph.positions[index].detach().cpu().tolist()
            expected = [float(coordinates[0]) * target_mpp, float(coordinates[1]) * target_mpp]
            if not (
                math.isclose(expected[0], float(row["x_um"]), rel_tol=0.0, abs_tol=1e-9)
                and math.isclose(expected[1], float(row["y_um"]), rel_tol=0.0, abs_tol=1e-9)
            ):
                raise AnalysisError(f"{pattern} mark row does not match its CellViT tensor row")
            indices.append(index)
        vectors = graph.x[indices].detach().cpu().to(dtype=torch.float64).tolist()
        summary = embedding_summary(vectors)
        widths.add(int(summary["embedding_width"]))
        relative = Path("patterns") / f"{pattern}.json"
        write_json(
            output / relative,
            {
                "schema_name": "marklab_crc_cellvit_nonspatial_specimen_summary",
                "schema_version": "1.0",
                "pattern_id": pattern,
                "patient_id": pattern_rows[0]["patient_id"],
                "group": pattern_rows[0]["group"],
                "source_graph_sha256": sha256(graph_path),
                "source_row_identity": "exact_point_id_suffix",
                **summary,
            },
        )
        manifest.append(
            {
                "pattern_id": pattern,
                "patient_id": pattern_rows[0]["patient_id"],
                "group": pattern_rows[0]["group"],
                "cell_count": summary["cell_count"],
                "embedding_width": summary["embedding_width"],
                "source_graph": str(graph_path.resolve()),
                "source_graph_sha256": sha256(graph_path),
                "summary": relative.as_posix(),
            }
        )
    if widths != {1280}:
        raise AnalysisError(f"CellViT embedding width differs: {sorted(widths)}")
    _write_manifest(output / "manifest.csv", manifest)
    write_json(
        output / "provenance.json",
        {
            "schema_name": "marklab_crc_cellvit_nonspatial_extraction",
            "schema_version": "1.0",
            "pattern_count": len(manifest),
            "patient_count": len({row["patient_id"] for row in manifest}),
            "embedding_width": 1280,
            "cell_selection": "exact_admitted_identity_only_rows",
            "spatial_coordinates_used_as_embedding_features": False,
            "molecular_labels_used_for_selection_or_features": False,
            "population_unit": "patient",
        },
    )
    return manifest


def prepare_requests(
    rows: list[dict[str, object]], output: Path, seed: int = SEED
) -> list[dict[str, object]]:
    """Write deterministic bounded graph/topology requests for two slides per patient."""
    if output.exists():
        raise AnalysisError(f"output already exists: {output}")
    by_pattern = _validate_marks(rows)
    output.mkdir(parents=True)
    patient_counts: dict[str, int] = {}
    for pattern_rows in by_pattern.values():
        patient = str(pattern_rows[0]["patient_id"])
        patient_counts[patient] = patient_counts.get(patient, 0) + 1
    manifest = []
    for pattern, pattern_rows in sorted(by_pattern.items()):
        patient = str(pattern_rows[0]["patient_id"])
        group = str(pattern_rows[0]["group"])
        base = Path("requests") / pattern
        requests = {
            "graph_baseline_request": base / "graph_baseline_50um.json",
            "graph_subsample_request": base / "graph_subsample_80_50um.json",
            "graph_jitter_request": base / "graph_jitter_1um_50um.json",
            "graph_radius_45_request": base / "graph_baseline_45um.json",
            "graph_radius_55_request": base / "graph_baseline_55um.json",
            "topology_subsample_request": base / "topology_subsample_80_200um.json",
            "topology_stability_request": base / "topology_stability_200um.json",
            "topology_scale_180_request": base / "topology_baseline_180um.json",
            "topology_scale_220_request": base / "topology_baseline_220um.json",
        }
        write_json(output / requests["graph_baseline_request"], _graph_request(pattern_rows, 50.0))
        write_json(
            output / requests["graph_subsample_request"],
            _graph_request(_subsample(pattern_rows, seed), 50.0),
        )
        write_json(
            output / requests["graph_jitter_request"],
            _graph_request(_jitter(pattern_rows, seed), 50.0),
        )
        write_json(output / requests["graph_radius_45_request"], _graph_request(pattern_rows, 45.0))
        write_json(output / requests["graph_radius_55_request"], _graph_request(pattern_rows, 55.0))
        write_json(
            output / requests["topology_subsample_request"],
            _witness_request(_subsample(pattern_rows, seed), 200.0),
        )
        write_json(
            output / requests["topology_stability_request"],
            _witness_stability_request(pattern_rows, seed),
        )
        write_json(
            output / requests["topology_scale_180_request"],
            _witness_request(pattern_rows, 180.0),
        )
        write_json(
            output / requests["topology_scale_220_request"],
            _witness_request(pattern_rows, 220.0),
        )
        manifest.append(
            {
                "pattern_id": pattern,
                "patient_id": patient,
                "group": group,
                "pattern_count_for_patient": patient_counts[patient],
                "cell_count": len(pattern_rows),
                **{name: path.as_posix() for name, path in requests.items()},
            }
        )
    _write_manifest(output / "manifest.csv", manifest)
    write_json(
        output / "design.json",
        {
            "schema_name": "marklab_crc_graph_topology_final_design",
            "schema_version": "1.0",
            "population_unit": "patient",
            "pattern_unit": "slide_nested_within_patient",
            "patient_count": len(patient_counts),
            "pattern_count": len(manifest),
            "cell_subsample_fraction": CELL_SUBSAMPLE_FRACTION,
            "coordinate_jitter_um": COORDINATE_JITTER_UM,
            "graph_radii_um": GRAPH_RADII_UM,
            "graph_times": GRAPH_TIMES,
            "topology_scales_um": TOPOLOGY_SCALES_UM,
            "topology_perturbation_replicates": TOPOLOGY_PERTURBATIONS,
            "seed": seed,
            "selection_uses_molecular_label": False,
            "finite_result_policy": "reject_non_finite_input_or_output",
        },
    )
    return manifest


def fusion_gate(stability: dict[str, dict[str, float]], increment: float) -> dict[str, object]:
    """Apply frozen stability and held-out incremental-information gates."""
    thresholds = {
        "cell_subsample": {"median": 0.9, "q10": 0.75},
        "specimen_leave_one_out": {"median": 0.8, "q10": 0.6},
        "coordinate_perturbation": {"median": 0.9, "q10": 0.75},
        "nearby_scale": {"median": 0.8, "q10": 0.6},
    }
    failed = []
    for comparison, required in thresholds.items():
        observed = stability.get(comparison, {})
        for statistic, threshold in required.items():
            value = observed.get(statistic)
            if value is None or not math.isfinite(float(value)) or float(value) < threshold:
                failed.append(f"{comparison}_{statistic}")
    if not math.isfinite(increment) or increment <= 0.0:
        failed.append("heldout_balanced_accuracy_increment")
    return {
        "eligible": not failed,
        "failed_checks": failed,
        "stability_thresholds": thresholds,
        "increment_threshold": "strictly_greater_than_zero_prespecified",
    }


def _read_marks(path: Path) -> list[dict[str, object]]:
    with path.open(newline="", encoding="utf-8") as source:
        reader = csv.DictReader(source)
        expected = [
            "pattern_id",
            "patient_id",
            "group",
            "point_id",
            "x_um",
            "y_um",
            "type_id",
        ]
        if reader.fieldnames != expected:
            raise AnalysisError(f"mark CSV header must be exactly {','.join(expected)}")
        return list(reader)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    prepare = commands.add_parser("prepare")
    prepare.add_argument("--marks", required=True, type=Path)
    prepare.add_argument("--out", required=True, type=Path)
    prepare.add_argument("--seed", type=int, default=SEED)
    extract = commands.add_parser("extract-nonspatial")
    extract.add_argument("--marks", required=True, type=Path)
    extract.add_argument("--inference-root", required=True, type=Path)
    extract.add_argument("--out", required=True, type=Path)
    return parser.parse_args()


def main() -> None:
    arguments = parse_args()
    if arguments.command == "prepare":
        prepare_requests(_read_marks(arguments.marks), arguments.out, arguments.seed)
    elif arguments.command == "extract-nonspatial":
        extract_nonspatial_embeddings(
            _read_marks(arguments.marks), arguments.inference_root, arguments.out
        )
    else:  # pragma: no cover - argparse owns the closed command set
        raise AssertionError(f"unknown command: {arguments.command}")


if __name__ == "__main__":
    main()
