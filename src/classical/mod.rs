mod analysis;
mod document;
mod geometry;
mod types;

pub use analysis::analyze_classical_spatial_pattern;
pub(crate) use analysis::{sample_conditional_csr, window_summary};
pub use document::{
    ClassicalCacheStatus, ClassicalSpatialResultDocument, ClassicalWorkflowIdentity,
    CLASSICAL_SPATIAL_FORMAT, CLASSICAL_SPATIAL_FORMAT_VERSION,
};
pub(crate) use geometry::SpatialGeometryPlan2D;
pub use types::{
    ClassicalConfigurationSummary, ClassicalGeometrySummary, ClassicalInferenceSummary,
    ClassicalNullDesign, ClassicalNullModel, ClassicalRandomizationUnit, ClassicalSpatialConfig,
    ClassicalSpatialError, ClassicalSpatialLimits, ClassicalSpatialResult, ClassicalSpatialStatus,
    ClassicalWindowSummary, HomogeneousKlPoint, KlPointStatus,
};
