# Migrating Result Format 0.2 to 0.3

Marklab now emits result format 0.3. The automatic 0.2 converter is not yet
implemented; until it is available, rerun the original inputs to produce a 0.3
document. Readers reject a 0.2 document rather than guessing at missing-state
semantics.

## Changes implemented so far

- A configured `rigid` registration now means orientation-preserving rotation
  plus translation. It no longer performs scale plus translation, and its
  `transform_type` metadata is `rigid`.
- `NeighborhoodEnrichmentResult.enrichment_ratio` changed from a required
  number to a nullable finite number.
- `NeighborhoodEnrichmentResult.z_score` changed from a required number to a
  nullable finite number.
- `enrichment_ratio_unavailable_reason` and
  `z_score_unavailable_reason` explain undefined values.
- Sparse enrichment no longer serializes infinity, and zero null variance no
  longer appears as a z-score of zero.
- A stratified spectrum declares
  `stratified_fixed_position_random_labeling` as its primary null.
- Mark-homogeneous spectrum strata are reported with
  `DegenerateSpatialStrataNull` and an insufficient-data spectrum rather than a
  numeric p-value of one.

Further field removals and renames required by the remediation plan will be
added here before 0.3 is release-ready.
