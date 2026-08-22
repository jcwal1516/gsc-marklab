mod artifact_plan;
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
mod transaction;
mod writer;

#[cfg(all(test, feature = "cli"))]
mod tests;

#[cfg(feature = "cli")]
pub(crate) use document::read_result_document_path_or_dir;
#[cfg(feature = "cli")]
pub(crate) use manifest::{RunManifestContext, RunManifestExecution, RunManifestInputs};
pub use result_types::{
    AnalysisResult, AnalysisSection, AnalysisStatus, AnisotropySummary, ArtifactStatus,
    BetaPosteriorGroupSummary, BetaPosteriorSummary, ComponentAnalysisSummary,
    ComponentModeSelection, CrossInteractionCurve, CrossInteractionPoint,
    CurveComparisonAvailability, CurveComparisonMethod, CurveComparisonResult, DiagnosticsResult,
    EnrichmentStatisticUnavailableReason, FunctionalSummary, FusedCellSummary,
    GraphSmoothingLabelPairSummary, GraphSmoothingSummary, Interpretation, InterpretationClass,
    LabelFraction, MarkPairCovariancePoint, MarkedPatternResult, MultimodalResult,
    MultiscaleResidualSummary, NeighborhoodEnrichmentResult, NeighborhoodTerritory, OutputManifest,
    PrePostResult, PrimaryEndpoint, PrimaryEndpointKind, Provenance, QcSummary,
    RegistrationSummary, ResidualTerritory, ResolvedComponentMode, ResultDocument, ScaleEnergyBand,
    ScaleEnergyPoint, SpectrumConfoundingConclusion, SpectrumNullInferenceSummary,
    SpectrumNullModel, SpectrumNullSensitivitySummary, SpectrumPoint, SpectrumSummary, StatusFlag,
    TerritoryPrePostSummary, TerritoryProfile, TimingStage, WindowSummary, RESULT_FORMAT_VERSION,
};
pub use writer::OutputWriter;
