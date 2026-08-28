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
pub use workflow::CategoricalCrossPairCorrelationAnalysisNode;
