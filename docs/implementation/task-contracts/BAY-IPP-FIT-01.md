# Task contract — BAY-IPP-FIT-01 fitted rectangular inhomogeneous Poisson process

Status: complete

Date: 2026-08-24

Parent requirements: BAY-01, BAY-02, BAY-PP, WS-40, WS-43, BAY-IPP-LIKE-01.

## User outcome

`marklab bayes fit-inhomogeneous-poisson` infers one log-linear intensity surface with pinned PyMC
NUTS using the exact rectangular event/midpoint-grid likelihood from BAY-IPP-LIKE-01.

## Frozen behavior

- reuse IC-0062 half-open micrometre rectangle, exact events, complete regular midpoint quadrature,
  derived equal cell weights, finite covariate/offset, and bounded work. Fitting requires 20–100,000
  events, 4–4,096 grid cells, and materially varying quadrature covariate;
- `intercept ~ Normal(caller mean,sd)` and `coefficient ~ Normal(caller mean,sd)` with positive SDs.
  The point-process log likelihood is exactly the IC-0062 event sum minus complete-grid integral;
- pinned PyMC 6.3.0 NUTS uses bounded chains/tune/draws/target/seed/timeout and shared finite prior,
  rank R-hat, bulk/tail ESS, E-BFMI, divergence, and depth gates;
- report posterior intercept/coefficient, every cell's derived midpoint/covariate/offset/observed
  count/intensity/expected-count/Pearson-residual summaries, and grid-based posterior-predictive total
  and zero-cell counts. Cellwise constant intensity is the declared quadrature approximation;
- strict environment/worker/request/event/quadrature/window identities, event-to-cell assignment,
  observed totals, finite positive intensities, resource bounds, and complete/nonconverged claim state
  are retained. No arbitrary polygon, interpolation, causal effect, or point-process adequacy claim.

## Validation and claims

A four-cell synthetic increasing-covariate fixture with counts `[4,7,14,27]` must recover positive
coefficient mean above `0.5`, produce positive finite cell intensities and predictive totals, and
have complete diagnostics with zero divergences/depth hits. This validates the exact adapter and
synthetic recovery only—not quadrature convergence, real tissue intensity, biology, causality, or
clinical evidence.
