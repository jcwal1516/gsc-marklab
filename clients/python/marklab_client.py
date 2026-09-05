"""Thin client for the native, durable Marklab multiplex study service.

This module uses only the Python standard library. It does not implement statistics or import
AnnData/Pandas/NumPy; anndata_slide consumes an existing AnnData object through its public API.
The bounded interchange profile is tested with AnnData 0.12.4, including a real H5AD round trip.
"""
from __future__ import annotations

import copy
import json
import math
from pathlib import Path
import subprocess
import tempfile
from typing import Any, Mapping, Sequence

MAXIMUM_RECIPE_BYTES = 16 * 1024 * 1024


class MarklabError(RuntimeError):
    """The native service could not admit, execute, resume or publish the requested study."""


def run_study(recipe: Mapping[str, Any], *, project: str | Path, out: str | Path,
              binary: str | Path = "marklab", through_slides: int | None = None,
              timeout_seconds: float = 600) -> dict[str, Any] | None:
    """Execute the same recipe/service as the CLI; return its result or None at a checkpoint.

    The caller supplies the complete prespecified design and independent patient identities.
    Existing nonempty output directories are rejected by the native transaction. Exceptions retain
    native diagnostics; partial durable work remains available for an explicit resume. Inspect the
    returned maturity/inference status before interpreting an effect or p-value.
    """
    encoded = json.dumps(recipe, allow_nan=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
    if len(encoded) > MAXIMUM_RECIPE_BYTES:
        raise ValueError("recipe exceeds the admitted 16 MiB profile; select a bounded ROI/panel")
    if through_slides is not None and (isinstance(through_slides, bool) or not isinstance(through_slides, int) or through_slides <= 0):
        raise ValueError("through_slides must be a positive integer")
    if not math.isfinite(timeout_seconds) or timeout_seconds <= 0:
        raise ValueError("timeout_seconds must be finite and positive")
    with tempfile.TemporaryDirectory(prefix="marklab-client-") as temporary:
        source = Path(temporary) / "recipe.json"
        source.write_bytes(encoded)
        command = [str(binary), "study", "run", "--recipe", str(source), "--project", str(project), "--out", str(out)]
        if through_slides is not None:
            command.extend(["--through-slides", str(through_slides)])
        try:
            completed = subprocess.run(command, check=False, capture_output=True, text=True, timeout=timeout_seconds)
        except (OSError, subprocess.TimeoutExpired) as error:
            raise MarklabError(f"native study launch/execution failed: {error}; durable progress may be resumed") from error
        if completed.returncode:
            raise MarklabError(f"native study exited {completed.returncode}: {completed.stderr.strip()}")
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
        raise MarklabError(f"cannot read native result {result_path}: {error}") from error
    if document.get("format") != "marklab.multiplex_study_result" or document.get("version") != 1:
        raise MarklabError("native result has an unsupported study profile")
    return document


def anndata_slide(adata: Any, *, slide_id: str, patient_id: str, group: str,
                  coordinate_frame_id: str, coordinate_unit: str, spatial_key: str,
                  window: Mapping[str, Any], channels: Sequence[Mapping[str, Any]],
                  layer: str | None = None, maximum_matrix_values: int = 2_000_000) -> dict[str, Any]:
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
        raise ValueError("coordinate_unit must be micrometer; supply an explicitly calibrated spatial array")
    if getattr(adata, "isbacked", False):
        raise ValueError("backed AnnData is outside this profile; materialize a bounded ROI first")
    if not isinstance(maximum_matrix_values, int) or isinstance(maximum_matrix_values, bool) or not 0 < maximum_matrix_values <= 8_000_000:
        raise ValueError("materialization limit must be in 1..=8000000 values")
    if not channels or len(channels) > 64:
        raise ValueError("select 1..=64 declared channels")
    names = [channel["id"] for channel in channels]
    if len(set(names)) != len(names):
        raise ValueError("channel identities must be unique")
    source_ids = list(adata.obs_names)
    if any(not isinstance(value, str) or not value for value in source_ids) or len(set(source_ids)) != len(source_ids):
        raise ValueError("AnnData observation identities must be unique nonempty strings")
    if len(set(adata.var_names)) != len(adata.var_names):
        raise ValueError("AnnData variable identities must be unique")
    count = len(source_ids)
    if count != adata.n_obs or count * len(channels) > maximum_matrix_values:
        raise ValueError("selected panel exceeds the materialization limit")
    coordinates = adata.obsm[spatial_key]
    if getattr(coordinates, "shape", None) != (count, 2):
        raise ValueError("spatial coordinates must have exactly one physical XY row per observation")
    if hasattr(coordinates, "to_numpy"):
        coordinates = coordinates.to_numpy()
    quantitative = [channel["id"] for channel in channels if channel["kind"] == "continuous"]
    if any(name not in adata.var_names for name in quantitative):
        raise ValueError("a selected quantitative channel is absent from AnnData variables")
    # Slice before to_df: AnnData documents that to_df densifies sparse matrices and drops obs.
    frame = adata[:, quantitative].to_df(layer=layer) if quantitative else None
    if frame is not None and (list(frame.index) != source_ids or list(frame.columns) != quantitative):
        raise ValueError("AnnData selected matrix and observation identities disagree")
    observations = {}
    for channel in channels:
        name = channel["id"]
        kind = channel["kind"]
        if kind not in {"continuous", "binary", "categorical"}:
            raise ValueError(f"unsupported channel kind: {kind}")
        series = frame[name] if kind == "continuous" else adata.obs[name]
        if list(series.index) != source_ids:
            raise ValueError(f"observation annotation {name} has a different row identity/order")
        codebook = channel.get("levels", [])
        if kind == "categorical" and (not codebook or len(set(codebook)) != len(codebook)):
            raise ValueError(f"categorical channel {name} requires a unique declared codebook")
        values = []
        for value, missing in zip(series.tolist(), series.isna().tolist(), strict=True):
            if missing:
                values.append(None)
            elif kind == "categorical":
                if value not in codebook:
                    raise ValueError(f"channel {name} has an undeclared category: {value!r}")
                values.append(codebook.index(value))
            else:
                number = float(value)
                if not math.isfinite(number) or (kind == "binary" and number not in (0.0, 1.0)):
                    raise ValueError(f"channel {name} has an invalid finite/binary observation")
                values.append(number)
        observations[name] = values
    # Length framing avoids aliases when slide/source identifiers themselves contain colons.
    cell_ids = [f"{len(slide_id.encode('utf-8'))}:{slide_id}:{value}" for value in source_ids]
    order = sorted(range(count), key=lambda row: cell_ids[row].encode("utf-8"))
    points = []
    for row in order:
        point = [float(coordinates[row, axis]) for axis in range(2)]
        if not all(math.isfinite(value) for value in point):
            raise ValueError("spatial coordinates must be finite micrometres")
        points.append(point)
    return {"slide_id":slide_id,"patient_id":patient_id,"group":group,
            "coordinate_frame_id":coordinate_frame_id,"window":copy.deepcopy(window),
            "cell_ids":[cell_ids[row] for row in order],"coordinates_um":points,
            "observations":{name:[values[row] for row in order] for name, values in observations.items()}}
