# Task contract — EMB-GRAPH-PERM-01 graph smoothness permutation test

Status: complete

Date: 2026-08-25

Parent requirements: EMB-01, GSP-01, NUL-01B, SIG-01H, WS-31, WS-50, WS-62,
EMB-GRAPH-ENERGY-01.

## User outcome

`marklab bayes graph-smoothness-permutation-test` tests whether complete embedding vectors have
unusually low SIGNAL-normalized Dirichlet energy on an exact declared graph.

## Frozen behavior

- consume the EMB-GRAPH-ENERGY-01 node/edge contract plus one exact nonempty permutation stratum per
  node, 20–10,000 permutations, deterministic seed, Laplacian convention, and total component-edge
  work bound; every stratum requires at least two nodes;
- compute observed `GraphDirichletEnergy(graph, embeddings, SIGNAL)` and reuse the same graph digest,
  degrees, Laplacian, numerator, and globally centered signal denominator for every null draw;
- permute complete signal rows within declared strata while keeping graph nodes/edges fixed; never
  permute signal dimensions independently;
- spatial smoothness is the prespecified low-energy alternative. Return the inclusive plus-one
  lower-tail p-value `(1 + count(null <= observed)) / (B + 1)`, null mean/SD/range, exact inclusive
  count, seed, and permutation policy;
- this conditional random-label result is not graph-selection validation, patient-level inference,
  local hotspot discovery, stationarity proof, real-asset admission, or biology.

## Validation and claims

A six-node two-component monotone chain must reproduce observed SIGNAL energy `2/7`, a lattice-valued
inclusive lower-tail p-value consistent with its retained count, and byte-identical seeded reruns.
