# Task contract — EMB-LOCAL-ROUGH-01 local embedding roughness map

Status: complete

Date: 2026-08-25

Parent requirements: EMB-01, GSP-01, SIG-01H, WS-50, WS-62,
EMB-GRAPH-ENERGY-01.

## User outcome

`marklab bayes local-embedding-roughness` emits a node-keyed descriptive artifact of weighted local
embedding disagreement on the exact declared graph.

## Frozen behavior

- consume the exact EMB-GRAPH-ENERGY-01 node/edge contract, a finite positive denominator epsilon,
  and a declared component-edge work bound;
- for every node accumulate each incident unique edge exactly once into
  `sum_j weight_ij * squared Euclidean signal difference`, divide by `max(weighted_degree_i, epsilon)`,
  and retain weighted degree, neighbor count, numerator, denominator, and local roughness;
- nodes with no incident positive-weight edge remain in the artifact with zero numerator/roughness,
  epsilon denominator, and typed `isolated_node` status; connected nodes use `available`;
- retain exact node order/feature names, graph digest, coordinate-free graph convention, epsilon, and
  work accounting. The map is experimental/descriptive: no p/q value, multiplicity-free hotspot label,
  spatial-scale optimality, patient inference, or biological interpretation is emitted.

## Validation and claims

A weighted three-node two-component chain plus one island must return local roughness values `1`,
`41/3`, `20`, and `0`, with the island explicitly unavailable for inferential interpretation.
