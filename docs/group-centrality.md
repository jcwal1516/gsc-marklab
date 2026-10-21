# Group centrality on a physical-radius graph

**Experimental** for the three descriptive group metrics under the graph conventions below.

Marklab's group-centrality workflow summarizes a categorical group on one
unweighted cell graph. Two cells are adjacent when their Euclidean distance in
micrometers is at most the declared radius. Node IDs are canonicalized before
the graph is constructed, and group IDs remain exact, case-sensitive strings.

For a group of nodes \(S\) in a graph with node set \(V\), the workflow reports:

- **Degree centrality:** the number of distinct nodes outside \(S\) adjacent to
  at least one member of \(S\), divided by \(|V-S|\).
- **Closeness centrality:** \(|V-S|\) divided by the sum, over nodes outside
  \(S\), of the minimum unweighted distance from any member of \(S\).
- **Average clustering:** the arithmetic mean of the unweighted local
  clustering coefficients for nodes in \(S\). Nodes of degree zero or one have
  local clustering zero and remain in the average.

These definitions match the functions used by Squidpy 1.8.3:
[`squidpy.gr.centrality_scores`](https://squidpy.readthedocs.io/en/stable/api/squidpy.gr.centrality_scores.html),
which delegates to NetworkX
[`group_degree_centrality`](https://networkx.org/documentation/stable/reference/algorithms/generated/networkx.algorithms.centrality.group_degree_centrality.html),
[`group_closeness_centrality`](https://networkx.org/documentation/stable/reference/algorithms/generated/networkx.algorithms.centrality.group_closeness_centrality.html),
and
[`average_clustering`](https://networkx.org/documentation/stable/reference/algorithms/generated/networkx.algorithms.cluster.average_clustering.html).
The package implementation is visible in the
[Squidpy source](https://squidpy.readthedocs.io/en/stable/_modules/squidpy/gr/_nhood.html).

## Disconnected graphs and finite results

NetworkX group closeness has a specific disconnected-graph convention. An
unreachable node outside the group remains in the numerator \(|V-S|\) but adds
zero to the distance sum. If the complete distance sum is zero, closeness is
zero. This differs from component-normalized node closeness and from harmonic
centrality. Marklab preserves the NetworkX behavior so that a supplied graph has
the same interpretation in Squidpy and Marklab. The resulting closeness can
exceed one on disconnected graphs and should not be read as a probability.

The request must contain at least two nonempty groups. Empty group IDs, group
IDs containing control characters, and a single all-node group are rejected
before division. Coordinates, the radius, and every returned score must be
finite. These rules make all successful result rows fully numeric; no
unavailable or nonfinite sentinel is emitted. The radius graph must contain at
least one edge; a graph consisting entirely of isolates is rejected.

## Bounded work

The caller declares limits for nodes, radius-grid candidate pairs, retained
edges, group-distance traversal work, clustering wedges, and working memory.
The memory bound includes graph assembly, the node-to-group association,
sparse adjacency, traversal scratch space, and compact result rows. The
workflow stops with an error when any declared bound is insufficient.

The result is compact. It records graph and resource counts plus one row per
group; it does not retain the input nodes, edge list, shortest-path matrix, or
per-node clustering coefficients.

## Public Squidpy comparison

[`validate_squidpy_centrality.py`](../examples/public-cohorts/validate_squidpy_centrality.py)
selects the published Schürch CODEX ROI `reg001_A` under the same source checks
as the categorical co-occurrence validation. It uses the already declared 50
µm radius, where the 1,107 cells form one connected graph with 21,418
undirected edges. The script independently constructs the radius graph,
computes all three metrics, supplies the identical symmetric adjacency matrix
to Squidpy 1.8.3, and compares the package and native results with a fixed
floating-point bound. The connected public graph avoids making the unusual
disconnected-closeness convention part of the cohort result; a focused graph
test covers that convention directly.

The native run passed exact graph/count checks and all 19 group rows. Native degree, closeness and
average-clustering scores differed from the independent calculation by at most
`3.3306690738754696e-16`; Squidpy/NetworkX differed by at most
`1.1102230246251565e-16`. The fixed per-score bound was
`512 * eps64 * (1 + abs(reference))` (largest bound `2.245411608001114e-13`).
This observed field does not establish a group-comparison test or tissue-cohort biomarker.

After building the CLI with the repository toolchain and preparing the
preserved Schürch source directory, run:

```bash
target/spatial-reference-venv/bin/python \
  examples/public-cohorts/validate_squidpy_centrality.py \
  /path/to/preserved-schurch-source \
  /new/external/validation-directory \
  --binary target/debug/marklab
```

The output directory is required to be new and outside the checkout. It
contains the native request, independent reference, native result, and a
machine-readable validation summary.
