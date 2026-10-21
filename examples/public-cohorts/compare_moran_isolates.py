#!/usr/bin/env python3
"""Exact isolated-row Moran/Geary oracle, with an optional PySAL comparison.

The graph is the binary symmetric radius-1.1 graph for positions [0, 1, 2, 3, 10]:
a four-cell path plus an isolate. The path ends have one neighbor and the inner cells
two, so row-standardized weights give different statistics from binary weights.
Run the independent package comparison in a disposable environment with:

    uv run --isolated --no-project --with esda==2.9.0 \
      --with libpysal==4.14.1 python compare_moran_isolates.py --esda
"""

from __future__ import annotations

import argparse
from fractions import Fraction


VALUES = tuple(map(Fraction, [1, 2, 4, 8, 16]))
DIRECTED_EDGES = ((0, 1), (1, 0), (1, 2), (2, 1), (2, 3), (3, 2))


def statistics(values: tuple[Fraction, ...]) -> tuple[Fraction, Fraction]:
    """Return canonical global Moran's I and Geary's C on the fixed graph."""
    count = len(values)
    mean = sum(values) / count
    centered = tuple(value - mean for value in values)
    denominator = sum(value * value for value in centered)
    weight_sum = Fraction(len(DIRECTED_EDGES))
    moran_numerator = sum(
        centered[left] * centered[right] for left, right in DIRECTED_EDGES
    )
    geary_numerator = sum(
        (values[left] - values[right]) ** 2 for left, right in DIRECTED_EDGES
    )
    moran = Fraction(count) * moran_numerator / (weight_sum * denominator)
    geary = (
        Fraction(count - 1) * geary_numerator / (2 * weight_sum * denominator)
    )
    return moran, geary


def compare_esda(expected: tuple[Fraction, Fraction]) -> None:
    """Require esda binary agreement and a distinct row-standardized result."""
    import numpy as np
    from esda import Geary, Moran
    from libpysal.weights import W

    values = np.asarray([float(value) for value in VALUES])
    neighbors = {0: [1], 1: [0, 2], 2: [1, 3], 3: [2], 4: []}
    for transformation in ["B", "R"]:
        weights = W(neighbors, silence_warnings=True)
        moran = float(Moran(values, weights, transformation=transformation, permutations=0).I)
        weights = W(neighbors, silence_warnings=True)
        geary = float(Geary(values, weights, transformation=transformation, permutations=0).C)
        errors = (abs(moran - float(expected[0])), abs(geary - float(expected[1])))
        if transformation == "B" and max(errors) >= 1e-15:
            raise ValueError(f"esda binary disagreement: I={moran}, C={geary}")
        if transformation == "R" and min(errors) < 1e-6:
            raise ValueError("this graph cannot distinguish row-standardized weights")
        print(f"esda {transformation}: I={moran:.17g}, C={geary:.17g}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--esda", action="store_true", help="also compare installed esda/libpysal")
    args = parser.parse_args()
    included = statistics(VALUES)
    dropped = statistics(VALUES[:4])
    print(f"include isolate: I={included[0]} ({float(included[0]):.17g}), "
          f"C={included[1]} ({float(included[1]):.17g})")
    print(f"drop isolate: I={dropped[0]} ({float(dropped[0]):.17g}), "
          f"C={dropped[1]} ({float(dropped[1]):.17g})")
    if args.esda:
        compare_esda(included)


if __name__ == "__main__":
    main()
