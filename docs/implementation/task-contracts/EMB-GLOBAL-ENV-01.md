# Task contract — EMB-GLOBAL-ENV-01 complete-vector spatial-dependence global envelope

Status: complete

Date: 2026-08-25

Parent requirements: EMB-01, INF-01B, NUL-01B, SIG-01H, WS-31, WS-50,
EMB-VARIO-01.

## User outcome

`marklab bayes embedding-spatial-dependence-envelope` tests the complete vector semivariogram curve
with deterministic within-stratum random labeling and a simultaneous ERL envelope.

## Frozen behavior

- consume 8–10,000 unique finite micrometre-coordinate objects with exact nonempty permutation strata,
  2–128 ordered `embedding_*` dimensions, contiguous physical bins, 20–10,000 permutations, alpha,
  deterministic seed, and a bound on every observed/permuted unordered pair visit;
- compute the observed omnibus vector semivariogram and, for every null draw, permute complete vector
  rows within declared strata while leaving coordinates, feature order, and each vector's dimensions
  intact. Never permute dimensions independently;
- include each nonempty physical bin in one two-sided extreme-rank-length family using average ranks,
  sorted pointwise extreme-rank vectors, normalized ERL depths, plus-one global p-value, and the
  depth-threshold simultaneous envelope; require `(B + 1) * alpha >= 1`;
- retain empty bins as typed descriptive unavailable rows outside the ERL family, exact pair counts,
  lower/upper simultaneous bounds, observed/critical depth, p-value, seed, alpha, and complete-vector
  permutation policy;
- this exact random-label test conditions on observed coordinates and declared strata. It is not CSR,
  edge-corrected inference, patient-level comparison, proof of stationarity, real-asset admission, or
  biological evidence.

## Validation and claims

An eight-object two-stratum fixture must conserve all 28 unordered pairs across three bins, produce a
finite ERL envelope and lattice-valued global p-value, and publish byte-identical results on a seeded
rerun while explicitly retaining complete-row permutation semantics.
