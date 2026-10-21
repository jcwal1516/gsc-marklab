"""Simplicial filtration and persistence helpers shared by the GUDHI alpha and witness workers."""

from __future__ import annotations

import math
from typing import Any

from .protocol import ContractError

Simplex = tuple[int, ...]


def oriented_boundary(simplex: Simplex) -> list[tuple[Simplex, int]]:
    """Faces of ``simplex`` with their alternating boundary signs."""
    return [
        (simplex[:index] + simplex[index + 1 :], 1 if index % 2 == 0 else -1)
        for index in range(len(simplex))
    ]


def sorted_filtration(tree: Any, maximum_dimension: int) -> list[tuple[Simplex, float]]:
    """Simplices up to ``maximum_dimension`` as sorted vertex tuples with their filtration values.

    The order is filtration value, then dimension, then lexicographic vertices.
    """
    raw = [
        (tuple(sorted(int(vertex) for vertex in simplex)), float(value))
        for simplex, value in tree.get_filtration()
        if len(simplex) - 1 <= maximum_dimension
    ]
    raw.sort(key=lambda item: (item[1], len(item[0]), item[0]))
    return raw


def filtration_simplices(
    raw: list[tuple[Simplex, float]],
    vertex_ids: list[Any],
    maximum_dimension: int,
    label: str,
) -> tuple[list[dict[str, Any]], list[int]]:
    """Describe each simplex with its boundary indices and count simplices by dimension.

    Rejects non-finite filtration values and faces that are absent, ordered later, or enter the
    filtration after their coface.
    """
    positions = {simplex: position for position, (simplex, _) in enumerate(raw)}
    counts = [0] * (maximum_dimension + 1)
    simplices = []
    for position, (simplex, value) in enumerate(raw):
        if not math.isfinite(value):
            raise ContractError(f"{label} filtration value is non-finite")
        boundary = []
        if len(simplex) > 1:
            for face, _ in oriented_boundary(simplex):
                face_position = positions.get(face)
                if (
                    face_position is None
                    or face_position >= position
                    or raw[face_position][1] > value
                ):
                    raise ContractError(
                        f"{label} face is absent, late, or has a larger filtration value"
                    )
                boundary.append(face_position)
        dimension = len(simplex) - 1
        counts[dimension] += 1
        simplices.append(
            {
                "vertex_ids": [vertex_ids[vertex] for vertex in simplex],
                "dimension": dimension,
                "filtration_alpha_squared": value,
                "boundary_indices": boundary,
            }
        )
    return simplices, counts


def boundary_of_boundary_residual(raw: list[tuple[Simplex, float]], prime: int) -> int:
    """Largest absolute coefficient of the boundary of a boundary over the field Z/prime."""
    maximum_residual = 0
    for simplex, _ in raw:
        if len(simplex) <= 2:
            continue
        coefficients: dict[Simplex, int] = {}
        for face, face_sign in oriented_boundary(simplex):
            for subface, subface_sign in oriented_boundary(face):
                total = coefficients.get(subface, 0) + face_sign * subface_sign
                coefficients[subface] = total % prime
        maximum_residual = max(maximum_residual, *(abs(value) for value in coefficients.values()))
    return maximum_residual


def persistence_by_dimension(
    tree: Any, coefficient_field: int, maximum_dimension: int
) -> dict[str, Any]:
    """Compute persistent cohomology and split each dimension into finite and essential classes.

    Essential classes are reported by birth only, so the result contains no JSON infinity.
    """
    tree.compute_persistence(
        homology_coeff_field=coefficient_field,
        min_persistence=-1.0,
        persistence_dim_max=True,
    )
    by_dimension = []
    for dimension in range(maximum_dimension + 1):
        finite_pairs = []
        essential_births = []
        for birth, death in tree.persistence_intervals_in_dimension(dimension):
            birth = float(birth)
            death = float(death)
            if math.isinf(death):
                essential_births.append(birth)
            elif death >= birth:
                finite_pairs.append({"birth": birth, "death": death})
            else:
                raise ContractError("persistence pair has death before birth")
        finite_pairs.sort(key=lambda pair: (pair["birth"], pair["death"]))
        essential_births.sort()
        by_dimension.append(
            {
                "dimension": dimension,
                "finite_pairs": finite_pairs,
                "essential_births": essential_births,
                "essential_count": len(essential_births),
            }
        )
    return {
        "algorithm": "gudhi_simplex_tree_persistent_cohomology",
        "essential_death_policy": "separate_birth_list_no_json_infinity",
        "by_dimension": by_dimension,
    }
