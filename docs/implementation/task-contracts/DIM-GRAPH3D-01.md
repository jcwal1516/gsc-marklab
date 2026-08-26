# Task contract — DIM-GRAPH3D-01 sparse physical 3-D graph

Status: complete

Date: 2026-08-25

Parent requirements: Part XI §98 `Build3DSpatialGraph`, DIM-01, WS-80, IC-0126.

`marklab spatial3d spatial-graph` consumes IC-0126's exact normalized cuboid/metric geometry plus a
nonnegative finite radial position-uncertainty bound for every point. The graph rule is physical
radius or undirected union-kNN. Distance basis is `nominal`, `possible` (`max(0,d-u_i-u_j)`), or
`guaranteed` (`d+u_i+u_j`). Edge weights are binary or Gaussian in the selected effective distance
with positive micrometre bandwidth. Radius/k/weight fields not used by the selected mode are errors.

Every unordered pair is evaluated once within caller/hard limits. kNN selection is per-node by
effective distance, then stable target ID, and the final graph is the undirected union. Edges are
canonicalized by endpoint ID and retain nominal/effective distance, combined uncertainty, and
weight. The SHA-256 digest covers a fixed version tag, metric/rule/basis/weight identity, normalized
point IDs/coordinate/uncertainty bits, and canonical edge fields. Adding direct `sha2 = 0.10.9` to
`marklab-spatial3d` reuses the existing locked dependency; no new registry version is introduced.

The hand oracle has points at x 0, 1, 3: radius 1.5 produces only A–B, while union-1NN produces A–B
and B–C. This is a bounded descriptive graph, not registration uncertainty inference or calibrated
biological adjacency.
