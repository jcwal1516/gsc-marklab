# Task contract — BAY-SVC-01 exact 1-D spatially varying coefficient

Status: complete

Date: 2026-08-24

Parent requirements: BAY-01, BAY-02, BAY-04, BAY-FIELD-A, BAY-REG-A, WS-40, WS-42.

## User outcome

`marklab bayes spatial-varying-coefficient` fits a Gaussian outcome with one global coefficient and
one one-dimensional Matérn-3/2 varying coefficient through the pinned PyMC lifecycle.

## Frozen behavior

- input has 8–64 unique exact micrometre coordinates with finite outcome, one finite global
  predictor, and one finite nonconstant spatial predictor; the design including intercept/global/
  spatial mean must be full rank;
- `eta=intercept+global_x*beta_global+spatial_z*(beta_spatial+delta(x))`, with explicit Normal
  priors, known positive observation noise, and inferred positive Matérn amplitude/length scales;
- delta uses exact dense Matérn-3/2 covariance projected into an orthonormal sum-to-zero subspace,
  separating the spatial mean coefficient from its field. No silent interpolation, unconstrained
  field mean, varying-intercept alias, or 2-D/tissue claim is admitted;
- strict backend/request/result/data/worker/lock identities, bounded dense work/runtime/output,
  deterministic seeds, prior/posterior/PPC finiteness, and established exact-NUTS diagnostics gate
  complete versus nonconverged;
- output retains global/spatial mean coefficients, kernel hyperparameters, every varying coefficient
  field value and latent deviation, exact centering constraint, and response PPC.

## Validation and claims

A twelve-coordinate synthetic fixture with positive global/spatial mean effects and a smooth
sum-zero coefficient deviation must complete, recover both positive effects, retain a centered
nonconstant field, and have zero divergences/depth hits. This is experimental synthetic 1-D
regression—not tissue validation, causal heterogeneity, biology, or clinical evidence.
