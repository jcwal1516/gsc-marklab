#!/usr/bin/env python3
"""Validate the QuPath nuclei adapter on one bounded biological PUMA ROI.

The source is PUMA v5's CC0 nuclei GeoJSON archive. The script selects one
reviewed 1024 by 1024 ROI, derives expected centroids independently with the
shoelace formula, and writes all derived output outside the checkout.
"""

from __future__ import annotations

import argparse
from collections import Counter
import json
import math
import sys
import urllib.request
import zipfile
from pathlib import Path


REPOSITORY = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPOSITORY / "clients/python"))

from upstream_formats import qupath_nuclei_slide  # noqa: E402


RECORD_ID = 15_050_523
RECORD_API = f"https://zenodo.org/api/records/{RECORD_ID}"
DOI = "10.5281/zenodo.15050523"
ARCHIVE_NAME = "01_training_dataset_geojson_nuclei.zip"
ARCHIVE_URL = f"{RECORD_API}/files/{ARCHIVE_NAME}/content"
ARCHIVE_BYTES = 15_975_301
MAXIMUM_ARCHIVE_BYTES = 20 * 1024 * 1024
MAXIMUM_METADATA_BYTES = 2 * 1024 * 1024
MEMBER = (
    "01_training_dataset_geojson_nuclei/"
    "training_set_metastatic_roi_008_nuclei.geojson"
)
MEMBER_BYTES = 424_687
MAXIMUM_MEMBER_BYTES = 1024 * 1024
ROI = "training_set_metastatic_roi_008"
ROI_SIZE_PX = 1024
SCALE_UM_PER_PIXEL = (0.23, 0.23)
ABSOLUTE_TOLERANCE_UM = 1e-9
EXPECTED_CLASS_COUNTS = {
    "nuclei_apoptosis": 22,
    "nuclei_lymphocyte": 5,
    "nuclei_tumor": 141,
}
CLASSIFICATION_CHANNEL = {
    "id": "nucleus_class",
    "label": "QuPath nucleus classification",
    "kind": "categorical",
    "levels": list(EXPECTED_CLASS_COUNTS),
    "measurement_status": "classified",
    "provenance": (
        "PUMA v5 QuPath-adjusted nuclei annotation classification.name; "
        f"{DOI}"
    ),
}


def new_external_directory(path: Path, label: str) -> Path:
    path = path.resolve()
    if path == REPOSITORY or REPOSITORY in path.parents:
        raise ValueError(f"{label} must remain outside the checkout")
    if path.exists():
        raise ValueError(f"{label} must be a new directory")
    path.mkdir(parents=True)
    return path


def bounded_download(url: str, destination: Path, maximum: int) -> int:
    request = urllib.request.Request(url, headers={"User-Agent": "Marklab-format-validation"})
    count = 0
    with urllib.request.urlopen(request, timeout=60) as response, destination.open("xb") as stream:
        while chunk := response.read(1024 * 1024):
            count += len(chunk)
            if count > maximum:
                raise ValueError(f"download exceeds the {maximum}-byte bound")
            stream.write(chunk)
    if count == 0:
        raise ValueError("download is empty")
    return count


def read_metadata() -> dict:
    request = urllib.request.Request(
        RECORD_API, headers={"User-Agent": "Marklab-format-validation"}
    )
    with urllib.request.urlopen(request, timeout=60) as response:
        encoded = response.read(MAXIMUM_METADATA_BYTES + 1)
    if not encoded or len(encoded) > MAXIMUM_METADATA_BYTES:
        raise ValueError("Zenodo metadata violates the admitted byte bound")
    try:
        metadata = json.loads(encoded)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot decode Zenodo metadata: {error}") from error
    files = metadata.get("files")
    matches = [entry for entry in files or [] if entry.get("key") == ARCHIVE_NAME]
    if (
        metadata.get("id") != RECORD_ID
        or metadata.get("doi") != DOI
        or metadata.get("metadata", {}).get("license", {}).get("id") != "cc-zero"
        or len(matches) != 1
        or matches[0].get("size") != ARCHIVE_BYTES
    ):
        raise ValueError("Zenodo record differs from the reviewed PUMA v5 source")
    return metadata


def inspect_archive(archive: Path) -> bytes:
    try:
        size = archive.stat().st_size
    except OSError as error:
        raise ValueError(f"cannot inspect preserved PUMA archive: {error}") from error
    if not archive.is_file() or size != ARCHIVE_BYTES or size > MAXIMUM_ARCHIVE_BYTES:
        raise ValueError("PUMA nuclei archive differs from the reviewed byte-size contract")
    try:
        with zipfile.ZipFile(archive) as source:
            member = source.getinfo(MEMBER)
            if member.is_dir() or member.file_size != MEMBER_BYTES:
                raise ValueError("selected PUMA GeoJSON member differs from the reviewed fixture")
            with source.open(member) as stream:
                encoded = stream.read(MAXIMUM_MEMBER_BYTES + 1)
    except (OSError, KeyError, zipfile.BadZipFile) as error:
        raise ValueError(f"cannot read selected PUMA GeoJSON member: {error}") from error
    if len(encoded) != MEMBER_BYTES:
        raise ValueError("selected PUMA GeoJSON member violates its byte-size contract")
    return encoded


def download_sources(directory: Path) -> tuple[Path, bytes]:
    directory = new_external_directory(directory, "raw-data directory")
    metadata = read_metadata()
    archive = directory / ARCHIVE_NAME
    if bounded_download(ARCHIVE_URL, archive, MAXIMUM_ARCHIVE_BYTES) != ARCHIVE_BYTES:
        raise ValueError("downloaded PUMA nuclei archive has an unexpected size")
    selected = inspect_archive(archive)
    (directory / "source.json").write_text(
        json.dumps(
            {
                "record": RECORD_API,
                "doi": metadata["doi"],
                "license": "CC0-1.0",
                "archive_url": ARCHIVE_URL,
                "archive_bytes": ARCHIVE_BYTES,
                "selected_member": MEMBER,
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    return directory, selected


def reuse_sources(directory: Path) -> tuple[Path, bytes]:
    """Validate and reuse a preserved PUMA nuclei archive without rewriting it."""
    directory = directory.resolve()
    if directory == REPOSITORY or REPOSITORY in directory.parents:
        raise ValueError("raw-data directory must remain outside the checkout")
    if not directory.is_dir():
        raise ValueError("reused raw-data directory must already exist")
    return directory, inspect_archive(directory / ARCHIVE_NAME)


def polygon_centroid(ring) -> tuple[float, float]:
    """Independent simple-polygon area centroid oracle."""
    cross_sum = 0.0
    x_sum = 0.0
    y_sum = 0.0
    for left, right in zip(ring, ring[1:]):
        cross = left[0] * right[1] - right[0] * left[1]
        cross_sum += cross
        x_sum += (left[0] + right[0]) * cross
        y_sum += (left[1] + right[1]) * cross
    if not math.isfinite(cross_sum) or cross_sum == 0:
        raise ValueError("selected PUMA fixture has a degenerate polygon")
    return x_sum / (3 * cross_sum), y_sum / (3 * cross_sum)


def independent_expected(encoded: bytes) -> dict:
    try:
        document = json.loads(encoded)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot decode selected PUMA GeoJSON: {error}") from error
    features = document.get("features") if isinstance(document, dict) else None
    if (
        not isinstance(document, dict)
        or document.get("type") != "FeatureCollection"
        or not isinstance(features, list)
    ):
        raise ValueError("selected PUMA fixture is not a GeoJSON FeatureCollection")
    expected = []
    seen = set()
    for feature in features:
        identity = feature.get("id")
        geometry = feature.get("geometry", {})
        properties = feature.get("properties", {})
        rings = geometry.get("coordinates")
        classification = properties.get("classification", {}).get("name")
        if (
            feature.get("type") != "Feature"
            or not isinstance(identity, str)
            or not identity
            or identity in seen
            or properties.get("objectType") != "annotation"
            or geometry.get("type") != "Polygon"
            or not isinstance(rings, list)
            or len(rings) != 1
            or not isinstance(classification, str)
        ):
            raise ValueError("selected PUMA fixture differs from the reviewed annotation profile")
        seen.add(identity)
        x_px, y_px = polygon_centroid(rings[0])
        expected.append((identity, x_px, y_px, classification))
    counts = Counter(item[3] for item in expected)
    if len(expected) != 168 or counts != EXPECTED_CLASS_COUNTS:
        raise ValueError("selected PUMA fixture differs from reviewed count/class invariants")
    prefix = f"{len(ROI.encode('utf-8'))}:{ROI}:"
    expected.sort(key=lambda item: f"{prefix}{item[0]}".encode("utf-8"))
    levels = CLASSIFICATION_CHANNEL["levels"]
    return {
        "cell_ids": [f"{prefix}{item[0]}" for item in expected],
        "coordinates_um": [
            [item[1] * SCALE_UM_PER_PIXEL[0], item[2] * SCALE_UM_PER_PIXEL[1]]
            for item in expected
        ],
        "observations": {
            CLASSIFICATION_CHANNEL["id"]: [levels.index(item[3]) for item in expected]
        },
        "class_counts": dict(counts),
    }


def maximum_coordinate_error(left, right) -> float:
    return max(
        (
            abs(a - b)
            for point_a, point_b in zip(left, right, strict=True)
            for a, b in zip(point_a, point_b, strict=True)
        ),
        default=0.0,
    )


def validate(data_directory: Path, output: Path, *, reuse_source: bool = False) -> None:
    source_mode = "reused_preserved_download" if reuse_source else "new_download"
    if reuse_source:
        data_directory, encoded = reuse_sources(data_directory)
    else:
        data_directory, encoded = download_sources(data_directory)
    output = new_external_directory(output, "validation output")
    selected = output / "puma-metastatic-roi-008-nuclei.geojson"
    selected.write_bytes(encoded)
    expected = independent_expected(encoded)
    width_um = ROI_SIZE_PX * SCALE_UM_PER_PIXEL[0]
    height_um = ROI_SIZE_PX * SCALE_UM_PER_PIXEL[1]
    window = {
        "type": "MultiPolygon",
        "coordinates": [[[
            [0.0, 0.0],
            [width_um, 0.0],
            [width_um, height_um],
            [0.0, height_um],
            [0.0, 0.0],
        ]]],
    }
    slide = qupath_nuclei_slide(
        selected,
        slide_id=ROI,
        patient_id="public-melanoma-fixture",
        group="format-validation",
        coordinate_frame_id=f"{ROI}-local-um",
        window=window,
        coordinate_scale_um=SCALE_UM_PER_PIXEL,
        classification_channel=CLASSIFICATION_CHANNEL,
    )
    coordinate_error = maximum_coordinate_error(
        slide["coordinates_um"], expected["coordinates_um"]
    )
    if slide["cell_ids"] != expected["cell_ids"]:
        raise ValueError("QuPath adapter changed PUMA feature identities")
    if slide["observations"] != expected["observations"]:
        raise ValueError("QuPath adapter changed PUMA classification names/codes")
    if coordinate_error > ABSOLUTE_TOLERANCE_UM:
        raise ValueError("QuPath adapter area centroids differ from the independent oracle")
    summary = {
        "source": "PUMA v5 melanoma histopathology nuclei annotations",
        "source_record": RECORD_API,
        "doi": DOI,
        "source_mode": source_mode,
        "license": "CC0-1.0",
        "archive_bytes": ARCHIVE_BYTES,
        "selected_member": MEMBER,
        "roi": ROI,
        "roi_size_px": [ROI_SIZE_PX, ROI_SIZE_PX],
        "scale_um_per_pixel": list(SCALE_UM_PER_PIXEL),
        "nucleus_count": len(expected["cell_ids"]),
        "class_counts": expected["class_counts"],
        "maximum_coordinate_error_um": coordinate_error,
        "absolute_tolerance_um": ABSOLUTE_TOLERANCE_UM,
        "centroid_oracle": "independent signed-shoelace area centroid",
        "profile": "classification-only QuPath annotation simple Polygons",
    }
    (output / "validation.json").write_text(
        json.dumps(summary, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps(summary, indent=2))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "data_directory", type=Path, help="external directory for preserved raw input"
    )
    parser.add_argument("output", type=Path, help="new external directory for validation results")
    parser.add_argument(
        "--reuse-source",
        action="store_true",
        help="reuse a preserved PUMA archive instead of requiring a new source directory",
    )
    args = parser.parse_args()
    validate(args.data_directory, args.output, reuse_source=args.reuse_source)


if __name__ == "__main__":
    main()
