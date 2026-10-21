use std::path::{Path, PathBuf};

use clap::Subcommand;
use marklab_longitudinal::{
    kalman_filter_and_smooth, nonlinear_gaussian_filter, particle_filter_and_smooth,
    phylogenetic_spatial_association, LinearGaussianStateSpaceSpec,
    PhylogeneticSpatialAssociationSpec, ScalarNonlinearFilterSpec, ScalarParticleSmootherSpec,
};
use serde::Serialize;

use super::exclusive_json_output::{publish_pretty_json, ExclusiveJsonOutputError};
use crate::cli_error::{into_marklab_error, CliError};

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Subcommand)]
pub(crate) enum LongitudinalCommand {
    /// Smooth repeated observations with a linear Gaussian state-space model.
    KalmanSmooth {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Filter repeated observations with a nonlinear state-space model.
    NonlinearFilter {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Smooth latent trajectories using particle-based inference.
    ParticleSmooth {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Assess association between phylogenetic and spatial distances.
    PhylogeneticSpatialAssociation {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
}

pub(crate) fn run_cli(command: LongitudinalCommand) -> Result<(), CliError> {
    match command {
        LongitudinalCommand::KalmanSmooth { input, out } => run_linear(input, out),
        LongitudinalCommand::NonlinearFilter { input, out } => run_nonlinear(input, out),
        LongitudinalCommand::ParticleSmooth { input, out } => run_particle(input, out),
        LongitudinalCommand::PhylogeneticSpatialAssociation { input, out } => {
            run_phylogenetic_association(input, out)
        }
    }
}

fn run_linear(input_path: PathBuf, output_path: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(input_path)?;
    let spec: LinearGaussianStateSpaceSpec = serde_json::from_slice(&bytes)?;
    let result =
        kalman_filter_and_smooth(spec).map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&output_path, &result)
}

fn run_nonlinear(input_path: PathBuf, output_path: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(input_path)?;
    let spec: ScalarNonlinearFilterSpec = serde_json::from_slice(&bytes)?;
    let result =
        nonlinear_gaussian_filter(spec).map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&output_path, &result)
}

fn run_particle(input_path: PathBuf, output_path: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(input_path)?;
    let spec: ScalarParticleSmootherSpec = serde_json::from_slice(&bytes)?;
    let result =
        particle_filter_and_smooth(spec).map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&output_path, &result)
}

fn run_phylogenetic_association(input_path: PathBuf, output_path: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(input_path)?;
    let spec: PhylogeneticSpatialAssociationSpec = serde_json::from_slice(&bytes)?;
    let result = phylogenetic_spatial_association(spec)
        .map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&output_path, &result)
}

fn read_input(input_path: PathBuf) -> Result<Vec<u8>, CliError> {
    crate::input_file::read_regular_file(
        &input_path,
        MAXIMUM_INPUT_BYTES,
        "input must be a regular file within 16 MiB",
    )
}

fn publish_json(path: &Path, result: &impl Serialize) -> Result<(), CliError> {
    publish_pretty_json(path, result, "longitudinal").map_err(|error| match error {
        ExclusiveJsonOutputError::OutputExists => {
            CliError::Input(format!("output already exists: {}", path.display()))
        }
        ExclusiveJsonOutputError::OutputMustNameFile => {
            CliError::Input("output must name a file".into())
        }
        ExclusiveJsonOutputError::Io { path, source } => CliError::Io { path, source },
        ExclusiveJsonOutputError::Json(error) => CliError::Json(error),
    })
}

pub(crate) fn execute(command: LongitudinalCommand) -> marklab::Result<()> {
    run_cli(command).map_err(into_marklab_error)
}
