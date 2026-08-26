# Task contract — BAY-SAR-LIKE-01 fixed-parameter Gaussian SAR likelihood

Status: complete

Date: 2026-08-24

Parent requirements: BAY-05, BAY-GMRF-A, WS-42.

## User outcome

`marklab bayes sar-likelihood` consumes exact row-standardized spatial weights, a complete response
and design table, declared coefficients, rho, and sigma, then evaluates the Gaussian spatial-lag or
spatial-error log likelihood including the required log absolute determinant.

## Frozen behavior

- weights use IC-0047 `not-required` symmetry, zero diagonal, row standardization, no islands, and
  at most 512 regions/200,000 edges;
- data bind every region exactly once and contain finite `y` plus 1–32 exact predictor columns;
  coefficients contain exactly one finite intercept and one finite value per predictor;
- `A=I-rho*W` must be nonsingular by deterministic partial-pivot LU. Lag residual is
  `A*y-X*beta`; error residual is `A*(y-X*beta)`;
- log likelihood is `log|det(A)| - n*log(sigma) - n/2*log(2*pi) - RSS/(2*sigma^2)`;
- lag mode under the required declared `descriptive` interpretation reports direct, indirect, and
  total effects from `beta_k*A^-1`; error mode reports no spillover impacts. No causal wording is
  admitted by version one.

## Validation and claims

For two mutually adjacent regions, rho `0.25`, sigma `1.2`, response `[1,2]`, design `x=[0,1]`,
intercept `0.5`, and slope `0.75`, lag log likelihood must be `-2.353864256690381`, with slope
direct/indirect/total effects `0.8/0.2/1.0`. Error log likelihood must be
`-2.4366008018292704`. This is a fixed-parameter experimental likelihood diagnostic, not a fit,
posterior, causal estimate, biological interaction, or clinical evidence.
