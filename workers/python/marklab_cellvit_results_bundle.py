#!/usr/bin/env python3
"""Seal and verify the concrete RESULTS-CELLVIT-2DAY-01 result bundle."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import sys
from typing import Any


SCHEMA_NAME = "marklab_cellvit_results_bundle"
SCHEMA_VERSION = "1.0"
INDEX_SCHEMA_NAME = "marklab_cellvit_results_index"
OBJECTIVE = "RESULTS-CELLVIT-2DAY-01"
MANIFEST_NAME = "bundle_manifest.json"
REQUIRED_METADATA = ("admission.json", "diagnostics.json", "provenance.json")
REQUIRED_LANES = {
    "coordinate_only",
    "scalar_mark",
    "vector_embedding",
    "annotation_combination",
    "patch_multiscale",
    "patient_level",
    "pymc_bayesian",
    "raw_patch_embedding",
}


class BundleError(ValueError):
    """The result bundle violates its closed version-one contract."""


def _read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise BundleError(f"cannot read JSON {path.name}: {error}") from error


def _exact_object(value: Any, keys: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        raise BundleError(f"{label} fields differ from the version-one contract")
    return value


def _regular_relative_file(bundle: Path, value: Any, label: str) -> Path:
    if not isinstance(value, str) or not value or Path(value).is_absolute():
        raise BundleError(f"{label} must be a nonempty relative path")
    relative = Path(value)
    if any(part in ("", ".", "..") for part in relative.parts):
        raise BundleError(f"{label} contains an invalid path component")
    path = bundle / relative
    if path.is_symlink() or not path.is_file():
        raise BundleError(f"{label} is not a regular bundle file")
    return path


def _validate_index(bundle: Path) -> dict[str, Any]:
    index_path = _regular_relative_file(bundle, "bundle_index.json", "bundle index")
    index = _exact_object(
        _read_json(index_path),
        {"schema_name", "schema_version", "objective", "lanes"},
        "bundle index",
    )
    if (
        index["schema_name"] != INDEX_SCHEMA_NAME
        or index["schema_version"] != SCHEMA_VERSION
        or index["objective"] != OBJECTIVE
    ):
        raise BundleError("bundle index identity differs from the version-one contract")
    for relative in REQUIRED_METADATA:
        _regular_relative_file(bundle, relative, relative)

    lanes = index["lanes"]
    if not isinstance(lanes, list):
        raise BundleError("bundle index lanes must be an array")
    seen: set[str] = set()
    for position, raw in enumerate(lanes):
        lane = _exact_object(
            raw,
            {"lane_id", "status", "result", "blocker"},
            f"bundle index lane {position}",
        )
        lane_id = lane["lane_id"]
        if not isinstance(lane_id, str) or lane_id not in REQUIRED_LANES or lane_id in seen:
            raise BundleError("bundle index lane IDs are invalid or duplicated")
        seen.add(lane_id)
        if lane["status"] == "available":
            _regular_relative_file(bundle, lane["result"], f"result for {lane_id}")
            if lane["blocker"] is not None:
                raise BundleError(f"available lane {lane_id} must not declare a blocker")
        elif lane["status"] == "unavailable":
            if lane["result"] is not None:
                raise BundleError(f"unavailable lane {lane_id} must not declare a result")
            blocker = lane["blocker"]
            if not isinstance(blocker, str) or not blocker.strip() or blocker != blocker.strip():
                raise BundleError(f"unavailable lane {lane_id} requires an exact blocker")
        else:
            raise BundleError(f"lane {lane_id} has an invalid status")
    if seen != REQUIRED_LANES:
        raise BundleError("bundle index does not declare every required CellViT result lane")
    return index


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while block := stream.read(1024 * 1024):
            digest.update(block)
    return digest.hexdigest()


def _bundle_files(bundle: Path) -> dict[str, Path]:
    files: dict[str, Path] = {}
    for path in sorted(bundle.rglob("*")):
        if path == bundle / MANIFEST_NAME:
            continue
        if path.is_symlink():
            raise BundleError(f"bundle contains symlink {path.relative_to(bundle)}")
        if path.is_file():
            relative = path.relative_to(bundle).as_posix()
            files[relative] = path
        elif not path.is_dir():
            raise BundleError(f"bundle contains unsupported entry {path.relative_to(bundle)}")
    return files


def _canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n").encode()


def seal(bundle: Path) -> dict[str, Any]:
    _validate_index(bundle)
    files = _bundle_files(bundle)
    if not files:
        raise BundleError("bundle has no files to seal")
    file_sha256 = {relative: _sha256(path) for relative, path in files.items()}
    manifest = {
        "schema_name": SCHEMA_NAME,
        "schema_version": SCHEMA_VERSION,
        "objective": OBJECTIVE,
        "index_sha256": file_sha256["bundle_index.json"],
        "file_sha256": file_sha256,
    }
    encoded = _canonical_bytes(manifest)
    temporary = bundle / f".{MANIFEST_NAME}.part"
    if temporary.exists():
        raise BundleError("bundle manifest staging path already exists")
    temporary.write_bytes(encoded)
    os.replace(temporary, bundle / MANIFEST_NAME)
    return {"status": "sealed", "files": len(files), "manifest_sha256": hashlib.sha256(encoded).hexdigest()}


def verify(bundle: Path) -> dict[str, Any]:
    _validate_index(bundle)
    manifest_path = _regular_relative_file(bundle, MANIFEST_NAME, "bundle manifest")
    manifest = _exact_object(
        _read_json(manifest_path),
        {"schema_name", "schema_version", "objective", "index_sha256", "file_sha256"},
        "bundle manifest",
    )
    if (
        manifest["schema_name"] != SCHEMA_NAME
        or manifest["schema_version"] != SCHEMA_VERSION
        or manifest["objective"] != OBJECTIVE
    ):
        raise BundleError("bundle manifest identity differs from the version-one contract")
    expected = manifest["file_sha256"]
    if not isinstance(expected, dict) or any(
        not isinstance(path, str)
        or not isinstance(digest, str)
        or len(digest) != 64
        or any(character not in "0123456789abcdef" for character in digest)
        for path, digest in expected.items()
    ):
        raise BundleError("bundle manifest file digests are invalid")
    files = _bundle_files(bundle)
    if set(files) != set(expected):
        raise BundleError("bundle file set differs from the sealed manifest")
    for relative, path in files.items():
        if _sha256(path) != expected[relative]:
            raise BundleError(f"digest mismatch for {relative}")
    if manifest["index_sha256"] != expected.get("bundle_index.json"):
        raise BundleError("bundle index digest does not match the manifest identity")
    return {
        "status": "verified",
        "files": len(files),
        "manifest_sha256": _sha256(manifest_path),
    }


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    for command in ("seal", "verify"):
        subparser = subparsers.add_parser(command)
        subparser.add_argument("--bundle", type=Path, required=True)
    return parser


def main() -> int:
    arguments = _parser().parse_args()
    bundle = arguments.bundle.resolve()
    if not bundle.is_dir():
        raise BundleError("bundle must be an existing directory")
    result = seal(bundle) if arguments.command == "seal" else verify(bundle)
    sys.stdout.buffer.write(_canonical_bytes(result))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except BundleError as error:
        print(f"CellViT results bundle failed: {error}", file=sys.stderr)
        raise SystemExit(2) from error
