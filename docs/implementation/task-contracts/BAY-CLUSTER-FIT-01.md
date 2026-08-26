# Task contract — BAY-CLUSTER-FIT-01 latent-parent cluster inference

Status: bounded binary specialization complete in BAY-ADV-CLUSTER-01; general
variable-dimension inference remains backend-blocked

Date: 2026-08-25

Parent requirements: BAY-01, BAY-02, BAY-PP, BAY-PP-B, BACK-01, WS-43,
BAY-THOMAS-SIM-01, BAY-MATERN-CLUSTER-SIM-01.

## Required outcome

Fit a typed Thomas or Matérn latent-parent model with unknown parent pattern, child allocations,
boundary-unobserved offspring, parameter updates, and auditable birth/death/move inference.

## Blocking evidence

`BAY-ADV-CLUSTER-01` now supplies the consumed finite binary specialization: label-invariant latent
parent counts, exact finite-state exchange inference, and replicated partial pooling. The evidence
below continues to block only a general continuous-window, unknown-cardinality RJMCMC backend; it
does not block the declared function's bounded production owner.

- local R is 4.5.2 but has no installed `spatstat`, NIMBLE, INLA, or other admitted point-process/RJ
  package; only base `cluster` matches the package-name probe;
- the pinned Python-3.12 environment owns PyMC 6.3.0, NumPy/SciPy, and ArviZ, but no admitted
  variable-dimension reversible-jump backend; fixed-shape PyMC NUTS cannot represent changing parent
  cardinality;
- installing a new backend globally is prohibited, and inventing a native reversible-jump engine
  without backend admission, calibration, and reference evidence would violate the advanced-method
  backend mandate.

## Resume condition

Admit and pin a maintained variable-dimension point-process backend with license/security/environment
identity, or approve a separately validated native RJMCMC workstream with exact detailed-balance,
boundary, simulation-recovery, and cross-reference tests. Minimum-contrast fitting and other
independent point-process workflows may proceed meanwhile.
