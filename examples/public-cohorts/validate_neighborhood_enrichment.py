#!/usr/bin/env python3
"""Check native fixed-margin enrichment on the public Schürch CODEX ROI.

Counts are compared to Squidpy. Exact null moments use independent rational
arithmetic. Monte Carlo z-scores are descriptive and have no acceptance threshold.
"""

from __future__ import annotations

import argparse
from fractions import Fraction
from importlib.metadata import version
import json
import math
from pathlib import Path
import subprocess

import anndata as ad
import numpy as np
import pandas as pd
from scipy.sparse import csr_matrix
import squidpy as sq

import validate_squidpy_cooccurrence as fixture


# Fixed before the first native comparison. Earlier Monte Carlo explorations are
# not used as acceptance evidence for this new workflow.
RADIUS_UM = 25.0
PERMUTATIONS = 9_999
SEED = 42
SCALED_ERROR_LIMIT = 1e-12


def write_json(path: Path, value) -> None:
    with path.open("x", encoding="utf-8") as stream:
        json.dump(value, stream, indent=2, allow_nan=False)
        stream.write("\n")


def falling(n: int, order: int) -> int:
    return math.prod(range(n - order + 1, n + 1)) if n >= order else 0


def exact_moments(n: int, na: int, nb: int, same: bool, edges: int, wedges: int, disjoint: int):
    """Edge-indicator moments under uniform fixed-count label allocation."""
    def probability(numerator: int, order: int) -> Fraction:
        denominator = falling(n, order)
        return Fraction(numerator, denominator) if denominator else Fraction(0)

    if same:
        p = probability(falling(na, 2), 2)
        q = probability(falling(na, 3), 3)
        r = probability(falling(na, 4), 4)
    else:
        p = probability(2 * na * nb, 2)
        q = probability(na * falling(nb, 2) + nb * falling(na, 2), 3)
        r = probability(4 * falling(na, 2) * falling(nb, 2), 4)
    mean = edges * p
    variance = edges * p + 2 * wedges * q + 2 * disjoint * r - mean * mean
    if variance < 0:
        raise ValueError("exact graph null variance is negative")
    return mean, variance


def finite_or_none(value):
    value = float(value)
    return value if math.isfinite(value) else None


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("out", type=Path)
    parser.add_argument("--binary", type=Path, default=fixture.REPOSITORY / "target/debug/marklab")
    args = parser.parse_args()
    if version("squidpy") != "1.8.3":
        raise ValueError("this comparison requires Squidpy 1.8.3")
    source = fixture.validate_source(args.source)
    output = fixture.new_external_directory(args.out)
    cells, coordinates, labels = fixture.read_fixture(source)
    classes = fixture.LEVELS
    n, k = len(cells), len(classes)
    request = fixture.request_document(cells)
    request["format"] = "marklab.categorical_enrichment"
    del request["radii_um"]
    request.update(radius_um=RADIUS_UM, permutations=PERMUTATIONS, seed=SEED)
    request["limits"] = {
        "maximum_input_bytes": 2 * 1024 * 1024,
        "maximum_points": 2_000,
        "maximum_classes": 32,
        "maximum_edges": 10_000,
        "maximum_permutations": 10_000,
        "maximum_permutation_work": 100_000_000,
        "maximum_retained_bytes": 64 * 1024 * 1024,
    }
    write_json(output / "request.json", request)
    write_json(output / "design.json", {
        "source_doi": fixture.SOURCE_DOI, "source_license": fixture.SOURCE_LICENSE,
        "roi": fixture.ROI, "radius_um": RADIUS_UM, "permutations": PERMUTATIONS,
        "seed": SEED, "null": "one global stratum, fixed class counts",
        "count_acceptance": "exact", "analytic_scaled_error_limit": SCALED_ERROR_LIMIT,
        "monte_carlo_z_acceptance": "unassessed; descriptive only",
        "graph": "undirected Euclidean radius, inclusive boundary, no self loops",
    })

    adjacency = fixture.squared_distances(coordinates) <= RADIUS_UM**2
    left, right = np.nonzero(np.triu(adjacency, 1))
    edges = len(left)
    degrees = adjacency.sum(axis=1).astype(np.int64)
    wedges = sum(int(d) * (int(d) - 1) // 2 for d in degrees)
    disjoint = edges * (edges - 1) // 2 - wedges
    frequencies = np.bincount(labels, minlength=k)
    directed_counts = fixture.pair_counts(
        fixture.squared_distances(coordinates), labels, np.asarray([RADIUS_UM])
    )[0]
    if (edges, wedges, disjoint) != (5_833, 89_271, 16_919_757):
        raise ValueError("public graph differs from the declared fixture")

    observations = pd.DataFrame({
        "cell_type": pd.Categorical([cell["class_id"] for cell in cells], categories=classes)
    }, index=[cell["id"] for cell in cells])
    data = ad.AnnData(obs=observations)
    data.obsp["spatial_connectivities"] = csr_matrix(adjacency, dtype=np.uint8)
    matched = sq.gr.nhood_enrichment(
        data, "cell_type", n_perms=PERMUTATIONS, seed=SEED, copy=True,
        n_jobs=1, show_progress_bar=False,
    )
    # Retain the package's default permutation count, while fixing its random seed
    # and execution concurrency for a repeatable descriptive comparison.
    defaults = sq.gr.nhood_enrichment(
        data, "cell_type", seed=SEED, copy=True, n_jobs=1, show_progress_bar=False,
    )
    package_counts_passed = bool(
        np.array_equal(matched.counts, directed_counts)
        and np.array_equal(defaults.counts, directed_counts)
    )

    native_path = output / "native-result.json"
    command = [str(args.binary.resolve()), "numerics", "categorical-enrichment",
               "--input", str(output / "request.json"), "--out", str(native_path)]
    run = subprocess.run(command, capture_output=True, text=True, cwd=fixture.REPOSITORY)
    (output / "native.log").write_text(run.stdout + run.stderr, encoding="utf-8")
    run.check_returncode()
    native = json.loads(native_path.read_text(encoding="utf-8"))
    if (
        native.get("format") != "marklab.categorical_enrichment_result"
        or native.get("version") != 1 or native.get("point_count") != n
        or native.get("classes") != classes
        or native.get("class_counts") != frequencies.tolist()
        or native.get("undirected_edge_count") != edges
        or native.get("adjacent_edge_pair_count") != wedges
        or native.get("disjoint_edge_pair_count") != disjoint
        or native.get("radius_um") != RADIUS_UM
        or native.get("permutations") != PERMUTATIONS or native.get("seed") != SEED
        or len(native.get("pairs", [])) != k * (k + 1) // 2
    ):
        raise ValueError("native result differs from the declared graph/profile")
    native_pairs = {
        (row["monte_carlo"]["label_a"], row["monte_carlo"]["label_b"]): row
        for row in native["pairs"]
    }
    if len(native_pairs) != k * (k + 1) // 2:
        raise ValueError("native result repeats a class pair")
    comparisons = []
    maximum_error = 0.0
    maximum_mc_difference = 0.0
    for a in range(k):
        for b in range(a, k):
            row = native_pairs[(classes[a], classes[b])]
            mean, variance = exact_moments(
                n, int(frequencies[a]), int(frequencies[b]), a == b, edges, wedges, disjoint
            )
            observed = int(directed_counts[a, b]) // (2 if a == b else 1)
            z = (observed - float(mean)) / math.sqrt(float(variance)) if variance else None
            expected = {"analytic_null_mean": float(mean), "analytic_null_variance": float(variance),
                        "analytic_z_score": z}
            errors = {}
            for field, reference in expected.items():
                actual = row[field]
                if reference is None:
                    if actual is not None or row["analytic_z_score_unavailable_reason"] != "zero_null_variance":
                        raise ValueError("undefined analytic z-score was not reported as unavailable")
                else:
                    if not isinstance(actual, (float, int)) or not math.isfinite(actual):
                        raise ValueError("native analytic quantity is not finite")
                    errors[field] = abs(actual - reference) / max(1.0, abs(reference))
                    maximum_error = max(maximum_error, errors[field])
            counts_passed = row["monte_carlo"]["observed_edges"] == observed
            mc_z, sq_z = row["monte_carlo"]["z_score"], finite_or_none(matched.zscore[a, b])
            mc_difference = abs(mc_z - sq_z) if mc_z is not None and sq_z is not None else None
            if mc_difference is not None:
                maximum_mc_difference = max(maximum_mc_difference, mc_difference)
            comparisons.append({
                "class_a": classes[a], "class_b": classes[b], "observed_undirected": observed,
                "exact_mean_fraction": str(mean), "exact_variance_fraction": str(variance),
                **expected, "scaled_errors": errors, "counts_passed": counts_passed,
                "analytic_passed": all(error <= SCALED_ERROR_LIMIT for error in errors.values()),
                "native_monte_carlo_z": mc_z, "squidpy_monte_carlo_z": sq_z,
                "squidpy_default_permutation_count_z": finite_or_none(defaults.zscore[a, b]),
                "monte_carlo_absolute_z_difference": mc_difference,
            })
    summary = {
        "versions": {name: version(name) for name in ["squidpy", "numpy", "scipy", "anndata"]},
        "cells": n, "classes": k, "undirected_edges": edges,
        "package_observed_counts_passed": package_counts_passed,
        "analytic_scaled_error_limit": SCALED_ERROR_LIMIT,
        "maximum_analytic_scaled_error": maximum_error,
        "reference_agreement_passed": package_counts_passed and all(
            row["counts_passed"] and row["analytic_passed"] for row in comparisons
        ),
        "monte_carlo_z_agreement": "unassessed",
        "maximum_descriptive_monte_carlo_z_difference": maximum_mc_difference,
        "monte_carlo_conventions": {
            "native": "deterministic domain-separated shuffle; sample SD (ddof=1)",
            "squidpy": "package RNG; population SD (ddof=0)",
            "default_permutations": 1000, "matched_permutations": PERMUTATIONS,
        },
        "comparisons": comparisons,
    }
    write_json(output / "summary.json", summary)
    print(json.dumps({key: value for key, value in summary.items() if key != "comparisons"}, indent=2))
    if not summary["reference_agreement_passed"]:
        raise SystemExit("neighborhood enrichment failed its fixed count/analytic criteria")


if __name__ == "__main__":
    main()
