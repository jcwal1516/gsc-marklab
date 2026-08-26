# Task contract — BAY-RESOURCE-01 hierarchical distance-to-resource response

Status: complete

Date: 2026-08-25

Parent requirements: GEN-01, BAY-03, WS-71.

## User outcome

`marklab bayes distance-to-resource` computes exact unsigned distances from cell coordinates to
declared resource segments and fits a patient-hierarchical nonlinear Gaussian response with
compartment, resource-density, and accessibility adjustment.

## Pseudocode ownership

This workflow owns the unsigned line-segment and known-noise exact-conjugate specialization of
`FitDistanceToResourceModel`. Signed compartment distances, unknown residual/between-patient scales,
GP responses, non-Gaussian outcomes, vessel-network accessibility, and real-cohort calibration
remain separate work.

## Frozen behavior

- consume 1–1,000 unique finite nondegenerate resource segments and 8–5,000 unique finite cell
  observations from at least three patients with two observations each;
- compute exact Euclidean point-to-segment distance and deterministically break equal-distance ties
  by sorted resource ID;
- construct a caller-prespecified linear hinge spline with at most 16 positive unique knots,
  sorted compartment indicators, resource-density/accessibility covariates, and one zero-mean
  patient random intercept per sorted patient;
- use proper zero-mean Normal priors with declared fixed scales and a known positive Gaussian noise
  scale, then solve the joint posterior exactly by bounded Cholesky algebra with at most 128 total
  coefficients and more observations than coefficients;
- retain fixed/patient marginal posterior summaries, a reference-compartment population distance
  curve, per-cell predictions, and posterior predictive residual checks across patients and nearest
  resources;
- report association only, never transport, resource causality, vascular physiology, or patient
  decision support.

## Validation and claims

A four-patient, twenty-cell vertical-segment fixture recovers every exact integer distance, a true
linear distance coefficient two and post-two-micrometre hinge coefficient three within `0.1`,
achieves posterior predictive RMSE below `0.05`, and reports all four patients plus the one nearest
resource in its predictive checks.
