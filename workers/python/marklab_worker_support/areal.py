"""Request validation and result assembly shared by the region-level PyMC workers.

The BYM and BYM2 Poisson disease-mapping workers share their region, ICAR-component, count,
and design validation and their result layout; the SAR worker shares the region, predictor, and
design checks; the BYM, BYM2, SAR, and spatially varying coefficient workers share one resource
block.
"""

from __future__ import annotations

from typing import Any

import numpy as np

from .protocol import MIB, ContractError, integer, number, obj

RESOURCE_KEYS = frozenset(
    {"maximum_observations", "maximum_total_iterations", "maximum_output_bytes", "timeout_seconds"}
)


def finite_vector(value: Any, length: int, path: str) -> np.ndarray:
    """Require a list of exactly ``length`` finite numbers; return it as float64."""
    if not isinstance(value, list) or len(value) != length:
        raise ContractError(f"{path} must contain exactly {length} entries")
    return np.asarray([number(item, f"{path}[]") for item in value], dtype=np.float64)


def region_ids(value: Any) -> list[str]:
    """Require 6-64 non-empty, whitespace-trimmed, strictly increasing region IDs."""
    if not isinstance(value, list) or not 6 <= len(value) <= 64:
        raise ContractError("region IDs must contain 6-64 entries")
    if any(
        not isinstance(region, str)
        or not region
        or region.strip() != region
        or (index > 0 and value[index - 1] >= region)
        for index, region in enumerate(value)
    ):
        raise ContractError("region IDs must be exact and strictly increasing")
    return value


def predictor_names(value: Any) -> list[str]:
    """Require 1-16 distinct, trimmed predictor names, none called ``intercept``."""
    if not isinstance(value, list) or not 1 <= len(value) <= 16:
        raise ContractError("predictor names must contain 1-16 entries")
    if any(
        not isinstance(name, str)
        or not name
        or name.strip() != name
        or name == "intercept"
        or name in value[:index]
        for index, name in enumerate(value)
    ):
        raise ContractError("predictor names are invalid")
    return value


def weights_digest(value: Any) -> str:
    """Require a 64-character hexadecimal digest of the spatial weights (either case)."""
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value.lower())
    ):
        raise ContractError("weights digest is invalid")
    return value


def design_matrix(value: Any, dimension: int, predictors: int) -> np.ndarray:
    """Require a row-major design whose columns, with an intercept, have full rank."""
    design = finite_vector(value, dimension * predictors, "design").reshape(dimension, predictors)
    if np.linalg.matrix_rank(np.column_stack([np.ones(dimension), design])) != predictors + 1:
        raise ContractError("design including intercept must have full column rank")
    return design


def dense_resources(value: Any, path: str, observations: int, iterations: int) -> int:
    """Validate the resource block of the small dense models; return the output byte limit.

    The models accept at most 64 observations and 400000 total NUTS iterations.
    """
    resources = obj(value, RESOURCE_KEYS, path)
    if observations > integer(resources["maximum_observations"], f"{path}.observations", 1, 64):
        raise ContractError("observation count exceeds resource limit")
    maximum_iterations = integer(
        resources["maximum_total_iterations"], f"{path}.iterations", 1, 400_000
    )
    output_limit = integer(resources["maximum_output_bytes"], f"{path}.output", 1, MIB)
    integer(resources["timeout_seconds"], f"{path}.timeout", 1, 3_600)
    if iterations > maximum_iterations:
        raise ContractError("iterations exceed resource limit")
    return output_limit


def bym_regions(request: dict[str, Any], transform_key: str) -> dict[str, Any]:
    """Validate the regions, ICAR components and transform, counts, and design of a BYM request.

    ``request[transform_key]`` is a row-major ``region x (region - rank deficiency)`` basis whose
    columns sum to zero within every connected component, so ``transform @ z`` is an ICAR field
    satisfying the per-component sum-to-zero constraints exactly.
    """
    regions = region_ids(request["region_ids"])
    dimension = len(regions)
    rank_deficiency = integer(request["rank_deficiency"], "rank_deficiency", 1, dimension - 1)
    components = request["components"]
    if not isinstance(components, list) or len(components) != rank_deficiency:
        raise ContractError("component count must equal rank deficiency")
    covered: set[int] = set()
    parsed_components: list[list[int]] = []
    for component_index, component in enumerate(components):
        if not isinstance(component, list) or len(component) < 2:
            raise ContractError("each component requires at least two regions")
        parsed = [
            integer(value, f"components[{component_index}][]", 0, dimension - 1)
            for value in component
        ]
        if any(value in covered for value in parsed) or len(set(parsed)) != len(parsed):
            raise ContractError("components overlap or repeat regions")
        covered.update(parsed)
        parsed_components.append(parsed)
    if covered != set(range(dimension)):
        raise ContractError("components must cover every region")
    constrained_dimension = dimension - rank_deficiency
    transform = finite_vector(
        request[transform_key], dimension * constrained_dimension, transform_key
    ).reshape(dimension, constrained_dimension)
    if any(
        abs(transform[component, :].sum(axis=0)).max() > 1e-10 for component in parsed_components
    ):
        raise ContractError("ICAR transform violates component sum-to-zero")
    weights_digest(request["weights_digest_sha256"])

    counts_raw = request["counts"]
    if not isinstance(counts_raw, list) or len(counts_raw) != dimension:
        raise ContractError("count vector length mismatch")
    counts = np.asarray(
        [integer(value, "counts[]", 0, 2**63 - 1) for value in counts_raw], dtype=np.int64
    )
    expected = finite_vector(request["expected"], dimension, "expected")
    if np.any(expected <= 0.0):
        raise ContractError("expected counts must be positive")
    predictors = predictor_names(request["predictor_names"])
    design = design_matrix(request["design"], dimension, len(predictors))
    return {
        "region_ids": regions,
        "predictor_names": predictors,
        "transform": transform,
        "components": parsed_components,
        "counts": counts,
        "expected": expected,
        "design": design,
    }


def bym_posterior_checks(
    prior: Any,
    posterior: Any,
    predictive: Any,
    monitored: list[str],
    derived: list[str],
) -> dict[str, Any]:
    """Collect posterior draws and finiteness checks of a Poisson BYM-type fit.

    Returns float64 draws for ``monitored + derived``, the replicated counts, and whether the
    prior (monitored variables and prior predictive counts) and posterior draws are finite.
    """
    arrays = {
        name: np.asarray(posterior["posterior"][name].values, dtype=np.float64)
        for name in monitored + derived
    }
    replicated = np.asarray(
        predictive["posterior_predictive"]["observed_count"].values, dtype=np.float64
    )
    prior_finite = bool(
        all(np.isfinite(prior["prior"][name].values).all() for name in monitored)
        and np.isfinite(prior["prior_predictive"]["observed_count"].values).all()
    )
    posterior_finite = bool(
        all(np.isfinite(value).all() for value in arrays.values()) and np.isfinite(replicated).all()
    )
    return {
        "arrays": arrays,
        "replicated": replicated,
        "prior_finite": prior_finite,
        "posterior_finite": posterior_finite,
    }


def component_sums_vanish(flat_field: np.ndarray, components: list[list[int]]) -> bool:
    """Whether every draw of a ``(draw, region)`` field sums to zero within each component."""
    return all(
        np.abs(flat_field[:, component].sum(axis=1)).max() <= 1e-8 for component in components
    )


def count_predictive_check(counts: np.ndarray, replicated: np.ndarray) -> dict[str, Any]:
    """Compare observed and replicated total counts and numbers of zero-count regions."""
    replicated_totals = replicated.sum(axis=-1)
    replicated_zeros = (replicated == 0).sum(axis=-1)
    return {
        "observed_total_count": int(counts.sum()),
        "replicated_total_mean": float(replicated_totals.mean()),
        "replicated_total_sd": float(replicated_totals.std(ddof=1)),
        "observed_zero_regions": int((counts == 0).sum()),
        "replicated_zero_regions_mean": float(replicated_zeros.mean()),
    }
