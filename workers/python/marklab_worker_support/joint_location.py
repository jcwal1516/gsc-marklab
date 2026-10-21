"""Request parsing and NUTS bookkeeping shared by the joint location workers.

Both workers fit a Poisson model for exact-window location counts jointly with a per-point
outcome (a categorical mark or a projected embedding) through shared patient-level factors,
holding out each patient's second pattern.
"""

from __future__ import annotations

import csv
import io
import math
from collections.abc import Callable
from typing import Any

import numpy as np

from .numpyro_nuts import nuts_diagnostics, sample_nuts
from .protocol import ContractError, sha256_hex, validate_backend
from .versions import JAX, NUMPYRO

LOCATION_FIELDS = (
    "pattern_id",
    "patient_id",
    "group",
    "cohort",
    "node_id",
    "type_id",
    "x_um",
    "y_um",
    "weight_um2",
    "window_area_um2",
    "covariate",
    "count",
    "window_sha256",
    "event_sha256",
)


def finite(value: str, name: str) -> float:
    """Parse a CSV cell as a finite float."""
    result = float(value)
    if not math.isfinite(result):
        raise ContractError(f"{name} is nonfinite")
    return result


def parse_csv(text: str, name: str) -> tuple[tuple[str, ...], list[dict[str, str]]]:
    """Return the header and rows of a non-empty CSV payload."""
    reader = csv.DictReader(io.StringIO(text))
    fields = tuple(reader.fieldnames or ())
    rows = list(reader)
    if not fields or not rows:
        raise ContractError(f"{name} is empty")
    return fields, rows


def validate_request_header(
    request: Any,
    *,
    fields: set[str],
    request_format: str,
    version: int,
    lock_sha: str,
    worker_sha: str,
) -> None:
    """Check the request field set, identity, NumPyro backend block, and JAX version."""
    if not isinstance(request, dict) or set(request) != fields:
        raise ContractError("request fields differ")
    if request["format"] != request_format or request["version"] != version:
        raise ContractError("request identity differs")
    validate_backend(
        request["backend"],
        "backend",
        name="numpyro",
        version=NUMPYRO,
        lock_sha256=lock_sha,
        worker_sha256=worker_sha,
    )
    if request["jax_version"] != JAX:
        raise ContractError("backend identity differs")


def verified_csv(request: dict[str, Any], name: str) -> str:
    """Return ``request[name + "_csv"]`` after checking it against ``name + "_sha256"``."""
    text = request[f"{name}_csv"]
    if not isinstance(text, str):
        raise ContractError("CSV payload differs")
    if sha256_hex(text.encode()) != request[f"{name}_sha256"]:
        raise ContractError(f"{name} digest differs")
    return text


def sample_model(
    config: dict[str, Any],
    model: Callable[[], None],
    seed: int,
    diagnostic_names: list[str] | None = None,
) -> tuple[dict[str, np.ndarray], dict[str, float | int]]:
    """Run NUTS and return draws flattened over chains and convergence diagnostics.

    Diagnostics cover ``diagnostic_names`` (default: every sample site).
    """
    samples, extra = sample_nuts(model, config, config["depth"], seed)
    names = list(samples) if diagnostic_names is None else diagnostic_names
    diagnostics = nuts_diagnostics(samples, names, extra, config["depth"])
    flat = {name: value.reshape((-1, *value.shape[2:])) for name, value in samples.items()}
    return flat, diagnostics


def combined_diagnostics(*parts: dict[str, float | int]) -> dict[str, float | int]:
    """Worst-case diagnostics over several fitted models (divergences and hits are summed)."""
    return {
        "maximum_r_hat": max(part["r_hat"] for part in parts),
        "minimum_bulk_ess": min(part["ess_bulk"] for part in parts),
        "minimum_tail_ess": min(part["ess_tail"] for part in parts),
        "minimum_ebfmi": min(part["minimum_ebfmi"] for part in parts),
        "divergences": sum(int(part["divergences"]) for part in parts),
        "max_tree_depth_hits": sum(int(part["max_tree_depth_hits"]) for part in parts),
    }


def diagnostics_complete(diagnostics: dict[str, float | int]) -> bool:
    """Fixed convergence policy of the joint location workers."""
    return bool(
        diagnostics["maximum_r_hat"] <= 1.01
        and diagnostics["minimum_bulk_ess"] >= 100.0
        and diagnostics["minimum_tail_ess"] >= 100.0
        and diagnostics["minimum_ebfmi"] >= 0.3
        and diagnostics["divergences"] == 0
        and diagnostics["max_tree_depth_hits"] == 0
    )
