# Marklab specification

This document gives the exact definitions behind Marklab's statistics, inference and
outputs, as currently implemented. For an overview, start with the [README](README.md).

## Scientific scope

Marklab measures how a label, either yes/no or a probability, is arranged across cells whose
positions are fixed. Every confirmatory test compares the real section with versions in
which the labels are shuffled among the same cells (random labeling). The multimodal
workflow is built for tumor-cell MMR immunohistochemistry; the single-section engine works
with any label. Results describe one tissue section. They do not show clonality, track
cells between sections, detect gain or loss of expression, or determine treatment response
or molecular MMR status.

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
enum (`kind` plus `result`). Unknown versions are rejected with
`UnsupportedFormatVersion`. The reader has a narrow 0.2 marked-pattern
converter for fields with unambiguous semantics; unsafe legacy states and 0.2
multimodal documents return a schema error telling you to rerun the analysis.
The supported kinds are `marked_pattern`, `multimodal`, `marked_prepost`, and
`multimodal_prepost`. Both pre/post commands accept either a result file or the
directory containing `result.json`.

## Availability and output files

Optional analysis and artifact state uses `available`, `disabled`,
`not_applicable`, or `insufficient_data`. Computation and I/O failures are
errors. Artifact write failures abort the operation. Empty analyses do not
create synthetic Parquet rows or placeholder territory data.

Output writers validate the result and core artifact plan before commit, write
all configured run artifacts into a temporary sibling directory, validate
required files, and rename the completed directory into place. They reject
non-empty and symbolic-link targets. A failed transaction removes its temporary
directory and does not expose a new final run directory.

## Statistics

Mark-pair covariance at a distance bin is the mean of
`(m_i - p_hat) * (m_j - p_hat)` over contributing cell pairs. It is not the
density-normalized point-process function commonly denoted `g(r)`.
Bins with no contributing pairs remain on the physical axis
with `count = 0` and `value = null`; they are excluded from inference. Curve
comparisons carry an availability state and a statistic that can be `null`, so an
unavailable test cannot be mistaken for an observed statistic of zero.

Multimodal cross-interaction curves likewise preserve every configured
distance bin. A bin with no geometric pair is null and inference-ineligible; a
geometrically available bin with no requested-label pair is an observed zero.
One indexed pair/bin plan is reused for every label pair and permutation.
Envelope bounds and `p_global` are the checked ERL global envelope over the
eligible count curves, not pointwise permutation extrema or a separate
maximum-bin statistic.

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
performed. Residual territories have no QC-overlap field.

The periodogram diagnostic rasterizes centered marks, applies one separable
Hann taper, and computes one 2-D FFT. Radial annuli use width
`1 / (max(raster_width, raster_height) * cell_size_um)`. Power is averaged over
all modes in each nonempty annulus, and `spectrum.low_k_shells` selects the
lowest nonempty shell means with equal weight. This is a Hann-tapered raster
periodogram, not a Bartlett segment-averaged estimator.
A taper or tapered field with no finite positive energy is unavailable, including a
one- or two-pixel dimension and boundary-only signal. Zero power is not used as an
unavailability sentinel.

The optional gridding diagnostic separately reports raster resolution and a matched
binary-mark comparison: identical bin-based Hann cell weights, identical physical
Fourier bins with wavelength no greater than the common maximum scale, and identical
shell normalization. The direct sum uses original cell coordinates; the FFT uses
assigned bin coordinates. The lowest requested shell means are divided by their sum
plus one reference shell. Relative difference above 25% is a descriptive sensitivity
warning, **not a calibrated agreement test**. Missing signal, reference bands, or work
capacity produces an explicit unavailable explanation. A negative artifact flag does
not validate the original permutation-whitened spectrum. Probability-mode spectra
remain separate from this explicitly binary-mark diagnostic.

Raster edge assignment tolerates only bounded floating-point roundoff; points
meaningfully below an edge stay in the lower pixel. The direct and FFT paths reuse
the same assigned pixels for Hann weights. Annuli use dimensionless DFT indices,
and selection counts nonempty eligible shells, including the next nonempty
reference shell even when a lower shell is empty.

Graph Fourier bands keep numerically unresolved eigenspaces together. Dense band
assignment uses eigenpair residuals and an operator-scale roundoff floor; an
eigenspace intersecting one band edge is assigned as exactly on that half-open
edge, while multiple unresolved edges are rejected. Observed and permuted band
energies use the same membership. Sparse low-mode extraction uses deterministic
dense starts and an independently seeded, deflated complement probe; a converged
probe exposing a missed lower mode rejects the basis. Residual and cutoff checks
remain bounded numerical evidence, not an exact eigenvalue-count certificate.

Anisotropy applies the same maximum-wavelength limit before run-symmetric whitening;
all observed and randomized runs use the median of the other runs at each mode.
A common positive scaling of the frequency tensor does not change its eigenvalue ratio
or direction; the tensor is normalized before applying the numerical eigenvalue floor.
A required undefined tensor makes anisotropy unavailable. The configured anisotropy
radius remains a maximum radial index, so a restricted physical range may contain no
eligible anisotropy modes. The `xi_um` readout denotes a selected peak wavelength,
not an independently estimated correlation-decay length or calibrated confidence interval.


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
The observed reference remains the median of the `B` permutation curves. Equal-tail tests require
`B + 1 >= 2 / alpha`; spectral division uses a shell-relative roundoff floor shared by
all `B + 1` runs, so small mark amplitudes do not suppress dimensionless whitening.
Identically zero powers retain zero summaries. Envelopes require
`(B + 1) * alpha >= 1`. Undefined
required null values are not dropped.

The resulting scalar rank construction has a finite-sample validity guarantee
under independent exact-uniform random labeling with fixed strata, counts, and
inference-eligible shells. The implemented seeded deterministic shuffle is a
reproducible pseudorandom generator; it is not an exact-uniform sampling
certificate, so that mathematical guarantee does not apply literally to its
finite seed space.

`analysis_effective_length_um` has one explicit precedence rule. Loaded cell
tables use the equivalent-area diameter of the tumor mask,
`sqrt(4 * area_um2 / pi)`. Component analyses, and programmatically constructed
patterns without a positive recorded mask length, use the diagonal of the
axis-aligned cell-coordinate bounding box. A degenerate point set has no
component effective length.

The maximum interpretable scale is calculated in one shared path as
`largest_interpretable_scale_fraction * analysis_effective_length_um`.
Spectrum wavelength is `2*pi/k`. Only shells whose wavelength is within the limit, mark-pair-covariance
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
writes `smoke.json`. Marked and multimodal scenarios invoke their production
engines, and pre/post scenarios use production comparison services. The output
retains attempted/completed/failed denominators, failure reasons, Wilson
intervals, seed, configuration, and engine version. Quick smoke thresholds do
not establish calibration; the separate 1,000-replicate scheduled
random-label control and its one-sided nominal-alpha acceptance are defined in
`docs/validation-methodology.md`.

`performance.memory_budget_mib` is an enforced multimodal execution limit.
The engine reserves conservative input, fused-cell, label, index, graph,
artifact, result, and telemetry storage before allocation; output-sensitive
graph, cross-interaction, and territory-neighborhood builders stop before the
next over-budget entry. Sequential permutation, ERL, profile, and diagnostic
scratch contributes to the same reported peak. Every timing row reports that
enforced conservative peak, which never exceeds the configured limit for a
successful run.

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

Marked and multimodal result schemas are disjoint. Marked results do not carry
registration or neighborhood placeholders. Multimodal DBSCAN output uses a
`NeighborhoodTerritory` with an explicit abnormal-cell support count and
cluster ID; unimplemented profile enrichment/cross-curve and QC-overlap fields
are omitted from format 0.3.

Configuration keys and method names removed after version 0.1 are not accepted
under their old names.

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

## Out of scope

- Python bindings, wheels or Python whole-slide support. The Python client in
  `clients/python` runs the `marklab` program; it is not a binding.
- Cell segmentation, running CellViT, slide viewers, or extracting cells from slides.
- GPU decoding, slide caching formats or model training.
- Trained graph neural networks. The graph message-passing diagnostic is a fixed
  calculation, not a learned model.
- Reading 0.1 result or configuration files.
- Guarantees for slide formats outside the WSI fixtures listed in
  `tests/fixtures/wsi/manifest.json`.

Bayesian models are in scope: they run in the pinned Python environment described in
[Python backends](docs/python-backends.md).
