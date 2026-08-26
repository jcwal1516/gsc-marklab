# Task contract — BAY-PSIS-LOO-01 held-out-unit-aware PSIS-LOO

Status: complete

Date: 2026-08-24

Parent requirements: BAY-02, WS-40, WS-44.

## User outcome

`marklab bayes psis-loo` computes Pareto-smoothed importance-sampling leave-one-out predictive
accuracy from a complete pointwise log-likelihood draw matrix while retaining the scientifically
declared held-out unit.

## Frozen behavior

- input is strict `chain,draw,unit_id,log_likelihood` CSV for 2–8 zero-based contiguous chains,
  100–100,000 zero-based contiguous draws per chain, and 2–10,000 exact unique held-out units. The
  matrix is complete and rectangular, ordering is canonicalized, all values are finite, and total
  cells/output/runtime are bounded;
- caller declares a nonempty exact held-out-unit kind and relative MCMC efficiency in `(0,1]`.
  Marklab does not infer that cells are valid independent held-out units;
- caller also declares an exact model name and likelihood target plus lowercase SHA-256 data and
  preprocessing identities. These comparison keys are request/result identity, not source-data
  authentication;
- pinned ArviZ 1.3.0/arviz-stats 1.3.1 computes PSIS-LOO on the supplied pointwise log likelihood,
  returning log-scale total/pointwise ELPD, standard error, effective parameter count, smoothed
  Pareto-k per unit, and the backend's sample-size-dependent good-k threshold;
- units above good-k are retained and explicitly require exact refit or grouped K-fold follow-up.
  A diagnostic warning is not relabeled reliable and no automatic refit is fabricated;
- strict environment/worker/request/input identities, backend versions, sample/unit dimensions,
  output finiteness, pointwise-total agreement, Pareto summaries, and resource bounds are retained.

## Validation and claims

Deterministic quantile draws from the conjugate posterior for four `Normal(mu,1)` observations and
`mu ~ Normal(0,1)` must agree with the exact leave-one-out Normal predictive ELPD within `0.03`,
retain all four patient IDs, and pass the backend good-k threshold. This validates the PSIS adapter
and small benign importance-ratio behavior only—not arbitrary influential data, automatic refits,
model selection, causal inference, biology, or clinical evidence.
