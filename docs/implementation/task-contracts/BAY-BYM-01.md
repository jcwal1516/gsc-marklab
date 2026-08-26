# Task contract — BAY-BYM-01 Poisson BYM disease-mapping model

Status: complete

Date: 2026-08-24

Parent requirements: BAY-02, BAY-05, BAY-GMRF-A, WS-40, WS-42.

## User outcome

`marklab bayes bym-fit` fits an experimental Poisson log-offset areal count model with separate
intrinsic-CAR structured and iid unstructured effects through the pinned PyMC lifecycle.

## Frozen behavior

- input uses IC-0047 preserved symmetric binary zero-diagonal weights with no islands, 6–64 exact
  regions, positive finite expected counts, nonnegative integer observed counts, and 1–16 full-rank
  finite predictors;
- `log(mu_i)=log(E_i)+intercept+X_i beta+u_i+v_i`; intercept/coefficients have explicit Normal
  priors, and structured/unstructured standard deviations have explicit HalfNormal priors;
- the structured field is an exact sum-to-zero ICAR on each connected component: deterministic
  orthonormal null-space coordinates and projected `D-W` precision define the noncentered field.
  No soft constraint, silent ridge, island prior, or proper-CAR substitution is used;
- strict backend/request/result identities, bounded resources, deterministic seed domains, prior and
  posterior predictive checks, and established exact-NUTS diagnostics control complete versus
  nonconverged state;
- output retains posterior fixed effects, both separate region fields, relative risk, component
  constraints/rank deficiency, and replicated total/zero-count summaries.

## Validation and claims

A twelve-region ring with expected count 100, a positive declared predictor effect, and smooth plus
iid synthetic latent effects must complete, recover a positive coefficient, retain one exact
sum-to-zero structured constraint, produce finite risks and posterior predictive summaries, and have
zero divergences/depth hits. This is synthetic experimental disease-mapping mechanics—not calibrated
epidemiology, biological causality, patient risk, or clinical evidence.
