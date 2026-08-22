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
    "kind": "marked_pattern | multimodal",
    "result": {}
  }
}
```

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

## Current scope

This document will be expanded as the remaining 0.3 model cleanup is
implemented. Pre/post document versioning and the complete 0.2 converter remain
open work; readers currently reject older versions.
