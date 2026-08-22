pub mod cell_table;
mod engine;
pub mod fusion;
mod null_sensitivity;

#[cfg(test)]
pub(crate) use engine::{multimodal_analysis_call_count, reset_multimodal_analysis_call_count};
pub use engine::{MultimodalAnalysisRun, MultimodalEngine, MultimodalInput};
pub use null_sensitivity::NullModelSensitivityResult;

#[cfg(all(test, feature = "cli"))]
mod tests;
