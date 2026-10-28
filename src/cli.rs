use std::path::PathBuf;

use crate::Result;
use clap::{Subcommand, ValueEnum};

macro_rules! bail {
    ($($argument:tt)*) => {
        return Err(MarklabError::Validation(format!($($argument)*)))
    };
}

mod batch_output_path;
mod dto;
mod schema;

mod analyze;
mod batch;
mod categorical_cross_pair_correlation;
mod categorical_mark_project;
mod categorical_pair;
mod classical;
mod inhomogeneous_bandwidth_selection;
mod inhomogeneous_categorical_cross_pair_correlation;
mod inhomogeneous_pair_correlation;
mod inhomogeneous_project;
mod inhomogeneous_spatial;
mod isotropic_pair_correlation;
mod isotropic_spatial;
mod microbio;
mod multimodal;
mod nearest_space;
mod piecewise_compartment_pair_correlation;
mod piecewise_compartment_spatial;
mod prepost;
mod profile;
mod project_output;
mod scalar_variogram;
mod simulate;
#[cfg(feature = "wsi")]
mod slide;
mod smoke;
mod translation_pair_correlation;
mod translation_spatial;

use batch_output_path::batch_output_path;
use dto::{
    AnalyzeRequest, ClassicalRequest, LandmarkRow, LogLevel, ManifestRow, MultimodalAnalyzeRequest,
    MultimodalManifestRow, NearestSpaceRequest, ObservabilityOptions,
};
use schema::HeInputFormat;

pub use schema::{Commands, MultimodalCommands, ProjectCommands};

/// Run one top-level library command parsed by the executable.
pub fn run(command: Commands) -> Result<()> {
    schema::run(command)
}

/// Run one `project` subcommand implemented by the library.
pub fn run_project(command: ProjectCommands) -> Result<()> {
    schema::run_project(command)
}

/// Run one `multimodal` subcommand implemented by the library.
pub fn run_multimodal(command: MultimodalCommands) -> Result<()> {
    schema::run_multimodal(command)
}
