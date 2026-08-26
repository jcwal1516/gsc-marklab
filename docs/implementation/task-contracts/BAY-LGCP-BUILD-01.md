# Task contract — BAY-LGCP-BUILD-01 exact rectangle-grid LGCP model construction

Status: complete

Date: 2026-08-24

Parent requirements: BAY-01, BAY-04, BAY-PP, BAY-PP-A, WS-42, WS-43.

## User outcome

`marklab bayes build-gridded-lgcp` constructs a typed log-Gaussian Cox-process model artifact on an
exact complete rectangular cell grid without fitting or claiming continuous-window exactness.

## Frozen behavior

- reuse IC-0062 half-open micrometre rectangle/events and one complete 2–64-cell regular midpoint
  covariate/offset grid. Derive exact full-cell area and assign every event to exactly one cell;
- center-evaluated covariate/offset is the declared cell rule. Cell counts retain exact event total;
- latent field has zero-mean two-dimensional Euclidean Matérn-3/2 covariance at cell midpoints with
  caller positive amplitude/length/jitter. Add jitter only to the diagonal and require deterministic
  dense Cholesky positive definiteness under bounded work;
- typed model declares finite Normal intercept/coefficient prior means/positive SDs and
  `count_j ~ Poisson(cell_area*exp(intercept+beta*x_j+offset_j+z_j))`;
- report every cell, exact dense covariance/digest, model/window/grid/data identities, and resource
  limits. This is model construction only: no posterior, continuous interpolation, mesh, or fit.

## Validation and claims

A `2x2` rectangle grid with one event in the first and last cells must retain counts `[1,0,0,1]`,
unit cell area, four midpoint cells, a symmetric positive-definite 16-entry covariance whose
diagonal is `amplitude^2+jitter`, and the exact event total. This validates model construction only—
not LGCP fit, quadrature convergence, posterior prediction, biology, causality, or clinical evidence.
