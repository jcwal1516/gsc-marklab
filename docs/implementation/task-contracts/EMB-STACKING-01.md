# Task contract — EMB-STACKING-01 patient-grouped predictive stacking

Status: complete

Date: 2026-08-25

Parent requirements: BAY-MM, EMB-PRED-01, WS-52, WS-53.

## User outcome

`marklab bayes predictive-stacking` optimizes simplex weights over patient-held-out model predictive
densities, forms patient mixture densities, and reports exact leave-one-patient sensitivity.

## Frozen behavior

- consume 8–500 unique patients, exact `patient` held-out-unit declarations, 2–16 unique `model_*`
  columns of finite grouped log predictive density, and bounded jackknife work;
- use pinned SciPy 1.18.1 SLSQP to maximize the sum of patient mixture log predictive densities under
  nonnegative sum-one weights; retain exact request/backend identities;
- return every patient's mixture log density and exact leave-one-patient-refit minimum/maximum weight
  for each model; Rust independently verifies the simplex, every log-sum-exp mixture, and objective;
- label weights as predictive optimization weights, never posterior model probabilities.

## Validation and claims

Eight patients split symmetrically between two models with densities `0.8/0.2` and `0.2/0.8` have the
hand-symmetric optimum `0.5/0.5`. This is bounded grouped predictive optimization, not model truth or
causal evidence.
