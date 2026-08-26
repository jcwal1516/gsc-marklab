# Task contract — EMB-PROJ-VARIO-01 split-safe projected embedding variograms

Status: complete

Date: 2026-08-25

Parent requirements: EMB-01, MRK-02D, SIG-01F, SIG-01H, WS-32, WS-50,
EMB-VARIO-01.

## User outcome

`marklab bayes projected-embedding-variograms` fits PCA only on declared training biological units,
freezes that transform, and computes component-by-distance variograms with max-T family control.

## Frozen behavior

- consume 8–10,000 unique finite objects with exact biological-unit, split, permutation-stratum,
  micrometre-coordinate, and 2–128 ordered `embedding_*` columns; each biological unit belongs to
  exactly one of `train`, `validation`, or `test`, and every split/stratum contains at least two rows;
- fit the mean and covariance only from `train` rows, use SciPy 1.18.1 symmetric eigendecomposition,
  retain 1–16 prespecified leading positive-variance components, and orient each eigenvector so its
  largest absolute loading is positive. Apply the frozen centered projection to every split;
- use the same exact contiguous distance-bin boundary policy as EMB-VARIO-01 and enforce a declared
  bound on all per-split unordered pair visits;
- compute one scalar semivariogram for every split × component × physical bin. Random-label complete
  projected vectors within split-specific permutation strata using a deterministic declared seed;
- studentize each observed component × nonempty-bin statistic against its permutation distribution
  and use each permutation's maximum absolute studentized deviation to return single-step max-T
  adjusted p-values over the complete component × scale family in that split. Degenerate null cells
  remain descriptive with an unavailable adjusted p-value;
- retain the complete projection artifact, training biological-unit IDs, split counts, backend/lock/
  worker identities, bins, seed, permutation count, and max-T family sizes. This is not patient-level
  group inference, feature selection, a fitted biological model, or real-asset validation.

## Validation and claims

A two-unit fixture with an extreme held-out second coordinate must retain the training-only center and
first-axis loading, produce identical train/test projected curves, and report the component × scale
max-T family without allowing the held-out rows to alter the projection artifact.
