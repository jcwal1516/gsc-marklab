#[cfg(feature = "parquet")]
mod curve_parquet;
mod figures;
mod result_types;
mod writer;

#[cfg(all(test, feature = "cli"))]
mod tests;

pub use result_types::{
    AnalysisResult, AnalysisSection, AnisotropySummary, ArtifactStatus, BetaBinomialGroupSummary,
    BetaBinomialSummary, ComponentAnalysisSummary, CrossInteractionCurve, CurveTestResult,
    DiagnosticsResult, FunctionalSummary, FusedCellSummary, GraphSmoothingLabelPairSummary,
    GraphSmoothingSummary, Interpretation, LabelFraction, MarkedPatternResult, MultimodalResult,
    NeighborhoodEnrichmentResult, OutputManifest, PairCorrelationPoint, PrePostResult,
    PrimaryEndpoint, Provenance, QcSummary, RegistrationSummary, ResultDocument, ScalogramPoint,
    SpectrumPoint, SpectrumSummary, StatusFlag, TerritoryFeature, TerritoryPrePostSummary,
    TerritoryProfile, TimingStage, WaveletSummary, WindowSummary,
};
pub use writer::OutputWriter;
