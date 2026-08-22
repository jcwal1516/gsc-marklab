pub mod cell_table;
mod engine;
pub mod fusion;
mod null_sensitivity;
mod registration_artifacts;

#[cfg(test)]
pub(crate) use engine::{multimodal_analysis_call_count, reset_multimodal_analysis_call_count};
pub use engine::{MultimodalAnalysisRun, MultimodalEngine, MultimodalInput};
pub use null_sensitivity::NullModelSensitivityResult;
pub use registration_artifacts::{
    CellExtrapolationRecord, LandmarkHullAvailability, RegistrationExtrapolation,
    RegistrationResidual,
};

#[cfg(all(test, feature = "cli"))]
mod tests;
