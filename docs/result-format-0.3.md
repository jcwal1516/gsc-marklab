# Result Format 0.3

Result documents have one top-level version and one tagged analysis payload:

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

`marked_prepost` and `multimodal_prepost` contain a `PrePostResult` payload and
are deliberately distinct kinds so consumers cannot silently mix comparison
families. The CLI writes this envelope to `prepost.json`. Both pre/post commands
accept either a result document file or a directory containing `result.json`.

The writer rejects non-finite floating-point values before serialization. An
undefined scientific statistic is represented by an absent numeric value and,
where applicable, a typed reason. It is never represented by zero, infinity,
NaN, or a string containing one of those values.

## Neighborhood enrichment

`NeighborhoodEnrichmentResult` has these statistic fields:

| Field | Type | Meaning |
| --- | --- | --- |
| `observed_edges` | integer | Observed undirected edges for the requested label pair. |
| `expected_edges` | finite number | Mean edge count under the configured permutation null. |
| `enrichment_ratio` | finite number or `null` | `observed_edges / expected_edges`; undefined when the denominator is zero or the calculation is non-finite. |
| `enrichment_ratio_unavailable_reason` | optional enum | `zero_expected_edges` or `non_finite_computation`. |
| `z_score` | finite number or `null` | Standardized difference from the permutation mean. |
| `z_score_unavailable_reason` | optional enum | `zero_null_variance`, `insufficient_null_samples`, or `non_finite_computation`. |
| `p_value` | finite number or `null` | Inclusive one-sided-high permutation p-value with plus-one correction. |
| `q_value` | finite number or `null` | Benjamini-Hochberg adjusted p-value when calculated. |

A p-value can remain available when the ratio or z-score is undefined.

CSV exports add the two reason columns. Parquet exports make the ratio and
z-score columns nullable and add nullable UTF-8 reason columns. Reports print
`undefined (<reason>)` instead of a numeric placeholder.

## Mark-pair covariance

`MarkedPatternResult.mark_pair_covariance` contains the global-envelope summary,
and `mark_pair_covariance_curve` contains `MarkPairCovariancePoint` values.
Each point's `covariance` is the mean centered mark product
`(m_i - p_hat) * (m_j - p_hat)` for contributing pairs in that distance bin;
it is a finite number or `null`. A covariance is `null`
exactly when `pair_count == 0`, meaning no cell pair contributed to that physical
distance bin. Empty bins remain in the curve so bin axes stay explicit, but
they are excluded from global-envelope inference and have no envelope bounds.

## Descriptive curve margins

Curve comparison rows use `margin` and `within_margin`. The latter is a
descriptive check of whether `max_abs_standardized_difference <= margin`; it is
not an inferential equivalence test. The unused 0.2 fields
`equivalence_margin`, `p_equivalence`, and `equivalent` are absent from format
0.3. Without a prespecified margin, `margin` and `within_margin` are `null` and
the interpretation states that no margin assessment is available.

## Beta posterior group diagnostic

The optional marked-pattern diagnostic is serialized as
`DiagnosticsResult.beta_posterior_groups: BetaPosteriorSummary`. It reports
independent conjugate beta posteriors for the pooled mark prevalence and for
component or coordinate-quadrant groups under a fixed `Beta(1, 1)` prior. It
does not fit a beta-binomial marginal model or a shared overdispersion
parameter, and it remains exploratory rather than a primary endpoint.

## Multiscale residual diagnostics

The former wavelet/MODWT fields have been removed because the implementation
does not perform a wavelet transform. Marked results now expose:

- `multiscale_residual: AnalysisSection<MultiscaleResidualSummary>`;
- `scale_energy: AnalysisSection<FunctionalSummary>`;
- `scale_energy_curve: ScaleEnergyPoint[]`;
- `residual_territories: AnalysisSection<ResidualTerritory[]>`.

`MultiscaleResidualSummary` reports
`local_difference_energy_fraction`, `residual_energy_fraction`,
`block_mean_variance_fraction`, and
`block_mean_to_local_difference_ratio`. These are normalized heuristic scores,
not wavelet coefficients. `ResidualTerritory` reports `analysis_scale_um`,
`residual_score`, and `supporting_marked_cells`. Its
`qc_overlap_fraction` is nullable and remains `null` until an actual QC overlap
calculation is available.

Artifacts are named `scale_energy.parquet`, `scale_energy.svg`,
`residual_territories.geojson`, and `residual_territory_overlay.svg`.

The raster spectral QC diagnostic is implemented and named internally as a
Hann-tapered raster periodogram. It is not serialized as a Bartlett estimator.
Its low-frequency quantity aggregates all modes into deterministic physical
radial shells before selecting the configured number of low-k shells.

## Input QC fractions

Every input QC fraction uses the number of cells inside the tumor mask as its
denominator. `valid_tumor_fraction`, `valid_ihc_fraction`, and
`internal_control_valid_fraction` count their respective valid states.
`artifact_excluded_fraction` and `nonviable_excluded_fraction` count each
independent exclusion flag, including overlaps. The existing
`valid_mask_fraction` is the final retained fraction after all validity and
exclusion filters. Optional fractions are `null` only when the corresponding
input state is unavailable. No result is constructed for a zero in-mask
denominator.

## Curve comparisons

Every `CurveComparisonResult` has an `availability` of `available` or
`insufficient_data` and a typed `method`. A `pooled_bin_permutation` row has a
finite `statistic` and `pooled_bin_p_value`; it shuffles already-aggregated bin
values and is not a spatial or per-cell permutation test. A
`descriptive_margin` row compares that statistic with an optional threshold and
has no p-value. Available comparisons have no `unavailable_reason`. An
unavailable comparison contains `statistic: null` plus a
diagnostic reason. Failed or inapplicable comparisons never use a statistic of
zero as a placeholder.

Pre/post axis alignment requires equal lengths and finite values. Corresponding
axis values match when `|a-b| <= 1e-12 + 1e-12 * max(|a|, |b|)`. A material
mismatch produces an `insufficient_data` curve-comparison result with a null
statistic and axis diagnostics.

## Component modes

`component_mode_selection` records the requested configuration mode, the
resolved `pooled`, `separate`, or `both` behavior, and a non-empty selection
reason. Pooled component results are `not_applicable`, not an available empty
vector. In `separate` mode, the pooled primary endpoint, spectrum,
mark-pair-covariance, anisotropy, multiscale residual summaries, and pooled curves are
`not_applicable`; component summaries carry the available component-specific
inference. `auto` resolves to `both` when more than one component exists and the
largest component fraction is below 0.80, otherwise to `pooled`.

## Current scope

This document will be expanded as the remaining 0.3 model cleanup is
implemented. The complete 0.2 converter remains open work; readers currently
reject older versions.
