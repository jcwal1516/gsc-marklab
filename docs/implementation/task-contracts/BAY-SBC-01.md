# Task contract — BAY-SBC-01 conjugate normal-mean simulation-based calibration

Status: complete

Date: 2026-08-24

Parent requirements: BAY-01, BAY-02, WS-40, WS-44.

## User outcome

`marklab bayes normal-mean-sbc` runs deterministic simulation-based calibration for the typed
Normal-mean/known-sigma model against its exact conjugate posterior sampler.

## Frozen behavior

- caller supplies finite Normal prior mean/positive SD, positive known observation SD, 1–1,000
  observations per replicate, 20–5,000 replicates, 20–5,000 exchangeable posterior draws, seed,
  bounded total simulated values/output/runtime, and fresh output;
- each replicate uses a domain-separated deterministic seed, draws truth from the declared prior,
  simulates observations, computes the exact conjugate posterior, and obtains independent posterior
  draws. Invalid replicate state is recorded as failure and contributes no fabricated rank;
- rank is the count of posterior draws strictly below truth with deterministic randomized tie
  handling. Retain truth/posterior summaries, equal-tail 95% coverage, posterior z-score, shrinkage,
  all ranks, and exact failures;
- assess the discrete-uniform rank histogram with chi-square statistic/p-value and ECDF/DKW envelope,
  plus coverage, z-score, shrinkage, and failure-rate summaries. Autocorrelation correction is
  explicitly unnecessary only because this contract uses independent exact posterior draws;
- strict backend/lock/worker/request identities, finite summaries, histogram/rank/count agreement,
  deterministic seeds, resource bounds, and approximate claim ceiling are retained.

## Validation and claims

For prior `Normal(0,1)`, known sigma `1`, four observations, 500 replicates, and 200 exact posterior
draws, ranks must be bounded and sum to 500, chi-square p-value exceed `0.01`, empirical 95% coverage
lie in `[0.90,0.99]`, z-score mean/SD be near 0/1, shrinkage equal `0.8`, no failures occur, and a
same-seed rerun is byte-identical. This validates SBC mechanics against a tractable exact sampler;
it does not calibrate NUTS, other models, real data, biology, causality, or clinical use.
