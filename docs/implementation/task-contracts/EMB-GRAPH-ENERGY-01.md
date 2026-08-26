# Task contract — EMB-GRAPH-ENERGY-01 graph Dirichlet energy

Status: complete

Date: 2026-08-25

Parent requirements: EMB-01, GSP-01, SIG-01H, WS-50, WS-62.

## User outcome

`marklab bayes graph-dirichlet-energy` computes exact scalar/vector graph roughness under an explicit
Laplacian and normalization convention.

## Frozen behavior

- consume 2–100,000 unique finite node rows with 1–128 ordered `signal_*` columns and 1–1,000,000
  unique unordered positive-weight edges resolving exact node IDs; the edge list is the canonical
  compact representation of a symmetric zero-diagonal `W`;
- for combinatorial `L=D-W`, compute `trace(X'LX)` as the compensated sum over unique edges of
  `weight * squared Euclidean signal difference`;
- for symmetric-normalized `L=I-D^-1/2 W D^-1/2`, require every node to have positive weighted degree
  and compute the equivalent unique-edge quadratic form after degree normalization;
- normalization `none` returns the numerator; `signal` divides by the globally feature-centered
  `trace(X_centered'X_centered)` and rejects zero signal variation; `edge_weight` divides by the full
  symmetric-matrix weight sum, exactly twice the unique-edge weight sum;
- retain exact counts/dimension, numerator/denominator/energy, graph digest, Laplacian convention,
  normalization, and a caller-declared component-edge work bound. This descriptive quantity has no
  null p-value, spatial-scale optimality, patient-level inference, or biology.

## Validation and claims

A three-node two-component weighted chain must produce combinatorial numerator `41`, centered signal
denominator `46/3`, and signal-normalized energy `123/46`.
