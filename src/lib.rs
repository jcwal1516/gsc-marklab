#![forbid(unsafe_code)]
//! Supported API for marked-pattern, multimodal, and bounded WSI analysis.
//!
//! The compatibility contract is the set of types re-exported from this crate
//! root. Algorithm and orchestration modules remain private.

#[cfg(all(test, feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
#[global_allocator]
static TEST_ALLOCATOR: dhat::Alloc = dhat::Alloc;

#[cfg(test)]
mod algorithm_tests;
mod api;
#[cfg(feature = "cli")]
mod cli;
mod common;
pub mod comparison;
mod config;
mod data;
mod diagnostics;
mod errors;
mod geom;
mod inference;
mod io;
mod multimodal;
mod multiscale_residual;
mod neighborhood;
mod output;
mod perf;
mod periodogram;
mod permutation;
#[cfg(feature = "cli")]
mod prepost;
mod qc;
mod registration;
mod spectra;
#[cfg(feature = "cli")]
mod synthetic_smoke;
#[cfg(feature = "wsi")]
mod wsi;

#[cfg(feature = "cli")]
#[doc(hidden)]
pub use cli::run_cli;

pub use api::{AnalysisEngine, MarkedAnalysisRun};
pub use config::{
    AnalysisConfig, AnalysisConfigSection, ComparisonSection, ComponentMode, CurveMargins,
    DiagnosticsSection, InferenceSection, MultiscaleResidualSection, NeighborhoodNullModel,
    NeighborhoodSection, OutputSection, PerformanceSection, PeriodogramSection, PermutationSection,
    PermutationStratum, RegistrationSection, RegistrationTransform, SpectrumSection, ThreadSetting,
    ValidationSection,
};
pub use data::{Pattern, PatternMeta, TumorWindow};
pub use errors::{MarklabError, Result};
pub use geom::mask::TumorMask;
pub use io::{PatternLoadDiagnostics, PatternLoadResult, PatternLoader};
pub use multimodal::{
    AnalysisMetadata, CellExtrapolationRecord, CellSection, FusedCell, HeCell, IhcCell,
    LandmarkHullAvailability, MultimodalAnalysisRun, MultimodalEngine, MultimodalInput,
    NullModelSensitivityResult, RegistrationExtrapolation, RegistrationResidual,
};
pub use neighborhood::graph::{SpatialEdge, SpatialGraph};
pub use output::{
    AnalysisResult, AnalysisSection, AnalysisStatus, AnisotropySummary, ArtifactStatus,
    BetaPosteriorGroupSummary, BetaPosteriorSummary, ComponentAnalysisSummary,
    ComponentModeSelection, CrossInteractionCurve, CrossInteractionPoint,
    CurveComparisonAvailability, CurveComparisonMethod, CurveComparisonResult, DiagnosticsResult,
    EnrichmentStatisticUnavailableReason, FunctionalSummary, FusedCellSummary,
    GraphSmoothingLabelPairSummary, GraphSmoothingSummary, Interpretation, InterpretationClass,
    LabelFraction, MarkPairCovariancePoint, MarkedPatternResult, MultimodalResult,
    MultiscaleResidualSummary, NeighborhoodEnrichmentResult, NeighborhoodTerritory, OutputManifest,
    OutputWriter, PrePostResult, PrimaryEndpoint, Provenance, QcSummary, RegistrationSummary,
    ResidualTerritory, ResolvedComponentMode, ResultDocument, ScaleEnergyPoint,
    SpectrumConfoundingConclusion, SpectrumNullInferenceSummary, SpectrumNullModel,
    SpectrumNullSensitivitySummary, SpectrumPoint, SpectrumSummary, StatusFlag,
    TerritoryPrePostSummary, TerritoryProfile, TimingStage, WindowSummary, RESULT_FORMAT_VERSION,
};
pub use registration::{landmarks::LandmarkPair, transform::Transform2D};
#[cfg(feature = "wsi")]
pub use wsi::{
    PlaneSelection, RegionRequest, RgbaRegion, SlideLevelMetadata, SlideMetadata, SlideOpenOptions,
    SlideReader, SlideSampleType, SlideSceneMetadata, SlideSeriesMetadata,
};
