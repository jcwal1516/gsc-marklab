#!/usr/bin/env python3
"""Durably replay the existing patient-unit raw CellViT M4 variograms."""

from __future__ import annotations

import argparse
import concurrent.futures
import csv
import hashlib
import json
import math
import os
from pathlib import Path
import struct
import subprocess
from typing import Any


MAXIMUM_PROCESSES = 6
MAXIMUM_TIMEOUT_SECONDS = 300
MEMORY_BUDGET_MIB = 16


class M4DurableError(ValueError):
    """The frozen patient M4 durable contract is invalid or incomplete."""


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def read_csv(path: Path) -> list[dict[str, str]]:
    try:
        with path.open(newline="", encoding="utf-8") as source:
            rows = list(csv.DictReader(source))
    except OSError as error:
        raise M4DurableError(f"cannot read CSV {path}: {error}") from error
    if not rows:
        raise M4DurableError(f"CSV is empty: {path}")
    return rows


def read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise M4DurableError(f"cannot read JSON {path}: {error}") from error
    if not isinstance(value, dict):
        raise M4DurableError(f"JSON root must be an object: {path}")
    return value


def write_json(path: Path, value: Any) -> None:
    path.write_text(
        json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False)
        + "\n",
        encoding="utf-8",
    )


def results_compatible(current: Any, reference: Any) -> bool:
    """Require identical structure with at most one ULP of finite float drift."""
    if isinstance(current, bool) or isinstance(reference, bool):
        return current is reference
    if isinstance(current, float) and isinstance(reference, float):
        if not math.isfinite(current) or not math.isfinite(reference):
            return False
        left = struct.unpack(">Q", struct.pack(">d", current))[0]
        right = struct.unpack(">Q", struct.pack(">d", reference))[0]
        return abs(left - right) <= 1
    if isinstance(current, dict) and isinstance(reference, dict):
        return set(current) == set(reference) and all(
            results_compatible(current[key], reference[key]) for key in current
        )
    if isinstance(current, list) and isinstance(reference, list):
        return len(current) == len(reference) and all(
            results_compatible(left, right) for left, right in zip(current, reference)
        )
    return type(current) is type(reference) and current == reference


def execute(
    input_root: Path,
    reference_results: Path,
    marklab: Path,
    output: Path,
    maximum_processes: int,
    timeout_seconds: int,
    *,
    replay: bool,
    resume_completed: bool = False,
) -> dict[str, Any]:
    """Run each patient as a durable project, or prove exact disabled replay."""
    input_root = input_root.resolve()
    reference_results = reference_results.resolve()
    marklab = marklab.resolve()
    output = output.resolve()
    if not marklab.is_file() or not os.access(marklab, os.X_OK):
        raise M4DurableError(f"marklab executable is unavailable: {marklab}")
    if (
        not 1 <= maximum_processes <= MAXIMUM_PROCESSES
        or not 1 <= timeout_seconds <= MAXIMUM_TIMEOUT_SECONDS
    ):
        raise M4DurableError("process or timeout bound is invalid")
    if replay:
        if not output.is_dir():
            raise M4DurableError("replay requires a completed execution directory")
    elif output.exists() or output.is_symlink():
        if not resume_completed or not output.is_dir() or (output / "execution_manifest.json").exists():
            raise M4DurableError(f"output already exists: {output}")
    else:
        output.mkdir(parents=True)
    admission = read_json(input_root / "admission.json")
    rows = read_csv(input_root / "patients.csv")
    patient_count = int(admission.get("patient_count", 0))
    dimension = int(admission.get("raw_embedding_width", 0))
    if patient_count != len(rows) or not 1 <= patient_count <= 256 or not 2 <= dimension <= 4096:
        raise M4DurableError("M4 patient count or embedding dimension differs")
    bins = (input_root / "bins.csv").resolve()
    try:
        bins.relative_to(input_root)
    except ValueError as error:
        raise M4DurableError("M4 bins escape the input root") from error
    bins_digest = sha256(bins)
    jobs = []
    seen = set()
    for row in rows:
        patient = row.get("patient_id", "")
        if not patient or Path(patient).name != patient or patient in seen:
            raise M4DurableError("M4 patient identity is invalid or duplicated")
        seen.add(patient)
        source = (input_root / row.get("input", "")).resolve()
        try:
            source.relative_to(input_root)
        except ValueError as error:
            raise M4DurableError("M4 patient input escapes its root") from error
        if sha256(source) != row.get("input_sha256"):
            raise M4DurableError(f"M4 patient input digest differs: {patient}")
        cell_count = int(row.get("cell_count", 0))
        pair_visits = cell_count * (cell_count - 1) // 2
        if cell_count < 2 or pair_visits <= 0:
            raise M4DurableError(f"M4 patient has too few cells: {patient}")
        reference = reference_results / f"{patient}.json"
        if not reference.is_file():
            raise M4DurableError(f"M4 reference result is absent: {patient}")
        result_root = "replay" if replay else "results"
        jobs.append(
            (
                patient,
                source,
                reference,
                output / "projects" / patient,
                output / result_root / f"{patient}.json",
                output / "results" / f"{patient}.json",
                cell_count,
                pair_visits,
            )
        )

    def run(job):
        patient, source, reference, project, result, baseline, cells, pairs = job
        result.parent.mkdir(parents=True, exist_ok=True)
        resumed = False
        environment = os.environ.copy()
        if replay:
            environment["MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION"] = "1"
        command = [
            str(marklab),
            "project",
            "vector-semivariogram",
            "--project",
            str(project),
            "--input",
            str(source),
            "--bins",
            str(bins),
            "--out",
            str(result),
            "--maximum-points",
            str(cells),
            "--maximum-dimension",
            str(dimension),
            "--maximum-pair-visits",
            str(pairs),
            "--memory-budget-mib",
            str(MEMORY_BUDGET_MIB),
        ]
        expected = "hit" if replay else "miss"
        if resume_completed and not replay and result.is_file():
            resumed = True
        else:
            try:
                completed = subprocess.run(
                    command,
                    capture_output=True,
                    text=True,
                    check=False,
                    timeout=timeout_seconds,
                    env=environment,
                )
            except subprocess.TimeoutExpired as error:
                raise M4DurableError(
                    f"M4 patient {patient} exceeded {timeout_seconds} seconds"
                ) from error
            if completed.returncode != 0 or f"cache_status={expected}" not in completed.stderr:
                raise M4DurableError(
                    f"M4 patient {patient} failed or did not report {expected}: "
                    f"{completed.stderr.strip()}"
                )
        if not results_compatible(read_json(result), read_json(reference)):
            raise M4DurableError(f"M4 patient {patient} differs from the frozen result")
        replay_equal = not replay or result.read_bytes() == baseline.read_bytes()
        if not replay_equal:
            raise M4DurableError(f"M4 patient {patient} replay bytes differ")
        ledger = project / "executions.jsonl"
        ledger_count = (
            sum(bool(line.strip()) for line in ledger.read_text(encoding="utf-8").splitlines())
            if ledger.is_file()
            else 0
        )
        if ledger_count != 1:
            raise M4DurableError(f"M4 patient {patient} ledger differs")
        return {
            "patient_id": patient,
            "cache_status": expected,
            "result_sha256": sha256(result),
            "replay_bytes_equal": replay_equal,
            "reference_compatible": True,
            "resumed_completed": resumed,
            "ledger_execution_count": ledger_count,
        }

    records = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=maximum_processes) as executor:
        futures = [executor.submit(run, job) for job in jobs]
        for future in concurrent.futures.as_completed(futures):
            records.append(future.result())
    records.sort(key=lambda row: row["patient_id"])
    result = {
        "schema_name": "marklab_tcga_crc_m4_durable_execution",
        "schema_version": "1.0",
        "population_unit": "patient",
        "replay": replay,
        "patient_count": len(records),
        "maximum_processes": maximum_processes,
        "timeout_seconds_per_process": timeout_seconds,
        "bins_sha256": bins_digest,
        "binary_sha256": sha256(marklab),
        "cache_status_counts": {
            status: sum(row["cache_status"] == status for row in records)
            for status in sorted({row["cache_status"] for row in records})
        },
        "all_replay_bytes_equal": all(row["replay_bytes_equal"] for row in records),
        "all_reference_compatible": all(row["reference_compatible"] for row in records),
        "resumed_completed_count": sum(row["resumed_completed"] for row in records),
        "all_ledgers_one_execution": all(
            row["ledger_execution_count"] == 1 for row in records
        ),
        "records": records,
    }
    write_json(output / ("replay_manifest.json" if replay else "execution_manifest.json"), result)
    return result


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input-root", required=True, type=Path)
    parser.add_argument("--reference-results", required=True, type=Path)
    parser.add_argument("--marklab", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--maximum-processes", type=int, default=MAXIMUM_PROCESSES)
    parser.add_argument("--timeout-seconds", type=int, default=120)
    parser.add_argument("--replay", action="store_true")
    parser.add_argument("--resume-completed", action="store_true")
    return parser.parse_args()


if __name__ == "__main__":
    arguments = parse_args()
    execute(
        arguments.input_root,
        arguments.reference_results,
        arguments.marklab,
        arguments.out,
        arguments.maximum_processes,
        arguments.timeout_seconds,
        replay=arguments.replay,
        resume_completed=arguments.resume_completed,
    )
