# Task contract — BAY-VIGP-01 variational inducing-point GP

Status: complete

Date: 2026-08-24

Parent requirements: BAY-02, BAY-04, BAY-FIELD-A, SCALE-01, WS-40, WS-42.

## User outcome

`marklab bayes variational-gp` fits the IC-0042 one-dimensional micrometre Matérn-3/2 field with
PyMC 6.3.0's maintained VFE inducing-point approximation and mean-field ADVI. It produces bounded
latent predictions, optimized ordered inducing locations, ELBO/multi-start diagnostics, and an
explicit `approximate_only` fit state.

## Frozen approximation

- Gaussian likelihood, Matérn-3/2 range convention, priors, jitter, and input/prediction schemas
  match IC-0042;
- 3–64 inducing coordinates are initialized at deterministic coordinate quantiles and optimized as
  an ordered transformed parameter. PyMC's VFE bound analytically owns the Gaussian inducing-state
  optimum while ADVI approximates hyperparameter/inducing-location uncertainty;
- exactly 2–4 independent deterministic starts run a fixed 1,000–100,000 iterations with explicit
  learning rate and 500–10,000 retained draws. The best finite terminal ELBO is selected;
- diagnostics retain every start's initial/final ELBO, tail relative change, selected start, finite
  state, and cross-start predictive RMSE. No R-hat/HMC diagnostic is fabricated.

Version one requires 8–2,000 observations, 1–2,048 predictions, `m<n`, and bounded
`starts*iterations*(n*m^2+m^3)` plus prediction work. It is not called exact GP inference.

## Validation and claims

The smooth-field fixture must agree with `marklab bayes gp-regression` predictions within a
prespecified RMSE and preserve finite positive uncertainty. The result remains experimental and
approximate-only, with no calibration, exactness, sparse-scale production, tissue, biological, or
clinical claim.

## Delivered evidence

- `marklab bayes variational-gp` owns ordered learnable quantile-initialized inducing coordinates,
  PyMC VFE Gaussian-likelihood bound, two-to-four independent mean-field ADVI starts, explicit
  ELBO/stability thresholds, selected-start hyperparameters/latent predictions, prior/posterior
  finiteness, and posterior-predictive mean/dispersion. Stable output is always
  `approximate_only`; unstable optimization is typed `nonconverged`.
- Gradient norm is explicitly unavailable from the admitted PyMC approximation and importance
  correction is explicitly `not_run`; neither diagnostic is fabricated.
- `cargo +1.96.0 test --locked --test bayes_variational_gp_cli -- --nocapture` passed two-start
  stability, ordered inducing locations, finite uncertainty, posterior predictive checks, and
  direct prediction RMSE <=0.35 against a complete `marklab bayes gp-regression` run on the same
  fixture.
