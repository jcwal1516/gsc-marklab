use std::path::PathBuf;

use clap::Subcommand;
use serde::{Deserialize, Serialize};

use self::{
    atlas::run_atlas, landmark_uncertainty::run_landmark_uncertainty,
    lddmm_landmarks::run_lddmm_landmarks, nonrigid::run_nonrigid,
    probabilistic_svf::run_probabilistic_svf, svf::run_svf,
};
use crate::cli_error::{into_marklab_error, CliError};

mod atlas;
mod landmark_uncertainty;
mod lddmm_landmarks;
mod nonrigid;
mod probabilistic_svf;
mod svf;

#[derive(Debug, Subcommand)]
pub(crate) enum RegistrationCommand {
    /// Register images with a nonrigid displacement model.
    Nonrigid {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Register images through a stationary velocity field.
    Svf {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Register landmarks through a diffeomorphic deformation model.
    LddmmLandmarks {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Estimate uncertainty in stationary-velocity-field registration.
    ProbabilisticSvf {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Propagate declared landmark uncertainty through registration.
    LandmarkUncertainty {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Build a registration atlas from multiple specimens.
    Atlas {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RegistrationImage {
    frame: String,
    spacing_um: [f64; 2],
    pixels: Vec<Vec<f64>>,
}

pub(crate) fn run_cli(command: RegistrationCommand) -> Result<(), CliError> {
    match command {
        RegistrationCommand::Nonrigid { input, out } => run_nonrigid(input, out),
        RegistrationCommand::Svf { input, out } => run_svf(input, out),
        RegistrationCommand::LddmmLandmarks { input, out } => run_lddmm_landmarks(input, out),
        RegistrationCommand::ProbabilisticSvf { input, out } => run_probabilistic_svf(input, out),
        RegistrationCommand::LandmarkUncertainty { input, out } => {
            run_landmark_uncertainty(input, out)
        }
        RegistrationCommand::Atlas { input, out } => run_atlas(input, out),
    }
}

pub(crate) fn execute(command: RegistrationCommand) -> marklab::Result<()> {
    run_cli(command).map_err(into_marklab_error)
}
