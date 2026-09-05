# Task contract — EMB-CALIBRATE-01 patient-OOF Platt probability calibration

Status: complete

Date: 2026-08-25

Parent requirements: EMB-PRED-01, COH-01, WS-52.

## User outcome

`marklab bayes calibrate-predictions` fits a Platt logistic calibrator on patient-level out-of-fold
training scores, freezes it before held-out evaluation, and reports calibrated probabilities plus
calibration diagnostics.

## Frozen behavior

- consume 16–100,000 unique patients split exactly between `training_oof` and `test`, with at least
  eight patients and both binary labels in each split, finite raw scores, 2–20 reliability bins, and
  bounded worker resources;
- use pinned SciPy 1.18.1 logistic optimization with Platt class-count target smoothing on
  `training_oof` only; held-out scores and labels cannot affect fitted intercept or slope;
- apply the frozen sigmoid calibrator to test scores and report Brier score, fixed-width ECE,
  calibration-in-the-large with calibrated logit as an offset, weakly stabilized logistic calibration
  slope, and nonempty reliability bins with Wilson 95% event-rate intervals;
- bind exact input/request/environment/worker identities and have Rust independently validate every
  probability, patient/label identity, Brier score, ECE, and reliability-bin assignment.

## Validation and claims

An eight-patient noisy monotone OOF fixture yields strictly increasing held-out probabilities. Flipping
every test label leaves calibrator parameters and all probabilities exactly unchanged, proving the fit
boundary; Brier and reliability counts are independently recomputed. This is synthetic calibration
evidence, not transportability or clinical utility.

## Native execution amendment — 2026-09-05

DEC-0416 / RUST-MIGRATION-01 replaces the production Python startup with a bounded native child and
library-owned CSV/scientific flow. Version 2 carries truthful native metadata; legacy worker readers
remain. All scientific fields and claim limits above retain their semantics. Eight oracle cases and
paired performance evidence, including two failed legacy cold comparisons, are recorded in the
migration contract and `audits/prediction_calibration_measurements.json`.
