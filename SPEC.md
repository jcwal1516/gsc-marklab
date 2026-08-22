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

Result format 0.3 is fixed by the library and cannot be configured. Its top
level is `format_version`, `provenance`, and the adjacently tagged `analysis`
enum (`kind` plus `result`). Older and unknown versions are rejected with
`UnsupportedFormatVersion` while the 0.2 migration converter is pending.
The supported kinds are `marked_pattern`, `multimodal`, `marked_prepost`, and
`multimodal_prepost`. Both pre/post CLIs accept either a result file or the
directory containing `result.json` through one resolver.

Optional analysis and artifact state uses `available`, `disabled`,
`not_applicable`, or `insufficient_data`. Computation and I/O failures are
errors. Artifact write failures abort the operation. Empty analyses do not
create synthetic Parquet rows or placeholder territory data.

Output writers validate the result and core artifact plan before commit, write
all configured run artifacts into a temporary sibling directory, validate
required files, and rename the completed directory into place. They reject
non-empty and symbolic-link targets. A failed transaction removes its temporary
directory and does not expose a new final run directory.

Mark-pair covariance at a distance bin is the mean of
`(m_i - p_hat) * (m_j - p_hat)` over contributing cell pairs. It is not the
density-normalized point-process function commonly denoted `g(r)`.
Bins with no contributing pairs remain on the physical axis
with `count = 0` and `value = null`; they are excluded from inference. Curve
comparisons use typed availability and a nullable statistic, so an unavailable
test cannot be mistaken for an observed statistic of zero.

Pre/post spectrum, mark-pair-covariance, and cross-interaction axes compare finite
values with `|a-b| <= 1e-12 + 1e-12 * max(|a|, |b|)`. This accepts harmless
floating-point reconstruction while preserving a typed axis-mismatch result
for materially different bins or modes.

Input QC fractions use all cells inside the tumor mask as their denominator.
`valid_tumor_fraction`, `valid_ihc_fraction`, and
`internal_control_valid_fraction` count each independent validity flag;
artifact and nonviable fractions count their independent exclusion flags; and
`valid_mask_fraction` is the final retained fraction after all filters. An
optional fraction is absent when its source column is unavailable. Overlapping
exclusions are counted in every applicable fraction, and a present but blank
internal-control value is invalid. A zero in-mask denominator is an input
error, not a numeric zero fraction.

Component spectrum modes are behaviorally distinct. `pooled` emits only pooled
endpoints, `separate` emits component summaries and marks every pooled endpoint
not applicable, and `both` emits both. `auto` selects `both` only when there are
multiple components and the largest contains less than 80% of cells; otherwise
it selects `pooled`. Every result records the requested mode, resolved mode, and
selection reason.

The `[multiscale_residual]` analysis is a three-part heuristic. It computes
mean squared horizontal/vertical neighbor differences, variance across 2x2
block means relative to total raster variance, and a normalized residual share.
These values are not transform coefficients. Residual territories are circular
marked-cell neighborhoods whose binomial standardized residual exceeds
`min_territory_z`, followed by greedy overlap suppression. The scale-to-radius
rule is `radius_um = sqrt(2) * analysis_scale_um`; no Gaussian filtering is
performed. QC overlap is `null` until an actual overlap calculation exists.

The periodogram diagnostic rasterizes centered marks, applies one separable
Hann taper, and computes one 2-D FFT. Radial annuli use width
`1 / (max(raster_width, raster_height) * cell_size_um)`. Power is averaged over
all modes in each nonempty annulus, and `spectrum.low_k_shells` selects the
lowest nonempty shell means with equal weight. This is a Hann-tapered raster
periodogram, not a Bartlett segment-averaged estimator.

## Inference

Extreme-rank-length envelopes match CRAN GET 1.0-7 `type="erl"`: the observed
curve is included with all permutations; pointwise ties use average ranks;
two-sided ranks are `min(r, N + 1 - r)`; rank vectors are sorted and compared
lexicographically; identical vectors remain tied. Only normalized `erl_depth`
is public. Checked-in GET oracle vectors cover ordinary, pointwise-tie, and
identical-vector cases.

Scalar alternatives are fixed:

- one-sided high: low-k excess, anisotropy, block-mean variance fraction, territory count;
- equal-tail two-sided: `xi_um` and fitted low-k exponent.

All scalar tests use inclusive ties and the plus-one correction. Spectrum
scalar reference curves use leave-one-out median whitening: each of the
`B + 1` curves is normalized by the shellwise median of the other `B` curves.
The reported observed whitening is unchanged because its reference remains the
median of the `B` permutation curves. Equal-tail tests require
`B + 1 >= 2 / alpha`; envelopes require `(B + 1) * alpha >= 1`. Undefined
required null values are not dropped.

The resulting scalar rank construction has a finite-sample validity guarantee
under independent exact-uniform random labeling with fixed strata, counts, and
inference-eligible shells. The implemented seeded deterministic shuffle is a
reproducible pseudorandom generator; it is not an exact-uniform sampling
certificate, so that mathematical guarantee does not apply literally to its
finite seed space.

The maximum interpretable scale is
`largest_interpretable_scale_fraction * L_eff_um`. Spectrum wavelength is
`2*pi/k`. Only shells whose wavelength is within the limit, mark-pair-covariance
points whose upper radius is within the limit, and multiscale residual scales
within the limit are inference eligible. Curve points may remain in output with
`inference_eligible: false`; they do not affect inference.

## Configuration 0.2

The authoritative example is [examples/config.toml](examples/config.toml).
Unknown and removed keys are rejected with a field path. Important fixed
controls are:

- `[analysis]`: `mark_label`, probabilistic marks, component mode;
- `[validation]`: sample, prevalence, area, shell, mask, and scale limits;
- `[spectrum]`, `[periodogram]`, `[multiscale_residual]`;
- `[permutation]`: count, seed, stratification, typed strata fields;
- `[inference]`: `family_wise_alpha`;
- `[diagnostics]`: default-off beta posterior group and graph-smoothing diagnostics;
- typed registration, neighborhood, comparison, performance, and output controls.

The `smoke` command runs deterministic synthetic-generator smoke checks and
writes `smoke.json`. These checks do not establish calibration. The current
multimodal smoke generator does not invoke the production multimodal engine;
that known limitation remains explicit until the real validation workflow
replaces it.

`registration.transform = "rigid"` is an orientation-preserving least-squares
two-dimensional rotation plus translation. It never estimates scale or fits a
reflection. `"affine"` permits the full configured affine model, including
scale and shear. Registration summaries serialize these models as `rigid` and
`affine`, respectively.

When `[permutation].stratified = true`, the stratified fixed-position null is
the primary spectrum null and an unstratified null is run as a sensitivity
analysis over the same modes and observed powers. The result is flagged as
confounded only when the unstratified low-k endpoint is significant at
`family_wise_alpha` and the evaluable stratified endpoint is not. If every
configured stratum is mark-homogeneous, the stratified spectrum null is
reported as degenerate and no numeric spectrum p-value is emitted. Result
format 0.3 persists both inference summaries, the threshold, primary-null
identity, and typed conclusion in `spectrum_null_sensitivity`; an unavailable
member is a tagged state rather than a numeric sentinel.

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
