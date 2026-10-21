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
ARCHITECTURE_AREA_LAYERS = frozenset({
    "window", "artifact", "epithelium", "lumens", "mucin", "front_exclusions",
})
ARCHITECTURE_LINE_LAYERS = frozenset({"declared_front"})


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

    document = _read_geojson(path, maximum_file_bytes)
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


def qupath_architecture_slide(
    path: str | Path,
    *,
    slide_id: str,
    patient_id: str,
    coordinate_frame_id: str,
    coordinate_scale_um: Sequence[float],
    layers: Mapping[str, str],
    boundary_uncertainty_um: float,
    clearance_um: float = 0.001,
    maximum_features: int = 100_000,
    maximum_vertices: int = 5_000_000,
    maximum_file_bytes: int = MAXIMUM_QUPATH_GEOJSON_BYTES,
) -> dict[str, Any]:
    """Convert one QuPath annotation export into a ``pathology architecture`` slide.

    ``layers`` maps every QuPath classification name in the file to ``window``,
    ``artifact``, ``epithelium``, ``lumens``, ``mucin``, ``front_exclusions``,
    ``declared_front``, ``region:<architecture class>`` or ``ignore``; an unmapped name is
    an error. Area layers need Polygon or MultiPolygon geometry and the declared front needs
    LineString or MultiLineString geometry. Coordinates are multiplied by the explicit
    micrometres-per-pixel scales; the frame keeps QuPath's image orientation (y down).

    The tissue window is the tissue union minus artifacts. Lumens and mucin are clipped to
    the window and removed from epithelium, so a gland can be drawn by its outer boundary.
    Regions of different classes must not overlap. Every polygon layer is shrunk by
    ``clearance_um`` (mitred) so pieces that touch at a point become disjoint, as the
    native validator requires; the default 0.001 um changes areas negligibly. Cells and
    spots are not read here; add them to the returned slide.
    """
    for name, value, low, high in (
        ("feature", maximum_features, 1, 1_000_000),
        ("vertex", maximum_vertices, 4, 20_000_000),
        ("byte", maximum_file_bytes, 1, MAXIMUM_QUPATH_GEOJSON_BYTES),
    ):
        if isinstance(value, bool) or not isinstance(value, int) or not low <= value <= high:
            raise ValueError(f"QuPath architecture {name} limit must be in {low}..={high}")
    if len(coordinate_scale_um) != 2 or any(
        isinstance(value, bool) or not isinstance(value, (int, float))
        or not math.isfinite(value) or value <= 0
        for value in coordinate_scale_um
    ):
        raise ValueError("coordinate_scale_um must be two finite positive micrometres per pixel")
    for name, value in (("boundary_uncertainty_um", boundary_uncertainty_um),
                        ("clearance_um", clearance_um)):
        if isinstance(value, bool) or not isinstance(value, (int, float)) or not (
            math.isfinite(value) and value >= 0
        ):
            raise ValueError(f"{name} must be finite and nonnegative")
    for name, layer in layers.items():
        if not isinstance(name, str) or not name or not isinstance(layer, str) or not (
            layer in ARCHITECTURE_AREA_LAYERS | ARCHITECTURE_LINE_LAYERS | {"ignore"}
            or (layer.startswith("region:") and layer != "region:")
        ):
            raise ValueError(f"invalid QuPath architecture layer mapping {name!r}: {layer!r}")
    try:
        import shapely
        from shapely.geometry import shape
    except ImportError as error:
        raise RuntimeError(
            "QuPath architecture import requires the upstream-formats optional dependency"
        ) from error

    document = _read_geojson(path, maximum_file_bytes)
    features = document.get("features")
    if not isinstance(features, list) or not features:
        raise ValueError("QuPath GeoJSON FeatureCollection must contain annotations")
    if len(features) > maximum_features:
        raise ValueError("QuPath GeoJSON exceeds the feature limit")
    scale_x, scale_y = (float(value) for value in coordinate_scale_um)
    parts: dict[str, list[Any]] = {}
    vertex_count = 0
    for index, feature in enumerate(features):
        label = f"feature {feature.get('id', index)!r}" if isinstance(feature, dict) else "feature"
        if not isinstance(feature, dict) or feature.get("type") != "Feature":
            raise ValueError("QuPath architecture objects must be GeoJSON Features")
        properties = feature.get("properties")
        if not isinstance(properties, dict) or properties.get("objectType") != "annotation":
            raise ValueError(f"{label} is not a QuPath annotation object")
        classification = properties.get("classification")
        name = classification.get("name") if isinstance(classification, dict) else None
        if not isinstance(name, str) or not name:
            raise ValueError(f"{label} has no classification name")
        if name not in layers:
            raise ValueError(f"{label} classification {name!r} is not mapped to a layer")
        layer = layers[name]
        if layer == "ignore":
            continue
        geometry = feature.get("geometry")
        kind = geometry.get("type") if isinstance(geometry, dict) else None
        expected = (
            ("LineString", "MultiLineString")
            if layer in ARCHITECTURE_LINE_LAYERS
            else ("Polygon", "MultiPolygon")
        )
        if kind not in expected:
            raise ValueError(f"{label} layer {layer} needs {' or '.join(expected)} geometry")
        coordinates, count = _scaled_coordinates(geometry["coordinates"], scale_x, scale_y)
        vertex_count += count
        if vertex_count > maximum_vertices:
            raise ValueError("QuPath GeoJSON exceeds the vertex limit")
        parsed = shape({"type": kind, "coordinates": coordinates})
        if parsed.is_empty or not parsed.is_valid:
            raise ValueError(f"{label} geometry is empty or invalid; repair it in QuPath")
        parts.setdefault(layer, []).append(parsed)

    def union(layer):
        return shapely.union_all(parts.get(layer, [])) if layer in parts else None

    window = union("window")
    if window is None:
        raise ValueError("QuPath architecture export needs a window (tissue) layer")
    if (artifact := union("artifact")) is not None:
        window = window.difference(artifact)
    lumens = union("lumens")
    lumens = lumens.intersection(window) if lumens is not None else None
    mucin = union("mucin")
    if mucin is not None:
        mucin = mucin.intersection(window)
        if lumens is not None:
            mucin = mucin.difference(lumens)
    epithelium = union("epithelium")
    if epithelium is None:
        raise ValueError("QuPath architecture export needs an epithelium layer")
    epithelium = epithelium.intersection(window)
    for carved in (lumens, mucin):
        if carved is not None:
            epithelium = epithelium.difference(carved)
    regions = {
        layer.removeprefix("region:"): shapely.union_all(geometries).intersection(window)
        for layer, geometries in sorted(parts.items())
        if layer.startswith("region:")
    }
    tolerance = 1e-9 * window.area
    names = sorted(regions)
    for left_index, left in enumerate(names):
        for right in names[left_index + 1:]:
            if regions[left].intersection(regions[right]).area > tolerance:
                raise ValueError(f"architecture regions {left!r} and {right!r} overlap")

    def area_json(geometry, name, required=False):
        if geometry is not None and clearance_um > 0:
            geometry = geometry.buffer(-clearance_um, join_style="mitre", mitre_limit=5.0)
        polygons = [] if geometry is None else _polygons(geometry)
        if not polygons:
            if required:
                raise ValueError(f"QuPath architecture {name} layer has no area inside the window")
            return None
        return {"type": "MultiPolygon", "coordinates": [_polygon_coordinates(polygon) for polygon in polygons]}

    slide: dict[str, Any] = {
        "slide_id": slide_id,
        "patient_id": patient_id,
        "coordinate_frame_id": coordinate_frame_id,
        "window": area_json(window, "window", required=True),
        "epithelium": area_json(epithelium, "epithelium", required=True),
        "boundary_uncertainty_um": float(boundary_uncertainty_um),
        "architecture_regions": [
            {"id": f"region-{name}", "class": name, "geometry": geometry}
            for name in names
            if (geometry := area_json(regions[name], f"region {name}")) is not None
        ],
    }
    for name, geometry in (("lumens", lumens), ("mucin", mucin),
                           ("front_exclusions", union("front_exclusions"))):
        if geometry is not None and name == "front_exclusions":
            geometry = geometry.intersection(window)
        if (value := area_json(geometry, name)) is not None:
            slide[name] = value
    if (front := union("declared_front")) is not None:
        inner = window.buffer(-clearance_um) if clearance_um > 0 else window
        lines = [line for line in _lines(front.intersection(inner)) if line.length > 0]
        if not lines:
            raise ValueError("QuPath declared front lies outside the tissue window")
        slide["declared_front"] = {
            "type": "MultiLineString",
            "coordinates": [[list(point) for point in line.coords] for line in lines],
        }
    return slide


def _read_geojson(path, maximum_file_bytes):
    path = Path(path)
    try:
        size = path.stat().st_size
        with path.open("rb") as stream:
            encoded = stream.read(maximum_file_bytes + 1)
    except OSError as error:
        raise ValueError(f"cannot read QuPath GeoJSON: {error}") from error
    if size == 0 or size > maximum_file_bytes or len(encoded) > maximum_file_bytes:
        raise ValueError(f"QuPath GeoJSON size {size} exceeds the admitted nonempty byte bound")

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
    return document


def _scaled_coordinates(value, scale_x, scale_y):
    """Scale nested GeoJSON positions, rejecting non-finite or non-XY positions."""
    if isinstance(value, list) and len(value) == 2 and all(
        isinstance(item, (int, float)) and not isinstance(item, bool) for item in value
    ):
        if not all(math.isfinite(item) for item in value):
            raise ValueError("QuPath positions must be finite numeric XY pairs")
        return [value[0] * scale_x, value[1] * scale_y], 1
    if not isinstance(value, list) or not value:
        raise ValueError("QuPath positions must be finite numeric XY pairs")
    scaled, count = [], 0
    for item in value:
        child, child_count = _scaled_coordinates(item, scale_x, scale_y)
        scaled.append(child)
        count += child_count
    return scaled, count


def _polygons(geometry):
    if geometry.geom_type == "Polygon":
        return [] if geometry.is_empty or geometry.area <= 0 else [geometry]
    if geometry.geom_type in ("MultiPolygon", "GeometryCollection"):
        return [polygon for part in geometry.geoms for polygon in _polygons(part)]
    return []


def _lines(geometry):
    if geometry.geom_type == "LineString":
        return [] if geometry.is_empty else [geometry]
    if geometry.geom_type in ("MultiLineString", "GeometryCollection"):
        return [line for part in geometry.geoms for line in _lines(part)]
    return []


def _polygon_coordinates(polygon):
    return [[list(point) for point in polygon.exterior.coords]] + [
        [list(point) for point in ring.coords] for ring in polygon.interiors
    ]


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
