# Task contract — SBI-SMC-ABC-01 staged SMC-ABC growth-front specialization

Status: complete

Date: 2026-08-25

Parent requirements: BAY-SBI, GEN-01, WS-73.

## User outcome

`marklab bayes smc-abc-growth-front` contracts a declared tolerance schedule over weighted
Fisher–KPP growth-rate particles and returns complete stage, importance, ESS, and simulation-work
evidence.

## Pseudocode ownership

This workflow owns the fixed-schedule uniform-prior Gaussian-perturbation growth-front specialization
of `SMC_ABC`. Adaptive-quantile schedules, multivariate kernels, general simulator/summary registries,
model discrepancy, and real calibration remain separate work.

## Frozen behavior

- require 1–32 strictly decreasing positive epsilons, 2–10,000 particles, a bounded uniform growth
  prior, positive mass scale, and at most 250 million declared cell-steps across all stage proposals;
- initialize from the prior, then sample weighted categorical ancestors and perturb by the prior
  stage's adapted Gaussian kernel, rejecting proposals outside prior support;
- call the canonical growth-front simulator, accept inclusive scaled mass distance, and fail
  explicitly when any stage cannot fill within its proposal limit;
- compute later-stage weights as prior density divided by the complete previous weighted Gaussian
  mixture density, normalize exactly, report ESS, and adapt the next kernel to `sqrt(2)` times the
  weighted posterior SD with a bounded positive floor;
- retain final particles, every stage's epsilon/attempt/acceptance/ESS/moments/next-kernel scale,
  total simulations and maximum declared work under a named ChaCha20 namespace;
- make synthetic-only, non-biological-calibration claims.

## Validation and claims

Three stages at epsilon `5,2,1` with 64 particles recover growth rate one within `0.03`, retain
normalized final weights within `1e-12`, keep every stage ESS above one, and replay byte-identically
from seed `20260825`.
