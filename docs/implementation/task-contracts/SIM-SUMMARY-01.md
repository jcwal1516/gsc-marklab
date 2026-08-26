# Task contract — SIM-SUMMARY-01 differentiable pair-summary matching

Status: complete

Date: 2026-08-25

Parent requirements: GEN-01, WS-73.

## User outcome

`marklab simulate summary-matching` compares observed and generated bounded point patterns through
an analytically differentiable soft pair-distance histogram and a prespecified weighted loss.

## Pseudocode ownership

This workflow owns the Gaussian pair-distance specialization of `SoftPairHistogram` and the
immediate pair-summary specialization of `SummaryMatchingLoss`. Other differentiable summaries,
edge corrections, intensity normalization, learned feature losses, and inferential validation
remain separate work.

## Frozen behavior

- consume two 2–100,000-point patterns with unique IDs in the same finite rectangle, 1–1,024
  strictly increasing nonnegative radius centers, positive bandwidth, aligned finite nonnegative
  weights with at least one positive value, and at most 250 million total pair-bin visits;
- evaluate a normalized Gaussian kernel at every unordered pair distance and radius center, dividing
  by exact pair count and bandwidth so each curve is a pair-probability-density estimate;
- retain each bin value and the analytic sum of derivatives with respect to pair distances;
- compute the prespecified weighted sum of binwise squared observed/generated differences with a
  complete component decomposition;
- declare no edge correction and require the same window. The result is a research surrogate, not
  inference, validation, or biological fidelity.

## Validation and claims

One pair exactly at a radius center yields `1/(bandwidth*sqrt(2*pi))`. A displaced pair produces
positive loss, while reversing an identical two-point generated pattern leaves the histogram
unchanged and gives exact zero loss.
