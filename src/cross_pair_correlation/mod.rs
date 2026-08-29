mod analysis;
mod types;
mod workflow;

pub use analysis::categorical_cross_pair_correlation;
pub(crate) use analysis::configuration_digest;
pub use types::{
    CategoricalCrossPairCorrelationConfig, CategoricalCrossPairCorrelationError,
    CategoricalCrossPairCorrelationInference, CategoricalCrossPairCorrelationPoint,
    CategoricalCrossPairCorrelationResult,
};
#[cfg(feature = "cli")]
pub(crate) use workflow::encode_result;
pub use workflow::CategoricalCrossPairCorrelationAnalysisNode;
