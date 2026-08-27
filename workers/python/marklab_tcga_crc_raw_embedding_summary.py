#!/usr/bin/env python3
"""Build projection-free patient summaries from raw TCGA CRC CellViT tensors."""

from __future__ import annotations

import argparse
import csv
import hashlib
import importlib
import json
import math
import os
import shutil
import sys
from pathlib import Path

import numpy as np


PROBABILITIES = tuple(index / 10.0 for index in range(11))
M0_FEATURES = [
    "embedding_stage_ordinal",
    "embedding_log1p_all_cell_count",
    "embedding_tumor_fraction",
    "embedding_inflammatory_fraction",
    "embedding_connective_fraction",
    "embedding_cell_density_per_mm2",
]


def summarize_raw_embeddings(
    values: np.ndarray, *, probabilities: tuple[float, ...] = PROBABILITIES
) -> tuple[list[str], list[float]]:
    matrix = np.asarray(values, dtype=np.float64)
    if (
        matrix.ndim != 2
        or matrix.shape[0] < 2
        or matrix.shape[1] < 2
        or not np.isfinite(matrix).all()
        or not probabilities
        or any(not 0.0 <= probability <= 1.0 for probability in probabilities)
    ):
        raise ValueError("raw embedding summary requires a finite nontrivial matrix and quantiles")
    summaries = [
        ("channel_mean", matrix.mean(axis=0)),
        ("channel_population_sd", matrix.std(axis=0, ddof=0)),
        ("cell_root_mean_square", np.sqrt(np.mean(matrix * matrix, axis=1))),
        ("cell_mean", matrix.mean(axis=1)),
        ("cell_population_sd", matrix.std(axis=1, ddof=0)),
    ]
    names = []
    result = []
    for label, vector in summaries:
        quantiles = np.quantile(vector, probabilities)
        for probability, value in zip(probabilities, quantiles):
            names.append(f"embedding_raw_{label}_q{int(round(probability * 100)):03d}")
            result.append(float(value))
    if any(not math.isfinite(value) for value in result):
        raise ValueError("raw embedding summary produced a non-finite value")
    return names, result


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_csv(path: Path) -> tuple[list[str], list[dict[str, str]]]:
    with path.open(newline="", encoding="utf-8") as source:
        reader = csv.DictReader(source)
        if reader.fieldnames is None:
            raise ValueError(f"CSV has no header: {path}")
        return list(reader.fieldnames), list(reader)


def finite(value: object, label: str) -> float:
    parsed = float(value)
    if not math.isfinite(parsed):
        raise ValueError(f"{label} must be finite")
    return parsed


def load_raw_tensor(path: Path, source_root: Path) -> np.ndarray:
    import torch

    source_text = str(source_root.resolve())
    sys.path.insert(0, source_text)
    sys.dont_write_bytecode = True
    try:
        importlib.invalidate_caches()
        module = importlib.import_module("cellvit")
        origin = Path(module.__file__).resolve()
        if not origin.is_relative_to(source_root.resolve()):
            raise ValueError("CellViT was not imported from the admitted source root")
        graph = torch.load(path, map_location="cpu", weights_only=False)
    finally:
        if source_text in sys.path:
            sys.path.remove(source_text)
    tensor = graph.get("x") if isinstance(graph, dict) else getattr(graph, "x", None)
    if tensor is None:
        raise ValueError(f"CellViT graph lacks raw features: {path}")
    values = tensor.detach().cpu().numpy()
    if values.ndim != 2 or values.shape[1] != 1280 or not np.isfinite(values).all():
        raise ValueError(f"CellViT graph has an invalid raw feature matrix: {path}")
    return values


def write_fold(
    path: Path,
    rows: list[dict],
    feature_names: list[str],
    *,
    query: bool,
    domain: str,
    provenance: str,
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fields = ["region_id", "patient_id", "site_id"]
    if not query:
        fields.append("split")
    fields.extend(["domain", "provenance_sha256", *feature_names])
    with path.open("w", newline="", encoding="utf-8") as target:
        writer = csv.writer(target, lineterminator="\n")
        writer.writerow(fields)
        for row in rows:
            prefix = [row["patient_id"], row["patient_id"], row["site_id"]]
            if not query:
                prefix.append("train")
            writer.writerow([*prefix, domain, provenance, *(repr(value) for value in row["features"])])


def build(args: argparse.Namespace) -> None:
    admitted_headers, admitted_rows = read_csv(args.admitted_patients)
    _, manifest_rows = read_csv(args.phenotype_manifest)
    manifest = {row["patient_id"]: row for row in manifest_rows}
    if len(manifest) != len(manifest_rows):
        raise ValueError("phenotype manifest patient identities are duplicated")
    sources = {
        str(args.admitted_patients.resolve()): sha256(args.admitted_patients),
        str(args.phenotype_manifest.resolve()): sha256(args.phenotype_manifest),
        str(args.environment_lock.resolve()): sha256(args.environment_lock),
    }
    patient_rows = []
    raw_names = None
    for admitted in admitted_rows:
        patient = admitted["patient_id"]
        if patient not in manifest:
            raise ValueError(f"admitted patient lacks a CellViT phenotype row: {patient}")
        source = manifest[patient]
        roi = source["roi_id"]
        cell_path = (args.feature_cells_root / Path(source["cells"]).name).resolve()
        slide_root = args.inference_root / roi
        tensors = sorted(slide_root.glob("*_cells.pt"))
        inference_manifest = slide_root / "inference_manifest.json"
        if len(tensors) != 1 or not cell_path.is_file() or not inference_manifest.is_file():
            raise ValueError(f"patient {patient} lacks one complete raw CellViT source")
        _headers, cells = read_csv(cell_path)
        indices = []
        prefix = f"{roi}:"
        for row in cells:
            cell_id = row["cell_id"]
            if not cell_id.startswith(prefix) or not cell_id[len(prefix) :].isdigit():
                raise ValueError(f"invalid retained CellViT cell identity: {cell_id}")
            indices.append(int(cell_id[len(prefix) :]))
        if len(indices) < 2 or len(set(indices)) != len(indices):
            raise ValueError(f"patient {patient} has invalid retained raw row identities")
        raw = load_raw_tensor(tensors[0], args.cellvit_source_root)
        if max(indices) >= len(raw):
            raise ValueError(f"patient {patient} retained raw row is out of range")
        names, summary = summarize_raw_embeddings(raw[np.asarray(indices, dtype=np.int64)])
        if raw_names is None:
            raw_names = names
        elif raw_names != names:
            raise AssertionError("raw summary feature names changed between patients")
        m0_query = args.m0_folds_root / "patient_held_out" / patient / "query.csv"
        _m0_headers, m0_rows = read_csv(m0_query)
        if len(m0_rows) != 1 or m0_rows[0]["patient_id"] != patient:
            raise ValueError(f"patient {patient} has an invalid M0 query")
        m0 = [finite(m0_rows[0].get(name), name) for name in M0_FEATURES]
        patient_rows.append(
            {
                "patient_id": patient,
                "site_id": admitted["site_id"],
                "label": admitted["label"],
                "project_id": admitted.get("project_id", ""),
                "m0": m0,
                "m3": [*m0, *summary],
            }
        )
        for path in (cell_path, tensors[0], inference_manifest, m0_query):
            sources[str(path.resolve())] = sha256(path)
    if raw_names is None:
        raise ValueError("no admitted raw embeddings")
    lane_features = {"m0": M0_FEATURES, "m3": [*M0_FEATURES, *raw_names]}
    provenance = hashlib.sha256(
        json.dumps(
            {"sources": dict(sorted(sources.items())), "features": lane_features},
            separators=(",", ":"),
            sort_keys=True,
        ).encode()
    ).hexdigest()

    out = args.out.resolve()
    if out.exists():
        raise ValueError(f"output already exists: {out}")
    out.parent.mkdir(parents=True, exist_ok=True)
    staging = out.with_name(f".{out.name}.{os.getpid()}.tmp")
    staging.mkdir()
    try:
        with (staging / "admitted_patients.csv").open("w", newline="", encoding="utf-8") as target:
            writer = csv.DictWriter(target, fieldnames=admitted_headers, lineterminator="\n")
            writer.writeheader()
            writer.writerows(admitted_rows)
        fold_manifest = []
        for lane, features in lane_features.items():
            for policy in ("patient_held_out", "site_held_out"):
                for query in patient_rows:
                    training = [
                        row
                        for row in patient_rows
                        if row["patient_id"] != query["patient_id"]
                        and (policy != "site_held_out" or row["site_id"] != query["site_id"])
                    ]
                    query_row = {**query, "features": query[lane]}
                    training_rows = [{**row, "features": row[lane]} for row in training]
                    fold = staging / "folds" / lane / policy / query["patient_id"]
                    domain = f"tcga_crc_{lane}_{policy}"
                    write_fold(fold / "query.csv", [query_row], features, query=True, domain=domain, provenance=provenance)
                    write_fold(fold / "training.csv", training_rows, features, query=False, domain=domain, provenance=provenance)
                    fold_manifest.append(
                        {
                            "lane": lane,
                            "holdout_policy": policy,
                            "patient_id": query["patient_id"],
                            "label": query["label"],
                            "site_id": query["site_id"],
                            "project_id": query["project_id"],
                            "candidate_count": len(training),
                            "training_path": (fold / "training.csv").relative_to(staging).as_posix(),
                            "query_path": (fold / "query.csv").relative_to(staging).as_posix(),
                            "training_sha256": sha256(fold / "training.csv"),
                            "query_sha256": sha256(fold / "query.csv"),
                            "provenance_sha256": provenance,
                        }
                    )
        with (staging / "folds.csv").open("w", newline="", encoding="utf-8") as target:
            writer = csv.DictWriter(target, fieldnames=list(fold_manifest[0]), lineterminator="\n")
            writer.writeheader()
            writer.writerows(fold_manifest)
        (staging / "admission.json").write_text(
            json.dumps(
                {
                    "schema_name": "marklab_tcga_crc_m3_raw_embedding_admission",
                    "schema_version": "1.0",
                    "population_unit": "patient",
                    "patient_count": len(patient_rows),
                    "raw_embedding_width": 1280,
                    "raw_summary_feature_count": len(raw_names),
                    "m3_feature_count": len(lane_features["m3"]),
                    "raw_summary_feature_names": raw_names,
                    "projection": "none",
                    "molecular_labels_used_for_features": False,
                    "fold_count": len(fold_manifest),
                    "provenance_sha256": provenance,
                    "source_sha256": dict(sorted(sources.items())),
                    "python_executable": sys.executable,
                    "python_version": sys.version,
                },
                indent=2,
                sort_keys=True,
            )
            + "\n",
            encoding="utf-8",
        )
        os.rename(staging, out)
    except BaseException:
        shutil.rmtree(staging, ignore_errors=True)
        raise


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--admitted-patients", required=True, type=Path)
    parser.add_argument("--phenotype-manifest", required=True, type=Path)
    parser.add_argument("--feature-cells-root", required=True, type=Path)
    parser.add_argument("--inference-root", required=True, type=Path)
    parser.add_argument("--cellvit-source-root", required=True, type=Path)
    parser.add_argument("--environment-lock", required=True, type=Path)
    parser.add_argument("--m0-folds-root", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    return parser.parse_args()


if __name__ == "__main__":
    build(parse_args())
