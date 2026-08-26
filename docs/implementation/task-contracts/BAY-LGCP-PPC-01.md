# Task contract — BAY-LGCP-PPC-01 explicit gridded LGCP posterior prediction

Status: complete

Date: 2026-08-24

Parent requirements: BAY-01, BAY-02, BAY-PP, BAY-PP-A, WS-43, BAY-LGCP-FIT-01.

## User outcome

`marklab bayes simulate-gridded-lgcp-posterior-predictive` fits the exact bounded IC-0066 model and
materializes deterministic replicated point-pattern artifacts from exact posterior draws on its
declared piecewise-constant grid.

## Frozen behavior

- reuse the full IC-0066 input, fixed-kernel noncentered PyMC NUTS lifecycle, diagnostic gates, and
  exact covariance/data identities; only complete fits can yield a publishable predictive artifact;
- caller requests 1–32 replicated patterns, a deterministic prediction seed, and a positive total
  point cap no greater than 100,000. Each replicate selects one exact stored chain/draw by a declared
  deterministic schedule, samples independent Poisson cell counts, then samples uniform locations
  inside each full rectangle cell because the admitted discretized intensity is constant there;
- retain source chain/draw, fixed effects, every latent effect/intensity/expected count, cell counts,
  every exact point/cell identity and coordinate, totals, and approximation diagnostics. Any count,
  coordinate, identity, resource, support, fit-state, or request mismatch is an error;
- do not claim a certified continuous intensity bound, thinning, within-cell covariate interpolation,
  arbitrary-window exactness, inferred kernel hyperparameters, model truth, or calibrated spatial fit.

## Validation and claims

The IC-0066 `3x3` synthetic positive-effect fixture with eight replicas must produce a complete fit,
bounded patterns whose points are inside and assigned to their exact cells, cell counts summing to
pattern totals, nonconstant replicate totals, and byte-identical output on a repeated seeded run.
This validates deterministic discretized posterior prediction only—not continuous LGCP simulation,
posterior-predictive adequacy, biology, causality, or clinical use.
