#!/usr/bin/env python3
"""Prepare and summarize a bounded patient-replicated CRC sparse Fourier lane."""

from __future__ import annotations

import argparse
import concurrent.futures
import csv
import hashlib
import importlib.util
import json
import math
import os
import subprocess
from pathlib import Path
from typing import Any


VARIANTS = {
    "baseline": "graph_baseline_request",
    "subsample": "graph_subsample_request",
    "jitter": "graph_jitter_request",
    "radius_45": "graph_radius_45_request",
    "radius_55": "graph_radius_55_request",
}
MAXIMUM_TOTAL_MODES = 128
BASIS_ITERATIONS = 512
BASIS_RESIDUAL_TOLERANCE = 1e-4
PCA_COMPONENTS = 3


class FourierPatientError(ValueError):
    """A patient Fourier request or result violates the frozen contract."""


def _load_summary_module():
    path = Path(__file__).with_name("marklab_crc_graph_topology_summary.py")
    spec = importlib.util.spec_from_file_location("marklab_crc_graph_topology_summary", path)
    if spec is None or spec.loader is None:
        raise FourierPatientError("cannot load the canonical patient summary owner")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise FourierPatientError(f"cannot read JSON {path}: {error}") from error
    if not isinstance(value, dict):
        raise FourierPatientError(f"JSON root must be an object: {path}")
    return value


def _write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False) + "\n",
        encoding="utf-8",
    )


def _read_csv(path: Path) -> list[dict[str, str]]:
    try:
        with path.open(newline="", encoding="utf-8") as source:
            return list(csv.DictReader(source))
    except OSError as error:
        raise FourierPatientError(f"cannot read CSV {path}: {error}") from error


def _write_csv(path: Path, fieldnames: list[str], rows: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as target:
        writer = csv.DictWriter(target, fieldnames=fieldnames, lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _component_count(nodes: list[dict[str, Any]], radius_um: float) -> tuple[int, int]:
    if len(nodes) < 2 or not math.isfinite(radius_um) or radius_um <= 0.0:
        raise FourierPatientError("component preflight requires nodes and a positive finite radius")
    ids: set[str] = set()
    coordinates: list[tuple[float, float]] = []
    for node in nodes:
        identity = str(node.get("id", ""))
        values = node.get("coordinates_um")
        signal = node.get("signal")
        if (
            not identity
            or identity.strip() != identity
            or identity in ids
            or not isinstance(values, list)
            or len(values) != 2
            or signal is None
        ):
            raise FourierPatientError("component preflight node identity or shape is invalid")
        point = (float(values[0]), float(values[1]))
        if not all(math.isfinite(value) for value in (*point, float(signal))):
            raise FourierPatientError("component preflight node values must be finite")
        ids.add(identity)
        coordinates.append(point)
    parent = list(range(len(nodes)))

    def find(index: int) -> int:
        while parent[index] != index:
            parent[index] = parent[parent[index]]
            index = parent[index]
        return index

    def union(left: int, right: int) -> None:
        left_root = find(left)
        right_root = find(right)
        if left_root != right_root:
            parent[right_root] = left_root

    radius_squared = radius_um * radius_um
    pair_work = 0
    for left, (left_x, left_y) in enumerate(coordinates):
        for right in range(left + 1, len(coordinates)):
            pair_work += 1
            right_x, right_y = coordinates[right]
            delta_x = left_x - right_x
            delta_y = left_y - right_y
            if delta_x * delta_x + delta_y * delta_y <= radius_squared:
                union(left, right)
    return len({find(index) for index in range(len(nodes))}), pair_work


def _fourier_request(
    source: dict[str, Any], component_count: int, nonzero_modes: int
) -> dict[str, Any]:
    nodes = source.get("nodes")
    if not isinstance(nodes, list):
        raise FourierPatientError("source graph request lacks nodes")
    node_count = len(nodes)
    mode_count = component_count + nonzero_modes
    if mode_count > MAXIMUM_TOTAL_MODES or mode_count > node_count:
        raise FourierPatientError(
            f"component nullspace plus {nonzero_modes} modes requires {mode_count}, "
            f"above the bounded {MAXIMUM_TOTAL_MODES}-mode/node capacity"
        )
    maximum_candidate_pairs = int(
        source.get("maximum_candidate_pairs", node_count * (node_count - 1) // 2)
    )
    maximum_edges = int(source.get("maximum_edges", maximum_candidate_pairs))
    maximum_ritz_rotations = max(1, component_count * 4_096)
    maximum_matrix_vector_work = (
        (BASIS_ITERATIONS + 1)
        * nonzero_modes
        * component_count
        * (node_count + 2 * maximum_edges)
    )
    maximum_orthogonalization_work = (
        (2 * (BASIS_ITERATIONS + 1) + 7)
        * node_count
        * nonzero_modes
        * nonzero_modes
        + maximum_ritz_rotations * 8 * nonzero_modes
    )
    return {
        "basis": {
            "nodes": nodes,
            "radius_um": float(source["radius_um"]),
            "mode_count": mode_count,
            "maximum_iterations": BASIS_ITERATIONS,
            "residual_tolerance": BASIS_RESIDUAL_TOLERANCE,
            "maximum_nodes": node_count,
            "maximum_candidate_pairs": maximum_candidate_pairs,
            "maximum_edges": maximum_edges,
            "maximum_components": component_count,
            "maximum_matrix_vector_work": maximum_matrix_vector_work,
            "maximum_orthogonalization_work": maximum_orthogonalization_work,
            "maximum_ritz_rotations": maximum_ritz_rotations,
            "maximum_working_bytes": 64 * 1024 * 1024,
            "maximum_retained_bytes": 4 * 1024 * 1024,
        },
        "maximum_projection_work": node_count * mode_count,
        "maximum_fourier_retained_bytes": 4_096 + 256 * mode_count,
    }


def prepare_requests(
    prepared: Path,
    output: Path,
    nonzero_modes: int,
    maximum_pair_work: int,
) -> list[dict[str, Any]]:
    """Convert frozen graph requests into fixed-nonzero sparse Fourier requests."""
    prepared = prepared.resolve()
    output = output.resolve()
    if output.exists() or output.is_symlink():
        raise FourierPatientError(f"output already exists: {output}")
    if not 1 <= nonzero_modes <= 32 or maximum_pair_work <= 0:
        raise FourierPatientError("nonzero-mode or pair-work control is invalid")
    source_manifest = _read_csv(prepared / "manifest.csv")
    if not source_manifest:
        raise FourierPatientError("prepared manifest is empty")
    patient_patterns: dict[str, int] = {}
    patient_groups: dict[str, str] = {}
    manifest: list[dict[str, Any]] = []
    pair_work = 0
    for row in sorted(source_manifest, key=lambda item: item["pattern_id"]):
        pattern = row["pattern_id"]
        patient = row["patient_id"]
        group = row["group"]
        if (
            not pattern
            or Path(pattern).name != pattern
            or not patient
            or group not in {"MSI", "MSS"}
        ):
            raise FourierPatientError("prepared pattern, patient, or group identity is invalid")
        patient_patterns[patient] = patient_patterns.get(patient, 0) + 1
        previous = patient_groups.setdefault(patient, group)
        if previous != group:
            raise FourierPatientError("patient molecular group is inconsistent")
        prepared_row: dict[str, Any] = {
            "pattern_id": pattern,
            "patient_id": patient,
            "group": group,
            "pattern_count_for_patient": int(row["pattern_count_for_patient"]),
            "cell_count": int(row["cell_count"]),
            "retained_nonzero_mode_count": nonzero_modes,
        }
        for variant, column in VARIANTS.items():
            if column not in row or not row[column]:
                raise FourierPatientError(f"prepared manifest lacks {column}")
            source_path = (prepared / row[column]).resolve()
            try:
                source_path.relative_to(prepared)
            except ValueError as error:
                raise FourierPatientError("source graph request escapes the prepared root") from error
            source = _read_json(source_path)
            nodes = source.get("nodes")
            if not isinstance(nodes, list):
                raise FourierPatientError(f"source graph request lacks nodes: {source_path}")
            component_count, used = _component_count(nodes, float(source["radius_um"]))
            pair_work += used
            if pair_work > maximum_pair_work:
                raise FourierPatientError("component-preflight pair work exceeds caller maximum")
            request = _fourier_request(source, component_count, nonzero_modes)
            relative = Path("requests") / pattern / f"{variant}.json"
            _write_json(output / relative, request)
            prepared_row[f"{variant}_request"] = relative.as_posix()
            prepared_row[f"{variant}_source_sha256"] = _sha256(source_path)
            prepared_row[f"{variant}_component_count"] = component_count
            prepared_row[f"{variant}_mode_count"] = component_count + nonzero_modes
        manifest.append(prepared_row)
    if not manifest or set(patient_patterns.values()) != {2}:
        raise FourierPatientError("every admitted patient must contribute exactly two patterns")
    _write_csv(output / "manifest.csv", list(manifest[0]), manifest)
    _write_json(
        output / "design.json",
        {
            "schema_name": "marklab_crc_sparse_fourier_patient_design",
            "schema_version": "1.0",
            "population_unit": "patient",
            "pattern_unit": "slide_nested_within_patient",
            "patient_count": len(patient_patterns),
            "pattern_count": len(manifest),
            "variants": list(VARIANTS),
            "retained_nonzero_mode_count": nonzero_modes,
            "maximum_total_mode_count": MAXIMUM_TOTAL_MODES,
            "basis_iterations": BASIS_ITERATIONS,
            "basis_residual_tolerance": BASIS_RESIDUAL_TOLERANCE,
            "component_preflight": "independent_exact_all_pairs_radius_audit_checked_against_Rust_result",
            "component_preflight_pair_work": pair_work,
            "maximum_component_preflight_pair_work": maximum_pair_work,
            "selection_uses_molecular_label": False,
            "finite_result_policy": "reject_non_finite_input_or_output",
        },
    )
    return manifest


def execute_requests(
    prepared: Path,
    marklab: Path,
    output: Path,
    maximum_processes: int,
    timeout_seconds: int,
    replay: bool,
) -> dict[str, Any]:
    """Execute or replay every fixed patient-pattern request in bounded processes."""
    prepared = prepared.resolve()
    marklab = marklab.resolve()
    output = output.resolve()
    if not marklab.is_file() or not os.access(marklab, os.X_OK):
        raise FourierPatientError(f"marklab executable is unavailable: {marklab}")
    if not 1 <= maximum_processes <= 6 or not 1 <= timeout_seconds <= 3_600:
        raise FourierPatientError("process or timeout bound is invalid")
    if replay:
        if not output.is_dir():
            raise FourierPatientError("replay requires the completed execution directory")
    else:
        if output.exists() or output.is_symlink():
            raise FourierPatientError(f"execution output already exists: {output}")
        output.mkdir(parents=True)
    manifest = _read_csv(prepared / "manifest.csv")
    jobs = []
    for row in manifest:
        pattern = row["pattern_id"]
        if Path(pattern).name != pattern:
            raise FourierPatientError("execution pattern identity is not a path-safe basename")
        for variant in VARIANTS:
            request = (prepared / row[f"{variant}_request"]).resolve()
            try:
                request.relative_to(prepared)
            except ValueError as error:
                raise FourierPatientError("execution request escapes the prepared root") from error
            project = output / "projects" / variant / pattern
            result_root = "replay" if replay else "results"
            result = output / result_root / variant / f"{pattern}.json"
            baseline = output / "results" / variant / f"{pattern}.json"
            jobs.append((pattern, variant, request, project, result, baseline))
    if not jobs or len(jobs) > 512:
        raise FourierPatientError("execution job count is empty or exceeds 512")

    def run(job):
        pattern, variant, request, project, result, baseline = job
        result.parent.mkdir(parents=True, exist_ok=True)
        environment = os.environ.copy()
        if replay:
            environment["MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION"] = "1"
        command = [
            str(marklab),
            "project",
            "sparse-radius-fourier-energy",
            "--project",
            str(project),
            "--input",
            str(request),
            "--out",
            str(result),
        ]
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
            raise FourierPatientError(
                f"{pattern}/{variant} exceeded {timeout_seconds} seconds"
            ) from error
        expected = "hit" if replay else "miss"
        marker = f"cache_status={expected}"
        if completed.returncode != 0 or marker not in completed.stderr:
            raise FourierPatientError(
                f"{pattern}/{variant} failed or did not report {marker}: "
                f"{completed.stderr.strip()}"
            )
        if not result.is_file():
            raise FourierPatientError(f"{pattern}/{variant} did not publish a result")
        bytes_equal = True
        if replay:
            bytes_equal = baseline.is_file() and baseline.read_bytes() == result.read_bytes()
            if not bytes_equal:
                raise FourierPatientError(f"{pattern}/{variant} replay bytes differ")
        return {
            "pattern_id": pattern,
            "variant": variant,
            "request": request.relative_to(prepared).as_posix(),
            "request_sha256": _sha256(request),
            "project": project.relative_to(output).as_posix(),
            "result": result.relative_to(output).as_posix(),
            "result_sha256": _sha256(result),
            "cache_status": expected,
            "replay_bytes_equal": bytes_equal,
        }

    records = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=maximum_processes) as executor:
        futures = [executor.submit(run, job) for job in jobs]
        for future in concurrent.futures.as_completed(futures):
            records.append(future.result())
    records.sort(key=lambda row: (row["pattern_id"], row["variant"]))
    cache_status_counts = {
        status: sum(row["cache_status"] == status for row in records)
        for status in sorted({row["cache_status"] for row in records})
    }
    result = {
        "schema_name": "marklab_crc_sparse_fourier_execution",
        "schema_version": "1.0",
        "replay": replay,
        "population_unit": "patient",
        "job_count": len(records),
        "maximum_processes": maximum_processes,
        "timeout_seconds_per_process": timeout_seconds,
        "cache_status_counts": cache_status_counts,
        "all_replay_bytes_equal": all(row["replay_bytes_equal"] for row in records),
        "records": records,
    }
    _write_json(output / ("replay_manifest.json" if replay else "execution_manifest.json"), result)
    return result


def fourier_features(
    document: dict[str, Any], expected_components: int, nonzero_modes: int
) -> dict[str, float]:
    """Extract fixed-count intensive Fourier features from one typed result."""
    if document.get("format") != "marklab.graph_sparse_radius_fourier_energy":
        raise FourierPatientError("Fourier result format differs")
    node_count = int(document.get("node_count", 0))
    component_count = int(document.get("component_count", 0))
    if (
        node_count <= 0
        or component_count != expected_components
        or int(document.get("returned_mode_count", 0)) != component_count + nonzero_modes
        or document.get("total_signal_status") != "positive"
    ):
        raise FourierPatientError("Fourier result count or signal state differs")
    modes = document.get("mode_energies")
    if not isinstance(modes, list):
        raise FourierPatientError("Fourier mode energies are absent")
    retained = [mode for mode in modes if not bool(mode["component_zero_mode"])]
    if len(retained) != nonzero_modes:
        raise FourierPatientError("Fourier result does not retain the fixed nonzero-mode count")
    total = float(document["total_signal_energy"])
    centered = float(document["centered_signal_energy"])
    component_zero = float(document["component_zero_energy"])
    nonzero = float(document["nonzero_low_frequency_energy"])
    isolated = int(document.get("isolated_node_count", 0))
    energies = [float(mode["energy"]) for mode in retained]
    eigenvalues = [float(mode["eigenvalue"]) for mode in retained]
    values = [total, centered, component_zero, nonzero, *energies, *eigenvalues]
    if (
        total <= 0.0
        or centered <= 0.0
        or not 0 <= isolated <= node_count
        or any(not math.isfinite(value) or value < 0.0 for value in values)
    ):
        raise FourierPatientError("Fourier result energy is invalid or degenerate")
    energy_sum = math.fsum(energies)
    if not math.isclose(energy_sum, nonzero, rel_tol=1e-12, abs_tol=1e-12):
        raise FourierPatientError("Fourier nonzero energy does not match its modes")
    probabilities = [energy / energy_sum for energy in energies] if energy_sum > 0.0 else []
    entropy = (
        -math.fsum(value * math.log(value) for value in probabilities if value > 0.0)
        / math.log(nonzero_modes)
        if energy_sum > 0.0 and nonzero_modes > 1
        else 0.0
    )
    weighted_eigenvalue = (
        math.fsum(value * energy for value, energy in zip(eigenvalues, energies)) / energy_sum
        if energy_sum > 0.0
        else 0.0
    )
    features = {
        "component_fraction": component_count / node_count,
        "isolated_fraction": isolated / node_count,
        "component_zero_energy_fraction": component_zero / total,
        f"low{nonzero_modes}_centered_energy_fraction": nonzero / centered,
        f"low{nonzero_modes}_mean_eigenvalue": math.fsum(eigenvalues) / nonzero_modes,
        f"low{nonzero_modes}_energy_weighted_eigenvalue": weighted_eigenvalue,
        f"low{nonzero_modes}_energy_entropy": entropy,
    }
    if any(not math.isfinite(value) for value in features.values()):
        raise FourierPatientError("Fourier feature is non-finite")
    return features


def _baseline_features(path: Path) -> tuple[dict[str, str], dict[str, list[float]]]:
    rows = _read_csv(path)
    by_patient: dict[str, dict[str, float]] = {}
    labels: dict[str, str] = {}
    for row in rows:
        patient = row.get("patient_id", "")
        group = row.get("group", "")
        feature = row.get("feature", "")
        value = float(row.get("value", "nan"))
        if not patient or group not in {"MSI", "MSS"} or not feature or not math.isfinite(value):
            raise FourierPatientError("baseline patient feature row is invalid")
        if labels.setdefault(patient, group) != group or feature in by_patient.setdefault(patient, {}):
            raise FourierPatientError("baseline patient feature identity is duplicate or inconsistent")
        by_patient[patient][feature] = value
    features = sorted(next(iter(by_patient.values()))) if by_patient else []
    if not features or any(sorted(values) != features for values in by_patient.values()):
        raise FourierPatientError("baseline patient features are incomplete")
    return labels, {
        patient: [values[feature] for feature in features]
        for patient, values in sorted(by_patient.items())
    }


def summarize(
    prepared: Path,
    execution: Path,
    baseline_path: Path,
    output: Path,
    seed: int,
) -> dict[str, Any]:
    """Seal patient-unit stability, held-out, and permutation evidence."""
    prepared = prepared.resolve()
    execution = execution.resolve()
    output = output.resolve()
    if output.exists() or output.is_symlink():
        raise FourierPatientError(f"output already exists: {output}")
    summary_owner = _load_summary_module()
    lane = summary_owner.load_lane_module()
    manifest = _read_csv(prepared / "manifest.csv")
    if not manifest:
        raise FourierPatientError("Fourier manifest is empty")
    labels: dict[str, str] = {}
    counts: dict[str, int] = {}
    nonzero_counts = {int(row["retained_nonzero_mode_count"]) for row in manifest}
    if len(nonzero_counts) != 1:
        raise FourierPatientError("Fourier manifest mixes nonzero-mode counts")
    nonzero_modes = next(iter(nonzero_counts))
    for row in manifest:
        patient = row["patient_id"]
        group = row["group"]
        if labels.setdefault(patient, group) != group:
            raise FourierPatientError("Fourier patient group is inconsistent")
        counts[patient] = counts.get(patient, 0) + 1
    if set(counts.values()) != {2} or len(labels) < 4 or set(labels.values()) != {"MSI", "MSS"}:
        raise FourierPatientError("Fourier summary requires two slides per patient and two groups")
    baseline_labels, baseline = _baseline_features(baseline_path.resolve())
    if baseline_labels != labels:
        raise FourierPatientError("baseline and Fourier patient identities or labels differ")

    variants: dict[str, dict[str, dict[str, float]]] = {name: {} for name in VARIANTS}
    result_paths: list[Path] = []
    specimen_rows: list[dict[str, Any]] = []
    for row in manifest:
        pattern = row["pattern_id"]
        for variant in VARIANTS:
            path = execution / "results" / variant / f"{pattern}.json"
            features = fourier_features(
                _read_json(path), int(row[f"{variant}_component_count"]), nonzero_modes
            )
            variants[variant][pattern] = features
            result_paths.append(path)
            if variant == "baseline":
                specimen_rows.extend(
                    {
                        "cohort": "CPTAC_COAD_CellViT",
                        "patient_id": row["patient_id"],
                        "specimen_id": pattern,
                        "feature": feature,
                        "value": value,
                    }
                    for feature, value in sorted(features.items())
                )
    patient = {
        name: summary_owner.patient_means(features, manifest)
        for name, features in variants.items()
    }
    stability = {
        "cell_subsample": lane.feature_stability(patient["baseline"], [patient["subsample"]]),
        "specimen_leave_one_out": lane.feature_stability(
            patient["baseline"],
            summary_owner.leave_one_specimen_alternatives(variants["baseline"], manifest),
        ),
        "coordinate_perturbation": lane.feature_stability(
            patient["baseline"], [patient["jitter"]]
        ),
        "nearby_scale": lane.feature_stability(
            patient["baseline"], [patient["radius_45"], patient["radius_55"]]
        ),
    }
    fourier_block = {
        name: [features[key] for key in sorted(features)]
        for name, features in patient["baseline"].items()
    }
    model_blocks = {
        "m0_m3_nonspatial": {"baseline": baseline},
        "fourier_only": {"fourier": fourier_block},
        "m0_m3_fourier": {"baseline": baseline, "fourier": fourier_block},
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
        models["m0_m3_nonspatial"], models["m0_m3_fourier"], seed + 100, lane
    )
    gate = lane.fusion_gate(stability, increment["balanced_accuracy_increment"])
    similarity: dict[str, Any] = {}
    similarity_matrices: dict[str, list[dict[str, Any]]] = {}
    for index, (name, blocks) in enumerate(model_blocks.items()):
        vectors = summary_owner.global_vectors(blocks, lane)
        matrix, effect = summary_owner.similarity_summary(vectors, labels, seed + 200 + index, lane)
        similarity[name] = effect
        similarity_matrices[name] = matrix

    source_digest = hashlib.sha256()
    for path in sorted(result_paths + [prepared / "design.json", baseline_path.resolve()]):
        source_digest.update(_sha256(path).encode("ascii"))
        source_digest.update(b"\n")
    provenance = source_digest.hexdigest()
    staging = output.with_name(f".{output.name}.{os.getpid()}.tmp")
    if staging.exists() or staging.is_symlink():
        raise FourierPatientError(f"staging path already exists: {staging}")
    staging.mkdir(parents=True)
    _write_json(
        staging / "stability.json",
        {
            "schema_name": "marklab_crc_sparse_fourier_patient_stability",
            "schema_version": "1.0",
            "population_unit": "patient",
            "diagnostics": stability,
        },
    )
    _write_csv(
        staging / "specimen_fingerprints.csv",
        ["cohort", "patient_id", "specimen_id", "feature", "value"],
        specimen_rows,
    )
    patient_rows = [
        {
            "cohort": "CPTAC_COAD_CellViT",
            "patient_id": patient_id,
            "group": labels[patient_id],
            "feature": feature,
            "value": value,
        }
        for patient_id, features in sorted(patient["baseline"].items())
        for feature, value in sorted(features.items())
    ]
    _write_csv(
        staging / "patient_fingerprints.csv",
        ["cohort", "patient_id", "group", "feature", "value"],
        patient_rows,
    )
    heldout = [
        {"model": name, **row}
        for name, result in models.items()
        for row in result["predictions"]
    ]
    _write_csv(staging / "heldout_predictions.csv", list(heldout[0]), heldout)
    for name, matrix in similarity_matrices.items():
        _write_csv(staging / "similarity" / f"{name}.csv", list(matrix[0]), matrix)
    summary = {
        "schema_name": "marklab_crc_sparse_fourier_patient_summary",
        "schema_version": "1.0",
        "population_unit": "patient",
        "patient_count": len(labels),
        "pattern_count": len(manifest),
        "retained_nonzero_mode_count": nonzero_modes,
        "models": models,
        "stability": stability,
        "incremental_information": increment,
        "fusion_gate": gate,
        "similarity": similarity,
        "leakage_checks": {
            "patient_held_out": True,
            "preprocessing_inside_each_training_fold": True,
            "site_held_out": "unavailable_exact_blocker_no_acquisition_site_field_in_admitted_CPTAC_manifest",
        },
        "interpretation_policy": "null_confounded_and_unstable_results_retained_without_threshold_or_subset_optimization",
        "claim_limitations": [
            "eight-patient resource-feasible exploratory subset",
            "two slides per patient are nested diagnostics and never independent population replicates",
            "hard CellViT classes are model outputs",
            "ordered graph modes have multiplicity and truncation limitations",
            "no clinical, causal, prospective, equivalence, or transportability claim",
        ],
        "source_result_sha256": provenance,
    }
    _write_json(staging / "summary.json", summary)
    artifacts = sorted(path for path in staging.rglob("*") if path.is_file())
    _write_json(
        staging / "manifest.json",
        {
            "schema_name": "marklab_crc_sparse_fourier_patient_bundle",
            "schema_version": "1.0",
            "population_unit": "patient",
            "source_result_sha256": provenance,
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
    prepare = commands.add_parser("prepare")
    prepare.add_argument("--prepared", required=True, type=Path)
    prepare.add_argument("--out", required=True, type=Path)
    prepare.add_argument("--nonzero-modes", type=int, default=8)
    prepare.add_argument("--maximum-pair-work", type=int, default=20_000_000)
    summary = commands.add_parser("summarize")
    summary.add_argument("--prepared", required=True, type=Path)
    summary.add_argument("--execution", required=True, type=Path)
    summary.add_argument("--baseline", required=True, type=Path)
    summary.add_argument("--out", required=True, type=Path)
    summary.add_argument("--seed", type=int, default=20260829)
    execute = commands.add_parser("execute")
    execute.add_argument("--prepared", required=True, type=Path)
    execute.add_argument("--marklab", required=True, type=Path)
    execute.add_argument("--out", required=True, type=Path)
    execute.add_argument("--maximum-processes", type=int, default=6)
    execute.add_argument("--timeout-seconds", type=int, default=300)
    execute.add_argument("--replay", action="store_true")
    return parser.parse_args()


def main() -> None:
    arguments = parse_args()
    if arguments.command == "prepare":
        prepare_requests(
            arguments.prepared,
            arguments.out,
            arguments.nonzero_modes,
            arguments.maximum_pair_work,
        )
    elif arguments.command == "summarize":
        summarize(
            arguments.prepared,
            arguments.execution,
            arguments.baseline,
            arguments.out,
            arguments.seed,
        )
    elif arguments.command == "execute":
        execute_requests(
            arguments.prepared,
            arguments.marklab,
            arguments.out,
            arguments.maximum_processes,
            arguments.timeout_seconds,
            arguments.replay,
        )
    else:  # pragma: no cover - argparse owns the closed command set
        raise AssertionError(f"unknown command: {arguments.command}")


if __name__ == "__main__":
    main()
