"""Exact-window request pieces and spatial posterior-predictive statistics.

Shared by the PyMC arbitrary-window LGCP adapter and the arbitrary-window IPP spatial
posterior-predictive check. Both describe an exact (multi)polygon window by weighted quadrature
nodes and compare observed and replicated node densities over fixed physical neighbor pairs.
"""

from __future__ import annotations

from typing import Any

import numpy as np

from .protocol import ContractError, integer, obj, sha256_digest

INPUT_DIGEST_KEYS = frozenset(
    {"events_digest", "event_membership_digest", "quadrature_digest", "window_logical_digest"}
)


def validate_input_identity(value: Any) -> dict[str, Any]:
    """Require the input block to hold exactly the four lowercase SHA-256 input digests."""
    identity = obj(value, INPUT_DIGEST_KEYS, "input")
    for name, digest in identity.items():
        sha256_digest(digest, f"input.{name}")
    return identity


def neighbor_pairs(raw_pairs: Any, node_count: int) -> list[tuple[int, int]]:
    """Parse a non-empty, strictly increasing list of ``[left, right]`` node pairs, left < right."""
    if not isinstance(raw_pairs, list) or not raw_pairs:
        raise ContractError("at least one physical neighbor pair is required")
    pairs = []
    previous = None
    for raw in raw_pairs:
        if not isinstance(raw, list) or len(raw) != 2:
            raise ContractError("physical neighbor pairs must contain two indices")
        pair = (
            integer(raw[0], "neighbor_pairs[].left", 0, node_count - 1),
            integer(raw[1], "neighbor_pairs[].right", 0, node_count - 1),
        )
        if pair[0] >= pair[1] or (previous is not None and pair <= previous):
            raise ContractError("physical neighbor pairs must be unique and canonical")
        pairs.append(pair)
        previous = pair
    return pairs


def density_statistics(
    counts: np.ndarray, weights: np.ndarray, pairs: list[tuple[int, int]]
) -> tuple[np.ndarray, np.ndarray]:
    """Return the variance of node densities and their mean absolute neighbor difference.

    Densities are counts per square micrometer (``counts / weights``) along the last axis.
    """
    density = counts / weights
    variance = np.var(density, axis=-1)
    differences = [np.abs(density[..., left] - density[..., right]) for left, right in pairs]
    contrast = np.mean(np.stack(differences), axis=0)
    return variance, contrast


def statistic_check(observed: float, replicated: np.ndarray) -> dict[str, float]:
    """Compare an observed statistic with its posterior-predictive replicates."""
    return {
        "observed": observed,
        "replicated_mean": float(replicated.mean()),
        "replicated_sd": float(replicated.std(ddof=1)),
        "probability_replicated_at_least_observed": float(np.mean(replicated >= observed)),
    }
