# Task contract — BAY-SENS-01 conjugate Normal prior sensitivity

Status: complete

Date: 2026-08-24

Parent requirements: BAY-01, BAY-02, WS-40, WS-44.

## User outcome

`marklab bayes normal-mean-prior-sensitivity` refits the typed Normal-mean/known-sigma model across
an explicit prior grid and reports posterior, exact leave-one-out predictive, and decision changes.

## Frozen behavior

- strict observation CSV has 2–100,000 finite scalar rows; strict prior CSV has 2–32 unique exact
  `prior_name,prior_mean,prior_sd` rows with finite mean/positive SD and one declared base prior;
- caller supplies known positive observation SD, finite decision threshold, decision posterior-
  probability threshold in `(0.5,1)`, positive material posterior-mean shift, timeout, and fresh
  output. Prior plausibility remains caller responsibility and is not inferred from results;
- pinned SciPy computes each exact conjugate posterior, probability `P(mu>threshold)`, binary
  decision at the declared probability threshold, and exact leave-one-out log predictive density;
- compare every alternative with base posterior mean/SD, decision probability/conclusion, and LOO
  ELPD. Flag exact conclusion changes and material mean shifts without selecting a prior by outcome;
- strict environment/worker/request/data/prior identities, finite summaries, exact observed
  sufficient statistics, resource bounds, failure-atomic publication, and experimental claim ceiling
  are retained.

## Validation and claims

For observations `[1,2,3,4]`, known sigma 1, base `Normal(0,1)`, wide `Normal(0,10)`, and skeptical
`Normal(-2,0.5)`, base posterior is `Normal(2,sqrt(0.2))`; at decision threshold 1/probability 0.95,
the skeptical prior changes the positive conclusion and exceeds material mean shift 0.5. This
validates exact conjugate sensitivity mechanics only—not prior plausibility, automatic robustness,
model truth, biology, causality, or clinical evidence.
