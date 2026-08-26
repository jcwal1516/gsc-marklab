# Task contract — SIM-COMPETE-01 two-species spatial competition

Status: complete

Date: 2026-08-25

Parent requirements: GEN-01, WS-70, WS-71.

## User outcome

`marklab simulate spatial-competition` advances two spatial Lotka–Volterra density fields with
explicit diffusion, asymmetric competition, declared treatment mortality, and interpretable
coexistence/exclusion diagnostics.

## Frozen behavior

- consume two finite density fields on the same 3–100,000-point regular 1-D micrometre grid, finite
  nonnegative diffusion/growth/competition/treatment controls, positive carrying capacities/time,
  an extinction threshold, trajectory cadence, and bounded cell × species × step work;
- apply a symmetric reaction–diffusion split: exact self-logistic flows surround simultaneous
  exponential cross-competition/treatment loss, and the complete reaction flow surrounds centered
  no-flux explicit diffusion for both species;
- enforce each species' `D*dt/dx^2 <= 0.5`, density interval `[0,K]`, finite results, and a shared
  maximum of 250 million cell-species-steps;
- retain complete initial/final fields, mass/maximum trajectories, first downward threshold events,
  final coexistence/exclusion status, and solver CFL/range/positivity diagnostics;
- interpret treatment only as a caller-declared external mortality rate, never an estimated causal
  treatment effect.

## Validation and claims

With zero growth/diffusion/competition, treatment `ln(2)` for one time unit maps species A exactly
from one to `0.5` while species B stays `0.25`. With equal growth, competition coefficients `2` and
`0.25`, the symmetric `0.4/0.4` state drives species A below `0.1`, retains species B above `0.8`,
and records the A extinction-threshold event.
