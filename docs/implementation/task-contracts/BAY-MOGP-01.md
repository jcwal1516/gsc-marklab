# Task contract — BAY-MOGP-01 identifiable two-output GP coregionalization

Status: complete

Date: 2026-08-24

Parent requirements: BAY-01, BAY-02, BAY-04, BAY-FIELD-A, WS-40, WS-42.

## User outcome

`marklab bayes multi-output-gp` jointly fits two complete scalar outputs observed at the same exact
one-dimensional micrometre coordinates using one latent Matérn-3/2 process and predicts both latent
outputs at an explicit bounded coordinate set.

## Frozen identifiable model

- each output has its own Normal mean and explicit known positive observation-noise SD;
- one latent GP has positive amplitude and length-scale HalfNormal priors;
- output A loading is fixed to 1, and output B loading is positive HalfNormal. This fixes the
  one-factor scale/sign convention and version one represents positive cross-output dependence;
- block covariance is `[[K, bK], [bK, b^2 K]]` plus output-specific noise and explicit jitter;
- output names are exact user-supplied identity and cannot match.

Version one requires 5–64 complete unique coordinates and 1–1,024 prediction coordinates, with an
explicit bounded dense-work contract. Missing outputs, arbitrary output counts, multiple latent
processes, negative loading, rotational post-processing, and modality-specific likelihoods are not
silently inferred.

## Validation and claims

A deterministic two-output smooth fixture must recover positive cross-output loading and jointly
interpolate both outputs with finite uncertainty under complete shared NUTS diagnostics. The result
is experimental coregionalization for supplied synthetic fields, not biological complementarity,
causal coupling, calibration, tissue-domain validity, or a general multimodal factor model.

## Delivered evidence

- `marklab bayes multi-output-gp` owns the exact one-factor block covariance, fixed loading A = 1,
  positive loading B, separate means, explicit known per-output noise, Matérn hyperparameters,
  bounded dense work, joint conditional latent predictions, and cross-output posterior predictive
  correlation.
- The first version inferred two near-zero noise scales and was rejected with 535 maximum-tree-depth
  hits after 142 seconds despite zero divergences. Freezing caller-supplied known noise removed those
  weak nuisance dimensions; the final recovery run completes in about 33 seconds with zero
  divergences/depth hits and the unchanged R-hat/ESS/MCSE/E-BFMI gate.
- `cargo +1.96.0 test --locked --test bayes_multi_output_gp_cli -- --nocapture` passed positive
  loading recovery plus both-output interpolation with finite uncertainty.
