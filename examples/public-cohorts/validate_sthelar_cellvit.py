#!/usr/bin/env python3
"""Validate a bounded real CellViT embedding subset from STHELAR.

The BioStudies slide archive is 1.67 GB, so this script uses HTTP byte ranges
and preserves only the reviewed Zarr metadata and chunks needed for the first
128 published CellViT rows. Derived files are written outside the checkout.
"""

from __future__ import annotations

import argparse
import csv
import io
import json
import math
from pathlib import Path
import urllib.request
import zipfile

import numpy as np
from numcodecs import Blosc, VLenUTF8
from numcodecs.blosc import _cbuffer_sizes


REPOSITORY = Path(__file__).resolve().parents[2]
STUDY_ACCESSION = "S-BIAD2146"
STUDY_DOI = "10.6019/S-BIAD2146"
PAPER_DOI = "10.1038/s41597-026-06937-6"
STUDY_API = f"https://www.ebi.ac.uk/biostudies/api/v1/studies/{STUDY_ACCESSION}"
ARCHIVE_URL = (
    "https://www.ebi.ac.uk/biostudies/files/S-BIAD2146/STHELAR/"
    "sdata_slides/sdata_heart_s0.zarr.zip"
)
ARCHIVE_BYTES = 1_670_520_884
MAXIMUM_STUDY_METADATA_BYTES = 256 * 1024
MAXIMUM_RANGE_BYTES = 48 * 1024 * 1024
RANGE_BLOCK_BYTES = 2 * 1024 * 1024
SUBSET_ROWS = 128
SOURCE_ROWS = 26_363
SOURCE_DIMENSION = 1_280
SOURCE_ROW_CHUNK = 1_648
SOURCE_COLUMN_CHUNK = 160
TABLE_ROWS = 26_366
MAXIMUM_SOURCE_CELL_ID_BYTES = 64
PREFIX = "sdata_heart_s0.zarr/"

MEMBER_SIZES = {
    "tables/features_cellvit/X/.zarray": 377,
    "tables/features_cellvit/X/.zattrs": 65,
    "tables/features_cellvit/obs/cell_id/.zarray": 401,
    "tables/features_cellvit/obs/cell_id/.zattrs": 72,
    "tables/features_cellvit/obs/cell_id/0": 124_320,
    "tables/table_cells/obs/cell_id/.zarray": 401,
    "tables/table_cells/obs/cell_id/.zattrs": 72,
    "tables/table_cells/obs/cell_id/0": 109_481,
    "tables/table_cells/obsm/spatial/.zarray": 373,
    "tables/table_cells/obsm/spatial/.zattrs": 65,
    "tables/table_cells/obsm/spatial/0/0": 96_869,
    "tables/table_cells/obsm/spatial/1/0": 95_329,
    **{
        f"tables/features_cellvit/X/0/{column}": size
        for column, size in enumerate(
            [
                1_001_114,
                999_753,
                1_002_557,
                1_000_647,
                1_003_452,
                1_003_073,
                1_001_635,
                1_001_503,
            ]
        )
    },
}

EXPECTED_ARRAYS = {
    "tables/features_cellvit/X/.zarray": {
        "shape": [SOURCE_ROWS, SOURCE_DIMENSION],
        "chunks": [SOURCE_ROW_CHUNK, SOURCE_COLUMN_CHUNK],
        "dtype": "<f4",
    },
    "tables/features_cellvit/obs/cell_id/.zarray": {
        "shape": [SOURCE_ROWS],
        "chunks": [SOURCE_ROWS],
        "dtype": "|O",
    },
    "tables/table_cells/obs/cell_id/.zarray": {
        "shape": [TABLE_ROWS],
        "chunks": [TABLE_ROWS],
        "dtype": "|O",
    },
    "tables/table_cells/obsm/spatial/.zarray": {
        "shape": [TABLE_ROWS, 2],
        "chunks": [13_183, 2],
        "dtype": "<f8",
    },
}


def new_external_directory(path: Path, label: str) -> Path:
    path = path.resolve()
    if path == REPOSITORY or REPOSITORY in path.parents:
        raise ValueError(f"{label} must remain outside the checkout")
    if path.exists():
        raise ValueError(f"{label} must be a new directory")
    path.mkdir(parents=True)
    return path


def bounded_url_read(url: str, maximum: int) -> bytes:
    request = urllib.request.Request(
        url, headers={"User-Agent": "Marklab-format-validation"}
    )
    with urllib.request.urlopen(request, timeout=60) as response:
        encoded = response.read(maximum + 1)
    if not encoded or len(encoded) > maximum:
        raise ValueError(f"remote response violates the {maximum}-byte bound")
    return encoded


def attribute_values(value, name: str) -> list[str]:
    found = []
    if isinstance(value, dict):
        if value.get("name") == name and isinstance(value.get("value"), str):
            found.append(value["value"])
        for child in value.values():
            found.extend(attribute_values(child, name))
    elif isinstance(value, list):
        for child in value:
            found.extend(attribute_values(child, name))
    return found


def validate_study_metadata(encoded: bytes) -> None:
    try:
        study = json.loads(encoded)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot decode BioStudies metadata: {error}") from error
    if (
        study.get("accno") != STUDY_ACCESSION
        or STUDY_DOI not in attribute_values(study, "DOI")
        or "CC BY 4.0" not in attribute_values(study, "License")
    ):
        raise ValueError("BioStudies record differs from the reviewed STHELAR source")


def read_study_metadata() -> bytes:
    encoded = bounded_url_read(STUDY_API, MAXIMUM_STUDY_METADATA_BYTES)
    validate_study_metadata(encoded)
    return encoded


class HttpRangeReader(io.RawIOBase):
    """Small seekable reader for zipfile backed by bounded aligned ranges."""

    def __init__(self, url: str):
        self._url = url
        self._position = 0
        self._blocks: dict[int, bytes] = {}
        self.transferred_bytes = 0
        request = urllib.request.Request(
            url,
            method="HEAD",
            headers={"User-Agent": "Marklab-format-validation"},
        )
        with urllib.request.urlopen(request, timeout=60) as response:
            self.length = int(response.headers["Content-Length"])
            if response.headers.get("Accept-Ranges") != "bytes":
                raise ValueError("BioStudies archive server no longer advertises byte ranges")
        if self.length != ARCHIVE_BYTES:
            raise ValueError("STHELAR archive differs from its reviewed byte length")

    def readable(self) -> bool:
        return True

    def seekable(self) -> bool:
        return True

    def tell(self) -> int:
        return self._position

    def seek(self, offset: int, whence: int = io.SEEK_SET) -> int:
        if whence == io.SEEK_SET:
            position = offset
        elif whence == io.SEEK_CUR:
            position = self._position + offset
        elif whence == io.SEEK_END:
            position = self.length + offset
        else:
            raise ValueError("unsupported seek mode")
        if position < 0:
            raise ValueError("cannot seek before the archive")
        self._position = position
        return position

    def _read_block(self, index: int) -> bytes:
        cached = self._blocks.get(index)
        if cached is not None:
            return cached
        start = index * RANGE_BLOCK_BYTES
        end = min(start + RANGE_BLOCK_BYTES, self.length) - 1
        request = urllib.request.Request(
            self._url,
            headers={
                "Range": f"bytes={start}-{end}",
                "User-Agent": "Marklab-format-validation",
            },
        )
        with urllib.request.urlopen(request, timeout=120) as response:
            if response.status != 206:
                raise ValueError("BioStudies archive server ignored a byte-range request")
            content_range = response.headers.get("Content-Range")
            if content_range != f"bytes {start}-{end}/{self.length}":
                raise ValueError("BioStudies returned an unexpected byte range")
            block = response.read(RANGE_BLOCK_BYTES + 1)
        if len(block) != end - start + 1:
            raise ValueError("BioStudies returned a truncated byte range")
        transferred = self.transferred_bytes + len(block)
        if transferred > MAXIMUM_RANGE_BYTES:
            raise ValueError("remote byte ranges exceed the admitted transfer bound")
        self.transferred_bytes = transferred
        self._blocks[index] = block
        return block

    def read(self, size: int = -1) -> bytes:
        if self._position >= self.length:
            return b""
        if size is None or size < 0:
            size = self.length - self._position
        size = min(size, self.length - self._position)
        parts = []
        remaining = size
        while remaining:
            index, offset = divmod(self._position, RANGE_BLOCK_BYTES)
            block = self._read_block(index)
            count = min(remaining, len(block) - offset)
            parts.append(block[offset : offset + count])
            self._position += count
            remaining -= count
        return b"".join(parts)


def validate_array_metadata(member: str, encoded: bytes) -> None:
    try:
        metadata = json.loads(encoded)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot decode {member}: {error}") from error
    expected = EXPECTED_ARRAYS[member]
    if any(metadata.get(name) != value for name, value in expected.items()):
        raise ValueError(f"{member} differs from its reviewed array layout")
    compressor = metadata.get("compressor")
    if (
        compressor != {
            "blocksize": 0,
            "clevel": 5,
            "cname": "lz4",
            "id": "blosc",
            "shuffle": 1,
        }
        or metadata.get("dimension_separator") != "/"
        or metadata.get("order") != "C"
        or metadata.get("zarr_format") != 2
    ):
        raise ValueError(f"{member} differs from its reviewed Zarr codec")


def selected_path(directory: Path, member: str) -> Path:
    return directory / "selected-zarr" / member


def download_sources(directory: Path) -> tuple[Path, int]:
    directory = new_external_directory(directory, "raw-data directory")
    study = read_study_metadata()
    (directory / "biostudies-study.json").write_bytes(study)
    remote = HttpRangeReader(ARCHIVE_URL)
    try:
        with zipfile.ZipFile(remote) as archive:
            for member, expected_size in MEMBER_SIZES.items():
                qualified = PREFIX + member
                info = archive.getinfo(qualified)
                if info.is_dir() or info.file_size != expected_size:
                    raise ValueError(f"unexpected STHELAR Zarr member: {qualified}")
                encoded = archive.read(info)
                if len(encoded) != expected_size:
                    raise ValueError(f"truncated STHELAR Zarr member: {qualified}")
                path = selected_path(directory, member)
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(encoded)
    except (OSError, KeyError, zipfile.BadZipFile) as error:
        raise ValueError(f"cannot read selected STHELAR Zarr members: {error}") from error
    source = {
        "study_accession": STUDY_ACCESSION,
        "dataset_doi": STUDY_DOI,
        "paper_doi": PAPER_DOI,
        "license": "CC-BY-4.0",
        "archive_url": ARCHIVE_URL,
        "archive_bytes": ARCHIVE_BYTES,
        "slide_id": "heart_s0",
        "selected_members": list(MEMBER_SIZES),
        "model_family": "CellViT",
        "reported_model_variant": "pretrained CellViT-SAM-H 40x",
        "encoder_architecture": "ViT-Huge",
        "embedding_dimension": SOURCE_DIMENSION,
        "exact_checkpoint_identity": "not encoded in the published SpatialData table",
        "extraction": (
            "nucleus token embeddings averaged within each patch and then across "
            "overlapping patches for each cell"
        ),
    }
    (directory / "source.json").write_text(
        json.dumps(source, indent=2) + "\n", encoding="utf-8"
    )
    return directory, remote.transferred_bytes


def reuse_sources(directory: Path) -> tuple[Path, int]:
    directory = directory.resolve()
    if directory == REPOSITORY or REPOSITORY in directory.parents:
        raise ValueError("raw-data directory must remain outside the checkout")
    if not directory.is_dir():
        raise ValueError("reused raw-data directory must already exist")
    study_path = directory / "biostudies-study.json"
    if (
        not study_path.is_file()
        or not 0 < study_path.stat().st_size <= MAXIMUM_STUDY_METADATA_BYTES
    ):
        raise ValueError("preserved BioStudies metadata violates its byte bound")
    validate_study_metadata(study_path.read_bytes())
    for member, expected_size in MEMBER_SIZES.items():
        path = selected_path(directory, member)
        if not path.is_file() or path.stat().st_size != expected_size:
            raise ValueError(f"preserved STHELAR member differs: {member}")
    try:
        source = json.loads((directory / "source.json").read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read preserved STHELAR metadata: {error}") from error
    if (
        source.get("study_accession") != STUDY_ACCESSION
        or source.get("dataset_doi") != STUDY_DOI
        or source.get("license") != "CC-BY-4.0"
        or source.get("archive_url") != ARCHIVE_URL
        or source.get("archive_bytes") != ARCHIVE_BYTES
        or source.get("selected_members") != list(MEMBER_SIZES)
    ):
        raise ValueError("preserved STHELAR metadata differs from the reviewed source")
    return directory, 0


def raw_member(directory: Path, member: str) -> bytes:
    return selected_path(directory, member).read_bytes()


def checked_blosc_size(encoded: bytes, maximum: int, label: str) -> int:
    """Inspect the fixed Blosc header before any decoded-output allocation."""
    if len(encoded) < 16:
        raise ValueError(f"{label} has a truncated Blosc header")
    try:
        decoded_bytes, compressed_bytes, block_bytes = _cbuffer_sizes(encoded)
    except (RuntimeError, TypeError, ValueError) as error:
        raise ValueError(f"cannot inspect {label} Blosc header: {error}") from error
    if (
        decoded_bytes <= 0
        or decoded_bytes > maximum
        or compressed_bytes != len(encoded)
        or block_bytes <= 0
        or block_bytes > decoded_bytes
    ):
        raise ValueError(f"{label} Blosc header violates its decoded-size bound")
    return decoded_bytes


def decode_numeric(
    encoded: bytes, dtype: str, shape: tuple[int, ...], label: str
) -> np.ndarray:
    data_type = np.dtype(dtype)
    components = math.prod(shape)
    expected_bytes = components * data_type.itemsize
    decoded_bytes = checked_blosc_size(encoded, expected_bytes, label)
    if decoded_bytes != expected_bytes:
        raise ValueError(f"{label} Blosc header differs from its exact array shape")
    output = np.empty(shape, dtype=data_type)
    try:
        decoded = Blosc().decode(encoded, out=output)
    except (RuntimeError, TypeError, ValueError) as error:
        raise ValueError(f"cannot decode bounded {label} chunk: {error}") from error
    if decoded is not output:
        raise ValueError(f"{label} decoder did not use the bounded output buffer")
    return output


def decode_strings(encoded: bytes, expected_count: int, label: str) -> np.ndarray:
    maximum_bytes = 4 + expected_count * (4 + MAXIMUM_SOURCE_CELL_ID_BYTES)
    decoded_bytes = checked_blosc_size(encoded, maximum_bytes, label)
    raw = bytearray(decoded_bytes)
    try:
        decoded_raw = Blosc().decode(encoded, out=raw)
    except (RuntimeError, TypeError, ValueError) as error:
        raise ValueError(f"cannot decode bounded {label} chunk: {error}") from error
    if decoded_raw is not raw or int.from_bytes(raw[:4], "little") != expected_count:
        raise ValueError(f"{label} chunk differs from its exact string count")
    offset = 4
    for _ in range(expected_count):
        if offset + 4 > len(raw):
            raise ValueError(f"{label} has a truncated string-length prefix")
        string_bytes = int.from_bytes(raw[offset : offset + 4], "little")
        offset += 4
        if string_bytes > MAXIMUM_SOURCE_CELL_ID_BYTES:
            raise ValueError(f"{label} contains an oversized source cell ID")
        end = offset + string_bytes
        if end > len(raw):
            raise ValueError(f"{label} has a truncated source cell ID")
        try:
            bytes(raw[offset:end]).decode("utf-8")
        except UnicodeError as error:
            raise ValueError(f"{label} contains invalid UTF-8: {error}") from error
        offset = end
    if offset != len(raw):
        raise ValueError(f"{label} has trailing variable-length string bytes")
    output = np.empty(expected_count, dtype=object)
    try:
        decoded = VLenUTF8().decode(raw, out=output)
    except (RuntimeError, TypeError, ValueError, UnicodeError) as error:
        raise ValueError(f"cannot decode bounded {label} strings: {error}") from error
    if decoded.shape != (expected_count,) or not np.shares_memory(decoded, output):
        raise ValueError(f"{label} string decoder did not use the bounded output array")
    for value in decoded:
        if (
            not isinstance(value, str)
            or not value
            or len(value.encode("utf-8")) > MAXIMUM_SOURCE_CELL_ID_BYTES
            or not value.isprintable()
        ):
            raise ValueError(f"{label} contains an invalid source cell ID")
    return decoded


def decode_sources(directory: Path) -> tuple[np.ndarray, list[str], np.ndarray, dict]:
    for member in EXPECTED_ARRAYS:
        validate_array_metadata(member, raw_member(directory, member))

    feature_ids_array = decode_strings(
        raw_member(directory, "tables/features_cellvit/obs/cell_id/0"),
        SOURCE_ROWS,
        "features_cellvit cell_id",
    )
    feature_ids = [str(value) for value in feature_ids_array]
    if len(feature_ids) != SOURCE_ROWS or len(set(feature_ids)) != SOURCE_ROWS:
        raise ValueError("CellViT source IDs are missing or duplicated")

    column_chunks = []
    for column in range(SOURCE_DIMENSION // SOURCE_COLUMN_CHUNK):
        encoded = raw_member(directory, f"tables/features_cellvit/X/0/{column}")
        chunk = decode_numeric(
            encoded,
            "<f4",
            (SOURCE_ROW_CHUNK, SOURCE_COLUMN_CHUNK),
            f"features_cellvit X/0/{column}",
        )
        column_chunks.append(chunk)
    first_row_chunk = np.concatenate(column_chunks, axis=1)
    subset = np.ascontiguousarray(first_row_chunk[:SUBSET_ROWS])
    if subset.shape != (SUBSET_ROWS, SOURCE_DIMENSION) or not np.isfinite(subset).all():
        raise ValueError("selected CellViT vectors are not finite 1280-component rows")

    table_ids_array = decode_strings(
        raw_member(directory, "tables/table_cells/obs/cell_id/0"),
        TABLE_ROWS,
        "table_cells cell_id",
    )
    table_ids = [str(value) for value in table_ids_array]
    if len(table_ids) != TABLE_ROWS or len(set(table_ids)) != TABLE_ROWS:
        raise ValueError("Xenium table IDs are missing or duplicated")
    positions = {cell_id: row for row, cell_id in enumerate(table_ids)}
    if any(cell_id not in positions for cell_id in feature_ids):
        raise ValueError("a CellViT row has no matching Xenium cell identity")

    spatial_rows = []
    for row_chunk, count in enumerate([13_183, 13_183]):
        encoded = raw_member(directory, f"tables/table_cells/obsm/spatial/{row_chunk}/0")
        values = decode_numeric(
            encoded, "<f8", (count, 2), f"table_cells spatial/{row_chunk}/0"
        )
        spatial_rows.append(values)
    spatial = np.concatenate(spatial_rows)
    if spatial.shape != (TABLE_ROWS, 2) or not np.isfinite(spatial).all():
        raise ValueError("Xenium spatial coordinates are incomplete or non-finite")
    subset_coordinates = np.asarray(
        [spatial[positions[cell_id]] for cell_id in feature_ids[:SUBSET_ROWS]],
        dtype="<f8",
    )
    missing_embedding_ids = sorted(set(table_ids) - set(feature_ids))
    if len(missing_embedding_ids) != TABLE_ROWS - SOURCE_ROWS:
        raise ValueError("unexpected CellViT-to-Xenium identity difference")
    facts = {
        "source_embedding_rows": SOURCE_ROWS,
        "source_xenium_rows": TABLE_ROWS,
        "source_cells_without_embeddings": missing_embedding_ids,
        "first_source_cell_id": feature_ids[0],
        "last_selected_source_cell_id": feature_ids[SUBSET_ROWS - 1],
    }
    return subset, feature_ids[:SUBSET_ROWS], subset_coordinates, facts


def write_cells_csv(path: Path, cell_ids: list[str], coordinates: np.ndarray) -> None:
    with path.open("x", newline="", encoding="utf-8") as stream:
        writer = csv.writer(stream, lineterminator="\n")
        writer.writerow(["cell_id", "source_embedding_row", "x_um", "y_um"])
        for row, (cell_id, point) in enumerate(zip(cell_ids, coordinates, strict=True)):
            writer.writerow([cell_id, row, repr(float(point[0])), repr(float(point[1]))])


def write_expected_json(path: Path, cell_ids: list[str], vectors: np.ndarray) -> None:
    expected = {
        "source": "selected published STHELAR features_cellvit Zarr chunks",
        "selection": "first 128 rows in published Zarr order",
        "dtype": "float32",
        "dimension": SOURCE_DIMENSION,
        "rows": [
            {
                "cell_id": cell_id,
                "values": [float(value) for value in vector],
            }
            for cell_id, vector in zip(cell_ids, vectors, strict=True)
        ],
    }
    with path.open("x", encoding="utf-8") as stream:
        json.dump(expected, stream, allow_nan=False, separators=(",", ":"))
        stream.write("\n")


def validate(data_directory: Path, output: Path, *, reuse_source: bool = False) -> None:
    if reuse_source:
        data_directory, transferred_bytes = reuse_sources(data_directory)
        source_mode = "reused_preserved_ranges"
    else:
        data_directory, transferred_bytes = download_sources(data_directory)
        source_mode = "new_bounded_range_download"
    output = new_external_directory(output, "validation output")
    vectors, cell_ids, coordinates, facts = decode_sources(data_directory)

    expected_path = output / "sthelar-heart-s0-first-128-expected.json"
    write_expected_json(expected_path, cell_ids, vectors)
    npy_path = output / "sthelar-heart-s0-first-128-cellvit.npy"
    with npy_path.open("xb") as stream:
        np.save(stream, vectors, allow_pickle=False)
    cells_path = output / "sthelar-heart-s0-first-128-cells.csv"
    write_cells_csv(cells_path, cell_ids, coordinates)

    loaded = np.load(npy_path, allow_pickle=False)
    exact_reload = loaded.dtype == vectors.dtype and np.array_equal(loaded, vectors)
    if not exact_reload:
        raise ValueError("derived NPY does not preserve the exact source float32 values")
    expected_reload = json.loads(expected_path.read_text(encoding="utf-8"))
    expected_ids = [row["cell_id"] for row in expected_reload["rows"]]
    expected_vectors = np.asarray(
        [row["values"] for row in expected_reload["rows"]], dtype="<f4"
    )
    exact_expected_reload = expected_ids == cell_ids and np.array_equal(
        expected_vectors, vectors
    )
    if not exact_expected_reload:
        raise ValueError("expected JSON does not preserve the exact source IDs and values")
    if not all(math.isfinite(float(value)) for value in coordinates.flat):
        raise ValueError("derived Xenium coordinates are not finite")

    summary = {
        "source": {
            "mode": source_mode,
            "study_accession": STUDY_ACCESSION,
            "dataset_doi": STUDY_DOI,
            "paper_doi": PAPER_DOI,
            "license": "CC-BY-4.0",
            "archive_url": ARCHIVE_URL,
            "archive_bytes": ARCHIVE_BYTES,
            "range_bytes_transferred": transferred_bytes,
            "selected_member_count": len(MEMBER_SIZES),
            **facts,
        },
        "embedding_subset": {
            "selection": "first 128 features_cellvit rows in published Zarr order",
            "rows": len(cell_ids),
            "dimension": int(vectors.shape[1]),
            "dtype": str(vectors.dtype),
            "finite": bool(np.isfinite(vectors).all()),
            "exact_source_float32_npy_reload": exact_reload,
            "exact_source_ids_and_float32_expected_json_reload": exact_expected_reload,
            "minimum": float(vectors.min()),
            "maximum": float(vectors.max()),
            "all_zero_rows": int(np.count_nonzero(~np.any(vectors != 0, axis=1))),
        },
        "cell_identity_and_coordinates": {
            "unique_ids": len(set(cell_ids)),
            "all_ids_match_xenium_table": True,
            "coordinate_source": "table_cells.obsm['spatial']",
            "coordinate_unit": "micrometer",
            "coordinate_unit_basis": (
                "STHELAR loads the source through spatialdata_io.xenium; the 10x Xenium "
                "cell-centroid contract defines x_centroid/y_centroid in micrometers. "
                "The stored Zarr axes themselves use the generic label 'unit'."
            ),
            "x_range_um": [float(coordinates[:, 0].min()), float(coordinates[:, 0].max())],
            "y_range_um": [float(coordinates[:, 1].min()), float(coordinates[:, 1].max())],
        },
        "derived_files": {
            "vectors_npy": npy_path.name,
            "cells_csv": cells_path.name,
            "independent_expected_json": expected_path.name,
        },
        "scope": {
            "legacy_cellvit_he_bundle_compatible": False,
            "reason": (
                "the legacy v1 bundle fixes different calibration and source fields; "
                "this validation supplies exact published IDs and vectors only"
            ),
            "no_inferred_fields": [
                "type_probability",
                "qc_pass",
                "nucleus_morphology",
                "image_pixel_coordinates",
                "checkpoint_content_identity",
            ],
        },
    }
    (output / "validation.json").write_text(
        json.dumps(summary, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps(summary, indent=2))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("data_directory", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument(
        "--reuse-source",
        action="store_true",
        help="reuse and revalidate a source directory created by an earlier run",
    )
    args = parser.parse_args()
    validate(args.data_directory, args.output, reuse_source=args.reuse_source)


if __name__ == "__main__":
    main()
