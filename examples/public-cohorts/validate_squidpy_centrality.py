#!/usr/bin/env python3
"""Validate group centrality against Squidpy on one Schürch CODEX ROI."""

from __future__ import annotations

import argparse
from importlib.metadata import version
import json
import math
from pathlib import Path
import subprocess

import numpy as np

from validate_squidpy_cooccurrence import (
    EXPECTED_DIRECTED_PAIR_COUNTS,
    LEVELS,
    REPOSITORY,
    ROI,
    SCALE_UM_PER_PIXEL,
    SOURCE_DOI,
    SOURCE_LICENSE,
    new_external_directory,
    read_fixture,
    squared_distances,
    validate_source,
)


RADIUS_UM = 50.0
EXPECTED_DIRECTED_PAIR_COUNT = EXPECTED_DIRECTED_PAIR_COUNTS[2]
EXPECTED_UNDIRECTED_EDGE_COUNT = EXPECTED_DIRECTED_PAIR_COUNT // 2
EXPECTED_COMPONENT_COUNT = 1
CENTRALITY_ULPS = 512


def request_document(cells: list[dict]) -> dict:
    return {
        "nodes": [
            {
                "id": cell["id"],
                "coordinates_um": [cell["x_um"], cell["y_um"]],
                "group_id": cell["class_id"],
            }
            for cell in cells
        ],
        "radius_um": RADIUS_UM,
        "maximum_nodes": 2_000,
        "maximum_candidate_pairs": 200_000,
        "maximum_edges": 25_000,
        "maximum_group_distance_work": 1_000_000,
        "maximum_clustering_wedges": 1_300_000,
        "maximum_working_bytes": 64 * 1024 * 1024,
    }


def exact_radius_graph(coordinates: np.ndarray):
    from scipy.sparse import csr_matrix
    from scipy.sparse.csgraph import connected_components

    distances2 = squared_distances(coordinates)
    selected = distances2 <= RADIUS_UM * RADIUS_UM
    rows, columns = np.nonzero(selected)
    adjacency = csr_matrix(
        (np.ones(len(rows), dtype=np.uint8), (rows, columns)),
        shape=selected.shape,
    )
    component_count, _ = connected_components(adjacency, directed=False)
    directed_edges = int(adjacency.nnz)
    if (
        directed_edges != EXPECTED_DIRECTED_PAIR_COUNT
        or component_count != EXPECTED_COMPONENT_COUNT
        or not np.array_equal(selected, selected.T)
    ):
        raise ValueError("Schürch 50 um graph differs from its reviewed profile")
    return selected, adjacency, component_count


def independent_reference(
    selected: np.ndarray, adjacency, labels: np.ndarray
) -> list[dict]:
    from scipy.sparse.csgraph import shortest_path

    node_count = len(labels)
    local_clustering = np.zeros(node_count, dtype=np.float64)
    for node in range(node_count):
        neighbors = np.flatnonzero(selected[node])
        degree = len(neighbors)
        if degree < 2:
            continue
        twice_triangles = int(np.count_nonzero(selected[np.ix_(neighbors, neighbors)]))
        local_clustering[node] = twice_triangles / (degree * (degree - 1))

    rows = []
    for group_index, group_id in enumerate(LEVELS):
        members = np.flatnonzero(labels == group_index)
        outside = labels != group_index
        external = np.any(selected[members], axis=0) & outside
        distances = shortest_path(
            adjacency,
            directed=False,
            unweighted=True,
            indices=members,
        )
        minimum_distances = np.min(np.atleast_2d(distances), axis=0)
        if not np.all(np.isfinite(minimum_distances[outside])):
            raise ValueError("reviewed 50 um graph unexpectedly became disconnected")
        distance_sum = float(np.sum(minimum_distances[outside], dtype=np.float64))
        outside_count = int(np.count_nonzero(outside))
        rows.append(
            {
                "group_id": group_id,
                "node_count": int(len(members)),
                "external_neighbor_count": int(np.count_nonzero(external)),
                "degree_centrality": float(np.count_nonzero(external) / outside_count),
                "closeness_centrality": float(outside_count / distance_sum),
                "average_clustering": float(
                    np.mean(local_clustering[members], dtype=np.float64)
                ),
            }
        )
    return rows


def bound(expected: float) -> float:
    return CENTRALITY_ULPS * np.finfo(np.float64).eps * (1.0 + abs(expected))


def compare_rows(actual: list[dict], expected: list[dict]) -> tuple[float, float]:
    if len(actual) != len(expected):
        raise ValueError("centrality group count differs from the reference")
    maximum_error = 0.0
    maximum_bound = 0.0
    for observed, reference in zip(actual, expected, strict=True):
        for field in ["group_id", "node_count", "external_neighbor_count"]:
            if observed.get(field) != reference[field]:
                raise ValueError(f"centrality row differs at {field}")
        for field in [
            "degree_centrality",
            "closeness_centrality",
            "average_clustering",
        ]:
            value = observed.get(field)
            if not isinstance(value, (int, float)) or not math.isfinite(value):
                raise ValueError(f"centrality row has invalid {field}")
            tolerance = bound(reference[field])
            error = abs(float(value) - reference[field])
            maximum_error = max(maximum_error, error)
            maximum_bound = max(maximum_bound, tolerance)
            if error > tolerance:
                raise ValueError(f"centrality row differs at {field}")
    return maximum_error, maximum_bound


def squidpy_reference(cells: list[dict], adjacency, expected: list[dict]) -> dict:
    import anndata as ad
    import networkx as nx
    import pandas as pd
    import squidpy as sq

    observations = pd.DataFrame(
        {
            "cell_type": pd.Categorical(
                [cell["class_id"] for cell in cells], categories=LEVELS
            )
        },
        index=[cell["id"] for cell in cells],
    )
    adata = ad.AnnData(obs=observations)
    adata.obsp["spatial_connectivities"] = adjacency
    actual = sq.gr.centrality_scores(
        adata,
        cluster_key="cell_type",
        copy=True,
        n_jobs=1,
        show_progress_bar=False,
    )
    if list(actual.index) != LEVELS or set(actual.columns) != {
        "degree_centrality",
        "closeness_centrality",
        "average_clustering",
    }:
        raise ValueError("Squidpy returned an unexpected centrality layout")
    rows = []
    for reference in expected:
        group_id = reference["group_id"]
        rows.append(
            {
                "group_id": group_id,
                "node_count": reference["node_count"],
                "external_neighbor_count": reference["external_neighbor_count"],
                "degree_centrality": float(actual.loc[group_id, "degree_centrality"]),
                "closeness_centrality": float(
                    actual.loc[group_id, "closeness_centrality"]
                ),
                "average_clustering": float(
                    actual.loc[group_id, "average_clustering"]
                ),
            }
        )
    maximum_error, maximum_bound = compare_rows(rows, expected)
    return {
        "package": "squidpy",
        "version": version("squidpy"),
        "networkx_version": nx.__version__,
        "connectivity_key": "spatial_connectivities",
        "n_jobs": 1,
        "maximum_absolute_formula_error": maximum_error,
        "maximum_acceptance_bound": maximum_bound,
    }


def compare_native(native: dict, expected: list[dict]) -> tuple[float, float]:
    if (
        native.get("format") != "marklab.graph_group_centrality"
        or native.get("version") != 1
        or native.get("node_count") != 1_107
        or native.get("edge_count") != EXPECTED_UNDIRECTED_EDGE_COUNT
        or native.get("component_count") != EXPECTED_COMPONENT_COUNT
        or native.get("group_count") != len(LEVELS)
        or native.get("radius_um") != RADIUS_UM
    ):
        raise ValueError("native centrality result has an unexpected profile")
    return compare_rows(native.get("groups", []), expected)


def validate(
    source_directory: Path,
    output_directory: Path,
    binary: Path,
    *,
    reference_only: bool,
) -> None:
    source_file = validate_source(source_directory)
    output = new_external_directory(output_directory)
    cells, coordinates, labels = read_fixture(source_file)
    request = request_document(cells)
    request_path = output / "request.json"
    request_path.write_text(json.dumps(request, indent=2) + "\n", encoding="utf-8")

    selected, adjacency, component_count = exact_radius_graph(coordinates)
    expected = independent_reference(selected, adjacency, labels)
    (output / "independent-reference.json").write_text(
        json.dumps(
            {
                "radius_um": RADIUS_UM,
                "node_count": len(cells),
                "edge_count": EXPECTED_UNDIRECTED_EDGE_COUNT,
                "component_count": component_count,
                "groups": expected,
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    squidpy = squidpy_reference(cells, adjacency, expected)

    native_summary = {"status": "not_run_reference_only"}
    if not reference_only:
        binary = binary.resolve()
        if not binary.is_file():
            raise ValueError(f"Marklab binary does not exist: {binary}")
        native_path = output / "native-result.json"
        run = subprocess.run(
            [
                str(binary),
                "graph",
                "group-centrality",
                "--input",
                str(request_path),
                "--out",
                str(native_path),
            ],
            cwd=REPOSITORY,
            capture_output=True,
            text=True,
        )
        if run.returncode != 0:
            raise ValueError(f"native group centrality failed: {run.stderr.strip()}")
        native = json.loads(native_path.read_text(encoding="utf-8"))
        maximum_error, maximum_bound = compare_native(native, expected)
        native_summary = {
            "status": "passed",
            "maximum_absolute_formula_error": maximum_error,
            "maximum_acceptance_bound": maximum_bound,
        }

    summary = {
        "passed": native_summary["status"] == "passed",
        "reference_passed": True,
        "source": {
            "doi": SOURCE_DOI,
            "license": SOURCE_LICENSE,
            "roi": ROI,
            "calibration_um_per_pixel": SCALE_UM_PER_PIXEL,
            "cells": len(cells),
            "classes": len(LEVELS),
        },
        "graph": {
            "radius_um": RADIUS_UM,
            "undirected_edges": EXPECTED_UNDIRECTED_EDGE_COUNT,
            "components": component_count,
            "coordinate_dtype": str(coordinates.dtype),
        },
        "acceptance": (
            "all exact counts match and each score satisfies "
            "abs(actual-reference) <= 512*eps64*(1+abs(reference))"
        ),
        "squidpy": squidpy,
        "native": native_summary,
    }
    (output / "validation.json").write_text(
        json.dumps(summary, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps(summary, indent=2))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source_directory", type=Path)
    parser.add_argument("output_directory", type=Path)
    parser.add_argument("--binary", type=Path, default=REPOSITORY / "target/debug/marklab")
    parser.add_argument(
        "--reference-only",
        action="store_true",
        help="validate only the independent and Squidpy references",
    )
    args = parser.parse_args()
    validate(
        args.source_directory,
        args.output_directory,
        args.binary,
        reference_only=args.reference_only,
    )


if __name__ == "__main__":
    main()
