#!/usr/bin/env python3
"""Seal patient-level graph/topology evidence for SCIENCE-CRC-FINAL-01."""

from __future__ import annotations

import argparse
import csv
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import random
from typing import Any, Iterable


SEED = 20260829
BOOTSTRAP_REPLICATES = 1000
PCA_COMPONENTS = 3


class SummaryError(ValueError):
    """A graph/topology result violates the frozen scientific summary contract."""


def _finite(value: object, name: str) -> float:
    try:
        parsed = float(value)
    except (TypeError, ValueError) as error:
        raise SummaryError(f"{name} is not numeric") from error
    if not math.isfinite(parsed):
        raise SummaryError(f"{name} is nonfinite")
    return parsed


def graph_features(document: dict[str, Any]) -> dict[str, float]:
    """Extract intensive order-two scattering and graph-geometry features."""
    if document.get("format") != "marklab.graph_sparse_radius_scattering":
        raise SummaryError("graph result format differs")
    node_count = int(document.get("node_count", 0))
    edge_count = int(document.get("edge_count", -1))
    isolates = int(document.get("isolated_node_count", -1))
    pairs = node_count * (node_count - 1) // 2
    if node_count < 2 or not 0 <= edge_count <= pairs or not 0 <= isolates <= node_count:
        raise SummaryError("graph result counts are invalid")
    result = {
        "edge_density": edge_count / pairs,
        "isolated_fraction": isolates / node_count,
        "coarse_mean_absolute": _finite(document.get("coarse_mean_absolute"), "coarse mean"),
        "coarse_energy_per_cell": _finite(document.get("coarse_energy"), "coarse energy")
        / node_count,
    }
    for row in document.get("first_order", []):
        level = int(row["level"])
        result[f"first_{level}_mean_absolute"] = _finite(row["mean_absolute"], "first mean")
        result[f"first_{level}_energy_per_cell"] = _finite(row["energy"], "first energy") / node_count
    for row in document.get("second_order", []):
        first = int(row["first_level"])
        second = int(row["second_level"])
        prefix = f"second_{first}_{second}"
        result[f"{prefix}_mean_absolute"] = _finite(row["mean_absolute"], "second mean")
        result[f"{prefix}_energy_per_cell"] = _finite(row["energy"], "second energy") / node_count
    if len(document.get("first_order", [])) != 4 or len(document.get("second_order", [])) != 6:
        raise SummaryError("graph scattering coefficient count differs")
    return result


def topology_features(document: dict[str, Any]) -> dict[str, float]:
    """Extract witness coverage, simplex, and persistence summaries."""
    if document.get("format") != "marklab.witness_persistence":
        raise SummaryError("topology result format differs")
    approximation = document.get("approximation", {})
    landmarks = int(approximation.get("landmark_count", 0))
    counts = document.get("filtration", {}).get("simplex_counts_by_dimension")
    dimensions = document.get("persistence", {}).get("by_dimension")
    if landmarks < 3 or not isinstance(counts, list) or len(counts) != 3:
        raise SummaryError("topology approximation counts differ")
    if not isinstance(dimensions, list) or [row.get("dimension") for row in dimensions] != [0, 1, 2]:
        raise SummaryError("topology persistence dimensions differ")
    result = {"coverage_radius_um": _finite(approximation.get("coverage_radius_um"), "coverage")}
    for dimension, count in enumerate(counts):
        result[f"dimension_{dimension}_simplex_count_per_landmark"] = int(count) / landmarks
    for row in dimensions:
        dimension = int(row["dimension"])
        finite_pairs = row.get("finite_pairs")
        essential = row.get("essential_births")
        if not isinstance(finite_pairs, list) or not isinstance(essential, list):
            raise SummaryError("topology persistence containers differ")
        total = math.fsum(
            _finite(pair.get("death"), "persistence death")
            - _finite(pair.get("birth"), "persistence birth")
            for pair in finite_pairs
        )
        if total < 0.0:
            raise SummaryError("topology persistence is negative")
        result[f"dimension_{dimension}_finite_count_per_landmark"] = len(finite_pairs) / landmarks
        result[f"dimension_{dimension}_essential_count_per_landmark"] = len(essential) / landmarks
        result[f"dimension_{dimension}_total_persistence_um_squared"] = total
    return result


def read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def read_csv(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as source:
        rows = list(csv.DictReader(source))
    if not rows:
        raise SummaryError(f"CSV has no rows: {path}")
    return rows


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def write_json(path: Path, document: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_csv(path: Path, fields: list[str], rows: Iterable[dict[str, object]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as target:
        writer = csv.DictWriter(target, fieldnames=fields, lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def load_lane_module():
    path = Path(__file__).with_name("marklab_crc_graph_topology_final.py")
    spec = importlib.util.spec_from_file_location("marklab_crc_graph_topology_final", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def patient_means(
    specimen: dict[str, dict[str, float]], manifest: list[dict[str, str]]
) -> dict[str, dict[str, float]]:
    by_patient: dict[str, list[str]] = {}
    for row in manifest:
        by_patient.setdefault(row["patient_id"], []).append(row["pattern_id"])
    if set(specimen) != {row["pattern_id"] for row in manifest}:
        raise SummaryError("specimen feature identities differ from the manifest")
    result = {}
    for patient, patterns in sorted(by_patient.items()):
        if len(patterns) != 2:
            raise SummaryError("patient does not have exactly two nested slide patterns")
        features = sorted(specimen[patterns[0]])
        if any(sorted(specimen[pattern]) != features for pattern in patterns):
            raise SummaryError("specimen feature identities differ")
        result[patient] = {
            feature: math.fsum(specimen[pattern][feature] for pattern in patterns) / 2.0
            for feature in features
        }
    return result


def leave_one_specimen_alternatives(
    specimen: dict[str, dict[str, float]], manifest: list[dict[str, str]]
) -> list[dict[str, dict[str, float]]]:
    by_patient: dict[str, list[str]] = {}
    for row in manifest:
        by_patient.setdefault(row["patient_id"], []).append(row["pattern_id"])
    for patterns in by_patient.values():
        patterns.sort()
    return [
        {patient: dict(specimen[patterns[index]]) for patient, patterns in sorted(by_patient.items())}
        for index in (0, 1)
    ]


def clinical_features(path: Path, patients: set[str]) -> dict[str, list[float]]:
    with path.open(newline="", encoding="utf-8") as source:
        rows = list(csv.reader(source, delimiter="\t"))
    if not rows or not rows[0] or rows[0][0] != "attrib_name":
        raise SummaryError("clinical technical-covariate table differs")
    columns = rows[0][1:]
    age_rows = [row for row in rows[1:] if row and row[0] == "Age"]
    gender_rows = [row for row in rows[1:] if row and row[0] == "Gender"]
    if len(age_rows) != 1 or len(gender_rows) != 1:
        raise SummaryError("clinical Age/Gender rows differ")
    result = {}
    for patient in sorted(patients):
        if patient not in columns:
            raise SummaryError(f"clinical table lacks patient {patient}")
        index = columns.index(patient) + 1
        age = _finite(age_rows[0][index], "clinical age")
        gender = gender_rows[0][index]
        if age <= 0.0 or gender not in {"Female", "Male"}:
            raise SummaryError("clinical technical covariate is invalid")
        result[patient] = [age, float(gender == "Male")]
    return result


def composition_features(
    marks: list[dict[str, str]], patients: set[str]
) -> dict[str, list[float]]:
    counts = {patient: {name: 0 for name in ("Neoplastic", "Inflammatory", "Connective")} for patient in patients}
    totals = {patient: 0 for patient in patients}
    for row in marks:
        patient = row["patient_id"]
        if patient not in counts or row["type_id"] not in counts[patient]:
            raise SummaryError("mark table patient or type differs")
        counts[patient][row["type_id"]] += 1
        totals[patient] += 1
    if any(total != 1024 for total in totals.values()):
        raise SummaryError("patient does not contribute two 512-cell patterns")
    return {
        patient: [counts[patient][name] / totals[patient] for name in ("Neoplastic", "Inflammatory", "Connective")]
        for patient in sorted(patients)
    }


def nonspatial_features(root: Path, manifest: list[dict[str, str]]) -> dict[str, list[float]]:
    specimen = {}
    for row in read_csv(root / "manifest.csv"):
        document = read_json(root / row["summary"])
        if document.get("schema_name") != "marklab_crc_cellvit_nonspatial_specimen_summary":
            raise SummaryError("nonspatial summary schema differs")
        mean = [_finite(value, "embedding mean") for value in document["mean"]]
        deviation = [_finite(value, "embedding SD") for value in document["standard_deviation"]]
        if len(mean) != 1280 or len(deviation) != 1280:
            raise SummaryError("nonspatial CellViT width differs")
        specimen[row["pattern_id"]] = mean + deviation
    patient = patient_means(
        {pattern: {f"embedding_{index:04d}": value for index, value in enumerate(values)} for pattern, values in specimen.items()},
        manifest,
    )
    return {name: [features[key] for key in sorted(features)] for name, features in patient.items()}


def global_vectors(
    blocks: dict[str, dict[str, list[float]]], lane: Any
) -> dict[str, list[float]]:
    try:
        import numpy as np
    except ImportError as error:
        raise SummaryError("pinned numerical environment is unavailable") from error
    patients = sorted(next(iter(blocks.values())))
    result = {patient: [] for patient in patients}
    for block_name in sorted(blocks):
        block = blocks[block_name]
        matrix = np.asarray([block[patient] for patient in patients], dtype=np.float64)
        mean = matrix.mean(axis=0)
        scale = matrix.std(axis=0)
        scale[scale == 0.0] = 1.0
        transformed = (matrix - mean) / scale
        if transformed.shape[1] > PCA_COMPONENTS:
            _, _, right = np.linalg.svd(transformed, full_matrices=False)
            transformed = transformed @ right[: min(PCA_COMPONENTS, right.shape[0])].T
        for patient, values in zip(patients, transformed.tolist()):
            result[patient].extend(float(value) for value in values)
    return result


def fold_vectors(
    blocks: dict[str, dict[str, list[float]]], lane: Any
) -> dict[str, dict[str, list[float]]]:
    patients = sorted(next(iter(blocks.values())))
    result = {}
    for heldout in patients:
        training = [patient for patient in patients if patient != heldout]
        vectors = {patient: [] for patient in patients}
        for name in sorted(blocks):
            transformed, query = lane._fold_transform(
                [blocks[name][patient] for patient in training], blocks[name][heldout], PCA_COMPONENTS
            )
            for patient, values in zip(training, transformed):
                vectors[patient].extend(values)
            vectors[heldout].extend(query)
        result[heldout] = vectors
    return result


def exact_label_permutation(
    labels: dict[str, str], blocks: dict[str, dict[str, list[float]]], lane: Any
) -> dict[str, object]:
    from itertools import combinations

    patients = sorted(labels)
    msi_count = sum(group == "MSI" for group in labels.values())
    observed = lane.heldout_model(labels, blocks, PCA_COMPONENTS)["balanced_accuracy"]
    values = []
    for selected in combinations(patients, msi_count):
        selected = set(selected)
        permuted = {patient: ("MSI" if patient in selected else "MSS") for patient in patients}
        values.append(lane.heldout_model(permuted, blocks, PCA_COMPONENTS)["balanced_accuracy"])
    return {
        "null": "whole_patient_molecular_labels_exchangeable_within_the_admitted_subset",
        "assignment_count": len(values),
        "observed_balanced_accuracy": observed,
        "p_value_inclusive_exact": sum(value >= observed for value in values) / len(values),
        "null_mean_balanced_accuracy": math.fsum(values) / len(values),
    }


def model_uncertainty(result: dict[str, Any], seed: int) -> dict[str, object]:
    rng = random.Random(seed)
    by_group = {
        group: [row for row in result["predictions"] if row["observed_group"] == group]
        for group in ("MSI", "MSS")
    }
    balanced = []
    retrieval = []
    for _ in range(BOOTSTRAP_REPLICATES):
        samples = {group: [rng.choice(rows) for _ in rows] for group, rows in by_group.items()}
        balanced.append(
            math.fsum(float(row["correct"]) for rows in samples.values() for row in rows)
            / sum(len(rows) for rows in samples.values())
        )
        retrieval.append(
            math.fsum(float(row["retrieval_group_correct"]) for rows in samples.values() for row in rows)
            / sum(len(rows) for rows in samples.values())
        )
    lane = load_lane_module()
    return {
        "resampling_unit": "whole_patient_stratified_by_molecular_group",
        "replicates": BOOTSTRAP_REPLICATES,
        "balanced_accuracy_interval_95": [lane._percentile(balanced, 0.025), lane._percentile(balanced, 0.975)],
        "retrieval_group_accuracy_interval_95": [lane._percentile(retrieval, 0.025), lane._percentile(retrieval, 0.975)],
    }


def incremental_summary(
    reference: dict[str, Any], candidate: dict[str, Any], seed: int, lane: Any
) -> dict[str, object]:
    reference_rows = {row["patient_id"]: row for row in reference["predictions"]}
    candidate_rows = {row["patient_id"]: row for row in candidate["predictions"]}
    patients = sorted(reference_rows)
    differences = [
        float(candidate_rows[patient]["correct"]) - float(reference_rows[patient]["correct"])
        for patient in patients
    ]
    rng = random.Random(seed)
    bootstrap = [
        math.fsum(rng.choice(differences) for _ in differences) / len(differences)
        for _ in range(BOOTSTRAP_REPLICATES)
    ]
    increment = candidate["balanced_accuracy"] - reference["balanced_accuracy"]
    return {
        "balanced_accuracy_increment": increment,
        "whole_patient_bootstrap_interval_95": [lane._percentile(bootstrap, 0.025), lane._percentile(bootstrap, 0.975)],
        "candidate_retrieval_group_accuracy_increment": candidate["retrieval_group_accuracy"]
        - reference["retrieval_group_accuracy"],
    }


def similarity_summary(
    vectors: dict[str, list[float]], labels: dict[str, str], seed: int, lane: Any
) -> tuple[list[dict[str, object]], dict[str, object]]:
    patients = sorted(vectors)
    rows = []
    within = []
    between = []
    for left in patients:
        row: dict[str, object] = {"patient_id": left}
        for right in patients:
            distance = lane._distance(vectors[left], vectors[right])
            row[right] = distance
            if left < right:
                (within if labels[left] == labels[right] else between).append(distance)
        rows.append(row)
    effect = math.fsum(between) / len(between) - math.fsum(within) / len(within)
    rng = random.Random(seed)
    bootstrap = []
    for _ in range(BOOTSTRAP_REPLICATES):
        sampled = []
        for group in ("MSI", "MSS"):
            group_patients = [patient for patient in patients if labels[patient] == group]
            sampled.extend((rng.choice(group_patients), group) for _ in group_patients)
        boot_within = []
        boot_between = []
        for index, (left, left_group) in enumerate(sampled):
            for right, right_group in sampled[index + 1 :]:
                target = boot_within if left_group == right_group else boot_between
                target.append(lane._distance(vectors[left], vectors[right]))
        bootstrap.append(
            math.fsum(boot_between) / len(boot_between) - math.fsum(boot_within) / len(boot_within)
        )
    return rows, {
        "effect_between_minus_within_distance": effect,
        "whole_patient_bootstrap_interval_95": [lane._percentile(bootstrap, 0.025), lane._percentile(bootstrap, 0.975)],
        "interpretation": "positive_values_mean_between_class_patients_are_farther_than_within_class_patients",
    }


def write_retrieval_inputs(
    root: Path,
    models: dict[str, dict[str, dict[str, list[float]]]],
    labels: dict[str, str],
    provenance_sha256: str,
    lane: Any,
) -> None:
    for model, blocks in models.items():
        for heldout, vectors in fold_vectors(blocks, lane).items():
            features = [f"embedding_{index}" for index in range(len(vectors[heldout]))]
            training_rows = []
            for patient in sorted(vectors):
                if patient == heldout:
                    continue
                training_rows.append(
                    {
                        "region_id": patient,
                        "patient_id": patient,
                        "site_id": "site_metadata_unavailable",
                        "split": "train",
                        "domain": "CPTAC_CRC",
                        "provenance_sha256": provenance_sha256,
                        **dict(zip(features, vectors[patient])),
                    }
                )
            query_rows = [
                {
                    "region_id": heldout,
                    "patient_id": heldout,
                    "site_id": "site_metadata_unavailable",
                    "domain": "CPTAC_CRC",
                    "provenance_sha256": provenance_sha256,
                    **dict(zip(features, vectors[heldout])),
                }
            ]
            destination = root / "retrieval_inputs" / model / heldout
            write_csv(
                destination / "training.csv",
                ["region_id", "patient_id", "site_id", "split", "domain", "provenance_sha256", *features],
                training_rows,
            )
            write_csv(
                destination / "query.csv",
                ["region_id", "patient_id", "site_id", "domain", "provenance_sha256", *features],
                query_rows,
            )


def build(arguments: argparse.Namespace) -> None:
    prepared = arguments.prepared.resolve()
    nonspatial_root = arguments.nonspatial.resolve()
    output = arguments.out.resolve()
    if output.exists() or output.is_symlink():
        raise SummaryError(f"output already exists: {output}")
    lane = load_lane_module()
    manifest = read_csv(prepared / "manifest.csv")
    marks = read_csv(arguments.marks.resolve())
    patients = {row["patient_id"] for row in manifest}
    labels = {row["patient_id"]: row["group"] for row in manifest}
    if len(patients) != 8 or set(labels.values()) != {"MSI", "MSS"}:
        raise SummaryError("admitted patient subset differs")

    graph_variants: dict[str, dict[str, dict[str, float]]] = {
        name: {} for name in ("baseline", "subsample", "jitter", "radius_45", "radius_55")
    }
    topology_variants: dict[str, dict[str, dict[str, float]]] = {
        name: {} for name in ("baseline", "subsample", "scale_180", "scale_220")
    }
    topology_coordinate_rows = []
    specimen_rows = []
    result_paths = []
    for row in manifest:
        pattern = row["pattern_id"]
        graph_paths = {
            "baseline": prepared / "results" / "graph" / pattern / "graph_baseline.json",
            "subsample": prepared / "results" / "graph" / pattern / "graph_subsample.json",
            "jitter": prepared / "results" / "graph" / pattern / "graph_jitter.json",
            "radius_45": prepared / "results" / "graph" / pattern / "graph_radius_45.json",
            "radius_55": prepared / "results" / "graph" / pattern / "graph_radius_55.json",
        }
        for name, path in graph_paths.items():
            graph_variants[name][pattern] = graph_features(read_json(path))
            result_paths.append(path)
        topology_paths = {
            "subsample": prepared / "results" / "topology" / pattern / "topology_subsample.json",
            "scale_180": prepared / "results" / "topology" / pattern / "topology_scale_180.json",
            "scale_220": prepared / "results" / "topology" / pattern / "topology_scale_220.json",
        }
        stability_path = prepared / "results" / "topology" / pattern / "topology_stability.json"
        stability = read_json(stability_path)
        topology_variants["baseline"][pattern] = topology_features(stability["baseline"])
        result_paths.append(stability_path)
        for name, path in topology_paths.items():
            topology_variants[name][pattern] = topology_features(read_json(path))
            result_paths.append(path)
        topology_coordinate_rows.append(
            {
                "pattern_id": pattern,
                "patient_id": row["patient_id"],
                "stable": bool(stability["stable_under_declared_thresholds"]),
                "minimum_landmark_id_match_fraction": stability["minimum_landmark_id_match_fraction"],
                "maximum_coverage_radius_change_um": stability["maximum_coverage_radius_change_um"],
                "maximum_simplex_count_l1_change": stability["maximum_simplex_count_l1_change"],
                "maximum_total_persistence_change_um_squared": stability[
                    "maximum_total_persistence_change_um_squared"
                ],
            }
        )
        for lane_name, features in (
            ("graph", graph_variants["baseline"][pattern]),
            ("topology", topology_variants["baseline"][pattern]),
        ):
            specimen_rows.extend(
                {
                    "cohort": "CPTAC_COAD_CellViT",
                    "lane": lane_name,
                    "patient_id": row["patient_id"],
                    "specimen_id": pattern,
                    "feature": feature,
                    "value": value,
                }
                for feature, value in sorted(features.items())
            )

    graph_patient = {name: patient_means(features, manifest) for name, features in graph_variants.items()}
    topology_patient = {name: patient_means(features, manifest) for name, features in topology_variants.items()}
    graph_stability = {
        "cell_subsample": lane.feature_stability(graph_patient["baseline"], [graph_patient["subsample"]]),
        "specimen_leave_one_out": lane.feature_stability(
            graph_patient["baseline"], leave_one_specimen_alternatives(graph_variants["baseline"], manifest)
        ),
        "coordinate_perturbation": lane.feature_stability(graph_patient["baseline"], [graph_patient["jitter"]]),
        "nearby_scale": lane.feature_stability(
            graph_patient["baseline"], [graph_patient["radius_45"], graph_patient["radius_55"]]
        ),
    }
    topology_direct_stable = all(row["stable"] for row in topology_coordinate_rows)
    topology_stability = {
        "cell_subsample": lane.feature_stability(topology_patient["baseline"], [topology_patient["subsample"]]),
        "specimen_leave_one_out": lane.feature_stability(
            topology_patient["baseline"], leave_one_specimen_alternatives(topology_variants["baseline"], manifest)
        ),
        "coordinate_perturbation": {
            "population_unit": "patient_with_slide_diagnostics",
            "stable_pattern_fraction": sum(row["stable"] for row in topology_coordinate_rows)
            / len(topology_coordinate_rows),
            "all_patterns_stable_under_declared_witness_thresholds": topology_direct_stable,
            "median": float(topology_direct_stable),
            "q10": float(topology_direct_stable),
        },
        "nearby_scale": lane.feature_stability(
            topology_patient["baseline"], [topology_patient["scale_180"], topology_patient["scale_220"]]
        ),
    }

    composition = composition_features(marks, patients)
    clinical = clinical_features(arguments.clinical.resolve(), patients)
    composition_technical = {
        patient: composition[patient] + clinical[patient] for patient in sorted(patients)
    }
    embedding = nonspatial_features(nonspatial_root, manifest)
    graph_block = {
        patient: [features[name] for name in sorted(features)]
        for patient, features in graph_patient["baseline"].items()
    }
    topology_block = {
        patient: [features[name] for name in sorted(features)]
        for patient, features in topology_patient["baseline"].items()
    }
    model_blocks = {
        "m0_composition_technical": {"composition_technical": composition_technical},
        "m0_m3_nonspatial": {
            "composition_technical": composition_technical,
            "nonspatial_embedding": embedding,
        },
        "graph_only": {"graph": graph_block},
        "m0_m3_graph": {
            "composition_technical": composition_technical,
            "nonspatial_embedding": embedding,
            "graph": graph_block,
        },
        "topology_only": {"topology": topology_block},
        "m0_m3_topology": {
            "composition_technical": composition_technical,
            "nonspatial_embedding": embedding,
            "topology": topology_block,
        },
    }
    models = {}
    for index, (name, blocks) in enumerate(model_blocks.items()):
        evaluation = lane.heldout_model(labels, blocks, PCA_COMPONENTS)
        evaluation["uncertainty"] = model_uncertainty(evaluation, SEED + index)
        evaluation["whole_patient_permutation"] = exact_label_permutation(labels, blocks, lane)
        models[name] = evaluation
    graph_increment = incremental_summary(models["m0_m3_nonspatial"], models["m0_m3_graph"], SEED + 100, lane)
    topology_increment = incremental_summary(
        models["m0_m3_nonspatial"], models["m0_m3_topology"], SEED + 101, lane
    )
    graph_gate = lane.fusion_gate(graph_stability, graph_increment["balanced_accuracy_increment"])
    topology_gate = lane.fusion_gate(
        topology_stability, topology_increment["balanced_accuracy_increment"]
    )
    final_blocks = dict(model_blocks["m0_m3_nonspatial"])
    included = []
    if graph_gate["eligible"]:
        final_blocks["graph"] = graph_block
        included.append("graph")
    if topology_gate["eligible"]:
        final_blocks["topology"] = topology_block
        included.append("topology")
    final_name = "final_fused_admitted_blocks"
    model_blocks[final_name] = final_blocks
    final_model = lane.heldout_model(labels, final_blocks, PCA_COMPONENTS)
    final_model["uncertainty"] = model_uncertainty(final_model, SEED + 200)
    final_model["whole_patient_permutation"] = exact_label_permutation(labels, final_blocks, lane)
    models[final_name] = final_model

    source_digest = hashlib.sha256()
    for path in sorted(result_paths + [prepared / "design.json", nonspatial_root / "provenance.json"]):
        source_digest.update(sha256(path).encode("ascii"))
        source_digest.update(b"\n")
    provenance_sha256 = source_digest.hexdigest()
    staging = output.with_name(f".{output.name}.{os.getpid()}.tmp")
    if staging.exists():
        raise SummaryError(f"staging path already exists: {staging}")
    staging.mkdir(parents=True)
    write_json(
        staging / "stability.json",
        {
            "schema_name": "marklab_crc_graph_topology_patient_stability",
            "schema_version": "1.0",
            "population_unit": "patient",
            "graph": graph_stability,
            "topology": topology_stability,
            "topology_coordinate_pattern_diagnostics": topology_coordinate_rows,
        },
    )
    heldout_rows = []
    for model, result in models.items():
        heldout_rows.extend({"model": model, **row} for row in result["predictions"])
    write_csv(
        staging / "heldout_predictions.csv",
        [
            "model",
            "patient_id",
            "observed_group",
            "predicted_group",
            "correct",
            "nearest_patient_id",
            "nearest_patient_group",
            "retrieval_group_correct",
            "centroid_distance_msi",
            "centroid_distance_mss",
        ],
        heldout_rows,
    )
    write_csv(
        staging / "specimen_fingerprints.csv",
        ["cohort", "lane", "patient_id", "specimen_id", "feature", "value"],
        specimen_rows,
    )
    patient_rows = []
    for lane_name, block in (("graph", graph_patient["baseline"]), ("topology", topology_patient["baseline"])):
        for patient, features in sorted(block.items()):
            patient_rows.extend(
                {
                    "cohort": "CPTAC_COAD_CellViT",
                    "lane": lane_name,
                    "patient_id": patient,
                    "group": labels[patient],
                    "feature": feature,
                    "value": value,
                }
                for feature, value in sorted(features.items())
            )
    write_csv(
        staging / "patient_fingerprints.csv",
        ["cohort", "lane", "patient_id", "group", "feature", "value"],
        patient_rows,
    )
    similarity = {}
    for index, (name, blocks) in enumerate(model_blocks.items()):
        vectors = global_vectors(blocks, lane)
        matrix, effect = similarity_summary(vectors, labels, SEED + 300 + index, lane)
        similarity[name] = effect
        write_csv(
            staging / "similarity" / f"{name}.csv",
            ["patient_id", *sorted(vectors)],
            matrix,
        )
        cohort_rows = []
        for patient in sorted(vectors):
            cohort_rows.extend(
                {
                    "patient_id": patient,
                    "group": labels[patient],
                    "feature": f"component_{component}",
                    "value": value,
                }
                for component, value in enumerate(vectors[patient])
            )
        write_csv(
            staging / "cohort_inputs" / f"{name}.csv",
            ["patient_id", "group", "feature", "value"],
            cohort_rows,
        )
    write_retrieval_inputs(staging, model_blocks, labels, provenance_sha256, lane)
    summary = {
        "schema_name": "marklab_crc_graph_topology_patient_summary",
        "schema_version": "1.0",
        "scientific_objective": "test whether graph and witness-topology summaries add stable patient-level CRC molecular-class information beyond composition, technical covariates, and nonspatial CellViT embeddings",
        "interpretation_policy": "null_confounded_and_unstable_results_retained_without_threshold_or_subset_optimization",
        "cohort": "CPTAC_COAD_CellViT",
        "patient_count": len(patients),
        "pattern_count": len(manifest),
        "population_unit": "patient",
        "models": models,
        "incremental_information": {"graph": graph_increment, "topology": topology_increment},
        "fusion": {
            "graph": graph_gate,
            "topology": topology_gate,
            "new_blocks_included": included,
            "final_model": final_name,
        },
        "similarity": similarity,
        "leakage_checks": {
            "patient_held_out": True,
            "preprocessing_inside_each_training_fold": True,
            "cohort": "single_CPTAC_COAD_cohort_no_cross_cohort_mixing",
            "site_held_out": "unavailable_exact_blocker_no_acquisition_site_field_in_admitted_CPTAC_manifest",
        },
        "claim_limitations": [
            "eight-patient resource-feasible exploratory subset",
            "two slides per patient are nested diagnostics and never independent population replicates",
            "hard CellViT classes and embeddings are model outputs, not ground-truth cell phenotypes",
            "no clinical, causal, prospective, equivalence, or transportability claim",
        ],
        "source_result_sha256": provenance_sha256,
    }
    write_json(staging / "summary.json", summary)
    files = sorted(path for path in staging.rglob("*") if path.is_file())
    write_json(
        staging / "manifest.json",
        {
            "schema_name": "marklab_crc_graph_topology_patient_bundle",
            "schema_version": "1.0",
            "population_unit": "patient",
            "source_result_sha256": provenance_sha256,
            "artifact_sha256": {
                path.relative_to(staging).as_posix(): sha256(path) for path in files
            },
            "result_format_compatibility": "0.3_preserved",
        },
    )
    os.rename(staging, output)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--prepared", required=True, type=Path)
    parser.add_argument("--marks", required=True, type=Path)
    parser.add_argument("--nonspatial", required=True, type=Path)
    parser.add_argument("--clinical", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    return parser.parse_args()


if __name__ == "__main__":
    build(parse_args())
