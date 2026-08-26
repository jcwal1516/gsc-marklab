# Task contract — BAY-HIER-01 partially pooled Gaussian patient model

Status: complete

Date: 2026-08-24

Parent requirements: BAY-01, BAY-02, BAY-03, BAY-HIER-A, WS-40, WS-41.

## User outcome

`marklab bayes hierarchical-normal` fits repeated finite scalar observations nested under explicit
patient IDs with a non-centered Gaussian varying-intercept model through the existing pinned PyMC
6.3.0 worker. It reports global mean, between-patient heterogeneity, patient-specific partial
pooling, variance partition, normalized NUTS diagnostics, and posterior predictive discrepancies.

## Frozen model

- `global_mean ~ Normal(global_prior_mean, global_prior_sd)`;
- `between_patient_sd ~ HalfNormal(between_patient_sd_prior)`;
- `patient_z[g] ~ Normal(0,1)` and `patient_mean[g] = global_mean + between_patient_sd * patient_z[g]`;
- `y_i ~ Normal(patient_mean[patient_i], known_sigma)` with explicit patient observation unit and
  no spatial component.

Patient IDs are exact, non-empty, unique after grouping, and sorted canonically. Version one
requires at least three patients and two observations per patient. Priors, known sigma, sampling,
seed, diagnostics, timeout, environment lock, and worker digest are explicit request identity.

## Validation and claims

Prior and posterior predictive draws must be finite. Complete status uses the existing R-hat,
bulk/tail ESS, divergence, and tree-depth policy across global mean, heterogeneity, and all patient
effects. A deterministic synthetic repeated-patient fixture must recover its global mean and
heterogeneity within prespecified Monte Carlo tolerances and shrink both extreme raw patient means
toward the posterior global mean. Nonconverged output is diagnostic-only.

This is an experimental statistical partial-pooling result for supplied patient-grouped scalar
observations. It is not spatial, multisite, causal, biological, clinical, calibrated, or evidence
of patient quality. No arbitrary formula, random-slope, missing-data, correlation, or generalized
backend registry is added.

## Delivered evidence

- `marklab bayes hierarchical-normal` owns the strict `patient_id,observation` boundary and exact
  non-centered model above. The result reports global/heterogeneity summaries, all sorted patient
  summaries, model-dependent shrinkage, variance partition, MCSE, E-BFMI, rank R-hat, bulk/tail
  ESS, divergences, depth hits, constraints, identifiability, and global/patient-dispersion
  posterior-predictive comparisons.
- `cargo +1.96.0 test --locked --package marklab-bayes` passed 6 unit/contract tests. `cargo
  +1.96.0 test --locked --test bayes_hierarchical_normal_cli -- --nocapture` passed the 24-row,
  six-patient recovery fixture with complete diagnostics, zero divergences/depth hits, global mean
  and heterogeneity recovery, and inward movement of both extreme patient means.
