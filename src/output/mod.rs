#[cfg(feature = "parquet")]
mod curve_parquet;
mod figures;
mod result_types;
mod writer;

#[cfg(all(test, feature = "cli"))]
mod tests;

pub use result_types::{
    AnalysisResult, AnalysisSection, AnisotropySummary, ArtifactStatus, BetaBinomialGroupSummary,
    BetaBinomialSummary, ComponentAnalysisSummary, ComponentModeSelection, CrossInteractionCurve,
    CrossInteractionPoint, CurveTestAvailability, CurveTestResult, DiagnosticsResult,
    EnrichmentStatisticUnavailableReason, FunctionalSummary, FusedCellSummary,
    GraphSmoothingLabelPairSummary, GraphSmoothingSummary, Interpretation, LabelFraction,
    MarkPairCovariancePoint, MarkedPatternResult, MultimodalResult, MultiscaleResidualSummary,
    NeighborhoodEnrichmentResult, OutputManifest, PrePostResult, PrimaryEndpoint, Provenance,
    QcSummary, RegistrationSummary, ResidualTerritory, ResolvedComponentMode, ResultDocument,
    ScaleEnergyPoint, SpectrumPoint, SpectrumSummary, StatusFlag, TerritoryFeature,
    TerritoryPrePostSummary, TerritoryProfile, TimingStage, WindowSummary, RESULT_FORMAT_VERSION,
};
pub use writer::OutputWriter;
