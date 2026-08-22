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
- `pair_correlation` and `pair_correlation_curve` are renamed to
  `mark_pair_covariance` and `mark_pair_covariance_curve`. The old keys are not
  accepted as aliases. `PairCorrelationPoint` is replaced by the semantically
  scoped `MarkPairCovariancePoint` and `CrossInteractionPoint` types.
- `MarkPairCovariancePoint.covariance` is nullable. `null` with
  `pair_count == 0` means the
  bin had no contributing cell pairs; it is not an observed correlation of
  zero.
- `CurveTestResult` is renamed to `CurveComparisonResult`, with typed
  `availability` and `method`. `p_difference` becomes
  `pooled_bin_p_value`; the name exposes that its null shuffles pooled,
  already-aggregated bins rather than cells or spatial labels.
- Marked-result `prepost_curve_tests` and pre/post-result `curve_tests` become
  `prepost_curve_comparisons` and `curve_comparisons`.
- Pre/post JSON is no longer an unversioned bare `PrePostResult`. It uses the
  normal 0.3 envelope with analysis kind `marked_prepost` or
  `multimodal_prepost`. Consumers must read the payload from
  `analysis.result`.
- `CurveComparisonResult.statistic` is nullable. Insufficient comparisons
  include `unavailable_reason` instead of a fake statistic of zero.
- Curve comparison fields `equivalence_margin` and `equivalent` are renamed to
  `margin` and `within_margin`. The never-computed `p_equivalence` placeholder
  is removed. These fields describe a threshold comparison, not an inferential
  equivalence test.
- `[comparison.equivalence_margins]` is renamed to `[comparison.margins]`
  without a compatibility alias.
- `[diagnostics].beta_binomial` is renamed to `beta_posterior_groups` without
  an alias. `DiagnosticsResult.beta_binomial`, `BetaBinomialSummary`, and
  `BetaBinomialGroupSummary` become `beta_posterior_groups`,
  `BetaPosteriorSummary`, and `BetaPosteriorGroupSummary`. The diagnostic name
  is `beta_posterior_group_summary_v1`.
- The interim `validate` CLI command is renamed to `smoke`, and its artifact is
  `smoke.json` rather than `validation.json`. Associated Rust types/functions
  use `SyntheticSmoke` and `run_*_synthetic_smoke` names. These scenarios are
  smoke checks, not calibration evidence; multimodal outcomes remain directly
  synthesized until COR-01 is remediated.
- The Task-derived `below_resolution_flag_rate` compatibility alias is removed;
  use `below_registration_resolution_flag_rate`.
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
- Marked results no longer contain multimodal-only placeholders for
  registration, fused cells, neighborhood enrichment, cross-interaction
  curves, territory profiles, or territory comparisons.
- The multimodal `TerritoryFeature` type becomes `NeighborhoodTerritory`.
  `supporting_cells` becomes `supporting_abnormal_cells`, and required
  `cluster_id` replaces optional `component_id`. The derived `scale_um`,
  ambiguous `z_or_power`, and unimplemented `qc_overlap_fraction` fields are
  removed without aliases.
- The unimplemented `ResidualTerritory.qc_overlap_fraction`,
  `TerritoryProfile.enrichment`, and `TerritoryProfile.cross_curves` fields are
  removed. Empty vectors or nulls from 0.2 must not be interpreted as completed
  analyses.
- Configuration `[wavelet]` is rejected; use `[multiscale_residual]`. Artifact
  filenames use the corresponding scale-energy and residual-territory names.
- Internal/public Rust names referring to a Bartlett periodogram are removed.
  The diagnostic is a Hann-tapered raster periodogram, and `low_k_shells` now
  counts radial shells rather than individual modes sorted by radius.
- A stratified spectrum declares
  `stratified_fixed_position_random_labeling` as its primary null.
- Marked results add `spectrum_null_sensitivity`. For a configured stratified
  spectrum this field records the primary-null identity, inference threshold,
  separate unstratified and stratified `p_global`/low-k p-values, and a typed
  confounding conclusion. It is `not_applicable` when stratification is not
  requested.
- Mark-homogeneous spectrum strata are reported with
  `DegenerateSpatialStrataNull` and an insufficient-data spectrum rather than a
  numeric p-value of one. Their sensitivity summary retains the evaluable
  unstratified inference and marks only the stratified inference insufficient.

Further field removals and renames required by the remediation plan will be
added here before 0.3 is release-ready.
