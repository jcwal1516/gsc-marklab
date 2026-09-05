# Task contract — EMB-LATE-FUSION-01 calibrated patient-level late fusion

Status: complete

Date: 2026-08-25

Parent requirements: EMB-PRED-01, EMB-PRED-02, WS-52.

## User outcome

`marklab bayes late-fusion` combines declared patient-level out-of-fold modality probabilities,
calibrates the fused score on separate patients, evaluates test patients, and reports missing-modality
and modality-ablation diagnostics.

## Frozen behavior

- consume 30–100,000 unique patients across `meta_train`, `calibration`, and `test`, with at least ten
  and both labels in each split, 2–16 unique `modality_*` probability columns, at least one available
  modality per patient, and an explicit `patient_level_out_of_fold` source declaration on every row;
- represent each modality by probability (neutral `0.5` when absent) plus an availability indicator;
  fit a positive-L2 SciPy 1.18.1 logistic meta-model on `meta_train` only;
- fit a Platt class-count-smoothed calibrator on fused `calibration` scores only and apply the frozen
  meta-model/calibrator to `test`;
- report test Brier score, observed availability-pattern Brier scores, and one test-time modality
  ablation per modality. Diagnostics are predictive contributions, not causal modality importance;
- bind exact input/request/environment/worker identities; Rust independently recomputes every raw and
  calibrated probability and the full Brier score.

## Validation and claims

A 30-patient two-modality fixture yields ten calibrated test predictions, complete and each
single-missing scenario, two modality ablations, and Brier below the uninformative `0.25` reference.
This is synthetic leakage-bound evidence, not real multimodal utility or transportability.
