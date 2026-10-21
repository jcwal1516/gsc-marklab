# Marklab specification

This document gives the exact definitions behind Marklab's statistics, inference and
outputs, as currently implemented. For an overview, start with the [README](README.md).

## Scientific scope

The marked-pattern engine measures how a label, either yes/no or a probability, is arranged
across cells whose positions are fixed. Its random-labeling tests compare the real section
with versions in which labels are shuffled among the same cells. Other workflows have
different declared nulls: classical spatial tests simulate locations, pathology spatial
studies condition on fitted intensity, and cohort tests permute independent patients.
The multimodal workflow is built for tumor-cell MMR immunohistochemistry; the single-section
engine works with any label. Single-section results describe one tissue section. They do not show clonality, track
cells between sections, detect gain or loss of expression, or determine treatment response
or molecular MMR status.

## Rust API

The result-document API includes:

```rust,ignore
AnalysisEngine::analyze_pattern(&Pattern) -> Result<MarkedPatternResult>
MultimodalEngine::analyze(&MultimodalInput) -> Result<MultimodalResult>
OutputWriter::write(&ResultDocument, output_directory, &OutputSection)
    -> Result<OutputManifest>
```

Result format 0.4 is fixed by the library and cannot be configured. Its top
level is `format_version`, `provenance`, and the adjacently tagged `analysis`
enum (`kind` plus `result`). Unknown versions are rejected with
`UnsupportedFormatVersion`. The reader accepts only 0.4; 0.2 and 0.3 documents
must be regenerated from the original inputs. There is no legacy converter.
The supported kinds are `marked_pattern`, `multimodal`, `marked_prepost`, and
`multimodal_prepost`. Both pre/post commands accept either a result file or the
directory containing `result.json`.

JSON decimal parsing uses serde_json's `float_roundtrip` mode so finite binary64
values written by the serializer retain their bits when read back. Strict schemas,
semantic checks, integer handling and resource limits still apply. Existing tagged-bit
artifact codecs remain readable. This is a numerical parsing correction within the
existing formats, not a change to statistical definitions. Cache identities change when
removing a formerly lossy decode/re-encode step changes canonical artifact bytes.

Cell-embedding provenance version 2 records the producer's declared vector dimension,
pooling, normalization, input channels, modality and stain. Its verified artifact graph
binds generic CSV/NPY sources without requiring CellViT-specific content labels or
calibration. Version 1 keeps the original fixed CellViT contract and canonical reader.
The generic NPY reader enforces finite, little-endian, row-major float32 values and the
existing byte/shape limits; the legacy CellViT wrapper additionally requires 1,280 columns.
The vector-semivariogram CLI's optional separate NPY input uses a version-2 result with
both source identities. Its statistic and the existing CSV-only version-1 result are
unchanged. These artifact versions do not change the main result format 0.4.

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

Categorical co-occurrence counts ordered pairs of distinct cells within each declared
physical radius, including coincident cells and the radius boundary. If `C_ab` is the
class-pair count, `T` the total directed count and `D_a`, `D_b` the corresponding
edge-incidence marginals, its score is `C_ab T / (D_a D_b)`. A zero marginal makes
the score unavailable. This is an edge-incidence normalization, not a global
cell-frequency normalization or a point-process pair-correlation function. See
[categorical co-occurrence](docs/categorical-cooccurrence.md).

Categorical neighborhood enrichment uses one fixed radius graph and one global
fixed-count label-permutation stratum. Exact edge-indicator moments are reported
separately from Monte Carlo mean, sample-standard-deviation z-score and inclusive
one-sided enrichment p-value. The exact standardized descriptor does not imply a
Gaussian null. Its version-1 result leaves existing multimodal enrichment fields
and result format 0.4 unchanged. See [the null and limits](docs/categorical-enrichment.md).

Group centrality uses an unweighted undirected radius graph. Group degree is the
number of distinct external neighbors divided by the number of nodes outside the
group. Group closeness follows NetworkX: that same outside-node count is divided
by the sum of shortest distances from the group to reachable outside nodes;
unreachable nodes add zero to the denominator. It returns zero when that sum is
zero and can exceed one on disconnected graphs. Average clustering includes zero
coefficients for leaves and isolates. Empty and all-node groups are rejected. See
[group centrality](docs/group-centrality.md) for the scientific limits.

Adaptive polygon-window SPDE factor fits use bounded block-coordinate warmup followed
by L-BFGS-B within the same caller-declared total iteration budget. The latter uses
`ftol = 10 * eps64` and the existing `gtol = 1e-7`. A finite fitted state is accepted
only when the raw gradient infinity norm is at most `2e-4`; an optimizer's successful
relative-objective stop cannot override that requirement. Model definitions and result
field meanings are unchanged. See [backend convergence](docs/python-backends.md#adaptive-spde-convergence).

The stable numerical primitives use the sample `n - 1` covariance definition when no
covariance weights are supplied. The implementation retains `1/n`-scaled compensated
cross products and divides by the directly computed `(n - 1)/n`, avoiding the extra
rounding in `1 - sum((1/n)^2)`. Unweighted effective sample size is exactly `n`.
Caller-weighted reliability covariance retains its effective-weight denominator.
This is a numerical correction, with no change to field meanings or artifact version.

Translation edge correction weights an ordered displacement by the reciprocal area of
`W intersect (W + h)`. For a validated single axis-aligned rectangular window without
holes, Marklab evaluates that same overlap analytically as
`max(0, width - |h_x|) * max(0, height - |h_y|)`. This avoids coordinate-cancellation
error in a polygon Boolean operation while preserving the estimator. All other polygon,
hole, and multipolygon windows continue through the bounded Boolean-intersection path.

Global Moran's I and Geary's C use the explicitly named radius-weight policy. The
existing `binary_symmetric` and `row_standardized` policies reject any isolated
observed row. The opt-in `binary_symmetric_including_isolates` retains all observed
rows, including isolates, in the sample size `n`, mean, variance denominator and
random-labeling population. Isolates have zero weights. With `S0 = sum(w_ij)` and
`D = sum((x_i - mean(x))^2)`, the statistics are
`I = n / S0 * sum(w_ij (x_i-mean(x)) (x_j-mean(x))) / D` and
`C = (n-1) / (2 S0) * sum(w_ij (x_i-x_j)^2) / D`.
Both require `S0 > 0` and `D > 0`; a zero-edge multiplex channel reports
`no_spatial_edges`. No cell or patient is silently removed. This adds an explicit
policy value without changing existing serialized definitions or result format 0.4.
See [the public API conventions](docs/public-api.md#spatial-autocorrelation-weights).

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

Separate component spectra subset the configured permutation strata with the component's
cell rows. A component whose strata are all mark-homogeneous has a degenerate null and reports
an unavailable endpoint and p-value. Declared scalar inputs compare any retained source cell
IDs against the supplied row identities. Direct marked-pattern analysis requires every supplied
probability row to be finite, in `[0, 1]`, and aligned with the cell rows.

Kernel pair-correlation estimates with positive normalization remain zero when no pair
contributes to the kernel. This applies to homogeneous and intensity-reweighted `g` and
cross-type `g`, with border, isotropic or translation correction. The
`no_pairs_in_kernel_support` status describes empty support; the estimate can be zero and
inference-eligible. Missing eligible centers or an undefined normalization remains unavailable.
Conditional averages such as mark-pair covariance still require contributing pairs.

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
format 0.4 persists both inference summaries, the threshold, primary-null
identity, and typed conclusion in `spectrum_null_sensitivity`; an unavailable
member is a tagged state rather than a numeric sentinel.

Marked and multimodal result schemas are disjoint. Marked results do not carry
registration or neighborhood placeholders. Multimodal DBSCAN output uses a
`NeighborhoodTerritory` with an explicit abnormal-cell support count and
cluster ID; unimplemented profile enrichment/cross-curve and QC-overlap fields
are omitted from format 0.4.

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
