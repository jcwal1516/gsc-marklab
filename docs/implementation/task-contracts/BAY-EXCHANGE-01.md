# Task contract — BAY-EXCHANGE-01 exchange MCMC for Gibbs inference

Status: complete for the IC-0197 finite-state exact specialization

Date: 2026-08-25

Parent requirements: BAY-01, BAY-02, BAY-PP, BAY-PP-B, BACK-01, WS-43,
BAY-GIBBS-SIM-01.

## Required outcome

Run exchange MCMC for a typed Gibbs posterior with exact prior/proposal/density ratios, auxiliary
patterns from an exact or sufficiently controlled internal simulator, and retained inner-simulation
diagnostics/approximation status.

## Blocking evidence

- local R 4.5.2 has no admitted `spatstat`, perfect-simulation, or exchange-MCMC package;
- the pinned Python environment has no admitted Gibbs perfect/controlled auxiliary sampler;
- IC-0072 is explicitly a finite empty-start terminal chain without a general mixing guarantee, so
  using it as an exact auxiliary draw would invalidate the exchange ratio while hiding approximation;
- installing a backend globally is prohibited and no current contract supplies a calibrated inner
  chain error bound.

## Resume condition

Admit a maintained exact/perfect Gibbs simulator, or establish model-specific controlled inner-chain
accuracy with simulation recovery, sensitivity to inner length/start, and explicit approximate-state
semantics before exchange inference is implemented. Independent fixed-pattern Geyer/multitype
mechanics may proceed.

## 2026-08-25 resolution

DEC-0213 admits a six-site binary Gibbs specialization with all 64 states exactly enumerated at
every proposal, so auxiliary draws are exact rather than terminal-chain approximations. The general
continuous-space blocker remains outside the accepted specialization.
