"""Fused Gromov-Wasserstein (FGW) pieces shared by the balanced and partial POT workers.

Both workers align two weighted point sets that carry feature vectors and pairwise structure
matrices. The feature cost is the squared Euclidean distance between scaled features; the
structural term compares scaled structure matrices with the square loss.
"""

from __future__ import annotations

import contextlib
import warnings
from collections.abc import Collection, Iterator
from typing import Any

import numpy as np
import ot

from . import versions
from .protocol import ContractError, exact, obj, validate_backend

MAXIMUM_SUPPORT_POINTS = 32
SUPPORT_KEYS = frozenset({"id", "mass", "features"})
REQUEST_KEYS = frozenset(
    {
        "format",
        "version",
        "backend",
        "source",
        "target",
        "source_structure_row_major",
        "target_structure_row_major",
        "alpha",
        "epsilon",
        "feature_scale",
        "structure_scale",
        "tolerance",
        "maximum_iterations",
        "resources",
    }
)
RESOURCE_KEYS = frozenset(
    {
        "maximum_source_points",
        "maximum_target_points",
        "maximum_feature_dimension",
        "maximum_output_bytes",
        "timeout_seconds",
    }
)
INITIALIZATIONS = ("independent_mass", "feature_emd", "structure_profile_emd")


def validate_request(
    request: Any,
    *,
    request_format: str,
    extra_request_keys: Collection[str],
    extra_resource_keys: Collection[str],
    lock_sha256: str,
    worker_sha256: str,
) -> dict[str, Any]:
    """Check the request shape and backend identity shared by both FGW contracts."""
    request = obj(request, REQUEST_KEYS | set(extra_request_keys), "request")
    exact(request["format"], request_format, "request.format")
    exact(request["version"], 1, "request.version")
    validate_backend(
        request["backend"],
        "request.backend",
        name="pot",
        version=versions.POT,
        lock_sha256=lock_sha256,
        worker_sha256=worker_sha256,
    )
    for support in ("source", "target"):
        rows = request[support]
        if not isinstance(rows, list) or not 1 <= len(rows) <= MAXIMUM_SUPPORT_POINTS:
            raise ContractError(f"{support} dimensions differ")
        for index, row in enumerate(rows):
            obj(row, SUPPORT_KEYS, f"{support}[{index}]")
    obj(request["resources"], RESOURCE_KEYS | set(extra_resource_keys), "resources")
    return request


def problem(
    request: dict[str, Any],
) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray]:
    """Return source and target masses, the feature cost, and the scaled structure matrices."""
    source = np.asarray([row["mass"] for row in request["source"]], dtype=np.float64)
    target = np.asarray([row["mass"] for row in request["target"]], dtype=np.float64)
    source_features = np.asarray([row["features"] for row in request["source"]], dtype=np.float64)
    target_features = np.asarray([row["features"] for row in request["target"]], dtype=np.float64)
    feature_difference = source_features[:, None, :] - target_features[None, :, :]
    feature_cost = np.sum((feature_difference / float(request["feature_scale"])) ** 2, axis=2)
    rows = len(source)
    columns = len(target)
    source_structure = np.asarray(request["source_structure_row_major"], dtype=np.float64)
    target_structure = np.asarray(request["target_structure_row_major"], dtype=np.float64)
    structure_scale = float(request["structure_scale"])
    return (
        source,
        target,
        feature_cost,
        source_structure.reshape(rows, rows) / structure_scale,
        target_structure.reshape(columns, columns) / structure_scale,
    )


def initial_plans(
    source: np.ndarray,
    target: np.ndarray,
    feature_cost: np.ndarray,
    source_structure: np.ndarray,
    target_structure: np.ndarray,
    mass: float = 1.0,
) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """Return the three starting plans named in ``INITIALIZATIONS``, each carrying ``mass``.

    The plans are the independent coupling, the exact transport plan for the feature cost, and
    the exact plan for the squared difference of structure profiles (structure @ mass).
    """
    independent = mass * np.outer(source, target)
    feature = mass * ot.emd(source, target, feature_cost)
    source_profile = source_structure @ source
    target_profile = target_structure @ target
    profile_cost = (source_profile[:, None] - target_profile[None, :]) ** 2
    structure = mass * ot.emd(source, target, profile_cost)
    return independent, feature, structure


def plan_objectives(
    plan: np.ndarray,
    feature_cost: np.ndarray,
    source_structure: np.ndarray,
    target_structure: np.ndarray,
) -> tuple[float, float, float]:
    """Return the feature objective, square-loss structural objective, and plan entropy."""
    feature_objective = float(np.sum(feature_cost * plan))
    difference = source_structure[:, None, :, None] - target_structure[None, :, None, :]
    weighted = (difference * difference) * plan[:, :, None, None] * plan[None, None, :, :]
    structural_objective = float(np.sum(weighted))
    positive = plan > 0.0
    entropy = -float(np.sum(plan[positive] * np.log(plan[positive])))
    return feature_objective, structural_objective, entropy


def plan_entries(
    request: dict[str, Any], plan: np.ndarray, feature_cost: np.ndarray
) -> list[dict[str, Any]]:
    """List every plan cell in row-major order with its support identities and feature cost."""
    columns = plan.shape[1]
    return [
        {
            "source_id": request["source"][index // columns]["id"],
            "target_id": request["target"][index % columns]["id"],
            "mass": float(mass),
            "feature_cost": float(feature_cost[index // columns, index % columns]),
        }
        for index, mass in enumerate(plan.ravel())
    ]


@contextlib.contextmanager
def no_solver_warnings() -> Iterator[None]:
    """Reject a fit during which POT emitted any warning."""
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        yield
    if caught:
        messages = "; ".join(str(item.message) for item in caught)
        raise ContractError(f"POT emitted solver warnings: {messages}")
