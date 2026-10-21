mod analysis;
mod types;

pub use analysis::continuous_mark_correlation;
pub(crate) use analysis::{configuration_digest, mark_statistics};
pub use types::{
    ContinuousMarkCorrelationComponentInference, ContinuousMarkCorrelationConfig,
    ContinuousMarkCorrelationError, ContinuousMarkCorrelationGeometrySummary,
    ContinuousMarkCorrelationInferenceSummary, ContinuousMarkCorrelationLimits,
    ContinuousMarkCorrelationPoint, ContinuousMarkCorrelationPointStatus,
    ContinuousMarkCorrelationResult,
};
