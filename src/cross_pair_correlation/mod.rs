mod analysis;
mod translation;
mod translation_workflow;
mod types;
mod workflow;

pub use analysis::categorical_cross_pair_correlation;
pub(crate) use analysis::configuration_digest;
pub use translation::{
    translation_categorical_cross_pair_correlation,
    TranslationCategoricalCrossPairCorrelationConfig,
    TranslationCategoricalCrossPairCorrelationPoint,
    TranslationCategoricalCrossPairCorrelationResult,
};
#[cfg(feature = "cli")]
pub(crate) use translation_workflow::encode_result as encode_translation_result;
pub use translation_workflow::TranslationCategoricalCrossPairCorrelationAnalysisNode;
pub use types::{
    CategoricalCrossPairCorrelationConfig, CategoricalCrossPairCorrelationError,
    CategoricalCrossPairCorrelationInference, CategoricalCrossPairCorrelationPoint,
    CategoricalCrossPairCorrelationResult,
};
#[cfg(feature = "cli")]
pub(crate) use workflow::encode_result;
pub use workflow::CategoricalCrossPairCorrelationAnalysisNode;
