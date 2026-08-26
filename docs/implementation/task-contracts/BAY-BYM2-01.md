# Task contract — BAY-BYM2-01 scaled-ICAR Poisson BYM2 model

Status: complete

Date: 2026-08-24

Parent requirements: BAY-02, BAY-05, BAY-GMRF-A, WS-40, WS-42.

## User outcome

`marklab bayes bym2-fit` fits the reparameterized Poisson areal model with an interpretable total
latent standard deviation and structured variance fraction through the pinned PyMC lifecycle.

## Frozen behavior

- data, weights, fixed effects, exact component constraints, and resource/diagnostic lifecycle reuse
  IC-0052;
- scale the unit-precision ICAR transform by the geometric mean of its generalized marginal
  variances so the scaled structured field has geometric-mean marginal variance exactly one within
  floating tolerance. The scale/convention are explicit output provenance;
- `combined_i=sigma*(sqrt(phi)*u_star_i+sqrt(1-phi)*v_i)`, with HalfNormal total sigma and
  Beta(alpha,beta) phi priors supplied explicitly; both raw fields are standard Normal and the
  structured field remains exactly sum-zero by component;
- `log(mu_i)=log(E_i)+intercept+X_i beta+combined_i`; strict identities, posterior/PPC finiteness,
  and established NUTS diagnostics control complete versus nonconverged;
- output reports sigma, phi, scaled structured/unstructured contributions, combined field, relative
  risk, constraints/rank deficiency, and count posterior predictive summaries.

## Validation and claims

The twelve-region BAY-BYM-01 ring must complete with positive fixed effect, sigma, and risks,
`0<=phi<=1`, exact component constraint, unit typical scaled ICAR variance, and zero
divergences/depth hits. It is experimental synthetic reparameterization evidence—not calibrated
epidemiology, patient risk, biology, causality, or clinical evidence.
