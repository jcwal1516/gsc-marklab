#[cfg(feature = "cli")]
mod cell_validation;
pub(crate) mod cells;
#[cfg(feature = "cli")]
mod cellvit;
#[cfg(feature = "cli")]
mod csv_input;
mod engine;
pub mod fusion;
pub(crate) mod labels;
mod null_sensitivity;
mod registration_artifacts;

pub use cells::{CellSection, FusedCell, HeCell, IhcCell};
#[cfg(test)]
pub(crate) use engine::{multimodal_analysis_call_count, reset_multimodal_analysis_call_count};
pub use engine::{MultimodalAnalysisRun, MultimodalEngine, MultimodalInput};
pub use null_sensitivity::NullModelSensitivityResult;
pub use registration_artifacts::{
    CellExtrapolationRecord, LandmarkHullAvailability, RegistrationExtrapolation,
    RegistrationResidual,
};

#[cfg(feature = "cli")]
pub(crate) use cellvit::load_cellvit_he_cell_table_csv;
#[cfg(feature = "cli")]
pub(crate) use csv_input::{load_he_cell_table_csv, load_ihc_cell_table_csv};

#[cfg(all(test, feature = "cli"))]
mod tests;
