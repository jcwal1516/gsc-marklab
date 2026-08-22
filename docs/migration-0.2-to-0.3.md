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
- `PairCorrelationPoint.value` is nullable. `null` with `count == 0` means the
  bin had no contributing cell pairs; it is not an observed correlation of
  zero.
- `CurveTestResult.statistic` is nullable and `availability` is typed.
  Insufficient comparisons include `unavailable_reason` instead of a fake
  statistic of zero.
- QC results add nullable `valid_tumor_fraction` and `valid_ihc_fraction`.
  Every QC fraction now uses all in-mask cells as its denominator;
  `valid_mask_fraction` specifically means the final retained fraction.
  Internal-control validity is no longer copied from that aggregate fraction.
- Marked results require `component_mode_selection`. `pooled` component results
  are now `not_applicable` rather than `available` with an empty vector, and
  `separate` no longer exposes pooled endpoints.
- Misleading wavelet terminology is removed without aliases:
  `wavelet` → `multiscale_residual`, `scalogram` → `scale_energy`,
  `scalogram_curve` → `scale_energy_curve`, and `wavelet_territories` →
  `residual_territories`. Summary fields use local-difference, residual-energy,
  and block-mean names. Residual territories have a distinct type with
  `analysis_scale_um`, `residual_score`, and `supporting_marked_cells`.
- Configuration `[wavelet]` is rejected; use `[multiscale_residual]`. Artifact
  filenames use the corresponding scale-energy and residual-territory names.
- A stratified spectrum declares
  `stratified_fixed_position_random_labeling` as its primary null.
- Mark-homogeneous spectrum strata are reported with
  `DegenerateSpatialStrataNull` and an insufficient-data spectrum rather than a
  numeric p-value of one.

Further field removals and renames required by the remediation plan will be
added here before 0.3 is release-ready.
