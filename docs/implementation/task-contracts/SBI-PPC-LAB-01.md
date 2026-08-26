# Task contract — SBI-PPC-LAB-01 growth-front posterior-predictive laboratory

Status: complete

Date: 2026-08-25

Parent requirements: BAY-SBI, WS-74.

## User outcome

`marklab bayes growth-front-posterior-predictive-lab` draws from a declared growth-rate posterior,
simulates canonical replicates, and returns complete mass/maximum-density intervals, discrepancy
p-values, failures, and misspecification flags.

## Pseudocode ownership

This workflow owns the two-summary Fisher–KPP specialization of `PosteriorPredictiveLaboratory`.
K/L/g, marks, compartments, graphs, topology, embeddings, multimodal summaries, multiplicity policy,
and real posterior/model validation remain separate work.

## Frozen behavior

- consume 2–1,000,000 finite nonnegative posterior growth-rate draws, observed final mass/maximum
  density, 20–100,000 requested replicates, strict interior interval probability/discrepancy alpha,
  and at most 250 million declared simulator cell-steps;
- select posterior draw indices by named ChaCha20 replay and call the canonical growth-front
  simulator for every replicate;
- retain every successful replicate's draw index/value and two summaries, plus every failed
  replicate/reason; fail if all replicates fail;
- for each summary retain observed and replicated mean, nearest-rank equal-tail interval, interval
  membership, finite-sample lower/upper tail probabilities, capped two-sided discrepancy p-value,
  and a flag for interval exclusion or p-value below alpha;
- retain the complete summary catalog, all flags/failures, seed namespace, and maximum declared work;
- report a synthetic posterior-predictive check, never general model validation.

## Validation and claims

One hundred replicates drawn from growth rates `0.95,1,1.05` retain the analytic rate-one mass and
maximum-density observations inside both 90% intervals, emit no misspecification flags or failures,
and replay byte-identically from seed `20260825`.
