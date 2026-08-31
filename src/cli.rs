use std::path::PathBuf;

use crate::Result;
use clap::{Parser, Subcommand, ValueEnum};

macro_rules! bail {
    ($($argument:tt)*) => {
        return Err(MarklabError::Validation(format!($($argument)*)))
    };
}

#[path = "cli/batch_output_path.rs"]
mod batch_output_path;
#[path = "cli/dto.rs"]
mod dto;
#[path = "cli/schema.rs"]
mod schema;

#[path = "cli/analyze.rs"]
mod analyze;
#[path = "cli/batch.rs"]
mod batch;
#[path = "cli/categorical_cross_pair_correlation.rs"]
mod categorical_cross_pair_correlation;
#[path = "cli/categorical_mark_project.rs"]
mod categorical_mark_project;
#[path = "cli/categorical_pair.rs"]
mod categorical_pair;
#[path = "cli/classical.rs"]
mod classical;
#[path = "cli/inhomogeneous_bandwidth_selection.rs"]
mod inhomogeneous_bandwidth_selection;
#[path = "cli/inhomogeneous_categorical_cross_pair_correlation.rs"]
mod inhomogeneous_categorical_cross_pair_correlation;
#[path = "cli/inhomogeneous_pair_correlation.rs"]
mod inhomogeneous_pair_correlation;
#[path = "cli/inhomogeneous_project.rs"]
mod inhomogeneous_project;
#[path = "cli/inhomogeneous_spatial.rs"]
mod inhomogeneous_spatial;
#[path = "cli/isotropic_pair_correlation.rs"]
mod isotropic_pair_correlation;
#[path = "cli/isotropic_spatial.rs"]
mod isotropic_spatial;
#[path = "cli/multimodal.rs"]
mod multimodal;
#[path = "cli/nearest_space.rs"]
mod nearest_space;
#[path = "cli/piecewise_compartment_pair_correlation.rs"]
mod piecewise_compartment_pair_correlation;
#[path = "cli/piecewise_compartment_spatial.rs"]
mod piecewise_compartment_spatial;
#[path = "cli/prepost.rs"]
mod prepost;
#[path = "cli/profile.rs"]
mod profile;
#[path = "cli/scalar_variogram.rs"]
mod scalar_variogram;
#[path = "cli/simulate.rs"]
mod simulate;
#[cfg(feature = "wsi")]
#[path = "cli/slide.rs"]
mod slide;
#[path = "cli/smoke.rs"]
mod smoke;
#[path = "cli/translation_pair_correlation.rs"]
mod translation_pair_correlation;
#[path = "cli/translation_spatial.rs"]
mod translation_spatial;

use batch_output_path::batch_output_path;
use dto::{
    AnalyzeRequest, ClassicalRequest, LandmarkRow, LogLevel, ManifestRow, MultimodalAnalyzeRequest,
    MultimodalManifestRow, NearestSpaceRequest, ObservabilityOptions,
};
use schema::HeInputFormat;

pub fn run_cli() -> Result<()> {
    schema::run_cli()
}
