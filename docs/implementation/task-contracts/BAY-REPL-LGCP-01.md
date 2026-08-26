# Task contract — BAY-REPL-LGCP-01 replicated hierarchical LGCP construction

Status: complete

Date: 2026-08-25

Parent requirements: BAY-01, BAY-03, BAY-04, BAY-PP, BAY-HIER-A, WS-43,
BAY-LGCP-FIT-01.

## User outcome

`marklab bayes build-replicated-hierarchical-lgcp` builds a patient-nested replicated LGCP artifact
that preserves each pattern's exact window/grid/covariate identity.

## Frozen behavior

- consume 6–1,000 unique patterns nested under at least three patients with at least two patterns per
  patient; each row binds exact SHA-256 window/grid/covariate identities, positive event count, and
  4–36 cells; require positive global/patient/field/jitter prior scales;
- explicit field policy is `independent-replicate-fields` or `shared-plus-replicate`. Both share
  population Matérn hyperparameters; the latter adds one patient-shared field plus independent
  replicate fields with separately declared amplitudes;
- declare global fixed effects and patient random intercepts, exact per-pattern Cox likelihoods, and
  population/patient/replicate summaries. Never concatenate patterns or treat specimens as patients;
- construction only: no joint posterior, exchangeability proof, calibrated cross-patient contrast,
  biology, causality, or clinical claim.

## Validation and claims

A three-patient/two-pattern fixture under shared-plus-replicate policy must preserve all six distinct
window/grid identities, patient counts, separate shared/replicate fields, total events, and explicit
no-concatenation semantics.
