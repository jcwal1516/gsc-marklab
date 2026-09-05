# Task contract — EMB-CONFORMAL-01 patient-level split-conformal classification

Status: complete

Date: 2026-08-25

Parent requirements: EMB-PRED-01, COH-01, WS-52.

## User outcome

`marklab bayes grouped-conformal` fits a regularized binary predictor on patient training rows,
calibrates a finite-sample-corrected nonconformity threshold on separate patients, and emits prediction
sets for test patients with overall/site/subgroup coverage diagnostics.

## Frozen behavior

- consume 30–100,000 unique patients split exactly among train, calibration, and test; require at
  least 12/10/8 rows, both training labels, 2–128 unique `feature_*` columns, explicit site/subgroup,
  positive L2 penalty, alpha in `(0,0.5)`, and enough calibration rows for the requested alpha;
- fit feature mean/population SD and a pinned SciPy 1.18.1 L2 logistic model on train only; compute
  calibration nonconformity as `1-p(true label)`;
- freeze corrected rank `ceil((n_calibration+1)*(1-alpha))` and its observed order statistic; include
  candidate outcome `y` exactly when `1-p(y) <= threshold`;
- report patient prediction sets and empirical coverage overall/by site/by subgroup, while warning that
  marginal exchangeability coverage is not conditional coverage and can fail under shift;
- bind exact input/request/environment/worker identities; Rust independently recomputes model
  probabilities, calibration order statistic, every prediction set, and all coverage counts.

## Validation and claims

A 30-patient two-feature noisy binary fixture proves distinct fit/calibration/test boundaries, corrected
rank/count, nonempty test sets, and exact site/subgroup accounting. This is synthetic exchangeability
evidence, not distribution-shift robustness or clinical coverage.
