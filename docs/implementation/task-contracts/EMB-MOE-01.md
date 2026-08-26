# Task contract — EMB-MOE-01 context-gated calibrated mixture of experts

Status: complete

Date: 2026-08-25

Parent requirements: BAY-MM, EMB-PRED-01, WS-52, WS-53.

## User outcome

`marklab bayes mixture-of-experts-fusion` learns patient-level context/availability simplex gates over
declared OOF expert probabilities, calibrates the mixture on separate patients, and reports context OOD.

## Frozen behavior

- consume 30–10,000 unique patients split among `gate_train`, `calibration`, and `test`, with both
  labels and at least eight patients each, 1–16 finite lowercase `context_*` features, 2–8 optional
  lowercase `expert_*` probabilities, and an explicit `patient_level_out_of_fold` source per row;
- reject context names containing site/scanner/stain/batch shortcuts; standardize context on gate train
  only and include expert availability indicators;
- use pinned SciPy 1.18.1 to minimize mixture log loss with positive L2 and entropy regularization;
  unavailable experts receive exact zero weight and available weights form a simplex;
- fit Platt calibration only on the calibration split; freeze a nearest-rank calibration context-distance
  OOD threshold; return calibrated test predictions, gates, availability, OOD, Brier, and mean gate
  entropy. Rust independently replays all values and metrics.

## Validation and claims

A 30-patient fixture learns expert A for negative context and expert B for positive context, assigns
exact `[0,1]`/`[1,0]` under single-expert availability, and beats Brier `0.25`. This is synthetic
predictive gating, not biological expertise, site robustness, causality, or clinical utility.
