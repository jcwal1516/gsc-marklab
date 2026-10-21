#!/usr/bin/env python3
"""Validate Marklab's Steinbock CSV and AnnData paths on upstream CC-BY test data.

This downloads only the official bounded Steinbock test-data archive. It never
downloads or executes source code. Raw input and the derived H5AD are written to
fresh directories outside the checkout.
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import shutil
import sys
import tarfile
import urllib.request
from pathlib import Path


REPOSITORY = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPOSITORY / "clients/python"))

from marklab_client import anndata_slide  # noqa: E402
from upstream_formats import steinbock_csv_slide  # noqa: E402


ARCHIVE_URL = (
    "https://github.com/BodenmillerGroup/TestData/releases/download/v1.0.7/"
    "210308_ImcTestData_steinbock.tar.gz"
)
ARCHIVE_NAME = "210308_ImcTestData_steinbock.tar.gz"
LICENSE_URL = "https://raw.githubusercontent.com/BodenmillerGroup/TestData/main/LICENSE"
MAXIMUM_ARCHIVE_BYTES = 24 * 1024 * 1024
MAXIMUM_LICENSE_BYTES = 64 * 1024
MAXIMUM_SOURCE_MEMBER_BYTES = 4 * 1024 * 1024
ROI = "20210305_NE_mockData1_1"
PREFIX = "datasets/210308_ImcTestData/steinbock"
SOURCE_FILES = {
    "images.csv": f"{PREFIX}/images.csv",
    "intensities.csv": f"{PREFIX}/intensities/{ROI}.csv",
    "regionprops.csv": f"{PREFIX}/regionprops/{ROI}.csv",
}
ABSOLUTE_TOLERANCE = 1e-12
CHANNEL_COLUMNS = {
    "Cytokeratin5": "Cytokeratin 5",
    "H3K27Ac": "H3K27Ac",
}
CHANNELS = [
    {
        "id": channel_id,
        "label": source_column,
        "kind": "continuous",
        "unit": "intensity_au",
        "measurement_status": "measured",
        "provenance": (
            "BodenmillerGroup/TestData v1.0.7 Steinbock mean intensity; "
            f"{source_column}"
        ),
    }
    for channel_id, source_column in CHANNEL_COLUMNS.items()
]


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


def download_sources(directory: Path) -> tuple[Path, int]:
    directory = new_external_directory(directory, "raw-data directory")
    license_path = directory / "LICENSE"
    bounded_download(LICENSE_URL, license_path, MAXIMUM_LICENSE_BYTES)
    if not license_path.read_text(encoding="utf-8").startswith(
        "Attribution 4.0 International"
    ):
        raise ValueError("upstream no longer carries the reviewed CC-BY-4.0 license")
    archive = directory / ARCHIVE_NAME
    archive_bytes = bounded_download(ARCHIVE_URL, archive, MAXIMUM_ARCHIVE_BYTES)
    # The v1.0.7 asset is named .tar.gz but is currently served as a plain POSIX tar.
    # Auto-detection also remains valid if the upstream asset is corrected to gzip.
    with tarfile.open(archive, "r:*") as source:
        for output_name, member_name in SOURCE_FILES.items():
            member = source.getmember(member_name)
            if not member.isfile() or member.size > MAXIMUM_SOURCE_MEMBER_BYTES:
                raise ValueError(f"unexpected upstream member: {member_name}")
            extracted = source.extractfile(member)
            if extracted is None:
                raise ValueError(f"cannot read upstream member: {member_name}")
            with (directory / output_name).open("xb") as destination:
                shutil.copyfileobj(extracted, destination)
    (directory / "source.json").write_text(
        json.dumps(
            {
                "archive_url": ARCHIVE_URL,
                "license": "CC BY 4.0",
                "license_url": LICENSE_URL,
                "archive_bytes": archive_bytes,
                "roi": ROI,
                "source_members": SOURCE_FILES,
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    return directory, archive_bytes


def reuse_sources(directory: Path) -> tuple[Path, int]:
    """Validate and reuse a source directory previously created by this script."""
    directory = directory.resolve()
    if directory == REPOSITORY or REPOSITORY in directory.parents:
        raise ValueError("raw-data directory must remain outside the checkout")
    if not directory.is_dir():
        raise ValueError("reused raw-data directory must already exist")
    bounds = {
        "LICENSE": MAXIMUM_LICENSE_BYTES,
        "source.json": MAXIMUM_LICENSE_BYTES,
        ARCHIVE_NAME: MAXIMUM_ARCHIVE_BYTES,
        **{name: MAXIMUM_SOURCE_MEMBER_BYTES for name in SOURCE_FILES},
    }
    sizes = {}
    for name, maximum in bounds.items():
        path = directory / name
        try:
            size = path.stat().st_size
        except OSError as error:
            raise ValueError(f"cannot inspect preserved source {name}: {error}") from error
        if not path.is_file() or not 0 < size <= maximum:
            raise ValueError(f"preserved source {name} violates its file-size bound")
        sizes[name] = size
    if not (directory / "LICENSE").read_text(encoding="utf-8").startswith(
        "Attribution 4.0 International"
    ):
        raise ValueError("preserved source does not carry the reviewed CC-BY-4.0 license")
    try:
        metadata = json.loads((directory / "source.json").read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read preserved source metadata: {error}") from error
    if (
        metadata.get("archive_url") != ARCHIVE_URL
        or metadata.get("license") != "CC BY 4.0"
        or metadata.get("license_url") != LICENSE_URL
        or metadata.get("roi") != ROI
        or metadata.get("source_members") != SOURCE_FILES
        or metadata.get("archive_bytes") != sizes[ARCHIVE_NAME]
    ):
        raise ValueError("preserved source metadata differs from the reviewed fixture")
    return directory, sizes[ARCHIVE_NAME]


def read_rows(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as stream:
        return list(csv.DictReader(stream))


def calibration(directory: Path) -> tuple[tuple[float, float], dict, dict]:
    matches = [row for row in read_rows(directory / "images.csv") if row["image"] == f"{ROI}.tiff"]
    if len(matches) != 1:
        raise ValueError("the selected ROI is absent or ambiguous in images.csv")
    row = matches[0]
    width_px, height_px = int(row["width_px"]), int(row["height_px"])
    width_um = float(row["acquisition_width_um"])
    height_um = float(row["acquisition_height_um"])
    scale = (width_um / width_px, height_um / height_px)
    if (
        width_px <= 0
        or height_px <= 0
        or not all(math.isfinite(value) and value > 0 for value in [width_um, height_um, *scale])
    ):
        raise ValueError("upstream acquisition calibration is not finite and positive")
    for object_row in read_rows(directory / "regionprops.csv"):
        y_px = float(object_row["centroid-0"])
        x_px = float(object_row["centroid-1"])
        if not (
            math.isfinite(x_px)
            and math.isfinite(y_px)
            and 0 <= x_px < width_px
            and 0 <= y_px < height_px
        ):
            raise ValueError(
                "Steinbock centroid-1/X or centroid-0/Y lies outside its image axis"
            )
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
    return scale, window, {
        "width_px": width_px,
        "height_px": height_px,
        "width_um": width_um,
        "height_um": height_um,
        "scale_um_per_pixel": list(scale),
    }


def independent_expected(directory: Path, scale: tuple[float, float]) -> dict:
    intensities = {row["Object"]: row for row in read_rows(directory / "intensities.csv")}
    regionprops = {row["Object"]: row for row in read_rows(directory / "regionprops.csv")}
    if intensities.keys() != regionprops.keys() or len(intensities) != 8:
        raise ValueError("expected the reviewed eight-object ROI with matching object tables")
    ids = sorted(intensities, key=lambda value: f"{len(ROI.encode())}:{ROI}:{value}".encode())
    return {
        "source_ids": ids,
        "cell_ids": [f"{len(ROI.encode())}:{ROI}:{value}" for value in ids],
        "coordinates_um": [
            [
                float(regionprops[value]["centroid-1"]) * scale[0],
                float(regionprops[value]["centroid-0"]) * scale[1],
            ]
            for value in ids
        ],
        "observations": {
            channel_id: [
                None if intensities[value][column] == "" else float(intensities[value][column])
                for value in ids
            ]
            for channel_id, column in CHANNEL_COLUMNS.items()
        },
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
        data_directory, archive_bytes = reuse_sources(data_directory)
    else:
        data_directory, archive_bytes = download_sources(data_directory)
    output = new_external_directory(output, "validation output")
    scale, window, calibration_summary = calibration(data_directory)
    expected = independent_expected(data_directory, scale)
    kwargs = {
        "slide_id": ROI,
        "patient_id": "upstream-test-patient",
        "group": "upstream-test-group",
        "coordinate_frame_id": f"{ROI}-local-um",
        "window": window,
        "channels": CHANNELS,
    }
    csv_slide = steinbock_csv_slide(
        data_directory / "intensities.csv",
        data_directory / "regionprops.csv",
        coordinate_scale_um=scale,
        channel_columns=CHANNEL_COLUMNS,
        **kwargs,
    )
    if csv_slide["cell_ids"] != expected["cell_ids"]:
        raise ValueError("CSV adapter changed upstream Object identities")
    csv_error = maximum_coordinate_error(csv_slide["coordinates_um"], expected["coordinates_um"])
    if csv_error > ABSOLUTE_TOLERANCE or csv_slide["observations"] != expected["observations"]:
        raise ValueError("CSV adapter changed calibrated coordinates or intensity values")

    import anndata
    import numpy as np
    import pandas as pd
    from scipy.sparse import csr_matrix

    matrix = np.asarray(
        [
            [
                np.nan if expected["observations"][channel["id"]][row] is None
                else expected["observations"][channel["id"]][row]
                for channel in CHANNELS
            ]
            for row in range(len(expected["source_ids"]))
        ],
        dtype=float,
    )
    adata = anndata.AnnData(
        csr_matrix(matrix),
        obs=pd.DataFrame(index=expected["source_ids"]),
        var=pd.DataFrame(index=[channel["id"] for channel in CHANNELS]),
    )
    adata.obsm["spatial_um"] = np.asarray(expected["coordinates_um"], dtype=float)
    h5ad_path = output / "steinbock-eight-cell-mock-fixture-derived.h5ad"
    with anndata.settings.override(allow_write_nullable_strings=True):
        adata.write_h5ad(h5ad_path)
    roundtrip = anndata_slide(
        anndata.read_h5ad(h5ad_path),
        coordinate_unit="micrometer",
        spatial_key="spatial_um",
        **kwargs,
    )
    h5ad_error = maximum_coordinate_error(
        roundtrip["coordinates_um"], expected["coordinates_um"]
    )
    if (
        roundtrip["cell_ids"] != expected["cell_ids"]
        or roundtrip["observations"] != expected["observations"]
        or h5ad_error > ABSOLUTE_TOLERANCE
    ):
        raise ValueError(
            "derived mock-fixture H5AD round trip changed IDs, coordinates, or values"
        )
    summary = {
        "source": "BodenmillerGroup/TestData v1.0.7 Steinbock release",
        "source_mode": source_mode,
        "data_provenance": "upstream IMC mock fixture; not biological cohort data",
        "license": "CC BY 4.0",
        "archive_bytes": archive_bytes,
        "roi": ROI,
        "object_count": len(expected["cell_ids"]),
        "calibration": calibration_summary,
        "channels": list(CHANNEL_COLUMNS),
        "missing_by_channel": {
            channel: sum(value is None for value in values)
            for channel, values in expected["observations"].items()
        },
        "csv_maximum_coordinate_error_um": csv_error,
        "h5ad_maximum_coordinate_error_um": h5ad_error,
        "absolute_tolerance_um": ABSOLUTE_TOLERANCE,
        "h5ad_scope": (
            "derived from preserved upstream mock-fixture CSV rows; not an upstream H5AD"
        ),
    }
    (output / "validation.json").write_text(
        json.dumps(summary, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps(summary, indent=2))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "data_directory",
        type=Path,
        help="external directory for preserved raw input",
    )
    parser.add_argument("output", type=Path, help="new external directory for validation results")
    parser.add_argument(
        "--reuse-source",
        action="store_true",
        help="reuse a prior preserved download instead of requiring a new source directory",
    )
    args = parser.parse_args()
    validate(args.data_directory, args.output, reuse_source=args.reuse_source)


if __name__ == "__main__":
    main()
