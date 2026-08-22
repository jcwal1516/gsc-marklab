# Supported Rust API

The crate-root re-exports in `src/lib.rs` are the supported Rust API. Internal
modules are private. Result-format compatibility is governed separately by the
0.3 document schema; Rust API changes follow normal crate-version compatibility
rules until the crate reaches 1.0.

| Public items | Why public | Stability expectation | Documentation and tests |
| --- | --- | --- | --- |
| `AnalysisEngine`, `MarkedAnalysisRun` | Run marked-pattern analysis and retain output artifact data without recomputation. | Supported high-level entry point. | README/SPEC; `api_contract`, engine, output, and CLI tests. |
| `MultimodalEngine`, `MultimodalInput`, `MultimodalAnalysisRun` | Run the complete registration/fusion/graph/multimodal workflow once. | Supported high-level entry point. | README/SPEC; public engine, one-build, telemetry, and library/CLI parity tests. |
| `AnalysisConfig` and `AnalysisConfigSection`, `ValidationSection`, `SpectrumSection`, `PeriodogramSection`, `MultiscaleResidualSection`, `PermutationSection`, `InferenceSection`, `RegistrationSection`, `NeighborhoodSection`, `ComparisonSection`, `CurveMargins`, `DiagnosticsSection`, `PerformanceSection`, `OutputSection` | Construct and inspect typed configuration programmatically. | Supported configuration surface; breaking key or semantic changes require migration documentation. | Example TOML, SPEC, config parser/validation tests. |
| `ComponentMode`, `PermutationStratum`, `RegistrationTransform`, `NeighborhoodNullModel`, `ThreadSetting` | Closed configuration choices that prevent invalid strings. | Serialized names are part of the configuration contract. | Config round-trip, invalid-key/value, component-mode, registration, and null-model tests. |
| `Pattern`, `PatternMeta`, `TumorWindow` | Construct validated in-memory marked input without filesystem behavior. | Supported domain input surface. | Pattern validation and public API tests. |
| `PatternLoader`, `PatternLoadResult`, `PatternLoadDiagnostics`, `TumorMask` | Decode input adapters into a validated `Pattern` while preserving the domain/I/O boundary. | Supported adapter boundary; physical input mappings may grow compatibly. | CSV/Parquet parity, mask, diagnostics, and public loader tests. |
| `HeCell`, `IhcCell`, `FusedCell`, `CellSection` | Construct multimodal cell input and consume application-retained fused rows. | Supported multimodal domain records. | Fusion, validation, serialization, and allocation tests. |
| `LandmarkPair`, `Transform2D`, `TransformKind` | Supply landmarks and inspect the fitted transform retained by a multimodal run. | Supported low-level registration values; transform kinds are closed and serialized as snake case. | Known rigid/affine, degeneracy, finite, and result-schema tests. |
| `SpatialGraph`, `SpatialEdge` | Consume the single graph retained by `MultimodalAnalysisRun`. | Read-oriented application artifact; edge ordering and resolution semantics are documented invariants. | Brute-force graph, tie, one-build, and registration-resolution tests. |
| `NullModelSensitivityResult`, `RegistrationResidual`, `RegistrationExtrapolation`, `CellExtrapolationRecord`, `LandmarkHullAvailability` | Consume multimodal run artifacts that are intentionally not duplicated inside the serialized result. | Supported application-run artifacts. | All-null, residual, extrapolation, and degenerate-hull tests. |
| `ResultDocument`, `AnalysisResult`, `Provenance`, `RESULT_FORMAT_VERSION` | Read, write, and discriminate versioned analysis documents. | Exact JSON shape is the result-format 0.3 contract. | 0.3 round-trip, pre/post round-trip, unknown-version/field/variant tests. |
| `AnalysisStatus`, `Interpretation`, `InterpretationClass`, `StatusFlag`, `AnalysisSection` | Represent closed machine states and typed section availability. | Serialized enum names are stable within result format 0.3. | Unknown-status/class rejection, availability, suppression, and finite-boundary tests. |
| `MarkedPatternResult`, `WindowSummary`, `QcSummary`, `PrimaryEndpoint`, `PrimaryEndpointKind`, `SpectrumSummary`, `SpectrumPoint`, `SpectrumNullModel`, `SpectrumNullInferenceSummary`, `SpectrumNullSensitivitySummary`, `SpectrumConfoundingConclusion`, `FunctionalSummary`, `MarkPairCovariancePoint`, `ScaleEnergyPoint`, `ScaleEnergyBand`, `AnisotropySummary`, `MultiscaleResidualSummary`, `ResidualTerritory`, `ComponentModeSelection`, `ResolvedComponentMode`, `ComponentAnalysisSummary` | Consume the marked result family and its implemented endpoint records. | Shape and serialized names are the 0.3 schema contract. | Result, engine-spectrum, QC, component-mode, output, and schema tests. |
| `MultimodalResult`, `RegistrationSummary`, `FusedCellSummary`, `NeighborhoodEnrichmentResult`, `EnrichmentStatisticUnavailableReason`, `CrossInteractionCurve`, `CrossInteractionPoint`, `NeighborhoodTerritory`, `TerritoryProfile`, `LabelFraction` | Consume the multimodal result family and typed undefined statistics. | Shape and serialized names are the 0.3 schema contract. | Multimodal engine/CLI, sparse enrichment, territory/profile, output, and schema tests. |
| `PrePostResult`, `TerritoryPrePostSummary`, `CurveComparisonResult`, `CurveComparisonMethod`, `CurveComparisonAvailability` | Consume versioned marked or multimodal comparison documents. | Shape and serialized names are the 0.3 schema contract. | Pre/post round-trip, axis, margin, diagnostic, file/directory, and CLI tests. |
| `DiagnosticsResult`, `BetaPosteriorSummary`, `BetaPosteriorGroupSummary`, `GraphSmoothingSummary`, `GraphSmoothingLabelPairSummary` | Consume explicitly enabled exploratory diagnostic output. | Shape and serialized names are the 0.3 schema contract; diagnostics remain non-primary. | Diagnostic interface, schema, report, and multimodal graph tests. |
| `OutputWriter`, `OutputManifest`, `ArtifactStatus` | Serialize finite documents and atomically write configured artifacts. | Supported output boundary. | Transaction failure, manifest parity, finite validation, and output integration tests. |
| `MarklabError`, `Result` | Use the crate's contextual error path. | Error variants are supported for programmatic handling; prose may gain context. | Error-path coverage across config, input, analysis, output, and WSI. |
| `PlaneSelection`, `RegionRequest`, `RgbaRegion`, `SlideOpenOptions`, `SlideReader`, `SlideSampleType`, `SlideMetadata`, `SlideSceneMetadata`, `SlideSeriesMetadata`, `SlideLevelMetadata` | Inspect and extract bounded WSI regions when the `wsi` feature is enabled. | Supported feature-gated adapter API. | Local codec/oracle, bounds, limit, CLI, and external-fixture tests. |

`AnalysisMetadata` is intentionally internal because it is application-owned
shared state rather than user input or a serialized result. The low-level
`comparison` module is also internal: callers consume versioned comparison
results instead of invoking orchestration helpers whose inferential contract
may evolve.

## Availability policy

`AnalysisSection<T>` has one interpretation across result families:

- `available`: the requested computation completed. For set-valued endpoints,
  an empty vector is valid only when the scientifically meaningful result is an
  empty set, such as no detected territories or no configured label pairs.
- `disabled`: configuration explicitly disabled the endpoint.
- `not_applicable`: the endpoint does not apply to the selected analysis family
  or component mode.
- `insufficient_data`: the endpoint was requested but declared preconditions
  were not met; `reason` explains the missing requirement.
- computation error: the public analysis call returns `Err(MarklabError)` and
  no successful result document is committed. A computation failure is not
  mislabeled as `insufficient_data` or encoded as a numeric sentinel.

Output artifacts use the analogous `ArtifactStatus` states. `written` is the
only success state and carries the committed relative path.
