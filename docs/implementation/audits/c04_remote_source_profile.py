#!/usr/bin/env python3
"""Emit privacy-safe aggregate evidence for the C-04 source-bundle profile."""

from __future__ import annotations

import array
import csv
import hashlib
import json
import math
import re
import struct
import sys
from pathlib import Path


HEADER = [
    "cell_id",
    "case_id",
    "specimen_id",
    "timepoint",
    "fragment_id",
    "roi_id",
    "native_row",
    "embedding_row",
    "x_px",
    "y_px",
    "x_um",
    "y_um",
    "cell_type_id",
    "cell_type_label",
    "type_probability",
    "nucleus_area_um2",
    "nucleus_perimeter_um",
    "eccentricity",
    "solidity",
    "circularity",
    "qc_pass",
    "block_500_id",
    "split",
]
IDENTIFIERS = {
    "cell_id",
    "case_id",
    "specimen_id",
    "timepoint",
    "fragment_id",
    "roi_id",
    "cell_type_label",
}
NONNEGATIVE = {
    "x_px",
    "y_px",
    "x_um",
    "y_um",
    "nucleus_area_um2",
    "nucleus_perimeter_um",
}
UNIT_INTERVAL = {"type_probability", "eccentricity", "solidity", "circularity"}
DECIMAL = re.compile(r"^(?:0|[1-9][0-9]*)(?:\.[0-9]+)?$")
UNSIGNED = re.compile(r"^(?:0|[1-9][0-9]*)$")
TOKEN = re.compile(r"^[A-Za-z0-9._-]+$")
SHA256 = re.compile(r"^[0-9a-f]{64}$")


def fail(code: str) -> "None":
    raise SystemExit(f"c04-audit-error:{code}")


def parse_npy(path: Path) -> tuple[int, int, str, str, bool]:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        magic = source.read(6)
        digest.update(magic)
        if magic != b"\x93NUMPY":
            fail("npy-magic")
        version_bytes = source.read(2)
        digest.update(version_bytes)
        if version_bytes not in {b"\x01\x00", b"\x02\x00"}:
            fail("npy-version")
        version = "1.0" if version_bytes == b"\x01\x00" else "2.0"
        length_width = 2 if version_bytes == b"\x01\x00" else 4
        length_bytes = source.read(length_width)
        digest.update(length_bytes)
        if len(length_bytes) != length_width:
            fail("npy-header-length")
        header_length = int.from_bytes(length_bytes, "little")
        if header_length > 64 * 1024:
            fail("npy-header-bound")
        header_bytes = source.read(header_length)
        digest.update(header_bytes)
        if len(header_bytes) != header_length or not header_bytes.endswith(b"\n"):
            fail("npy-header-truncated")
        if (6 + 2 + length_width + header_length) % 16 != 0:
            fail("npy-header-alignment")
        try:
            header = header_bytes.decode("ascii")
        except UnicodeDecodeError:
            fail("npy-header-ascii")
        keys = re.findall(r"'([^']+)'\s*:", header)
        if len(keys) != 3 or set(keys) != {"descr", "fortran_order", "shape"}:
            fail("npy-header-keys")
        if not re.search(r"'descr'\s*:\s*'<f4'", header):
            fail("npy-descriptor")
        if not re.search(r"'fortran_order'\s*:\s*False", header):
            fail("npy-order")
        shape = re.search(r"'shape'\s*:\s*\(\s*([0-9]+)\s*,\s*([0-9]+)\s*\)", header)
        if shape is None:
            fail("npy-shape")
        rows, dimension = (int(shape.group(1)), int(shape.group(2)))
        expected_bytes = rows * dimension * 4
        observed_bytes = 0
        all_finite = True
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
            observed_bytes += len(chunk)
            if len(chunk) % 4 != 0:
                fail("npy-payload-alignment")
            values = array.array("f")
            values.frombytes(chunk)
            if sys.byteorder != "little":
                values.byteswap()
            all_finite = all_finite and all(math.isfinite(value) for value in values)
        if observed_bytes != expected_bytes:
            fail("npy-payload-length")
    return rows, dimension, version, digest.hexdigest(), all_finite


def parse_csv(path: Path) -> tuple[int, str, bool, bool]:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    seen_ids: set[str] = set()
    row_count = 0
    all_rows_exact = True
    all_qc_true = True
    with path.open(newline="", encoding="utf-8") as source:
        reader = csv.DictReader(source)
        if reader.fieldnames != HEADER:
            fail("csv-header")
        for index, row in enumerate(reader):
            row_count += 1
            if any(value is None or value == "" for value in row.values()):
                fail("csv-empty")
            for field in IDENTIFIERS:
                value = row[field]
                if len(value.encode("utf-8")) > 128 or any(ord(char) < 0x20 for char in value):
                    fail("csv-identifier")
            source_id = row["cell_id"]
            if source_id in seen_ids:
                fail("csv-duplicate-source-id")
            seen_ids.add(source_id)
            for field in ("native_row", "embedding_row", "cell_type_id"):
                if UNSIGNED.fullmatch(row[field]) is None:
                    fail("csv-unsigned")
            all_rows_exact = all_rows_exact and int(row["native_row"]) == index
            all_rows_exact = all_rows_exact and int(row["embedding_row"]) == index
            for field in NONNEGATIVE | UNIT_INTERVAL:
                value = row[field]
                if len(value) > 32 or DECIMAL.fullmatch(value) is None:
                    fail("csv-decimal")
                number = float(value)
                if not math.isfinite(number) or number < 0:
                    fail("csv-nonnegative")
                if field in UNIT_INTERVAL and number > 1:
                    fail("csv-unit-interval")
            if len(row["block_500_id"].encode("utf-8")) > 32:
                fail("csv-block-id")
            if len(row["split"]) > 32 or TOKEN.fullmatch(row["split"]) is None:
                fail("csv-split")
            all_qc_true = all_qc_true and row["qc_pass"] == "True"
    return row_count, digest.hexdigest(), all_rows_exact, all_qc_true


def parse_manifest(path: Path, rows: int) -> str:
    raw = path.read_bytes()
    try:
        manifest = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError):
        fail("manifest-json")
    expected = {
        "schema_name": "cellvit_he_bundle",
        "schema_version": "1.0",
        "embedding_dtype": "float32",
        "embedding_width": 1280,
        "deduplicated_cells": 0,
        "row_alignment": "cells.csv embedding_row equals embeddings.npy row",
        "block_um": 500.0,
        "coordinate_unit": "micrometers",
        "n_cells": rows,
    }
    if any(manifest.get(key) != value for key, value in expected.items()):
        fail("manifest-profile")
    sources = manifest.get("sources")
    if not isinstance(sources, list) or len(sources) != 1:
        fail("manifest-sources")
    source = sources[0]
    if source.get("mpp") != 0.37744 or source.get("graph_position_scale") != 0.662356930902925:
        fail("manifest-scale")
    for field in ("cells_json_sha256", "cells_pt_sha256"):
        if not isinstance(source.get(field), str) or SHA256.fullmatch(source[field]) is None:
            fail("manifest-source-digest")
    return hashlib.sha256(raw).hexdigest()


def main() -> None:
    if len(sys.argv) != 2:
        fail("usage")
    root = Path(sys.argv[1])
    manifests = sorted(root.rglob("bundle_manifest.json"))
    if not manifests:
        fail("no-bundles")
    entries: list[tuple[str, str, str, int, int]] = []
    total_rows = 0
    row_min: int | None = None
    row_max = 0
    versions: dict[str, int] = {}
    all_finite = True
    all_rows_exact = True
    all_qc_true = True
    for manifest_path in manifests:
        directory = manifest_path.parent
        npy_path = directory / "embeddings.npy"
        csv_path = directory / "cells.csv"
        if not npy_path.is_file() or not csv_path.is_file():
            fail("bundle-files")
        rows, dimension, npy_version, npy_digest, finite = parse_npy(npy_path)
        csv_rows, csv_digest, exact_rows, qc_true = parse_csv(csv_path)
        if rows != csv_rows:
            fail("bundle-row-count")
        manifest_digest = parse_manifest(manifest_path, rows)
        total_rows += rows
        row_min = rows if row_min is None else min(row_min, rows)
        row_max = max(row_max, rows)
        versions[npy_version] = versions.get(npy_version, 0) + 1
        all_finite = all_finite and finite
        all_rows_exact = all_rows_exact and exact_rows
        all_qc_true = all_qc_true and qc_true
        entries.append((npy_digest, csv_digest, manifest_digest, rows, dimension))
    aggregate = hashlib.sha256()
    for npy_digest, csv_digest, manifest_digest, rows, dimension in sorted(entries):
        aggregate.update(bytes.fromhex(npy_digest))
        aggregate.update(bytes.fromhex(csv_digest))
        aggregate.update(bytes.fromhex(manifest_digest))
        aggregate.update(struct.pack(">QQ", rows, dimension))
    report = {
        "format": "marklab.c04_remote_source_profile_audit",
        "version": 1,
        "bundles": len(entries),
        "rows": total_rows,
        "minimum_bundle_rows": row_min,
        "maximum_bundle_rows": row_max,
        "dimension": 1280 if all(entry[4] == 1280 for entry in entries) else None,
        "npy_versions": versions,
        "all_finite": all_finite,
        "source_rows_exact": all_rows_exact,
        "all_qc_pass_true": all_qc_true,
        "aggregate_digest": aggregate.hexdigest(),
    }
    print(json.dumps(report, sort_keys=True, separators=(",", ":")))


if __name__ == "__main__":
    main()
