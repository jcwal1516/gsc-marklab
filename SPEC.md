# Marklab implemented contracts

This document describes implemented behavior. It is not a roadmap.

## Scientific scope

`marklab` analyzes spatial organization of a binary or probabilistic mark on
fixed cell positions. Its confirmatory null is fixed-position random labeling.
The current multimodal schema specializes in tumor-cell MMR-IHC workflows, but
the marked-pattern engine accepts any validated binary or probabilistic mark.
Outputs are section-level diagnostics, not evidence of clonality, same-cell
tracking, gain/loss, treatment response, or molecular MMR status.

## Rust API

The supported analysis operations are:

```rust,ignore
AnalysisEngine::analyze_pattern(&Pattern) -> Result<MarkedPatternResult>
MultimodalEngine::analyze(&MultimodalInput) -> Result<MultimodalResult>
OutputWriter::write(&ResultDocument, output_directory, &OutputSection)
    -> Result<OutputManifest>
```

Result format 0.2 is fixed by the library and cannot be configured. Its top
level is `format_version`, `provenance`, and the adjacently tagged `analysis`
enum (`kind` plus `result`). Format 0.1 is rejected with
`UnsupportedFormatVersion`; there is no converter.

Optional analysis and artifact state uses `available`, `disabled`,
`not_applicable`, or `insufficient_data`. Computation and I/O failures are
errors. Artifact write failures abort the operation. Empty analyses do not
create synthetic Parquet rows or placeholder territory data.

## Inference

Extreme-rank-length envelopes match CRAN GET 1.0-7 `type="erl"`: the observed
curve is included with all permutations; pointwise ties use average ranks;
two-sided ranks are `min(r, N + 1 - r)`; rank vectors are sorted and compared
lexicographically; identical vectors remain tied. Only normalized `erl_depth`
is public. Checked-in GET oracle vectors cover ordinary, pointwise-tie, and
identical-vector cases.

Scalar alternatives are fixed:

- one-sided high: low-k excess, anisotropy, coarse variance fraction, territory count;
- equal-tail two-sided: `xi_um` and fitted low-k exponent.

All scalar tests use inclusive ties and the plus-one correction. Equal-tail
tests require `B + 1 >= 2 / alpha`; envelopes require
`(B + 1) * alpha >= 1`. Undefined required null values are not dropped.

The maximum interpretable scale is
`largest_interpretable_scale_fraction * L_eff_um`. Spectrum wavelength is
`2*pi/k`. Only shells whose wavelength is within the limit, pair-correlation
points whose upper radius is within the limit, and wavelet scales within the
limit are inference eligible. Curve points may remain in output with
`inference_eligible: false`; they do not affect inference.

## Configuration 0.2

The authoritative example is [examples/config.toml](examples/config.toml).
Unknown and removed keys are rejected with a field path. Important fixed
controls are:

- `[analysis]`: `mark_label`, probabilistic marks, component mode;
- `[validation]`: sample, prevalence, area, shell, mask, and scale limits;
- `[spectrum]`, `[periodogram]`, `[wavelet]`;
- `[permutation]`: count, seed, stratification, typed strata fields;
- `[inference]`: `family_wise_alpha`;
- `[diagnostics]`: default-off beta-binomial and graph-smoothing diagnostics;
- typed registration, neighborhood, comparison, performance, and output controls.

Removed 0.1 keys and method-selection strings have no aliases.

## WSI

The default-off `wsi` feature pins `wsi-rs` 0.5.0. The official lockfile
resolves J2K 0.7.3; downstream library users may resolve a different
semver-compatible J2K patch.

The adapter exposes crate-owned slide metadata, region, plane-selection, and
RGBA types. Region coordinates are unsigned level-relative pixels. Indices are
zero-based. No implicit padding occurs. The first adapter supports U8 samples
only and returns straight, interleaved R/G/B/A bytes.

Guarantees are limited to the fixture matrix in
`tests/fixtures/wsi/manifest.json`; they are not guarantees for every upstream
container or codec variant.

## Non-goals

- Python bindings, wheels, or Python WSI support;
- segmentation, CellViT execution, viewers, or cell extraction from slides;
- GPU decoding, slide caching formats, or model training;
- real Bayesian inference or trained GNNs;
- backward-compatible 0.1 result or configuration readers;
- broad guarantees for formats outside the tracked WSI fixture matrix.
