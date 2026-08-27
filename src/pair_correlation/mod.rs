mod analysis;
mod types;

pub(crate) use analysis::configuration_digest;
pub use analysis::homogeneous_pair_correlation;
pub use types::{
    HomogeneousPairCorrelationConfig, HomogeneousPairCorrelationError,
    HomogeneousPairCorrelationInference, HomogeneousPairCorrelationResult, PairCorrelationKernel,
    PairCorrelationPoint, PairCorrelationPointStatus,
};
