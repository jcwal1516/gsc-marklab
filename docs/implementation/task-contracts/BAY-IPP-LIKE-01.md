# Task contract — BAY-IPP-LIKE-01 rectangular inhomogeneous Poisson likelihood

Status: complete

Date: 2026-08-24

Parent requirements: BAY-PP, BAY-PP-A, WS-22, WS-40, WS-43.

## User outcome

`marklab bayes inhomogeneous-poisson-likelihood` evaluates a fixed log-linear point-process
likelihood on an exact rectangular window using one complete regular midpoint quadrature grid.

## Frozen behavior

- caller supplies a finite positive-area half-open micrometre rectangle, 1–100,000 unique exact-ID
  event points inside it, and finite event covariate/offset values;
- quadrature declares 1–1,024 x/y cells and supplies exactly one finite covariate/offset row for
  every zero-based `(ix,iy)` cell. Node coordinates and equal positive cell weights are derived from
  the rectangle and grid, proving complete nonoverlapping midpoint coverage with weights summing to
  exact window area;
- for finite intercept and one finite coefficient, event term is the sum of
  `intercept + beta*covariate + offset`; integral is the stable sum of
  `cell_area*exp(intercept + beta*covariate + offset)` at every midpoint. Non-finite exponential or
  accumulation is an error, not saturation;
- report event term, integral, difference, exact event/node/window/grid identities, parameter
  interpretation, and bounded work. This is a fixed-parameter likelihood, not posterior fitting;
- no polygon approximation, hidden edge correction, interpolation, random window, pooled patterns,
  or implicit physical unit is admitted.

## Validation and claims

Two events in a `2 x 1 um` rectangle, a complete `2 x 1` zero-covariate/offset grid, intercept
`log(2)`, and coefficient zero must give event term `2*log(2)`, integral `4`, and log likelihood
`2*log(2)-4`. This validates quadrature/likelihood mechanics only—not fitted intensity, process fit,
biology, causality, or clinical evidence.
