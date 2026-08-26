# Task contract — BAY-COMPARE-01 compatible Bayesian predictive model comparison

Status: complete

Date: 2026-08-24

Parent requirements: BAY-02, WS-40, WS-44, BAY-PSIS-LOO-01.

## User outcome

`marklab bayes compare-models` compares two or more compatible PSIS-LOO artifacts through their
pointwise held-out predictive scores and reports pairwise ELPD differences with uncertainty.

## Frozen behavior

- PSIS-LOO artifacts now retain exact nonempty model name and likelihood-target declarations plus
  caller-supplied lowercase SHA-256 data and preprocessing identities. These declarations bind the
  static request/result but do not authenticate the caller's source data;
- comparison accepts 2–16 strict version-one PSIS-LOO JSON files with unique model names and exact
  equality of held-out-unit kind, likelihood target, data identity, preprocessing identity, and
  ordered unit IDs. Drift is an error rather than an implicit comparison;
- rank models by descending total ELPD with lexical model name as deterministic exact-tie break.
  For every lexical model pair, report A-minus-B pointwise ELPD sum and
  `sqrt(n * sample_variance(pointwise_differences))` standard error;
- preserve every source request/artifact identity and Pareto reliability. Any source requiring
  refit/K-fold makes the comparison `requires_refit_or_kfold`; it is not silently promoted by a
  favorable ELPD;
- strict input byte/count bounds, unknown-field rejection, finite totals/pointwise agreement,
  Pareto summaries, failure-atomic publication, and deterministic ordering are retained.

## Validation and claims

Two otherwise identical reliable four-patient PSIS artifacts whose second pointwise log likelihood
is shifted down by exactly `1.5` must rank the first model best and report A-minus-B ELPD difference
`6` with standard error `0`. This validates compatibility and pairwise arithmetic only—not model
truth, automatic selection, stacking, arbitrary K-fold designs, causality, biology, or clinical
evidence.
