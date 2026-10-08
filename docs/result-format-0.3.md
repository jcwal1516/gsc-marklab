# Result format 0.3

Every Marklab analysis writes a `result.json` document in this format. This page lists
what the document contains and how missing values are represented.

## The document

```json
{
  "format_version": "0.3",
  "provenance": {
    "program": "marklab",
    "crate_version": "..."
  },
  "analysis": {
    "kind": "marked_pattern | multimodal | marked_prepost | multimodal_prepost",
    "result": {}
  }
}
```

`kind` says what produced the document:

| Kind | Produced by | `result` contains |
| --- | --- | --- |
| `marked_pattern` | `marklab analyze` | `MarkedPatternResult` |
| `multimodal` | `marklab multimodal analyze` | `MultimodalResult` |
| `marked_prepost` | `marklab prepost` | `PrePostResult` comparing two single-section results |
| `multimodal_prepost` | `marklab multimodal prepost` | `PrePostResult` comparing two multimodal results |

The two comparison kinds are kept separate so single-section and multimodal comparisons
cannot be mixed by accident. Comparison commands write `prepost.json` and accept either a
result file or a directory containing `result.json`.

## Conventions

- **No fake numbers.** Every number is finite. A statistic that cannot be calculated is
  `null`, usually with a reason field next to it. It is never written as `0`, infinity,
  `NaN`, or a string such as `"NaN"`. A real observed zero is written as `0`.
- **Fixed vocabularies.** Status values, class names, transform types, endpoint names,
  null models and band names come from fixed lists. Documents with unknown values or
  unknown fields are rejected when read.
- **Errors are not results.** If a computation fails, the command exits with an error and
  writes no result document.

Fixed lists used throughout:

- Analysis `status`: `ok` or `suppressed`.
- Interpretation `class`: `multimodal_summary`, `separate_components`,
  `suppressed_qc_artifact`, `suppressed`, `insufficient_data`, `coarse_excess`,
  `low_frequency_suppression` or `random_like`.
- Registration `transform_type`: `identity`, `rigid` or `affine`.

## Availability

Each measurement is wrapped in `AnalysisSection`, which has one of four states:

| State | Meaning |
| --- | --- |
| `available` | Computed |
| `disabled` | Turned off in the configuration |
| `not_applicable` | Does not apply to this kind of analysis or component mode |
| `insufficient_data` | Requested, but a requirement was not met; `reason` explains which |

An available empty list is a real answer only when the true result can be empty, such as
no residual territories found or no label pairs configured.

## Single-section results (`MarkedPatternResult`)

### Window length scales

`WindowSummary.analysis_effective_length_um` is the characteristic size of the analyzed
region. For a whole section loaded with a tissue mask it is the diameter of a circle with
the same area as the mask, `sqrt(4 * area_um2 / pi)`. For a single component, or a pattern
built in code without a recorded mask size, it is the diagonal of the bounding box of the
cell coordinates. A component whose cells all sit on one point has no length, so no
spectrum is computed for it.

`SpectrumSummary.max_interpretable_scale_um` is the largest scale the spectrum reports:
`largest_interpretable_scale_fraction * analysis_effective_length_um`. Every endpoint uses
this one definition.

### Mark-pair covariance

`mark_pair_covariance_curve` measures, at each distance, whether pairs of cells that far
apart tend to share the label. Each `MarkPairCovariancePoint.covariance` is the mean of
`(m_i - p_hat) * (m_j - p_hat)` over the cell pairs in that distance bin, where `m` is the
label and `p_hat` is the overall labeled fraction. A positive value means cells that far
apart tend to carry the same label (both labeled or both unlabeled), which is what
clustering of labeled cells produces.

A bin with no cell pairs has `pair_count = 0` and `covariance: null`. It stays in the curve
so the distance axis is complete, but it is left out of the global envelope test and has no
envelope bounds. `mark_pair_covariance` holds the global-envelope test result for the
whole curve.

This covariance is not the pair correlation function `g(r)` used in point-process
statistics.

### Spectrum null-model sensitivity

When the configuration shuffles labels within groups (stratified permutation),
`spectrum_null_sensitivity` checks whether a low-frequency signal is real or only reflects
those groups. It contains:

- `primary_null`: `stratified_fixed_position_random_labeling`;
- `family_wise_alpha`: the significance level used;
- `unstratified` and `stratified` inference sections, each with `p_global` and, when it
  can be computed, `low_k_excess_p_value`;
- `conclusion`: `confounded_by_spatial_strata`, `both_significant`,
  `no_unstratified_signal`, `degenerate_stratified_null` or `not_evaluable`.

The conclusion is `confounded_by_spatial_strata` only when the signal is significant
without groups and evaluable but not significant within groups. Both tests use the same
spectral modes and observed power and differ only in how labels are shuffled. If every
group has only one label value, the stratified section is `insufficient_data` and the
conclusion is `degenerate_stratified_null`. Without stratification the whole section is
`not_applicable`.

### Multiscale residuals

These measure how much of the label pattern sits in local differences versus broader
blocks, and find regions where labeled cells are over-represented:

- `multiscale_residual` (`MultiscaleResidualSummary`): `local_difference_energy_fraction`,
  `residual_energy_fraction`, `block_mean_variance_fraction` and
  `block_mean_to_local_difference_ratio`. These are normalized heuristic scores.
- `scale_energy` (`FunctionalSummary`) and `scale_energy_curve` (`ScaleEnergyPoint[]`):
  signal energy across scale bands, with permutation envelopes.
- `residual_territories` (`ResidualTerritory[]`): regions with an excess of labeled
  cells, each with `analysis_scale_um`, `residual_score` and `supporting_marked_cells`.

Files: `scale_energy.parquet`, `scale_energy.svg`, `residual_territories.geojson` and
`residual_territory_overlay.svg`.

The raster spectrum check uses a Hann-tapered periodogram. Its low-frequency value groups
all frequencies into fixed physical rings before taking the configured number of
lowest-frequency rings.

### Component modes

Separate tissue pieces (components) can be analyzed pooled, separately or both.
`component_mode_selection` records the requested mode, the resolved behavior (`pooled`,
`separate` or `both`) and why. `auto` resolves to `both` when there is more than one
component and the largest holds less than 80% of the cells, and to `pooled` otherwise.

In `separate` mode the pooled endpoint, spectrum, mark-pair covariance, anisotropy,
multiscale residuals and pooled curves are `not_applicable`, and the per-component
summaries carry the results. In pooled mode, per-component results are `not_applicable`.

### Input quality fractions

Every input QC fraction divides by the number of cells inside the tissue mask:

- `valid_tumor_fraction`, `valid_ihc_fraction` and `internal_control_valid_fraction`
  count cells with each valid state;
- `artifact_excluded_fraction` and `nonviable_excluded_fraction` count each exclusion
  flag, so a cell excluded for both reasons counts in both;
- `valid_mask_fraction` is the share of cells kept after all filters.

An optional fraction is `null` only when the input did not provide that information. If no
cells are inside the mask, no result is produced.

### Exploratory beta posterior

When enabled, `DiagnosticsResult.beta_posterior_groups` (`BetaPosteriorSummary`) reports
the labeled fraction overall and per component or coordinate quadrant, each with an
independent `Beta(1, 1)` prior. It is a simple descriptive summary: it does not fit a shared
overdispersion model, and it is not a primary endpoint.

## Multimodal results (`MultimodalResult`)

### Neighborhood enrichment

`NeighborhoodEnrichmentResult` asks whether two cell types are neighbors more often than
when labels are shuffled:

| Field | Type | Meaning |
| --- | --- | --- |
| `observed_edges` | integer | Neighbor links between the two labels in the real section |
| `expected_edges` | number | Average number of links in shuffled sections |
| `enrichment_ratio` | number or `null` | `observed_edges / expected_edges` |
| `enrichment_ratio_unavailable_reason` | optional | `zero_expected_edges` or `non_finite_computation` |
| `z_score` | number or `null` | Standardized distance from the shuffled average |
| `z_score_unavailable_reason` | optional | `zero_null_variance`, `insufficient_null_samples` or `non_finite_computation` |
| `p_value` | number or `null` | One-sided permutation p-value (more links than chance), counting the observed value as one of the permutations |
| `q_value` | number or `null` | Benjamini–Hochberg adjusted p-value, when calculated |

The p-value can be available even when the ratio or z-score is not. CSV exports add the
two reason columns, Parquet exports make the ratio and z-score columns nullable with
reason columns, and reports print `undefined (<reason>)`.

### Cross-interaction curves

`CrossInteractionCurve` counts pairs of the two requested labels by distance. Bins include
their lower edge and exclude their upper edge, so a pair exactly `max_r_um` apart is not
counted. The same distance plan is reused for the real labels, every label pair and every
permutation.

A bin with no cells that far apart has `value: null`, `inference_eligible: false` and no
envelope bounds. A bin where cells are that far apart but none are the requested pair has
`value: 0`, a real zero. `lower_global_envelope`, `upper_global_envelope` and `p_global`
come from one extreme-rank-length global envelope test over all eligible bins.

### Territories

Multimodal territories (`NeighborhoodTerritory`) are DBSCAN clusters of abnormal cells,
with `center_x_um`, `center_y_um`, `radius_um`, `supporting_abnormal_cells` and
`cluster_id`. `TerritoryProfile` gives the cell-type fractions inside a territory and
`below_registration_resolution`, which is true when the territory is smaller than the
alignment error between the two sections.

`RegistrationSummary` has no separate success flag: if registration fails its checks, the
run fails and no document is written.

## Comparisons (`PrePostResult`)

Each `CurveComparisonResult` has `availability` (`available` or `insufficient_data`) and a
`method`:

- `pooled_bin_permutation` gives a `statistic` and `pooled_bin_p_value`. It shuffles the
  already-summarized bin values, so it is not a spatial or per-cell permutation test.
- `descriptive_margin` compares the statistic with an optional threshold and has no
  p-value. `within_margin` is true when `max_abs_standardized_difference <= margin`. This
  is a descriptive check, not an equivalence test. With no margin set in advance, `margin`
  and `within_margin` are `null`.

An unavailable comparison has `statistic: null` and a reason; failures never show a
statistic of `0`.

The two curves must use the same distance axis. Axis values count as equal when
`|a-b| <= 1e-12 + 1e-12 * max(|a|, |b|)`. A mismatch makes the comparison
`insufficient_data`, with axis diagnostics.

Single-section results never contain comparison fields; comparisons exist only in the
`marked_prepost` and `multimodal_prepost` kinds. Single-section results also contain no
multimodal fields (registration, fused cells, neighborhood enrichment, cross-interaction or
territory profiles).

## Changes from 0.2

Marklab can read a 0.2 single-section document and convert the parts with an unambiguous
0.3 meaning. It rejects 0.2 multimodal documents, documents with filled-in legacy
multimodal or curve-test fields, unsupported null conventions, malformed shapes and unknown
fields. Regenerate those from the original inputs; the converter never guesses a missing
value or result.

What changed:

- `WindowSummary.l_eff_um` became `analysis_effective_length_um`, with the single
  definition above.
- The wavelet/MODWT fields were replaced by the multiscale residual fields, because the
  method was never a wavelet transform. The spectrum check is a Hann-tapered periodogram,
  not a Bartlett estimator.
- The unimplemented `qc_overlap_fraction` field was removed.
- `equivalence_margin`, `p_equivalence` and `equivalent` were replaced by the
  descriptive `margin` and `within_margin`.
- `prepost_curve_comparisons` was removed from single-section results; reading it is an
  error.
- The generic `TerritoryFeature` type (`scale_um`, `z_or_power`, optional `component_id`)
  was replaced by `NeighborhoodTerritory`, and the never-filled `enrichment` and
  `cross_curves` lists were removed from `TerritoryProfile`.
