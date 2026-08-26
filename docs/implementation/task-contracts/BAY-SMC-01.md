# Task contract — BAY-SMC-01 annealed SMC normal-mean inference

Status: complete

Date: 2026-08-24

Parent requirements: BAY-01, BAY-02, WS-40.

## User outcome

`marklab bayes normal-mean-smc` runs annealed sequential Monte Carlo for the typed conjugate
normal-mean model and returns posterior particles, evidence, temperature/ESS paths, acceptance, and
complete resampling ancestry.

## Frozen behavior

- reuse the IC-0039 Normal prior/known-sigma likelihood with 1–100,000 finite observations;
- use pinned PyMC 6.3.0 IMH SMC with 100–10,000 particles per each of 2–4 independent deterministic
  chains, caller ESS target and mutation correlation threshold strictly in `(0,1)`;
- a source-bound audited kernel records every stage's beta, conditional ESS, acceptance rate, and
  exact systematic-resampling ancestor index for every particle. Beta must increase and end at one;
  ESS must remain positive and ancestry indices in bounds;
- report each chain's log marginal likelihood plus mean/SD, particle posterior summary/PPC, exact
  backend/lock/worker/request/data identities, bounded output/runtime, and `complete` only when all
  particle/stage/evidence/predictive checks pass. No NUTS R-hat, E-BFMI, or divergence fields are
  fabricated;
- SMC output is deterministic for the exact request/environment and remains experimental.

## Validation and claims

For observations `[1,2,3,4]`, prior `Normal(0,1)`, and known sigma one, posterior is
`Normal(2,sqrt(0.2))` and exact log evidence is `-9.48047308903574`. Two 1,000-particle chains must
agree with posterior moments/evidence within declared Monte Carlo tolerances, end at beta one, retain
complete bounded ancestry, and report a complete state. This is inference-engine validation, not
scientific, biological, causal, or clinical evidence.
