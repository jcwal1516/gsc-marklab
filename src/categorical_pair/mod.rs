mod analysis;
mod types;

pub use analysis::categorical_mark_connection_cross_k;
pub(crate) use analysis::configuration_digest;
pub use types::{
    CategoricalPairComponentInference, CategoricalPairConfig, CategoricalPairError,
    CategoricalPairGeometrySummary, CategoricalPairInferenceSummary, CategoricalPairLimits,
    CategoricalPairPoint, CategoricalPairPointStatus, CategoricalPairResult,
};
