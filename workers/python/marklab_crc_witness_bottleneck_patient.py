#!/usr/bin/env python3
"""Run exact witness bottleneck stability on the frozen CRC patient subset."""

from __future__ import annotations

import argparse
import csv
from concurrent.futures import ThreadPoolExecutor
import hashlib
import importlib.util
from itertools import combinations
import json
import math
import os
from pathlib import Path
import random
import subprocess
from typing import Any


SEED = 20260829
MAXIMUM_PROCESSES = 6
MAXIMUM_PATTERNS = 64
MAXIMUM_SUBPROCESS_SECONDS = 1_200
BOTTLENECK_THRESHOLD_UM_SQUARED = 600.0


class WorkflowError(ValueError):
    """The patient bottleneck workflow contract was violated."""


def _lane_module():
    path = Path(__file__).with_name("marklab_crc_graph_topology_final.py")
    spec = importlib.util.spec_from_file_location("marklab_crc_graph_topology_final", path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, document: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _read_manifest(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as source:
        reader = csv.DictReader(source)
        expected = ["pattern_id", "patient_id", "group", "cell_count", "request"]
        if reader.fieldnames != expected:
            raise WorkflowError(f"manifest header must be exactly {','.join(expected)}")
        rows = list(reader)
    if not rows or len(rows) > MAXIMUM_PATTERNS:
        raise WorkflowError("pattern count is outside the bounded range")
    return rows


def prepare(marks_path: Path, output: Path, seed: int) -> None:
    if output.exists() or output.is_symlink():
        raise WorkflowError(f"output already exists: {output}")
    lane = _lane_module()
    by_pattern = lane._validate_marks(lane._read_marks(marks_path))
    if not by_pattern or len(by_pattern) > MAXIMUM_PATTERNS:
        raise WorkflowError("pattern count is outside the bounded range")
    patient_patterns: dict[str, int] = {}
    manifest = []
    for pattern, rows in sorted(by_pattern.items()):
        patient = str(rows[0]["patient_id"])
        group = str(rows[0]["group"])
        patient_patterns[patient] = patient_patterns.get(patient, 0) + 1
        stability = lane._witness_stability_request(rows, seed)
        replicate_count = int(stability["perturbation_replicates"])
        maximum_simplices = int(stability["maximum_simplices"])
        dimension_count = int(stability["maximum_dimension"]) + 1
        request = {
            "stability": stability,
            "maximum_bottleneck_distance_um_squared": BOTTLENECK_THRESHOLD_UM_SQUARED,
            "maximum_bottleneck_comparisons": replicate_count * dimension_count,
            "maximum_bottleneck_interval_budget": 2
            * maximum_simplices
            * replicate_count
            * dimension_count,
            "maximum_bottleneck_backend_executions": 1,
            "bottleneck_timeout_seconds": 180,
            "maximum_total_backend_executions": replicate_count + 2,
        }
        relative = Path("requests") / pattern / "witness_bottleneck_stability.json"
        write_json(output / relative, request)
        manifest.append(
            {
                "pattern_id": pattern,
                "patient_id": patient,
                "group": group,
                "cell_count": len(rows),
                "request": relative.as_posix(),
            }
        )
    if any(count != 2 for count in patient_patterns.values()):
        raise WorkflowError("every patient must have exactly two nested slide patterns")
    output.mkdir(parents=True, exist_ok=True)
    with (output / "manifest.csv").open("w", newline="", encoding="utf-8") as target:
        writer = csv.DictWriter(target, fieldnames=list(manifest[0]), lineterminator="\n")
        writer.writeheader()
        writer.writerows(manifest)
    write_json(
        output / "design.json",
        {
            "schema_name": "marklab_crc_witness_bottleneck_patient_design",
            "schema_version": "1.0",
            "population_unit": "patient",
            "pattern_unit": "slide_nested_within_patient",
            "patient_count": len(patient_patterns),
            "pattern_count": len(manifest),
            "patterns_per_patient": 2,
            "selection_uses_molecular_label": False,
            "coordinate_jitter_um": 1.0,
            "perturbation_replicates": 4,
            "maximum_bottleneck_distance_um_squared": BOTTLENECK_THRESHOLD_UM_SQUARED,
            "maximum_processes": MAXIMUM_PROCESSES,
            "maximum_total_backend_executions": len(manifest) * 6,
            "seed": seed,
            "finite_result_policy": "finite_distances_only_with_typed_infinite_essential_mismatch",
        },
    )


def _execute_pattern(
    row: dict[str, str], prepared: Path, binary: Path, output: Path
) -> dict[str, object]:
    pattern = row["pattern_id"]
    request = prepared / row["request"]
    project = output / "projects" / pattern
    miss = output / "results" / pattern / "miss.json"
    hit = output / "results" / pattern / "hit.json"
    command = [
        str(binary),
        "project",
        "witness-persistence-bottleneck-stability",
        "--project",
        str(project),
        "--input",
        str(request),
        "--out",
        str(miss),
    ]
    try:
        completed = subprocess.run(
            command,
            capture_output=True,
            text=True,
            timeout=MAXIMUM_SUBPROCESS_SECONDS,
            check=True,
        )
    except subprocess.CalledProcessError as error:
        raise WorkflowError(
            f"pattern {pattern} durable miss failed: {error.stderr.strip()}"
        ) from error
    if "cache_status=miss" not in completed.stderr:
        raise WorkflowError(f"pattern {pattern} did not report a durable miss")
    replay = command.copy()
    replay[-1] = str(hit)
    environment = os.environ.copy()
    environment["MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION"] = "1"
    try:
        completed = subprocess.run(
            replay,
            capture_output=True,
            text=True,
            timeout=MAXIMUM_SUBPROCESS_SECONDS,
            check=True,
            env=environment,
        )
    except subprocess.CalledProcessError as error:
        raise WorkflowError(
            f"pattern {pattern} backend-disabled replay failed: {error.stderr.strip()}"
        ) from error
    if "cache_status=hit" not in completed.stderr:
        raise WorkflowError(f"pattern {pattern} did not report a durable hit")
    result = read_json(miss)
    if result.get("format") != "marklab.witness_persistence_bottleneck_stability":
        raise WorkflowError(f"pattern {pattern} result format differs")
    ledger = project / "executions.jsonl"
    ledger_count = sum(
        bool(line.strip()) for line in ledger.read_text(encoding="utf-8").splitlines()
    )
    return {
        "pattern_id": pattern,
        "patient_id": row["patient_id"],
        "group": row["group"],
        "miss": miss.relative_to(output).as_posix(),
        "hit": hit.relative_to(output).as_posix(),
        "miss_sha256": sha256(miss),
        "hit_sha256": sha256(hit),
        "result_bytes_equal": miss.read_bytes() == hit.read_bytes(),
        "ledger_execution_count": ledger_count,
        "total_backend_executions_on_miss": result["total_backend_executions"],
    }


def execute(prepared: Path, binary: Path, output: Path, maximum_processes: int) -> None:
    if output.exists() or output.is_symlink():
        raise WorkflowError(f"output already exists: {output}")
    if not binary.is_file():
        raise WorkflowError(f"marklab binary is absent: {binary}")
    if maximum_processes < 1 or maximum_processes > MAXIMUM_PROCESSES:
        raise WorkflowError(f"maximum processes must be in 1..={MAXIMUM_PROCESSES}")
    rows = _read_manifest(prepared / "manifest.csv")
    output.mkdir(parents=True)
    with ThreadPoolExecutor(max_workers=maximum_processes) as pool:
        executions = list(
            pool.map(
                lambda row: _execute_pattern(row, prepared, binary, output),
                rows,
            )
        )
    executions.sort(key=lambda row: str(row["pattern_id"]))
    if any(not row["result_bytes_equal"] for row in executions):
        raise WorkflowError("at least one backend-disabled replay differs")
    if any(row["ledger_execution_count"] != 1 for row in executions):
        raise WorkflowError("at least one durable project has more than one execution")
    total_backends = sum(int(row["total_backend_executions_on_miss"]) for row in executions)
    if total_backends > len(rows) * 6:
        raise WorkflowError("aggregate backend executions exceed the prepared bound")
    write_json(
        output / "execution_manifest.json",
        {
            "schema_name": "marklab_crc_witness_bottleneck_patient_execution",
            "schema_version": "1.0",
            "population_unit": "patient",
            "pattern_count": len(rows),
            "miss_count": len(executions),
            "backend_disabled_hit_count": len(executions),
            "all_result_bytes_equal": True,
            "all_ledgers_one_execution": True,
            "total_backend_executions_on_misses": total_backends,
            "maximum_processes": maximum_processes,
            "binary_sha256": sha256(binary),
            "executions": executions,
        },
    )


def _exact_group_comparison(patient_rows: list[dict[str, object]]) -> dict[str, object]:
    patients = sorted(str(row["patient_id"]) for row in patient_rows)
    values = {
        str(row["patient_id"]): float(row["maximum_finite_bottleneck_distance_um_squared"])
        for row in patient_rows
    }
    msi = {str(row["patient_id"]) for row in patient_rows if row["group"] == "MSI"}
    if len(msi) < 2 or len(patients) - len(msi) < 2:
        raise WorkflowError("both molecular groups require at least two patients")

    def difference(group_a: set[str]) -> float:
        group_b = set(patients) - group_a
        return sum(values[patient] for patient in group_a) / len(group_a) - sum(
            values[patient] for patient in group_b
        ) / len(group_b)

    observed = difference(msi)
    assignments = [set(selected) for selected in combinations(patients, len(msi))]
    p_value = sum(abs(difference(group)) + 1e-12 >= abs(observed) for group in assignments) / len(
        assignments
    )
    msi_values = [values[patient] for patient in sorted(msi)]
    mss_values = [values[patient] for patient in patients if patient not in msi]
    generator = random.Random(SEED + 151)
    bootstrap = []
    for _ in range(1_000):
        bootstrap.append(
            sum(generator.choice(msi_values) for _ in msi_values) / len(msi_values)
            - sum(generator.choice(mss_values) for _ in mss_values) / len(mss_values)
        )
    lane = _lane_module()
    return {
        "estimand": "mean_patient_maximum_finite_bottleneck_distance_msi_minus_mss_um_squared",
        "mean_difference_msi_minus_mss": observed,
        "whole_patient_bootstrap_interval_95": [
            lane._percentile(bootstrap, 0.025),
            lane._percentile(bootstrap, 0.975),
        ],
        "null_family": "whole_patient_population_independence",
        "alternative": "two_sided",
        "exact_assignment_count": len(assignments),
        "exact_two_sided_p_value": p_value,
    }


def summarize(prepared: Path, executed: Path, output: Path) -> None:
    if output.exists() or output.is_symlink():
        raise WorkflowError(f"output already exists: {output}")
    design = read_json(prepared / "design.json")
    execution = read_json(executed / "execution_manifest.json")
    rows = _read_manifest(prepared / "manifest.csv")
    if (
        design.get("population_unit") != "patient"
        or execution.get("population_unit") != "patient"
        or not execution.get("all_result_bytes_equal")
        or not execution.get("all_ledgers_one_execution")
        or execution.get("pattern_count") != len(rows)
    ):
        raise WorkflowError("patient design or durable execution proof differs")
    patterns_by_patient: dict[str, list[dict[str, object]]] = {}
    group_by_patient: dict[str, str] = {}
    for row in rows:
        result = read_json(executed / "results" / row["pattern_id"] / "miss.json")
        maximum = float(result["maximum_finite_bottleneck_distance_um_squared"])
        if (
            result.get("format") != "marklab.witness_persistence_bottleneck_stability"
            or not math.isfinite(maximum)
            or maximum < 0.0
            or result.get("maximum_bottleneck_distance_um_squared_allowed")
            != BOTTLENECK_THRESHOLD_UM_SQUARED
        ):
            raise WorkflowError(f"pattern {row['pattern_id']} bottleneck result differs")
        patient = row["patient_id"]
        if patient in group_by_patient and group_by_patient[patient] != row["group"]:
            raise WorkflowError(f"patient {patient} has conflicting groups")
        group_by_patient[patient] = row["group"]
        patterns_by_patient.setdefault(patient, []).append(
            {
                "pattern_id": row["pattern_id"],
                "maximum_finite_bottleneck_distance_um_squared": maximum,
                "has_infinite_essential_mismatch": bool(
                    result["has_infinite_essential_mismatch"]
                ),
                "stable_under_all_declared_thresholds": bool(
                    result["stable_under_all_declared_thresholds"]
                ),
            }
        )
    patient_rows = []
    for patient, patterns in sorted(patterns_by_patient.items()):
        if len(patterns) != 2:
            raise WorkflowError(f"patient {patient} does not have exactly two slide patterns")
        patient_rows.append(
            {
                "patient_id": patient,
                "group": group_by_patient[patient],
                "pattern_count": 2,
                "maximum_finite_bottleneck_distance_um_squared": max(
                    float(row["maximum_finite_bottleneck_distance_um_squared"])
                    for row in patterns
                ),
                "has_infinite_essential_mismatch": any(
                    bool(row["has_infinite_essential_mismatch"]) for row in patterns
                ),
                "stable_across_both_patterns": all(
                    bool(row["stable_under_all_declared_thresholds"]) for row in patterns
                ),
            }
        )
    stable_count = sum(bool(row["stable_across_both_patterns"]) for row in patient_rows)
    output.mkdir(parents=True)
    with (output / "patient_results.csv").open("w", newline="", encoding="utf-8") as target:
        writer = csv.DictWriter(target, fieldnames=list(patient_rows[0]), lineterminator="\n")
        writer.writeheader()
        writer.writerows(patient_rows)
    write_json(
        output / "summary.json",
        {
            "schema_name": "marklab_crc_witness_bottleneck_patient_summary",
            "schema_version": "1.0",
            "population_unit": "patient",
            "pattern_unit": "slide_nested_within_patient",
            "patient_count": len(patient_rows),
            "pattern_count": len(rows),
            "stable_patient_count": stable_count,
            "stable_patient_fraction": stable_count / len(patient_rows),
            "all_patients_stable": stable_count == len(patient_rows),
            "patients_with_infinite_essential_mismatch": sum(
                bool(row["has_infinite_essential_mismatch"]) for row in patient_rows
            ),
            "maximum_patient_finite_bottleneck_distance_um_squared": max(
                float(row["maximum_finite_bottleneck_distance_um_squared"])
                for row in patient_rows
            ),
            "maximum_bottleneck_distance_um_squared_allowed": BOTTLENECK_THRESHOLD_UM_SQUARED,
            "group_comparison": _exact_group_comparison(patient_rows),
            "promotion_status": (
                "stable_patient_topological_endpoint"
                if stable_count == len(patient_rows)
                else "unstable_not_promoted"
            ),
            "fusion_status": "not_added_without_prespecified_stability",
            "claim_limitation": "bounded exploratory CRC subset; slides and perturbations are nested diagnostics, never population replicates",
            "durable_replay": {
                "miss_count": execution["miss_count"],
                "backend_disabled_hit_count": execution["backend_disabled_hit_count"],
                "all_result_bytes_equal": execution["all_result_bytes_equal"],
                "all_ledgers_one_execution": execution["all_ledgers_one_execution"],
            },
        },
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    prepare_parser = commands.add_parser("prepare")
    prepare_parser.add_argument("--marks", required=True, type=Path)
    prepare_parser.add_argument("--out", required=True, type=Path)
    prepare_parser.add_argument("--seed", type=int, default=SEED)
    execute_parser = commands.add_parser("execute")
    execute_parser.add_argument("--prepared", required=True, type=Path)
    execute_parser.add_argument("--binary", required=True, type=Path)
    execute_parser.add_argument("--out", required=True, type=Path)
    execute_parser.add_argument("--maximum-processes", type=int, default=MAXIMUM_PROCESSES)
    summarize_parser = commands.add_parser("summarize")
    summarize_parser.add_argument("--prepared", required=True, type=Path)
    summarize_parser.add_argument("--executed", required=True, type=Path)
    summarize_parser.add_argument("--out", required=True, type=Path)
    return parser.parse_args()


def main() -> None:
    arguments = parse_args()
    if arguments.command == "prepare":
        prepare(arguments.marks, arguments.out, arguments.seed)
    elif arguments.command == "execute":
        execute(
            arguments.prepared,
            arguments.binary,
            arguments.out,
            arguments.maximum_processes,
        )
    elif arguments.command == "summarize":
        summarize(arguments.prepared, arguments.executed, arguments.out)
    else:  # pragma: no cover
        raise AssertionError(f"unknown command: {arguments.command}")


if __name__ == "__main__":
    main()
