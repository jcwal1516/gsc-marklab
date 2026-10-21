"""Thin client for the native, durable Marklab multiplex study service.

This module uses only the Python standard library. It does not implement statistics or import
AnnData/Pandas/NumPy; anndata_slide consumes an existing AnnData object through its public API.
The bounded interchange profile is tested with AnnData 0.12.4, including a real H5AD round trip.
"""

from __future__ import annotations

import copy
import json
import math
import subprocess
import tempfile
from collections.abc import Iterable, Mapping, Sequence
from pathlib import Path
from typing import Any

MAXIMUM_RECIPE_BYTES = 16 * 1024 * 1024


class MarklabError(RuntimeError):
    """The native service could not admit, execute, resume or publish the requested study."""


def cell_table_slide(
    rows: Iterable[Mapping[str, Any]],
    *,
    slide_id: str,
    patient_id: str,
    group: str,
    coordinate_frame_id: str,
    window: Mapping[str, Any],
    channels: Sequence[Mapping[str, Any]],
    cell_id_column: str,
    x_column: str,
    y_column: str,
    coordinate_scale_um: Sequence[float],
    channel_columns: Mapping[str, str],
    maximum_cells: int = 100_000,
) -> dict[str, Any]:
    """Convert a bounded cell table, such as CSV DictReader rows, to a study slide.

    The caller declares the cell/marker columns, patient and group, observation window,
    and micrometres per source coordinate unit on each axis. No calibration, phenotype,
    threshold or tissue outline is inferred. Empty strings and None are missing marker
    values; numeric zero remains observed. Unknown categories and non-finite numbers fail.
    All rows must belong to this one slide; split a multi-ROI table before calling.
    IDs, coordinates and observations are sorted together without modifying the input.
    """
    if (
        isinstance(maximum_cells, bool)
        or not isinstance(maximum_cells, int)
        or not 0 < maximum_cells <= 1_000_000
    ):
        raise ValueError("cell limit must be in 1..=1000000")
    if len(coordinate_scale_um) != 2 or any(
        isinstance(value, bool) or not math.isfinite(value) or value <= 0
        for value in coordinate_scale_um
    ):
        raise ValueError("coordinate scales must be two finite positive micrometre values")
    if not 1 <= len(channels) <= 64:
        raise ValueError("select 1..=64 declared channels")
    names = [channel["id"] for channel in channels]
    if len(set(names)) != len(names) or set(channel_columns) != set(names):
        raise ValueError("channel identities must be unique with one column mapping each")
    for channel in channels:
        if channel["kind"] not in {"continuous", "binary", "categorical"}:
            raise ValueError(f"unsupported channel kind: {channel['kind']}")
        levels = channel.get("levels", [])
        if channel["kind"] == "categorical" and (
            not levels or len(set(levels)) != len(levels)
        ):
            raise ValueError("categorical channels require a unique declared codebook")
    required = {cell_id_column, x_column, y_column, *channel_columns.values()}
    source_ids, coordinates = [], []
    observations = {name: [] for name in names}
    seen = set()
    for row in rows:
        if len(source_ids) >= maximum_cells or (len(source_ids) + 1) * len(names) > 8_000_000:
            raise ValueError("cell table exceeds the cell/marker materialization limit")
        if not required.issubset(row):
            raise ValueError("cell table is missing a declared column")
        identity = row[cell_id_column]
        if not isinstance(identity, str) or not identity or identity in seen:
            raise ValueError("cell identities must be unique nonempty strings")
        seen.add(identity)
        source_ids.append(identity)
        coordinates.append([
            float(row[x_column]) * coordinate_scale_um[0],
            float(row[y_column]) * coordinate_scale_um[1],
        ])
        for channel in channels:
            name, kind = channel["id"], channel["kind"]
            value = row[channel_columns[name]]
            if value is None or value == "":
                number = None
            elif kind == "categorical":
                levels = channel["levels"]
                if value not in levels:
                    raise ValueError(f"channel {name} has an undeclared category: {value!r}")
                number = levels.index(value)
            else:
                number = float(value)
                if not math.isfinite(number) or (kind == "binary" and number not in (0, 1)):
                    raise ValueError(f"channel {name} has an invalid finite/binary observation")
            observations[name].append(number)
    return _assemble_slide(
        source_ids, coordinates, observations, slide_id=slide_id, patient_id=patient_id,
        group=group, coordinate_frame_id=coordinate_frame_id, window=window,
    )


def run_study(
    recipe: Mapping[str, Any],
    *,
    project: str | Path,
    out: str | Path,
    binary: str | Path = "marklab",
    through_slides: int | None = None,
    timeout_seconds: float = 600,
) -> dict[str, Any] | None:
    """Execute the same recipe/service as the CLI; return its result or None at a checkpoint.

    The caller supplies the complete prespecified design and independent patient identities.
    Existing nonempty output directories are rejected by the native transaction. Exceptions retain
    native diagnostics; partial durable work remains available for an explicit resume. Inspect the
    returned maturity/inference status before interpreting an effect or p-value.
    """
    text = json.dumps(recipe, allow_nan=False, sort_keys=True, separators=(",", ":"))
    encoded = text.encode("utf-8")
    if len(encoded) > MAXIMUM_RECIPE_BYTES:
        raise ValueError(
            "recipe exceeds the admitted 16 MiB profile; select a bounded ROI/panel"
        )
    if through_slides is not None and (
        isinstance(through_slides, bool)
        or not isinstance(through_slides, int)
        or through_slides <= 0
    ):
        raise ValueError("through_slides must be a positive integer")
    if not math.isfinite(timeout_seconds) or timeout_seconds <= 0:
        raise ValueError("timeout_seconds must be finite and positive")
    with tempfile.TemporaryDirectory(prefix="marklab-client-") as temporary:
        source = Path(temporary) / "recipe.json"
        source.write_bytes(encoded)
        command = [
            str(binary),
            "study",
            "run",
            "--recipe",
            str(source),
            "--project",
            str(project),
            "--out",
            str(out),
        ]
        if through_slides is not None:
            command.extend(["--through-slides", str(through_slides)])
        try:
            completed = subprocess.run(
                command,
                check=False,
                capture_output=True,
                text=True,
                timeout=timeout_seconds,
            )
        except (OSError, subprocess.TimeoutExpired) as error:
            raise MarklabError(
                f"native study launch/execution failed: {error}; durable progress may be resumed"
            ) from error
        if completed.returncode:
            raise MarklabError(
                f"native study exited {completed.returncode}: {completed.stderr.strip()}"
            )
    if through_slides is not None:
        return None
    result_path = Path(out) / "result.json"
    try:
        with result_path.open("rb") as stream:
            result = stream.read(MAXIMUM_RECIPE_BYTES + 1)
        if len(result) > MAXIMUM_RECIPE_BYTES:
            raise MarklabError("native result exceeds the admitted output profile")
        document = json.loads(result)
    except (OSError, ValueError) as error:
        raise MarklabError(
            f"cannot read native result {result_path}: {error}"
        ) from error
    if (
        document.get("format") != "marklab.multiplex_study_result"
        or document.get("version") != 1
    ):
        raise MarklabError("native result has an unsupported study profile")
    return document


def anndata_slide(
    adata: Any,
    *,
    slide_id: str,
    patient_id: str,
    group: str,
    coordinate_frame_id: str,
    coordinate_unit: str,
    spatial_key: str,
    window: Mapping[str, Any],
    channels: Sequence[Mapping[str, Any]],
    layer: str | None = None,
    maximum_matrix_values: int = 2_000_000,
) -> dict[str, Any]:
    """Import one bounded, in-memory AnnData slide into the native recipe profile.

    Continuous channel IDs select variables from X or the explicitly named layer. Binary and
    categorical IDs select obs columns; categories map through the declared ordered codebook.
    Coordinates must already be calibrated micrometres in obsm[spatial_key], with exactly two
    columns. No pixel conversion, affine registration, threshold or biological status is inferred.
    Missing observations become null, while observed zeros remain zeros. Lossless slide-qualified
    source IDs and all values/coordinates are reordered together into native canonical row order.
    Backed/lazy/dask arrays are outside this admitted profile; materialize a bounded ROI first.
    """
    if coordinate_unit != "micrometer":
        raise ValueError(
            "coordinate_unit must be micrometer; supply an explicitly calibrated spatial array"
        )
    if getattr(adata, "isbacked", False):
        raise ValueError(
            "backed AnnData is outside this profile; materialize a bounded ROI first"
        )
    if (
        not isinstance(maximum_matrix_values, int)
        or isinstance(maximum_matrix_values, bool)
        or not 0 < maximum_matrix_values <= 8_000_000
    ):
        raise ValueError("materialization limit must be in 1..=8000000 values")
    if not channels or len(channels) > 64:
        raise ValueError("select 1..=64 declared channels")
    names = [channel["id"] for channel in channels]
    if len(set(names)) != len(names):
        raise ValueError("channel identities must be unique")
    source_ids = list(adata.obs_names)
    if any(not isinstance(value, str) or not value for value in source_ids) or len(
        set(source_ids)
    ) != len(source_ids):
        raise ValueError(
            "AnnData observation identities must be unique nonempty strings"
        )
    if len(set(adata.var_names)) != len(adata.var_names):
        raise ValueError("AnnData variable identities must be unique")
    count = len(source_ids)
    if count != adata.n_obs or count * len(channels) > maximum_matrix_values:
        raise ValueError("selected panel exceeds the materialization limit")
    coordinates = adata.obsm[spatial_key]
    if getattr(coordinates, "shape", None) != (count, 2):
        raise ValueError(
            "spatial coordinates must have exactly one physical XY row per observation"
        )
    if hasattr(coordinates, "to_numpy"):
        coordinates = coordinates.to_numpy()
    quantitative = [
        channel["id"] for channel in channels if channel["kind"] == "continuous"
    ]
    if any(name not in adata.var_names for name in quantitative):
        raise ValueError(
            "a selected quantitative channel is absent from AnnData variables"
        )
    # Slice before to_df: AnnData documents that to_df densifies sparse matrices and drops obs.
    frame = adata[:, quantitative].to_df(layer=layer) if quantitative else None
    if frame is not None and (
        list(frame.index) != source_ids or list(frame.columns) != quantitative
    ):
        raise ValueError("AnnData selected matrix and observation identities disagree")
    observations = {}
    for channel in channels:
        name = channel["id"]
        kind = channel["kind"]
        if kind not in {"continuous", "binary", "categorical"}:
            raise ValueError(f"unsupported channel kind: {kind}")
        series = frame[name] if kind == "continuous" else adata.obs[name]
        if list(series.index) != source_ids:
            raise ValueError(
                f"observation annotation {name} has a different row identity/order"
            )
        codebook = channel.get("levels", [])
        if kind == "categorical" and (
            not codebook or len(set(codebook)) != len(codebook)
        ):
            raise ValueError(
                f"categorical channel {name} requires a unique declared codebook"
            )
        values = []
        for value, missing in zip(series.tolist(), series.isna().tolist(), strict=True):
            if missing:
                values.append(None)
            elif kind == "categorical":
                if value not in codebook:
                    raise ValueError(
                        f"channel {name} has an undeclared category: {value!r}"
                    )
                values.append(codebook.index(value))
            else:
                number = float(value)
                if not math.isfinite(number) or (
                    kind == "binary" and number not in (0.0, 1.0)
                ):
                    raise ValueError(
                        f"channel {name} has an invalid finite/binary observation"
                    )
                values.append(number)
        observations[name] = values
    return _assemble_slide(
        source_ids, coordinates, observations, slide_id=slide_id, patient_id=patient_id,
        group=group, coordinate_frame_id=coordinate_frame_id, window=window,
    )


def _assemble_slide(
    source_ids, coordinates, observations, *, slide_id, patient_id, group,
    coordinate_frame_id, window,
):
    # Length framing avoids aliases when slide/source identifiers themselves contain colons.
    cell_ids = [
        f"{len(slide_id.encode('utf-8'))}:{slide_id}:{value}" for value in source_ids
    ]
    order = sorted(range(len(source_ids)), key=lambda row: cell_ids[row].encode("utf-8"))
    points = []
    for row in order:
        point = [float(coordinates[row][axis]) for axis in range(2)]
        if not all(math.isfinite(value) for value in point):
            raise ValueError("spatial coordinates must be finite micrometres")
        points.append(point)
    return {
        "slide_id": slide_id,
        "patient_id": patient_id,
        "group": group,
        "coordinate_frame_id": coordinate_frame_id,
        "window": copy.deepcopy(window),
        "cell_ids": [cell_ids[row] for row in order],
        "coordinates_um": points,
        "observations": {
            name: [values[row] for row in order]
            for name, values in observations.items()
        },
    }
