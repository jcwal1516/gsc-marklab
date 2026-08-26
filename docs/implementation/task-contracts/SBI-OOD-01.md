# Task contract — SBI-OOD-01 simulation-bank KNN/conformal support detection

Status: complete

Date: 2026-08-25

Parent requirements: BAY-SBI, WS-73.

## User outcome

`marklab bayes simulation-ood` tests whether one observed summary vector lies within a declared
simulation bank using reference-fit standardization, exact KNN distance, and a held-out calibration
threshold/conformal p-value.

## Pseudocode ownership

This workflow owns the identity-encoder, KNN-distance plus conformal-calibration specialization of
`DetectSimulationOOD`. Learned encoders, density models, classifiers, conditional/local support,
and real biological simulation banks remain separate work.

## Frozen behavior

- consume 1–128 exact named summary features, 3–100,000 reference simulations, 4–100,000 disjoint
  calibration simulations, one separately identified observation, `k` within reference size, a
  strict interior calibration quantile, and at most 250 million feature-distance visits;
- fit feature means and population SDs only on reference simulations and reject every zero-variance
  feature;
- standardize reference, calibration, and observation with frozen reference parameters and score
  each non-reference vector by mean Euclidean distance to its exact `k` nearest references;
- freeze the threshold at nearest rank `ceil(q*n_calibration)`, flag only strict exceedance, and
  compute conformal p-value `(1 + # calibration scores >= observed)/(n+1)`;
- retain sorted calibration scores, threshold/rank, observed score/status, conformal p-value,
  encoder parameters, and exact distance work;
- describe simulation support only, never model validity or biological fidelity.

## Validation and claims

A `[10,10]` observation is out of support relative to a unit-square reference/calibration bank,
while the calibration-like `[0.2,0.2]` control is in support under `k=2` and quantile `0.75`.
