mod generators;
mod marked;
mod model;
mod multimodal;
mod multimodal_observation;
mod policy;
mod statistics;

#[cfg(test)]
#[path = "synthetic_smoke/tests.rs"]
mod tests;

pub use marked::run_synthetic_smoke;
pub use model::MultimodalSyntheticSmokeSummary;
pub use multimodal::run_multimodal_synthetic_smoke;

#[cfg(test)]
use marked::{run_generator, smoke_config, summarize_analyses};
#[cfg(test)]
use multimodal::{
    multimodal_scenario_configuration, run_multimodal_generator, summarize_multimodal_outcomes,
};
#[cfg(test)]
use multimodal_observation::ObservedMultimodalOutcome;
