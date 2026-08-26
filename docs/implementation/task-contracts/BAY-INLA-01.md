# Task contract — BAY-INLA-01 nested Laplace Poisson-lognormal approximation

Status: complete

Date: 2026-08-24

Parent requirements: BAY-01, BAY-02, BAY-GMRF-A, WS-40.

## User outcome

`marklab bayes poisson-lognormal-inla` performs an INLA-style nested Laplace approximation for a
small latent Gaussian Poisson model and compares its marginals with pinned PyMC NUTS.

## Frozen behavior

- input has 3–16 exact-ID nonnegative integer counts with finite positive exposures;
- conditional on precision tau, independent latent log rates have Normal(caller mean, tau^-1),
  counts follow Poisson(exposure*exp(x_i)), and tau has caller Gamma(shape,rate) prior;
- integrate on an explicit evenly spaced log-tau grid of 21–201 points within finite increasing
  bounds. At every point, exact-gradient optimization finds all latent modes; diagonal negative
  Hessians must be positive. The Laplace ratio includes log-tau Jacobian and determinant terms;
- normalize grid density with trapezoidal quadrature, require finite positive mass and small declared
  endpoint mass, then integrate conditional Gaussian latent marginals and transformed tau moments;
- run a bounded pinned PyMC NUTS fit to the same model and report method-specific diagnostics plus
  latent-mean RMSE. INLA output is always `approximate_only` when grid/optimizer/HMC comparison gates
  pass, otherwise `nonconverged`; it is never exact posterior;
- strict backend/lock/worker/request/data identities, grid/mode/Hessian diagnostics, resource bounds,
  and finite posterior/PPC summaries are retained.

## Validation and claims

A five-region heterogeneous count fixture on 81 log-tau points must normalize weights to one,
retain low endpoint mass, produce positive tau and finite latent marginals, agree with NUTS latent
means within RMSE `0.15`, and have zero NUTS divergences/depth hits. This validates nested
approximation mechanics only—not exact inference, spatial dependence, epidemiology, biology,
causality, or clinical evidence.
