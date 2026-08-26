# Task contract — EMB-VARIO-01 omnibus vector semivariogram

Status: complete

Date: 2026-08-25

Parent requirements: EMB-01, MRK-02D, SIG-01F, SIG-01H, WS-32, WS-50,
BAY-EMBED-FACTOR-01.

## User outcome

`marklab bayes vector-semivariogram` computes the exact rotation-invariant embedding dispersion curve
required as a simpler validation comparator for the joint location–embedding factor model.

## Frozen behavior

- consume 2–10,000 unique finite objects with micrometre coordinates and 2–4,096 exact ordered
  `embedding_*` columns, plus 1–256 unique contiguous physical distance bins;
- use each eligible unordered pair exactly once, assigning distances to `[lower, upper)` bins except
  the final bin, whose upper edge is inclusive; pairs outside the declared bin range are ineligible;
- accumulate `0.5 * weight * ||embedding_i - embedding_j||^2` and positive pair weights in `f64`
  using compensated sums, with a caller-declared bound on all visited unordered pairs;
- use unit weights when no weight table is supplied. A supplied table must contain exactly one finite
  positive weight for every eligible unordered pair and no ineligible or duplicate pair;
- report physical bin boundaries, pair counts, weight sums, optional semivariance, and inference
  eligibility. Empty bins are typed unavailable, never NaN; inference eligibility requires at least
  two contributing pairs;
- retain exact input, bin, and optional weight content identities. This exact descriptive curve has
  no edge correction, null distribution, p-value, patient-level inference, real-asset provenance, or
  biological interpretation and does not close the broader EMB-01/WS-50 requirements.

## Validation and claims

A three-object two-dimensional hand fixture must produce semivariances `2` and `8/3` under declared
pair weights, preserve boundary assignment at the shared bin edge, and agree after an orthogonal
rotation of every complete embedding vector.
