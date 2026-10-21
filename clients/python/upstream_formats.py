"""Bounded adapters for concrete upstream single-cell interchange formats."""

from __future__ import annotations

import csv
import json
import math
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from marklab_client import cell_table_slide


MAXIMUM_STEINBOCK_CSV_BYTES = 64 * 1024 * 1024
MAXIMUM_MCMICRO_CSV_BYTES = 64 * 1024 * 1024
MCMICRO_RESERVED_COLUMNS = frozenset({
    "CellID", "X_centroid", "Y_centroid", "Area", "MajorAxisLength", "MinorAxisLength",
    "Eccentricity", "Solidity", "Extent", "Orientation",
})
MAXIMUM_QUPATH_GEOJSON_BYTES = 64 * 1024 * 1024


def qupath_nuclei_slide(
    path: str | Path,
    *,
    slide_id: str,
    patient_id: str,
    group: str,
    coordinate_frame_id: str,
    window: Mapping[str, Any],
    coordinate_scale_um: Sequence[float],
    classification_channel: Mapping[str, Any],
    maximum_nuclei: int = 100_000,
    maximum_vertices: int = 2_000_000,
    maximum_file_bytes: int = MAXIMUM_QUPATH_GEOJSON_BYTES,
) -> dict[str, Any]:
    """Import bounded QuPath/PUMA nuclear annotations as classified cell points.

    This profile accepts one GeoJSON ``FeatureCollection`` of QuPath annotation
    objects with simple, hole-free Polygon geometry. Each polygon becomes one point
    at its area centroid. The caller supplies the categorical channel/codebook,
    explicit micrometres-per-source-unit scales, identities, and physical window.
    Detection/cell measurements and general QuPath object graphs are outside this
    classification-only profile.
    """
    if (
        isinstance(maximum_file_bytes, bool)
        or not isinstance(maximum_file_bytes, int)
        or not 0 < maximum_file_bytes <= MAXIMUM_QUPATH_GEOJSON_BYTES
    ):
        raise ValueError("QuPath GeoJSON byte limit must be in 1..=67108864")
    if (
        isinstance(maximum_nuclei, bool)
        or not isinstance(maximum_nuclei, int)
        or not 0 < maximum_nuclei <= 1_000_000
    ):
        raise ValueError("QuPath nuclei limit must be in 1..=1000000")
    if (
        isinstance(maximum_vertices, bool)
        or not isinstance(maximum_vertices, int)
        or not 4 <= maximum_vertices <= 10_000_000
    ):
        raise ValueError("QuPath vertex limit must be in 4..=10000000")
    if classification_channel.get("kind") != "categorical":
        raise ValueError("QuPath classification channel must be categorical")

    path = Path(path)
    try:
        size = path.stat().st_size
        with path.open("rb") as stream:
            encoded = stream.read(maximum_file_bytes + 1)
    except OSError as error:
        raise ValueError(f"cannot read QuPath GeoJSON: {error}") from error
    if size == 0 or size > maximum_file_bytes or len(encoded) > maximum_file_bytes:
        raise ValueError(
            f"QuPath GeoJSON size {size} exceeds the admitted nonempty byte bound"
        )

    def unique_fields(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"QuPath GeoJSON has a duplicate JSON field: {key}")
            result[key] = value
        return result

    try:
        document = json.loads(encoded, object_pairs_hook=unique_fields)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot decode QuPath GeoJSON: {error}") from error

    if not isinstance(document, dict) or document.get("type") != "FeatureCollection":
        raise ValueError("QuPath GeoJSON must be a FeatureCollection")
    features = document.get("features")
    if not isinstance(features, list) or not features:
        raise ValueError("QuPath GeoJSON FeatureCollection must contain nuclei")
    if len(features) > maximum_nuclei:
        raise ValueError("QuPath GeoJSON exceeds the nuclei limit")

    try:
        from shapely.geometry import Polygon
    except ImportError as error:
        raise RuntimeError(
            "QuPath nuclei import requires the upstream-formats optional dependency"
        ) from error

    rows = []
    seen = set()
    vertex_count = 0
    for feature in features:
        if not isinstance(feature, dict) or feature.get("type") != "Feature":
            raise ValueError("QuPath nuclei must be GeoJSON Features")
        identity = feature.get("id")
        if not isinstance(identity, str) or not identity or identity in seen:
            raise ValueError(
                "QuPath nucleus identities must be unique nonempty strings; duplicate found"
            )
        seen.add(identity)
        properties = feature.get("properties")
        if not isinstance(properties, dict) or properties.get("objectType") != "annotation":
            raise ValueError("QuPath nuclei must be classification-only annotation objects")
        if set(properties) != {"objectType", "classification"}:
            raise ValueError("QuPath nuclei must contain only annotation classification properties")
        classification = properties.get("classification")
        if not isinstance(classification, dict):
            raise ValueError("QuPath nucleus classification must be an object with a name")
        class_name = classification.get("name")
        if not isinstance(class_name, str) or not class_name:
            raise ValueError("QuPath nucleus classification requires a nonempty name")

        geometry = feature.get("geometry")
        if not isinstance(geometry, dict) or geometry.get("type") != "Polygon":
            raise ValueError("QuPath nuclei require simple Polygon geometry")
        rings = geometry.get("coordinates")
        if not isinstance(rings, list) or not rings:
            raise ValueError("QuPath nucleus Polygon requires one exterior ring")
        if len(rings) != 1:
            raise ValueError("QuPath nucleus Polygon holes are outside this profile")
        ring = rings[0]
        if not isinstance(ring, list) or len(ring) < 4:
            raise ValueError("QuPath nucleus Polygon ring requires at least four positions")
        vertex_count += len(ring)
        if vertex_count > maximum_vertices:
            raise ValueError("QuPath GeoJSON exceeds the vertex limit")
        positions = []
        for position in ring:
            if (
                not isinstance(position, list)
                or len(position) != 2
                or any(
                    isinstance(value, bool)
                    or not isinstance(value, (int, float))
                    or not math.isfinite(value)
                    for value in position
                )
            ):
                raise ValueError("QuPath Polygon positions must be finite numeric XY pairs")
            positions.append((float(position[0]), float(position[1])))
        if positions[0] != positions[-1]:
            raise ValueError("QuPath Polygon exterior ring must be closed")
        polygon = Polygon(positions)
        if (
            polygon.is_empty
            or not polygon.is_valid
            or not math.isfinite(polygon.area)
            or polygon.area <= 0
        ):
            raise ValueError("QuPath nucleus Polygon must be valid with positive finite area")
        centroid = polygon.centroid
        if centroid.is_empty or not math.isfinite(centroid.x) or not math.isfinite(centroid.y):
            raise ValueError("QuPath nucleus Polygon must have a finite area centroid")
        rows.append(
            {
                "id": identity,
                "x": centroid.x,
                "y": centroid.y,
                "classification": class_name,
            }
        )

    channel_id = classification_channel.get("id")
    if not isinstance(channel_id, str) or not channel_id:
        raise ValueError("QuPath classification channel requires a nonempty string id")
    return cell_table_slide(
        rows,
        slide_id=slide_id,
        patient_id=patient_id,
        group=group,
        coordinate_frame_id=coordinate_frame_id,
        window=window,
        channels=[classification_channel],
        cell_id_column="id",
        x_column="x",
        y_column="y",
        coordinate_scale_um=coordinate_scale_um,
        channel_columns={channel_id: "classification"},
        maximum_cells=maximum_nuclei,
    )


def steinbock_csv_slide(
    intensities_path: str | Path,
    regionprops_path: str | Path,
    *,
    slide_id: str,
    patient_id: str,
    group: str,
    coordinate_frame_id: str,
    window: Mapping[str, Any],
    channels: Sequence[Mapping[str, Any]],
    coordinate_scale_um: Sequence[float],
    channel_columns: Mapping[str, str],
    maximum_cells: int = 100_000,
    maximum_file_bytes: int = MAXIMUM_STEINBOCK_CSV_BYTES,
) -> dict[str, Any]:
    """Import one Steinbock intensity/regionprops CSV pair as a study slide.

    Steinbock identifies rows with ``Object`` in both files. Regionprops follows
    scikit-image axis order: ``centroid-0`` is row/Y and ``centroid-1`` is column/X.
    The files do not carry a general physical-coordinate contract, so the caller must
    supply explicit positive micrometres-per-pixel scales and a window in that same
    local physical frame. Only continuous intensity channels are admitted here.
    """
    if (
        isinstance(maximum_file_bytes, bool)
        or not isinstance(maximum_file_bytes, int)
        or not 0 < maximum_file_bytes <= MAXIMUM_STEINBOCK_CSV_BYTES
    ):
        raise ValueError("Steinbock CSV byte limit must be in 1..=67108864")
    if (
        isinstance(maximum_cells, bool)
        or not isinstance(maximum_cells, int)
        or not 0 < maximum_cells <= 1_000_000
    ):
        raise ValueError("Steinbock cell limit must be in 1..=1000000")
    if any(channel.get("kind") != "continuous" for channel in channels):
        raise ValueError("Steinbock intensity channels must be continuous")
    reserved_columns = {"Object", "centroid-0", "centroid-1"}
    if reserved_columns.intersection(channel_columns.values()):
        raise ValueError(
            "Steinbock intensity channel columns cannot use reserved identity/centroid names"
        )
    intensity_fields, intensities = _read_identity_rows(
        intensities_path, "Object", maximum_file_bytes, maximum_cells, "Steinbock intensity"
    )
    regionprop_fields, regionprops = _read_identity_rows(
        regionprops_path, "Object", maximum_file_bytes, maximum_cells, "Steinbock regionprops"
    )
    required_intensity_fields = {"Object", *channel_columns.values()}
    if not required_intensity_fields.issubset(intensity_fields):
        raise ValueError("Steinbock intensity CSV is missing a declared column")
    if not {"Object", "centroid-0", "centroid-1"}.issubset(regionprop_fields):
        raise ValueError(
            "Steinbock regionprops CSV requires Object, centroid-0 (Y), and centroid-1 (X)"
        )
    if intensities.keys() != regionprops.keys():
        raise ValueError("Steinbock intensity and regionprops object identities differ")
    rows = []
    for object_id, intensity in intensities.items():
        region = regionprops[object_id]
        rows.append(
            {
                "Object": object_id,
                "centroid-0": region["centroid-0"],
                "centroid-1": region["centroid-1"],
                **{column: intensity[column] for column in channel_columns.values()},
            }
        )
    return cell_table_slide(
        rows,
        slide_id=slide_id,
        patient_id=patient_id,
        group=group,
        coordinate_frame_id=coordinate_frame_id,
        window=window,
        channels=channels,
        cell_id_column="Object",
        x_column="centroid-1",
        y_column="centroid-0",
        coordinate_scale_um=coordinate_scale_um,
        channel_columns=channel_columns,
        maximum_cells=maximum_cells,
    )


def mcmicro_csv_slide(
    quantification_path: str | Path,
    *,
    slide_id: str,
    patient_id: str,
    group: str,
    coordinate_frame_id: str,
    window: Mapping[str, Any],
    channels: Sequence[Mapping[str, Any]],
    coordinate_scale_um: Sequence[float],
    channel_columns: Mapping[str, str],
    maximum_cells: int = 100_000,
    maximum_file_bytes: int = MAXIMUM_MCMICRO_CSV_BYTES,
) -> dict[str, Any]:
    """Import one MCMICRO single-cell quantification CSV (MCQuant output) as a study slide.

    MCQuant writes one row per segmented cell: ``CellID``, one mean-intensity column per
    marker, ``X_centroid`` (column) and ``Y_centroid`` (row) in pixels of the segmentation
    mask, then morphology columns. The table carries no physical calibration, so the caller
    must supply explicit positive micrometres-per-pixel scales, for example from the mask's
    OME-XML ``PhysicalSizeX``/``PhysicalSizeY``, and a window in that local physical frame.
    Only continuous intensity channels are admitted; morphology columns cannot be channels.
    """
    if (
        isinstance(maximum_file_bytes, bool)
        or not isinstance(maximum_file_bytes, int)
        or not 0 < maximum_file_bytes <= MAXIMUM_MCMICRO_CSV_BYTES
    ):
        raise ValueError("MCMICRO CSV byte limit must be in 1..=67108864")
    if (
        isinstance(maximum_cells, bool)
        or not isinstance(maximum_cells, int)
        or not 0 < maximum_cells <= 1_000_000
    ):
        raise ValueError("MCMICRO cell limit must be in 1..=1000000")
    if any(channel.get("kind") != "continuous" for channel in channels):
        raise ValueError("MCMICRO intensity channels must be continuous")
    if MCMICRO_RESERVED_COLUMNS.intersection(channel_columns.values()):
        raise ValueError(
            "MCMICRO channel columns cannot use reserved identity, centroid or morphology names"
        )
    fields, rows = _read_identity_rows(
        quantification_path, "CellID", maximum_file_bytes, maximum_cells,
        "MCMICRO quantification",
    )
    if not {"X_centroid", "Y_centroid", *channel_columns.values()}.issubset(fields):
        raise ValueError(
            "MCMICRO quantification CSV requires X_centroid, Y_centroid and every declared "
            "channel column"
        )
    return cell_table_slide(
        rows.values(),
        slide_id=slide_id,
        patient_id=patient_id,
        group=group,
        coordinate_frame_id=coordinate_frame_id,
        window=window,
        channels=channels,
        cell_id_column="CellID",
        x_column="X_centroid",
        y_column="Y_centroid",
        coordinate_scale_um=coordinate_scale_um,
        channel_columns=channel_columns,
        maximum_cells=maximum_cells,
    )


def _read_identity_rows(path, identity, maximum_file_bytes, maximum_cells, source):
    """Read a bounded CSV whose rows carry a unique, nonempty ``identity`` column."""
    path = Path(path)
    try:
        size = path.stat().st_size
    except OSError as error:
        raise ValueError(f"cannot inspect {source} CSV: {error}") from error
    if size == 0 or size > maximum_file_bytes:
        raise ValueError(
            f"{source} CSV size {size} exceeds the admitted nonempty byte bound"
        )
    try:
        with path.open("rb") as stream:
            remaining = maximum_file_bytes

            def bounded_lines():
                nonlocal remaining
                while line := stream.readline(remaining + 1):
                    remaining -= len(line)
                    if remaining < 0:
                        raise ValueError(f"{source} CSV exceeds the admitted byte bound")
                    yield line.decode("utf-8")

            reader = csv.DictReader(bounded_lines())
            fieldnames = reader.fieldnames or ()
            if len(set(fieldnames)) != len(fieldnames):
                raise ValueError(f"{source} CSV has a duplicate column name")
            fields = set(fieldnames)
            if identity not in fields:
                raise ValueError(f"{source} CSV requires the {identity} column")
            rows = {}
            for row in reader:
                if len(rows) == maximum_cells:
                    raise ValueError(f"{source} CSV exceeds the cell limit")
                object_id = row[identity]
                if not object_id or object_id in rows:
                    raise ValueError(
                        f"{source} CSV has an empty or duplicate {identity} identity"
                    )
                rows[object_id] = row
    except (OSError, UnicodeError, csv.Error) as error:
        raise ValueError(f"cannot read {source} CSV: {error}") from error
    if not rows:
        raise ValueError(f"{source} CSV has no objects")
    return fields, rows
