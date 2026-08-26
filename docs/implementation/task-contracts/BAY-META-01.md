# Task contract — BAY-META-01 Bayesian random-effects site meta-analysis

Status: complete

Date: 2026-08-24

Parent requirements: BAY-01, BAY-02, BAY-03, BAY-HIER-A, WS-40, WS-41.

## User outcome

`marklab bayes meta-analysis` fits explicit site/cohort effect estimates with known positive standard
errors and one declared finite site covariate through pinned PyMC 6.3.0. It reports global
intercept, covariate slope, heterogeneity, latent site effects, and a predictive latent effect for
one declared new-site covariate.

## Frozen model

- `global_effect ~ Normal(global_prior_mean, global_prior_sd)`;
- `covariate_effect ~ Normal(0, covariate_prior_sd)`;
- `heterogeneity ~ HalfNormal(heterogeneity_prior_sd)`;
- `site_effect[s] ~ Normal(global_effect + covariate[s] * covariate_effect, heterogeneity)`;
- `observed_effect[s] ~ Normal(site_effect[s], site_standard_error[s])`;
- `new_site_effect ~ Normal(global_effect + new_site_covariate * covariate_effect, heterogeneity)`.

Version one requires at least five unique exact site IDs and nonconstant raw covariates; it performs
no hidden centering or scaling. All prior, covariate-name, new-site, sampling, environment, worker,
diagnostic, and resource conventions are explicit identity.

## Validation and claims

A synthetic eight-site fixture must recover its known intercept, covariate slope, and heterogeneity
within prespecified Monte Carlo tolerances and predict the latent effect at covariate zero. All
parameters and site effects pass the shared finite/MCSE/E-BFMI/R-hat/ESS/divergence/depth gate and
effect-level posterior predictive checks.

The output is experimental model-based synthesis for supplied estimates and standard errors. It
does not establish exchangeability, transportability, causality, biological validity, publication
bias control, real-world calibration, or a clinical effect.

## Delivered evidence

- `marklab bayes meta-analysis` owns the exact input/model/result above. PyMC samples the
  analytically marginalized observed likelihood `Normal(global + x*gamma, sqrt(tau^2 + se^2))`;
  Marklab reconstructs each latent site effect from its exact conditional Normal distribution and
  generates the requested new-site latent effect. This preserves the declared hierarchy while
  avoiding the divergent centered latent geometry found by the red recovery run.
- `cargo +1.96.0 test --locked --package marklab-bayes` passed 7 unit/contract tests. `cargo
  +1.96.0 test --locked --test bayes_meta_analysis_cli -- --nocapture` passed the eight-site
  intercept/slope/heterogeneity recovery and new-site prediction fixture with complete diagnostics
  and zero divergences/depth hits.
