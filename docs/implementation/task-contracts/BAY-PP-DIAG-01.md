# Task contract — BAY-PP-DIAG-01 posterior-predictive point-process diagnostics

Status: complete

Date: 2026-08-25

Parent requirements: BAY-02, BAY-PP, WS-43, BAY-LGCP-PPC-01.

## User outcome

`marklab bayes point-process-posterior-predictive-diagnostics` compares observed and supplied
posterior-predictive replicated patterns using identical exact-window count and translation-corrected
K estimators with a simultaneous max-deviation envelope.

## Frozen behavior

- consume exact observed and replicated `pattern_id,point_id,x_um,y_um` rows in one positive-area
  half-open rectangle, 1–100 patterns, 20–1,000 complete replicate indices containing every pattern,
  2–256 increasing positive radii below window diagonal, alpha in `(0,0.5)`, exact content identities,
  and bounded pair visits;
- for every pattern/replica use the same count and rectangle translation-corrected ordered-pair K
  estimator on the same eligible radii. Aggregate curves by unweighted mean across declared patterns;
- form a simultaneous envelope from replicate mean/SD and the nearest-rank `1-alpha` quantile of each
  replica's maximum absolute standardized curve deviation; retain observed/replicate curves, count
  distributions, pointwise center/SD, critical value, bounds, and exceeded radii;
- diagnostics have no binary pass/model-truth simplification. No g/mark statistic is fabricated; each
  additional summary requires its own same-estimator contract.

## Validation and claims

A two-pattern/twenty-replica rectangle fixture must conserve pattern/replica identities and counts,
produce finite translation K curves and simultaneous bounds, and retain exceeded-radius diagnostics
without interpreting consistency as model truth.
