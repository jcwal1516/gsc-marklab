# Task contract — EMB-COMPLEMENT-01 nested patient-held-out cell–patch complementarity

Status: complete

Date: 2026-08-25

Parent requirements: EMB-PATCH, FR-02, EMB-PRED-01, WS-51, WS-52, COH-PAIR-01.

## User outcome

`marklab bayes test-cell-patch-complementarity` compares prespecified M0–M5 feature sets under fully
nested patient-held-out ridge regression and patient-paired incremental error inference.

## Frozen behavior

- consume 24–10,000 unique patient rows with finite target, exact 2–10 outer and 2–10 inner fold IDs,
  and at least one varying feature in each prespecified group: technical/clinical/compartment/
  acquisition (M0), cell (M1), patch (M2), neighboring-cell context (M4), and independently measured
  features (M5); M3 combines cell+patch;
- require every outer fold nonempty and every outer-training subset to contain every inner fold. For
  each M0–M5 and outer fold, fit standardization only on inner-training patients, select one finite
  nonnegative ridge alpha by mean inner-validation RMSE with smallest-alpha tie break, then refit
  preprocessing/model on the complete outer-training patients and predict the untouched outer test;
- use pinned SciPy 1.18.1 linear algebra through a static Python 3.12 worker bound to the existing
  29-package lock. Retain all patient predictions, selected hyperparameters, RMSE/MAE, and calibration
  intercept/slope per model;
- compare prespecified increments M1–M0, M2–M0, M3–M1, M3–M2, M4–M3, and M5–M4 using the existing
  Marklab complete-pair patient sign-flip owner on absolute errors; retain two-sided plus-one p-values
  and expanded-minus-base mean error effects;
- retain input/backend/worker/lock/split/feature/seed identities and explicit resource/timeout bounds.
  Synthetic validation is experimental predictive-design evidence only, not real incremental value,
  calibration, biology, clinical utility, or stable FR-02/EMB-PRED-01 promotion.

## Validation and claims

A 24-patient four-outer/three-inner-fold fixture whose target is supplied by the patch feature must
produce complete once-held-out predictions, make M2 materially outperform M0, retain all six model
sets and six paired increments, and report negative M2-minus-M0 absolute-error effect.
