# Task contract — BAY-SAR-FIT-01 fitted Gaussian SAR lag/error model

Status: complete

Date: 2026-08-24

Parent requirements: BAY-02, BAY-05, BAY-GMRF-A, WS-40, WS-42.

## User outcome

`marklab bayes sar-fit` fits an experimental Gaussian spatial-lag or spatial-error regression through
the pinned PyMC 6.3.0/Python 3.12 lifecycle, using IC-0050 likelihood semantics and normalized NUTS
diagnostics.

## Frozen behavior

- inputs reuse IC-0050 exact island-free row-standardized weights and complete response/design data,
  limited to 6–64 regions and 1–16 predictors;
- priors are explicit Normal intercept/coefficient, Uniform rho on caller-declared symmetric
  `(-rho_bound,rho_bound)` with `0<rho_bound<1`, and HalfNormal sigma;
- both modes include `log|det(I-rho W)|`; lag residual is `(I-rho W)y-X beta`, while error
  residual is `(I-rho W)(y-X beta)`;
- strict request/result/environment/worker digests, bounded runtime/output/iterations, deterministic
  seed domains, prior/posterior/predictive finiteness, rank R-hat, bulk/tail ESS, MCSE, E-BFMI,
  divergences, and tree-depth hits use the established exact-NUTS gate;
- lag mode reports posterior descriptive direct/indirect/total impact distributions for non-intercept
  coefficients. Error mode reports none. Version one admits no causal interpretation.

## Validation and claims

An eight-region row-standardized ring generated from lag rho `0.3`, intercept `0.7`, slope `1.2`,
and bounded Gaussian-scale residuals must complete, recover a positive slope and admissible positive
rho, retain observed response summaries in posterior-predictive output, and report positive slope
direct/total impacts. The result remains experimental synthetic-model evidence, not validated tissue,
biology, causality, or clinical evidence. Applying error mode to that lag-generated fixture must
execute the separate error residual/predictive path and retain a truthful diagnostic-only
`nonconverged` state when its frozen ESS gate is not met.
