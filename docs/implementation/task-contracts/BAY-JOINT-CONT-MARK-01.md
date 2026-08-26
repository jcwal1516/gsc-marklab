# Task contract — BAY-JOINT-CONT-MARK-01 joint location–continuous-mark model construction

Status: complete

Date: 2026-08-25

Parent requirements: BAY-01, BAY-04, BAY-PP, BAY-MM, WS-43, BAY-JOINT-MARK-01.

## User outcome

`marklab bayes build-joint-continuous-mark-model` builds a typed exact-grid joint location and
continuous-mark latent-field artifact with identified shared-field loadings.

## Frozen behavior

- consume unique finite points with continuous mark, location covariate/offset, and mark covariate in
  an exact half-open rectangle plus a complete regular location grid; require at least eight points,
  materially varying mark/covariates, positive known mark-noise SD, positive Matérn amplitude/physical
  length/jitter, and positive loading/private-field prior scales;
- location log intensity has fixed loading one on a zero-mean shared Matérn field plus a private
  location field; Gaussian identity-link mark mean has positive HalfNormal shared loading plus a
  private mark field. Fixed shared-field amplitude/length establish scale; positive mark loading fixes
  sign. Private fields are independent under the declared kernel;
- retain exact data/grid/units/prior/kernel/identifiability identities and require comparison with the
  nested zero-shared-loading separate location/mark models;
- model construction only: no posterior shared-field correlation, coupling support, biology,
  causality, or clinical claim.

## Validation and claims

An eight-point `2x2` fixture must preserve exact factorization, fixed location loading one, positive
mark-loading prior, physical Matérn identity, and mandatory joint-versus-separate comparison.
