pub mod cell_table;
mod engine;
pub mod fusion;

#[cfg(test)]
pub(crate) use engine::{multimodal_analysis_call_count, reset_multimodal_analysis_call_count};
pub use engine::{MultimodalEngine, MultimodalInput};

#[cfg(all(test, feature = "cli"))]
mod tests;
