#[cfg(feature = "parquet")]
mod curve_parquet;
#[cfg(feature = "cli")]
mod document;
mod figures;
mod manifest;
mod marked_artifacts;
#[cfg(feature = "csv")]
mod multimodal_artifacts;
mod result_types;
mod writer;

#[cfg(all(test, feature = "cli"))]
mod tests;

#[cfg(feature = "cli")]
pub(crate) use document::read_result_document_path_or_dir;
#[cfg(feature = "cli")]
pub(crate) use manifest::{RunManifestContext, RunManifestExecution, RunManifestInputs};
pub use result_types::{
    AnalysisResult, AnalysisSection, AnisotropySummary, ArtifactStatus, BetaPosteriorGroupSummary,
    BetaPosteriorSummary, ComponentAnalysisSummary, ComponentModeSelection, CrossInteractionCurve,
    CrossInteractionPoint, CurveComparisonAvailability, CurveComparisonMethod,
    CurveComparisonResult, DiagnosticsResult, EnrichmentStatisticUnavailableReason,
    FunctionalSummary, FusedCellSummary, GraphSmoothingLabelPairSummary, GraphSmoothingSummary,
    Interpretation, LabelFraction, MarkPairCovariancePoint, MarkedPatternResult, MultimodalResult,
    MultiscaleResidualSummary, NeighborhoodEnrichmentResult, OutputManifest, PrePostResult,
    PrimaryEndpoint, Provenance, QcSummary, RegistrationSummary, ResidualTerritory,
    ResolvedComponentMode, ResultDocument, ScaleEnergyPoint, SpectrumPoint, SpectrumSummary,
    StatusFlag, TerritoryFeature, TerritoryPrePostSummary, TerritoryProfile, TimingStage,
    WindowSummary, RESULT_FORMAT_VERSION,
};
pub use writer::OutputWriter;
