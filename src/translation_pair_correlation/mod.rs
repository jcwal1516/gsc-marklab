mod analysis;
mod document;
mod types;

pub use analysis::analyze_translation_pair_correlation;
pub(crate) use analysis::translation_pair_correlation_configuration_digest;
pub use document::{
    TranslationPairCorrelationResultDocument, TRANSLATION_PAIR_CORRELATION_FORMAT,
    TRANSLATION_PAIR_CORRELATION_FORMAT_VERSION,
};
pub use types::{
    TranslationPairCorrelationConfig, TranslationPairCorrelationConfigurationSummary,
    TranslationPairCorrelationGeometrySummary, TranslationPairCorrelationPoint,
    TranslationPairCorrelationResult,
};
