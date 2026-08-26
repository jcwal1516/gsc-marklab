# Task contract — BAY-REPL-CLUSTER-01 replicated hierarchical cluster inference

Status: complete for the IC-0197 synthetic Thomas specialization

Date: 2026-08-25

Parent requirements: BAY-01, BAY-03, BAY-PP, BAY-SBI, WS-43, BAY-CLUSTER-FIT-01,
BAY-THOMAS-SIM-01, BAY-MATERN-CLUSTER-SIM-01.

## Required outcome and blocker

Fit patient/site-partially-pooled log kappa, mean offspring, and scale for replicated Thomas/Matérn
patterns while retaining per-pattern windows, latent-parent/boundary treatment, patient contrasts, and
calibration. BAY-CLUSTER-FIT-01 lacks an admitted variable-dimension latent-parent backend; BAY-SBI
lacks an admitted/calibrated summary/simulator inference lifecycle. The existing simulators alone do
not establish posterior inference. Resume after either backend is admitted with simulation recovery,
SBC/coverage, boundary sensitivity, and exact hierarchy semantics. Independent PPC may proceed.

## 2026-08-25 resolution

DEC-0213 supplies a label-invariant birth/death parent-count specialization and consumes its
per-pattern kappa, offspring, and scale summaries in explicit partial pooling across six independent
patients. Real-pattern calibration and general latent-parent inference remain promotion limits.
