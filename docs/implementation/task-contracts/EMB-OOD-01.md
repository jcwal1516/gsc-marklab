# Task contract — EMB-OOD-01 validation-calibrated shrinkage Mahalanobis OOD

Status: complete

Date: 2026-08-25

Parent requirements: EMB-PRED-01, WS-52.

## User outcome

`marklab bayes ood-score` fits a frozen shrinkage Mahalanobis representation score on training units,
calibrates its decision threshold on held-out validation domains, and scores test units only.

## Frozen behavior

- consume 2–128 unique ordered `embedding_*` features and at most 100,000 unique finite train,
  validation, and test units with explicit domain identities;
- require at least dimension-plus-one training units, three validation units from domains absent from
  training, one test unit, and positive variation in every training feature;
- fit the population mean/covariance on training only and multiply every off-diagonal covariance by
  `1-shrinkage`, leaving variances unchanged; reject a non-positive-definite result;
- use Euclidean norm after Cholesky whitening as the score and the nearest-rank caller-prespecified
  validation quantile as the frozen threshold; strict exceedance is OOD.

## Validation and claims

The symmetric four-corner two-dimensional training oracle has identity covariance. Validation scores
`0,1,2` at quantile `2/3` give threshold one; test scores are `sqrt(0.5)` retained and `sqrt(8)` OOD.
This is a representation-domain diagnostic, not biological novelty or a clinical exclusion rule.
