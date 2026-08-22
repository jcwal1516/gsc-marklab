mod artifacts;
mod common;
mod diagnostics;
mod marked;
mod multimodal;
mod prepost;

pub use artifacts::{ArtifactStatus, OutputManifest};
pub use common::{
    AnalysisResult, AnalysisSection, AnalysisStatus, Interpretation, InterpretationClass,
    Provenance, ResultDocument, StatusFlag, TimingStage, RESULT_FORMAT_VERSION,
};
pub use diagnostics::{
    BetaPosteriorGroupSummary, BetaPosteriorSummary, DiagnosticsResult,
    GraphSmoothingLabelPairSummary, GraphSmoothingSummary,
};
pub use marked::{
    AnisotropySummary, ComponentAnalysisSummary, ComponentModeSelection, FunctionalSummary,
    MarkPairCovariancePoint, MarkedPatternResult, MultiscaleResidualSummary, PrimaryEndpoint,
    PrimaryEndpointKind, QcSummary, ResidualTerritory, ResolvedComponentMode, ScaleEnergyBand,
    ScaleEnergyPoint, SpectrumConfoundingConclusion, SpectrumNullInferenceSummary,
    SpectrumNullModel, SpectrumNullSensitivitySummary, SpectrumPoint, SpectrumSummary,
    WindowSummary,
};
pub use multimodal::{
    CrossInteractionCurve, CrossInteractionPoint, EnrichmentStatisticUnavailableReason,
    FusedCellSummary, LabelFraction, MultimodalResult, NeighborhoodEnrichmentResult,
    NeighborhoodTerritory, RegistrationSummary, TerritoryProfile,
};
pub use prepost::{
    CurveComparisonAvailability, CurveComparisonMethod, CurveComparisonResult, PrePostResult,
    TerritoryPrePostSummary,
};
