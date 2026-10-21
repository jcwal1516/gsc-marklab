"""Compare `marklab numerics mapper-graph` with KeplerMapper on public cells.

Run from the repository root with a Python that has numpy, scikit-learn and kmapper:
    python -I tests/fixtures/mapper/compare_kmapper.py \
        /path/to/marklab /path/to/CRC_clusters_neighborhoods_markers.csv /path/to/new/output

Fixed before any comparison was run (2026-10-10):
* Data: Schurch et al. (2020, CC BY 4.0), the first three TMA A spots by file name, all cells.
  Positions are pixels x 0.377442 um.
* Features: arcsinh(intensity / 5) of CD8, CD4, CD68, CD163, cytokeratin, vimentin, Ki67 and
  CD45, each centred and divided by its population standard deviation within the spot.
* Lenses: (a) the first principal-component score of the features, signed so that the
  loading of largest magnitude is positive; (b) each cell's distance to the nearest tumor cell
  (0 for tumor cells).
* Covers: 10 intervals with overlap 0.3, and 15 intervals with overlap 0.5. Clustering joins
  cells within Euclidean feature distance 1.5; nodes sharing at least one cell are joined.
  12 cases.
* Reference: kmapper.KeplerMapper().map with kmapper.Cover(n_cubes, perc_overlap) and
  sklearn DBSCAN(eps=1.5, min_samples=1), whose clusters are the connected components of the
  same distance graph. Its node names count only non-empty cubes, so graphs are compared by
  membership, not by name.
* Criterion: identical multisets of node member sets and of edges (unordered pairs of member
  sets), and Marklab's shared-cell counts equal the reference intersections. Cells whose lens
  lies within 1e-9 of the lens range from a reference interval end are counted and reported
  before comparison; they do not relax the criterion.
"""

import collections
import json
import pathlib
import subprocess
import sys
import warnings

import kmapper
import numpy as np
import sklearn
from sklearn.cluster import DBSCAN

UM_PER_PIXEL = 0.377442
MARKERS = [
    "CD8 - cytotoxic T cells:Cyc_3_ch_2", "CD4 - T helper cells:Cyc_6_ch_3",
    "CD68 - macrophages:Cyc_18_ch_4", "CD163 - macrophages:Cyc_17_ch_3",
    "Cytokeratin - epithelia:Cyc_10_ch_2", "Vimentin - cytoplasm:Cyc_8_ch_2",
    "Ki67 - proliferation:Cyc_5_ch_4", "CD45 - hematopoietic cells:Cyc_4_ch_2",
]
COVERS = [(10, 0.3), (15, 0.5)]
THRESHOLD = 1.5
BOUNDARY = 1e-9


def read_spots(csv_path):
    import csv

    rows = collections.defaultdict(list)
    with open(csv_path, newline="") as handle:
        for row in csv.DictReader(handle):
            if row["TMA_AB"] == "A":
                rows[row["File Name"]].append(row)
    return [(spot, rows[spot]) for spot in sorted(rows)[:3]]


def lenses(rows, features):
    _, _, vt = np.linalg.svd(features, full_matrices=False)
    loading = vt[0] * np.sign(vt[0][np.argmax(np.abs(vt[0]))])
    positions = np.array([[float(r["X:X"]), float(r["Y:Y"])] for r in rows]) * UM_PER_PIXEL
    tumor = positions[[r["ClusterName"] == "tumor cells" for r in rows]]
    nearest = np.array([np.min(np.hypot(*(tumor - p).T)) for p in positions])
    return {"pc1": features @ loading, "tumor_distance": nearest}


def reference_graph(lens, features, intervals, overlap):
    mapper = kmapper.KeplerMapper(verbose=0)
    graph = mapper.map(lens.reshape(-1, 1), features,
                       cover=kmapper.Cover(n_cubes=intervals, perc_overlap=overlap),
                       clusterer=DBSCAN(eps=THRESHOLD, min_samples=1))
    nodes = {name: frozenset(members) for name, members in graph["nodes"].items()}
    edges = collections.Counter()
    for source, targets in graph["links"].items():
        for target in targets:
            pair = frozenset([nodes[source], nodes[target]])
            edges[pair] += 1
    cover = kmapper.Cover(n_cubes=intervals, perc_overlap=overlap)
    centers = cover.fit(np.c_[np.arange(len(lens)), lens])
    ends = np.concatenate([[c[0] - cover.radius_[0], c[0] + cover.radius_[0]]
                           for c in centers])
    spread = lens.max() - lens.min()
    near = int(np.sum(np.min(np.abs(lens[:, None] - ends[None, :]), axis=1)
                      <= BOUNDARY * spread))
    return collections.Counter(nodes.values()), edges, near


def main():
    if len(sys.argv) != 4:
        raise SystemExit("expected Marklab executable, Schurch CSV and a new output directory")
    executable = str(pathlib.Path(sys.argv[1]).resolve(strict=True))
    output = pathlib.Path(sys.argv[3])
    output.mkdir(parents=True, exist_ok=False)
    cases = []
    for spot, rows in read_spots(sys.argv[2]):
        raw = np.arcsinh(np.array([[float(r[m]) for m in MARKERS] for r in rows]) / 5.0)
        features = (raw - raw.mean(axis=0)) / raw.std(axis=0)
        for lens_name, lens in lenses(rows, features).items():
            for intervals, overlap in COVERS:
                stem = f"{spot}-{lens_name}-{intervals}-{overlap}"
                request = {
                    "format": "marklab.mapper_graph_request", "version": 1,
                    "lens_name": lens_name, "feature_names": MARKERS,
                    "cells": [{"id": str(index), "lens": float(value),
                               "features": [float(v) for v in features[index]]}
                              for index, value in enumerate(lens)],
                    "cover": {"intervals": intervals, "overlap_fraction": overlap},
                    "distance_threshold": THRESHOLD, "minimum_shared_cells": 1,
                    "limits": {"maximum_cells": 10000,
                               "maximum_distance_evaluations": 100_000_000},
                }
                input_path = output / f"{stem}-input.json"
                result_path = output / f"{stem}-result.json"
                input_path.write_text(json.dumps(request))
                subprocess.run([executable, "numerics", "mapper-graph", "--input",
                                str(input_path), "--out", str(result_path)], check=True)
                result = json.loads(result_path.read_text())
                members = {node["id"]: frozenset(int(m) for m in node["members"])
                           for node in result["nodes"]}
                marklab_nodes = collections.Counter(members.values())
                marklab_edges = collections.Counter(
                    frozenset([members[e["source"]], members[e["target"]]])
                    for e in result["edges"])
                shared_ok = all(
                    e["shared_cells"] == len(members[e["source"]] & members[e["target"]])
                    for e in result["edges"])
                with warnings.catch_warnings():
                    warnings.simplefilter("ignore")
                    nodes, edges, near = reference_graph(lens, features, intervals, overlap)
                passed = marklab_nodes == nodes and marklab_edges == edges and shared_ok
                cases.append({"case": stem, "cells": len(rows), "nodes": sum(nodes.values()),
                              "edges": sum(edges.values()),
                              "marklab_nodes": len(result["nodes"]),
                              "marklab_edges": len(result["edges"]),
                              "components": result["components"],
                              "cells_near_interval_ends": near, "passed": passed})
                print(stem, len(rows), sum(nodes.values()), sum(edges.values()),
                      result["components"], near, passed)
    summary = {"format": "marklab.mapper_reference_comparison", "version": 1,
               "versions": {"kmapper": kmapper.__version__, "scikit-learn": sklearn.__version__,
                            "numpy": np.__version__},
               "threshold": THRESHOLD, "cases": cases,
               "passed": all(case["passed"] for case in cases)}
    (output / "summary.json").write_text(json.dumps(summary, indent=2))
    print("passed", summary["passed"])
    if not summary["passed"]:
        raise SystemExit("Mapper comparison failed; see summary.json")


if __name__ == "__main__":
    main()
