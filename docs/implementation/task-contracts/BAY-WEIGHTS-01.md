# Task contract — BAY-WEIGHTS-01 spatial-weights validation

Status: complete

Date: 2026-08-24

Parent requirements: BAY-05, BAY-GMRF-A, FND-04, WS-42.

## User outcome

`marklab bayes validate-weights` validates an explicit region inventory and sparse directed weight
table, identifies connected components/islands, applies the declared preservation or row
standardization policy, and publishes a deterministic semantic digest for downstream CAR/SAR/GMRF.

## Frozen behavior

- region IDs are exact, unique, non-empty, and define matrix order after lexical sorting;
- edge rows are unique directed `(source,target)` pairs with finite nonnegative weights. Version one
  rejects signed weights and zero stored edges;
- diagonal policy is `zero` or `allow`; symmetry is `required` or `not-required`; normalization is
  `preserve` or `row-standardize`. Required symmetry compares exact reciprocal weights;
- weak undirected connectivity identifies components. A region with no positive incident edge is
  an island. Row-standardization divides each positive outgoing row by its sum and leaves islands
  zero;
- canonical SHA-256 binds version, sorted region IDs, policies, and sorted normalized directed
  weights using exact `f64` bits.

Version one accepts 1–100,000 regions and at most 2,000,000 edges under checked memory/work bounds.
It is a graph/weights contract, not a fitted spatial model.

## Validation and claims

A four-region symmetric chain plus one island must produce two components, the exact island ID,
unit standardized outgoing sums for connected rows, zero island sum, and byte-deterministic digest.

## Delivered evidence

- `validate_spatial_weights` owns exact sorted regions, directed positive edges, diagonal/symmetry/
  normalization policy, weak components/islands, normalized sparse rows, and a domain-separated
  SHA-256 over exact normalized `f64` bits. `marklab bayes validate-weights` is the immediate caller.
- Package and two-run CLI tests pass for the four-region chain/island fixture with two components,
  island `d`, connected row sums exactly 1, zero island row, and byte-identical JSON/digest.
