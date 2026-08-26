# Task contract — BAY-LGCP-FIT-01 fitted dense gridded LGCP

Status: complete

Date: 2026-08-24

Parent requirements: BAY-01, BAY-02, BAY-04, BAY-PP-A, WS-42, WS-43,
BAY-LGCP-BUILD-01.

## User outcome

`marklab bayes fit-gridded-lgcp` fits the exact rectangle-cell LGCP model artifact with a bounded
noncentered dense Matérn latent field through pinned PyMC NUTS.

## Frozen behavior

- reuse IC-0065 exact 4–36 cell counts/midpoints/areas/covariates/offsets and fixed positive
  Matérn amplitude/physical length/jitter with deterministic Cholesky;
- caller supplies finite Normal intercept/coefficient means/positive SDs. PyMC samples independent
  standard-Normal field coordinates and maps them through the exact supplied Cholesky; cell counts
  follow the IC-0065 Poisson area-offset likelihood;
- bounded chains/tune/draws/target/seed/timeout and shared prior/posterior finiteness, rank R-hat,
  bulk/tail ESS, E-BFMI, divergence, depth, support, and identifiability gates apply;
- report fixed effects, every cell's latent effect/intensity/expected-count/Pearson-residual posterior,
  and posterior-predictive total/zero-cell counts. Retain exact model/data/covariance identities;
- complete remains experimental. No inferred kernel hyperparameters, continuous interpolation,
  within-cell locations, mesh/SPDE, quadrature convergence, or model-truth claim is admitted.

## Validation and claims

A `3x3` synthetic grid with x-covariate `[-1,0,1]` and counts `[2,5,12]` in each row must produce a
positive coefficient mean above `0.3`, nonconstant finite latent effects, positive cell intensities,
finite predictive totals, and complete diagnostics with zero divergences/depth hits. This validates
synthetic dense-field fitting only—not calibrated LGCP inference, biology, causality, or clinical use.
