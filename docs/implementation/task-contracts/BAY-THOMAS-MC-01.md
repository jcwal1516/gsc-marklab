# Task contract — BAY-THOMAS-MC-01 Thomas minimum-contrast fit

Status: complete

Date: 2026-08-25

Parent requirements: BAY-02, BAY-PP, BAY-PP-B, WS-43, BAY-THOMAS-SIM-01.

## User outcome

`marklab bayes fit-thomas-minimum-contrast` fits typed Thomas parent intensity, Gaussian scale,
and derived mean offspring from an observed K curve and observed point intensity through pinned SciPy.

## Frozen behavior

- consume exact `radius_um,observed_k_um2,weight` CSV with 8–1,000 strictly increasing positive
  radii, finite nonnegative K, finite positive weights, positive observed intensity, positive bounded
  kappa/sigma search intervals, bounded iterations/timeout, and fresh output;
- use theoretical `K(r)=pi*r^2+(1/kappa)*(1-exp(-r^2/(4*sigma^2)))` and fourth-root
  minimum contrast. Pinned SciPy 1.18.1 least-squares optimizes log kappa/log sigma inside exact
  caller bounds; mean offspring is observed intensity divided by fitted parent intensity;
- report the weighted full-range primary fit, unweighted full-range sensitivity fit, and weighted
  interior-range sensitivity fit, with parameters, objectives, convergence/evaluation/gradient state,
  every observed/fitted curve row, identities, and exact arithmetic revalidation in Rust;
- do not call K-contrast a likelihood, infer latent parents, claim unique practical identifiability,
  or fabricate likelihood comparison while BAY-CLUSTER-FIT-01 is blocked.

## Validation and claims

An exact synthetic K curve from kappa `0.002`, sigma `10 um`, and observed intensity `0.02/um2`
must recover kappa/sigma and derived mu `10` within tight numerical tolerance under all three fits
with near-zero primary objective. This validates the theoretical-curve/optimizer boundary only—not
finite-sample calibration, latent-parent recovery, biology, causality, or clinical use.
