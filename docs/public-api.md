# Public Rust API

The supported Rust API is what `src/lib.rs` exports. The core analysis types (engine,
configuration, cell patterns, results, multimodal runs, pre/post comparison, registration,
slides and errors) sit at the crate root. Method families live in public modules:

| Module | Contents |
|---|---|
| `marklab::spatial` | K/L functions and pair correlation (homogeneous, isotropic, translation, inhomogeneous and piecewise-compartment), cross-type pair correlation, nearest-neighbour spacing, and the arbitrary-window Poisson likelihood |
| `marklab::marks` | categorical, continuous, ordinal, probability and soft-class mark statistics, mark-weighted K, neighbourhood composition, the `MarkTable` mark columns, radius-dependent categorical co-occurrence (`categorical_cooccurrence`, `categorical_cooccurrence_from_json`; see [co-occurrence](categorical-cooccurrence.md)) and categorical neighbourhood enrichment with exact null moments (`categorical_enrichment_from_json`; see [enrichment](categorical-enrichment.md)) |
| `marklab::autocorrelation` | global Moran and Geary, local multivariate Moran, scalar variograms, and per-marker assay summaries |
| `marklab::compartments` | compartment partitions and the cell-mixing, contact, fragmentation and interface analyses |
| `marklab::embeddings` | cell, patch, region and slide embedding artifacts, their CSV, Arrow and Parquet readers and writers, and the embedding analyses |
| `marklab::workflow` | workflow graphs and the local scheduler, the marked-analysis nodes, and the marked pre/post DAG |
| `marklab::pathology` | pathology maps, pathology studies and multiplex studies |
| `marklab::backend` | locating the pinned Python backend environment |

Each method's durable workflow node (`*AnalysisNode`) is exported next to the method, for
example `marklab::spatial::ClassicalSpatialAnalysisNode`. Internal modules are private.

**Stability.** The crate is pre-1.0, so the Rust API follows normal semver rules for
0.x versions. The JSON written to `result.json` is a separate, stricter contract
([result format 0.4](result-format-0.4.md)): its shape and field names do not change
within 0.4. Experimental workflows write their own versioned documents, separate from
0.4. For example, the categorical-pair workflow writes `marklab.categorical-pair/2`;
its fields and inference semantics are documented in
[directed categorical pairs](categorical-pairs.md).

## Analyze one section

Run the label-shuffling analysis on one tissue section.

- `AnalysisEngine`, `MarkedAnalysisRun`: the entry point. `MarkedAnalysisRun` keeps the
  computed data so that output files can be written without recomputing.
- `Pattern`, `PatternMeta`, `TumorWindow`: build a validated set of cells and the region
  they occupy directly in memory.
- `PatternLoader`, `PatternLoadResult`, `PatternLoadDiagnostics`, `TumorMask`: read cell
  tables (CSV or Parquet) and a tissue mask from disk into a `Pattern`, with a report of
  which rows were kept.

## Configure an analysis

- `AnalysisConfig`, built from the sections `AnalysisConfigSection`, `ValidationSection`,
  `SpectrumSection`, `PeriodogramSection`, `MultiscaleResidualSection`,
  `PermutationSection`, `InferenceSection`, `RegistrationSection`, `NeighborhoodSection`,
  `ComparisonSection`, `CurveMargins`, `DiagnosticsSection`, `PerformanceSection` and
  `OutputSection`. These mirror the TOML configuration file
  ([`examples/config.toml`](../examples/config.toml)). Changing a key or what it means
  requires migration notes.
- `ComponentMode`, `PermutationStratum`, `RegistrationTransform`, `NeighborhoodNullModel`,
  `ThreadSetting`: enums for settings with a fixed set of choices, so invalid strings
  cannot be written. Their serialized names are part of the configuration format.

## Analyze H&E and IHC together

- `MultimodalEngine`, `MultimodalInput`, `MultimodalAnalysisRun`: run alignment, cell
  matching, the neighbor graph and the multimodal analysis in one call.
- `HeCell`, `IhcCell`, `FusedCell`, `CellSection`: input cells from each stain and the
  matched cells produced by a run.
- `LandmarkPair`, `Transform2D`, `TransformKind`: landmarks you supply to align the two
  sections, and the fitted transform (rigid or affine).
- `SpatialGraph`, `SpatialEdge`: the neighbor graph built during a run.
- `NullModelSensitivityResult`, `RegistrationResidual`, `RegistrationExtrapolation`,
  `CellExtrapolationRecord`, `LandmarkHullAvailability`: extra run data that is not
  repeated in `result.json`, such as how well the landmarks fit and which cells fall
  outside the landmark region.

## Compare two sections

- `compare_marked_prepost`, `compare_multimodal_prepost`,
  `compare_multimodal_prepost_with_margin`: compare two finished analyses, such as before
  and after treatment. These work without the `cli` feature. Single-section and multimodal
  results use separate functions so they cannot be mixed up.
- `PrePostResult`, `TerritoryPrePostSummary`, `CurveComparisonResult`,
  `CurveComparisonMethod`, `CurveComparisonAvailability`: the comparison results.

## Read and write results

- `ResultDocument`, `AnalysisResult`, `Provenance`, `RESULT_FORMAT_VERSION`: the
  `result.json` document. Only the current format version is read.
- `AnalysisStatus`, `Interpretation`, `InterpretationClass`, `StatusFlag`,
  `AnalysisSection`: overall run state and the availability wrapper used by every
  measurement (see [Availability](#availability) below).
- Single-section results: `MarkedPatternResult`, `WindowSummary`, `QcSummary`,
  `PrimaryEndpoint`, `PrimaryEndpointKind`, `SpectrumSummary`, `SpectrumPoint`,
  `SpectrumNullModel`, `SpectrumNullInferenceSummary`, `SpectrumNullSensitivitySummary`,
  `SpectrumConfoundingConclusion`, `FunctionalSummary`, `MarkPairCovariancePoint`,
  `ScaleEnergyPoint`, `ScaleEnergyBand`, `AnisotropySummary`,
  `MultiscaleResidualSummary`, `ResidualTerritory`, `ComponentModeSelection`,
  `ResolvedComponentMode`, `ComponentAnalysisSummary`.
- Multimodal results: `MultimodalResult`, `RegistrationSummary`, `FusedCellSummary`,
  `NeighborhoodEnrichmentResult`, `EnrichmentStatisticUnavailableReason`,
  `CrossInteractionCurve`, `CrossInteractionPoint`, `NeighborhoodTerritory`,
  `TerritoryProfile`, `LabelFraction`.
- Optional exploratory diagnostics, written only when enabled: `DiagnosticsResult`,
  `BetaPosteriorSummary`, `BetaPosteriorGroupSummary`, `GraphSmoothingSummary`,
  `GraphSmoothingLabelPairSummary`.
- `OutputWriter`, `OutputManifest`, `ArtifactStatus`: write a result and its files. The
  output directory appears only after everything has been written.

Result metadata with a fixed vocabulary uses enums in its method namespace. For example,
`marks::CategoricalPairInferenceSummary::null_model` uses `MarkNullModel`, its
`permutation_unit` uses `MarkPermutationUnit`, and its `alternative` uses the existing
`InferenceAlternative`. Geometry and intensity metadata use the corresponding enums in
`spatial`; measurement status uses `MeasurementStatus`. Rust callers must use these
variants instead of constructing or comparing strings. Their explicit serialized names
preserve existing JSON bytes and field order. `IntensityCrossFit::BalancedCellIdRank`
retains its fold count in the existing string spelling. Identifiers, user-defined labels,
units, digests, format versions, and explanatory prose remain textual.

The scientific companion crates type the result fields that a caller acts on, using shared
vocabularies with the same exact spellings:

| Field | Type |
|---|---|
| `claim_status` | `marklab_core::backend::ClaimStatus` (every claim the project can emit) |
| `fit_state` | `marklab_core::backend::FitState` |
| `maturity`, `backend_capability` (model descriptors) | `marklab_core::backend::{ModelMaturity, BackendCapability}` |
| `statistical_unit` | `marklab_core::vocabulary::StatisticalUnit` |
| `null_model` | `marklab_core::vocabulary::NullHypothesis` |
| validation-ledger `status`, `overall_status` | `marklab_core::vocabulary::{ValidationStatus, ValidationSuiteStatus}` |
| `agreement_status`, `sensitivity_status`, `sensitivity_state` | `marklab_bayes::{AgreementStatus, SensitivityStatus}` |
| per-result `status` fields | an enum next to the result type, such as `marklab_sbi::SupportStatus` or `marklab_causal::ExposureMeanStatus` |

Each enum has `as_str()` and `Display` for its serialized spelling, and deserialization
accepts only the listed spellings. `marklab_core::vocabulary_enum!` defines these
vocabularies with an explicit spelling per variant. Model-description labels (likelihood,
link, prior, kernel and method names), finite-result policy statements, identifiers and
units remain textual; they describe a model rather than gate how a result may be used. The
numerical primitives in `marklab-numerics` keep a textual claim because that crate sits
below `marklab-core`.

## Errors

- `MarklabError`, `Result`: the crate's error type. You can match on the variants; the
  message text may gain detail over time.

## Whole-slide images (`wsi` feature)

- `SlideReader`, `SlideOpenOptions`, `SlideMetadata`, `SlideSceneMetadata`,
  `SlideSeriesMetadata`, `SlideLevelMetadata`, `SlideSampleType`, `PlaneSelection`,
  `RegionRequest`, `RgbaRegion`: open a slide, read its metadata, check a region request
  against size limits before decoding anything, and extract the region as RGBA pixels.

## Multiplex panels

- `marks::{AssayMarkDeclaration, AssayMarkValues, ScalarMarkColumn, MarkTable}`
  (`ScalarMarkColumn::assay`, `MarkTable::assay_column`): add named marker columns to a cell table. Values can be
  numbers, yes/no readings or categories, and may be missing for some cells.
- `autocorrelation::{AssaySpatialInput, AssaySpatialLimits, AssaySpatialOutcome,
  AssaySpatialSummary, AssaySpatialUnavailable, summarize_assay_spatial}`: compute
  Moran's I and Geary's C for one marker on one slide. Cells missing that marker are left
  out rather than filled in.
- `pathology::{analyze_multiplex_study, execute_multiplex_study, publish_multiplex_study,
  MultiplexStudyResult, MultiplexStudyRun, MultiplexStudyTarget}`: run a whole panel
  across slides and patients, the same code `marklab study` and the Python client call.
  See [multiplex studies](multiplex-study.md).

## Spatial autocorrelation weights

`GlobalMoranWeightPolicy::BinarySymmetricIncludingIsolates` (recipe value
`binary_symmetric_including_isolates`) retains observed cells that have no neighbor.
Their graph rows are zero, but their values remain in the mean, variance, sample size
and permutation support. A graph with no edges is unavailable. The existing binary
and row-standardized policies still reject isolates. Choose the policy before comparing
groups: deleting isolates changes the sampled population according to local cell density.

The explicit binary convention agrees with [PySAL/esda Moran](https://pysal.org/esda/v2.9.0/generated/esda.Moran.html)
and [Geary](https://pysal.org/esda/v2.9.0/generated/esda.Geary.html) with `transformation="B"`.
These packages default to row-standardized weights. For five cells at positions
`0,1,2,3,10`, radius `1.1`, and values `1,2,4,8,16`, esda 2.9.0/libpysal 4.14.1 with
`transformation="B"` and Marklab both give `I=113/372` and `C=35/372`. The path's end
cells have one neighbor and its inner cells two, so this example distinguishes the
conventions: row-standardized weights give `I=601/1984` and `C=295/2976`, and dropping
the isolated cell gives `I=29/115` and `C=42/115`. [spdep](https://r-spatial.github.io/spdep/articles/subgraphs.html) rejects
isolates by default; accepting them requires `zero.policy=TRUE`, and matching the
all-cell normalization also requires `adjust.n=FALSE` (its default is `TRUE`).

This descriptive statistic does not establish a tissue interaction mechanism. An extreme
isolated value can substantially change the global centering and variance even though it
participates in no edge. Marker missingness changes the observed support separately from
the isolate policy. The multiplex study uses these slide summaries for patient inference;
it does not turn individual cells into independent patient replicates.

## Pathology maps and studies

- `pathology::{analyze_pathology_maps, publish_pathology_maps, PathologyMapsResult}`: marker and
  density profiles around annotated structures, cell-type mix maps and single-marker
  hotspot maps. See [pathology maps](pathology-maps.md).
- `pathology::{analyze_pathology_composition, analyze_pathology_spatial_study,
  analyze_pathology_scan}`, their matching `publish_*` functions and result types: the
  composition, spatial-study and enrichment-scan workflows. See
  [pathology studies](pathology-studies.md).

## Embedding artifact files

Embedding analyses can consume vectors from any upstream model through their declared
input types. `EmbeddingSpatialPoint` carries a finite vector and physical coordinates;
`VectorSemivariogramSpec` declares feature order. `CellEmbeddingTable::from_rows` accepts
a caller-declared positive dimension. `EmbeddingNpyMatrix` and `EmbeddingNpySummary`
read bounded, finite, row-major little-endian float32 matrices with 1–65,536 components.
These shared inputs do not require CellViT output. The `CellVitNpyMatrix` and
`import_cellvit_he_bundle_*` interfaces retain the optional fixed CellViT profile.

`CellEmbeddingTensorContract::new` declares dimension, pooling, output normalization,
input channel order/normalization, modality and stain for any model. These declarations
describe the supplied values; Marklab does not normalize or pool them again. Paired
input mean/std arrays have at most 256 channels and positive standard deviations;
an explicit `none` channel order permits empty arrays for non-image inputs.
`CellEmbeddingProvenance` writes this contract as version 2 and exposes `format_version`
for the matching artifact schema. Its verified graph binds ordinary `text/csv` cell
metadata and `application/x-npy` vectors, with no CellViT content labels or calibration.
Identity, checkpoint, dependency, finite-value and memory checks still apply.

The original `CellEmbeddingTensorContract::rgb_he_raw` continues to write version 1,
including the original CellViT tensor declarations. The reader accepts both versions;
existing version-1 canonical bytes and strict profile checks remain unchanged. Generic
and legacy provenance use `application/vnd.marklab.embedding-provenance.v2+json` and
`application/vnd.marklab.embedding-provenance.v1+json`, respectively. The columnar table
format already supports declared dimensions and stays at version 1.

The Arrow IPC and Parquet files that `marklab::embeddings` writes are Marklab's own storage
format, not an interchange format. Each file follows one fixed physical profile: column
layout, page and batch sizes, encodings, application metadata keys, and (for Parquet) a
`created_by` string naming the exact `marklab-embeddings` and `parquet` crate versions. The
readers accept only files in that profile, and the Parquet reader also requires the footer and
page headers to re-encode to the same bytes. Files written by other tools such as pyarrow,
or by Marklab built against a different `parquet` release, are rejected rather than
reinterpreted. Use the shared analysis input types or an importer that matches the actual
source profile, and treat the columnar files as artifacts Marklab itself wrote. The
CellViT importer is appropriate only for its declared bundle; it is not a required
route for other models.

Columnar operations use `ColumnarError`, with `ArrowIpcFailure` or `ParquetFailure`
identifying a closed physical-profile rejection, and `ColumnarPublicationError` for
publication. These replace the separate embedding and multiscale error types.
`MultiscaleEmbeddingTableReadBindings` replaces the patch-only binding name.

`publish_columnar`, `verify_columnar`, and `read_columnar` dispatch by `ArrowIpc` or
`Parquet` and a typed entity marker. Reads accept `Read + Seek`; use
`with_verified_columnar_reader` for managed artifacts so the store retains its existing
locking, pre-read verification, post-read verification, and failure precedence. Read
contexts supply logical information omitted from the file: exact artifact bindings,
cell extraction statuses, or the counterpart half of a split cell-patch link. A loaded
region or slide table is an ordinary domain value. Obtaining a verification receipt
still requires the existing graph or finalization proof.

## Scientific companion crates

The non-Bayesian analyses previously exported by `marklab-bayes` now live with their
scientific domains. Import them directly from the corresponding workspace crate:

| Crate/module | Analyses |
|---|---|
| `marklab_graph` | Graph-signal energies, restricted permutation tests, and group degree, group closeness and average clustering on physical-radius graphs (`graph_group_centrality_workflow`, `GraphGroupCentralitySpec`, `GraphGroupCentralityResult`; see [group centrality](group-centrality.md)) |
| `marklab_transport` | Balanced and unbalanced Sinkhorn transport, partial transport, and full, partial or unbalanced (KL-relaxed) fused Gromov–Wasserstein alignment |
| `marklab_prediction` | Grouped conformal prediction, late fusion, mixtures of experts, and region retrieval |
| `marklab_embeddings::analysis` | Spatial embedding curves, kernel and multiscale similarities, cross-modal covariance, factor-model construction, and cell/patch complementarity |
| `marklab_simulation` | Thomas and Matérn cluster simulation and Thomas minimum-contrast estimation |

The scientific types and functions retain their names. This is a Rust import-path change;
serialized requests, result fields, and scientific calculations retain their existing
contracts. Shared worker identity, completion state, claim vocabulary, and boundary errors
live in `marklab_core::backend`. `marklab_bayes` retains its existing names for those shared
types, including `BayesError` as the shared `WorkerError` type. Shared geometric records live
in `marklab_data`, with the existing root exports preserved.

`marklab_bayes::protocol` owns the typed request/result records and scientific result
validation for the arbitrary-window and replicated model families. The command-line
application retains file reading, worker execution, durable scheduling, and publication.

The moved crates share a few helpers that are now public: `marklab_numerics::{sigmoid,
clipped_logit, all_finite}`, `marklab_numerics::summation::{FiniteNeumaierSum,
FiniteNeumaierError}`, and `marklab_core::derive_simulation_seed`, which derives the
namespaced simulation seeds used by the Thomas, Matérn cluster and Strauss samplers.

The advanced Bayesian, causal and 3-D worker specifications are typed and reject unknown
fields, so a misspelled key fails with serde's field-level message instead of being ignored.
The GeoJSON `bbox` member is accepted on adaptive-SPDE windows.

`marklab_numerics::evaluate_stable_primitives` and `numerics stable-primitives` use
sample covariance with the `n - 1` denominator when `covariance_weights` is absent.
The implementation accumulates `1/n`-scaled residual products before applying the
directly computed `(n - 1)/n` correction, retaining the scaled calculation's overflow
headroom. In the existing three-row oracle, the first diagonal value is now exactly
`1.0`; computing the correction through squared normalized weights previously produced
`0.9999999999999999`. This follows the ordinary sample convention documented by
[NumPy](https://numpy.org/doc/stable/reference/generated/numpy.cov.html).
Caller-supplied reliability weights retain their existing calculation, so explicitly
supplying equal weights can differ from the unweighted path at floating-point roundoff.
The quantity, field meanings and standalone version-1 result contract are unchanged.

## Python backends

- `backend::{python_backend_assets_root, python_backend_interpreter, python_backend_cache,
  python_backend_command, PythonBackendRuntimeError}`: find the installed Python worker
  files, interpreter and compile cache, and build the command that starts a worker.
  `python_backend_command` clears the environment, sets `PATH` to the system directories
  or to `MARKLAB_BACKEND_PATH`, and stops Python from importing files from the worker
  directory ahead of installed packages; keep those settings if you modify the command. See [Python backends](python-backends.md).

## Kept internal

`AnalysisMetadata` is shared state used inside a run, not input or output, so it is not
public. The low-level curve statistics behind comparisons are also internal: use the
three `compare_*` functions and save their output instead.

## Availability

Every measurement in a result is wrapped in `AnalysisSection<T>`, which is one of:

- `available`: computed. An empty list is a real answer only when the true result is
  empty, for example no hotspot regions found.
- `disabled`: turned off in the configuration.
- `not_applicable`: does not apply to this kind of analysis.
- `insufficient_data`: requested, but a requirement was not met, such as too few cells.
  `reason` says which one.

If the computation itself fails, the function returns `Err(MarklabError)` and no result
is written. A failure is never reported as `insufficient_data` or as a placeholder number.

Output files use the matching `ArtifactStatus` states. `written` is the only success
state and records the file's path.
