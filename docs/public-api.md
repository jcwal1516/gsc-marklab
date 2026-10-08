# Public Rust API

Everything re-exported from the crate root (`src/lib.rs`) is the supported Rust API.
Internal modules are private.

**Stability.** The crate is pre-1.0, so the Rust API follows normal semver rules for
0.x versions. The JSON written to `result.json` is a separate, stricter contract
([result format 0.3](result-format-0.3.md)): its shape and field names do not change
within 0.3. Experimental workflows (multiplex studies, pathology maps and studies) write
their own versioned documents (v1), separate from 0.3.

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
  `result.json` document. Reading a 0.2 document converts the parts that have an
  unambiguous 0.3 meaning and rejects the rest.
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

## Errors

- `MarklabError`, `Result`: the crate's error type. You can match on the variants; the
  message text may gain detail over time.

## Whole-slide images (`wsi` feature)

- `SlideReader`, `SlideOpenOptions`, `SlideMetadata`, `SlideSceneMetadata`,
  `SlideSeriesMetadata`, `SlideLevelMetadata`, `SlideSampleType`, `PlaneSelection`,
  `RegionRequest`, `RgbaRegion`: open a slide, read its metadata, check a region request
  against size limits before decoding anything, and extract the region as RGBA pixels.

## Multiplex panels

- `AssayMarkDeclaration`, `AssayMarkValues`, `ScalarMarkColumn::assay`,
  `MarkTable::assay_column`: add named marker columns to a cell table. Values can be
  numbers, yes/no readings or categories, and may be missing for some cells.
- `AssaySpatialInput`, `AssaySpatialLimits`, `AssaySpatialOutcome`,
  `AssaySpatialSummary`, `AssaySpatialUnavailable`, `summarize_assay_spatial`: compute
  Moran's I and Geary's C for one marker on one slide. Cells missing that marker are left
  out rather than filled in.
- `analyze_multiplex_study`, `execute_multiplex_study`, `publish_multiplex_study`,
  `MultiplexStudyResult`, `MultiplexStudyRun`, `MultiplexStudyTarget`: run a whole panel
  across slides and patients, the same code `marklab study` and the Python client call.
  See [multiplex studies](multiplex-study.md).

## Pathology maps and studies

- `analyze_pathology_maps`, `publish_pathology_maps`, `PathologyMapsResult`: marker and
  density profiles around annotated structures, cell-type mix maps and single-marker
  hotspot maps. See [pathology maps](pathology-maps.md).
- `analyze_pathology_composition`, `analyze_pathology_spatial_study`,
  `analyze_pathology_scan`, their matching `publish_*` functions and result types: the
  composition, spatial-study and enrichment-scan workflows. See
  [pathology studies](pathology-studies.md).

## Python backends

- `python_backend_assets_root`, `python_backend_interpreter`, `python_backend_cache`,
  `python_backend_command`, `PythonBackendRuntimeError`: find the installed Python worker
  files, interpreter and compile cache, and build the command that starts a worker.
  `python_backend_command` clears the environment and stops Python from importing files
  from the worker directory ahead of installed packages; keep those settings if you
  modify the command. See [Python backends](python-backends.md).

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
