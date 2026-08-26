# Task contract — BAY-GP-01 exact one-dimensional Gaussian-process regression

Status: complete

Date: 2026-08-24

Parent requirements: BAY-01, BAY-02, BAY-04, BAY-FIELD-A, WS-40, WS-42.

## User outcome

`marklab bayes gp-regression` fits one finite scalar field at exact unique one-dimensional physical
coordinates and predicts at an explicit bounded coordinate set using pinned PyMC 6.3.0 NUTS and an
exact dense Matérn-3/2 covariance.

## Frozen model and limits

- coordinate dimension is one and unit is micrometres; inputs are never inferred from row order;
- `mean ~ Normal(mean_prior_mean, mean_prior_sd)`;
- amplitude, length scale in micrometres, and observation-noise SD use explicit positive HalfNormal
  priors;
- covariance is `amplitude^2 * (1 + sqrt(3)r/length) * exp(-sqrt(3)r/length)` plus
  `noise_sd^2 + jitter` on the observed diagonal;
- exact Cholesky inference is limited to 5–128 unique observations and 1–2,048 finite prediction
  coordinates. Jitter is explicit, finite, positive, and part of request identity.

The worker reports posterior hyperparameters, exact conditional predictive mean/variance/interval
at every requested coordinate, and posterior-predictive observed-field mean/SD discrepancies. It
uses bounded `O(n^2)` matrices and `O(n^3)` Cholesky work; no sparse/inducing/nearest-neighbor claim
is made.

## Validation and claims

A deterministic smooth synthetic field must recover held-in interpolation targets within declared
Monte Carlo tolerance with finite positive uncertainty and complete shared NUTS diagnostics. The
result is experimental model-based interpolation for the supplied coordinate frame only—not a
tissue-window, anisotropy, nonstationarity, causal, biological, clinical, real-data, or calibrated
spatial-field claim.

## Delivered evidence

- `marklab bayes gp-regression` owns the exact one-dimensional micrometre frame, Matérn-3/2 range
  convention `sqrt(3)r/length`, dense covariance, positive inferred hyperparameters, explicit
  jitter, bounded model-evaluation/conditioning work, latent conditional predictions, and
  observed-field posterior predictive summaries.
- The first 1,000-warmup/0.95-target fit was correctly typed nonconverged with 353 divergence
  events. The prespecified model passes with 2,000 warmup draws and target acceptance 0.99 without
  weakening the zero-divergence gate.
- `cargo +1.96.0 test --locked --package marklab-bayes` passed 8 unit/contract tests. `cargo
  +1.96.0 test --locked --test bayes_gp_regression_cli -- --nocapture` passed all three smooth-field
  interpolation targets with finite positive uncertainty, complete diagnostics, and zero
  divergences/depth hits.
