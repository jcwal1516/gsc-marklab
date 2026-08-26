# Task contract — BAY-EMBED-FACTOR-01 joint location–embedding latent-factor construction

Status: complete

Date: 2026-08-25

Parent requirements: BAY-01, BAY-04, BAY-PP, BAY-MM, WS-43, EMB-SPAT-01,
BAY-JOINT-CONT-MARK-01.

## User outcome

`marklab bayes build-joint-location-embedding-factor-model` builds a typed exact-grid point-location
and high-dimensional embedding latent-factor field artifact with rotational identifiability.

## Frozen behavior

- consume 8–2,000 unique finite points in an exact rectangle with location covariate/offset and 2–128
  exact `embedding_*` columns plus a complete regular location grid; require every embedding dimension
  and location covariate to vary materially and 1–16 factors less than both rows and dimensions;
- declare `embedding_i=W z_i+diagonal_noise`, zero-mean 2-D Matérn factor fields, and log location
  intensity depending on the factors. Fix rotation/sign by a lower-triangular first-K loading block with
  positive diagonal; use regularized shrinkage for remaining loadings/location-factor coefficients;
- retain exact dimensions/feature names/data/grid/kernel/prior/identifiability identities and choose
  HMC versus variational recommendation from declared bounded problem size;
- require validation against simpler vector variograms and kernel mark correlation. Construction only:
  no fitted factors, tissue domains, biology, causality, or clinical claim.

## Validation and claims

An eight-point three-dimensional embedding fixture with two factors must retain feature order, exact
K, positive-diagonal lower-triangular constraint, physical Matérn field semantics, and required simpler
spatial-summary comparisons.
