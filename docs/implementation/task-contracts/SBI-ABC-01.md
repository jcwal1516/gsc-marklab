# Task contract — SBI-ABC-01 bounded rejection ABC growth-front specialization

Status: complete

Date: 2026-08-25

Parent requirements: BAY-SBI, GEN-01, WS-73.

## User outcome

`marklab bayes rejection-abc-growth-front` performs deterministic bounded rejection ABC for the
Fisher–KPP growth-rate parameter and returns every accepted proposal, summary discrepancy, and
posterior diagnostic.

## Pseudocode ownership

This workflow owns the uniform-prior, scalar-summary Fisher–KPP specialization of `RejectionABC`.
General priors/simulators/summaries, adaptive epsilon, multivariate distances, model discrepancy,
and real biological calibration remain separate work.

## Frozen behavior

- consume one valid growth-front initial condition and fixed diffusion/carrying/time/front controls;
- draw a nonnegative growth rate from a declared finite uniform prior using namespace
  `rejection_abc_growth_front_v1_chacha20`;
- call the canonical growth-front simulator for every proposal and summarize only final total
  density mass;
- standardize the simulated-minus-observed mass by a caller-declared positive scale, accept on
  inclusive absolute distance `<= epsilon`, and fail explicitly if the requested accepted count is
  not reached by the proposal limit;
- require 1–10,000 accepted draws, at most one million proposals, per-proposal simulator work, and
  at most 250 million maximum declared cell-steps across proposals;
- retain proposal order, parameter, simulated summary, signed standardized residual, distance,
  acceptance rate, and sample posterior mean/SD/range; make synthetic-only claims.

## Validation and claims

Uniform density `0.25`, `K=1`, `T=ln(3)`, and observed final mass one recover true growth rate one
within `0.03` from thirty accepted draws under a bounded uniform `[0.5,1.5)` prior. Repeated seed
`20260825` produces byte-identical output and every retained distance is at most epsilon one.
