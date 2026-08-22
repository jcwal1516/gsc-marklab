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
mod comparison;
mod config;
mod data;
mod diagnostics;
mod errors;
mod geom;
mod inference;
mod io;
mod multimodal;
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
mod validation;
mod wavelet;
#[cfg(feature = "wsi")]
mod wsi;

#[cfg(feature = "cli")]
#[doc(hidden)]
pub use cli::run_cli;

pub use api::AnalysisEngine;
pub use config::{
    AnalysisConfig, AnalysisConfigSection, ComparisonSection, ComponentMode, DiagnosticsSection,
    EquivalenceMargins, InferenceSection, NeighborhoodNullModel, NeighborhoodSection,
    OutputSection, PerformanceSection, PeriodogramSection, PermutationSection, PermutationStratum,
    RegistrationSection, RegistrationTransform, SpectrumSection, ThreadSetting, ValidationSection,
    WaveletSection,
};
pub use data::{Pattern, PatternMeta, TumorWindow};
pub use errors::{MarklabError, Result};
pub use geom::mask::TumorMask;
pub use multimodal::{
    cell_table::{CellSection, FusedCell, HeCell, IhcCell},
    MultimodalEngine, MultimodalInput,
};
pub use output::{
    AnalysisResult, AnalysisSection, AnisotropySummary, ArtifactStatus, BetaBinomialGroupSummary,
    BetaBinomialSummary, ComponentAnalysisSummary, CrossInteractionCurve, CurveTestResult,
    DiagnosticsResult, FunctionalSummary, FusedCellSummary, GraphSmoothingLabelPairSummary,
    GraphSmoothingSummary, Interpretation, LabelFraction, MarkedPatternResult, MultimodalResult,
    NeighborhoodEnrichmentResult, OutputManifest, OutputWriter, PairCorrelationPoint,
    PrePostResult, PrimaryEndpoint, Provenance, QcSummary, RegistrationSummary, ResultDocument,
    ScalogramPoint, SpectrumPoint, SpectrumSummary, StatusFlag, TerritoryFeature,
    TerritoryPrePostSummary, TerritoryProfile, TimingStage, WaveletSummary, WindowSummary,
};
pub use registration::landmarks::LandmarkPair;
#[cfg(feature = "wsi")]
pub use wsi::{
    PlaneSelection, RegionRequest, RgbaRegion, SlideLevelMetadata, SlideMetadata, SlideOpenOptions,
    SlideReader, SlideSampleType, SlideSceneMetadata, SlideSeriesMetadata,
};
