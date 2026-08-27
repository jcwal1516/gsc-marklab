mod analysis;
mod types;

pub(crate) use analysis::configuration_digest;
pub use analysis::continuous_mark_weighted_k;
pub use types::{
    MarkWeightedKComponentInference, MarkWeightedKConfig, MarkWeightedKError,
    MarkWeightedKGeometrySummary, MarkWeightedKInferenceSummary, MarkWeightedKLimits,
    MarkWeightedKPoint, MarkWeightedKPointStatus, MarkWeightedKResult,
};
