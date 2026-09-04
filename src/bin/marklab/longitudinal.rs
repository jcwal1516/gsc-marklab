use std::{
    fs,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use marklab_longitudinal::{
    kalman_filter_and_smooth, nonlinear_gaussian_filter, particle_filter_and_smooth,
    phylogenetic_spatial_association, LinearGaussianStateSpaceSpec,
    PhylogeneticSpatialAssociationSpec, ScalarNonlinearFilterSpec, ScalarParticleSmootherSpec,
};
use serde::Serialize;
use thiserror::Error;

use super::exclusive_json_output::{publish_pretty_json, ExclusiveJsonOutputError};

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct LongitudinalCli {
    #[command(subcommand)]
    command: LongitudinalTopLevel,
}

#[derive(Debug, Subcommand)]
enum LongitudinalTopLevel {
    Longitudinal {
        #[command(subcommand)]
        command: LongitudinalCommand,
    },
}

#[derive(Debug, Subcommand)]
enum LongitudinalCommand {
    KalmanSmooth {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    NonlinearFilter {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    ParticleSmooth {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    PhylogeneticSpatialAssociation {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Debug, Error)]
pub(crate) enum LongitudinalCliError {
    #[error("invalid longitudinal input: {0}")]
    Input(String),
    #[error("failed to access {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid longitudinal JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub(crate) fn run_cli() -> Result<(), LongitudinalCliError> {
    match LongitudinalCli::parse().command {
        LongitudinalTopLevel::Longitudinal {
            command: LongitudinalCommand::KalmanSmooth { input, out },
        } => run_linear(input, out),
        LongitudinalTopLevel::Longitudinal {
            command: LongitudinalCommand::NonlinearFilter { input, out },
        } => run_nonlinear(input, out),
        LongitudinalTopLevel::Longitudinal {
            command: LongitudinalCommand::ParticleSmooth { input, out },
        } => run_particle(input, out),
        LongitudinalTopLevel::Longitudinal {
            command: LongitudinalCommand::PhylogeneticSpatialAssociation { input, out },
        } => run_phylogenetic_association(input, out),
    }
}

pub(crate) fn into_marklab_error(error: LongitudinalCliError) -> marklab::MarklabError {
    marklab::MarklabError::Validation(error.to_string())
}

fn run_linear(input_path: PathBuf, output_path: PathBuf) -> Result<(), LongitudinalCliError> {
    let bytes = read_input(input_path)?;
    let spec: LinearGaussianStateSpaceSpec = serde_json::from_slice(&bytes)?;
    let result = kalman_filter_and_smooth(spec)
        .map_err(|error| LongitudinalCliError::Input(error.to_string()))?;
    publish_json(&output_path, &result)
}

fn run_nonlinear(input_path: PathBuf, output_path: PathBuf) -> Result<(), LongitudinalCliError> {
    let bytes = read_input(input_path)?;
    let spec: ScalarNonlinearFilterSpec = serde_json::from_slice(&bytes)?;
    let result = nonlinear_gaussian_filter(spec)
        .map_err(|error| LongitudinalCliError::Input(error.to_string()))?;
    publish_json(&output_path, &result)
}

fn run_particle(input_path: PathBuf, output_path: PathBuf) -> Result<(), LongitudinalCliError> {
    let bytes = read_input(input_path)?;
    let spec: ScalarParticleSmootherSpec = serde_json::from_slice(&bytes)?;
    let result = particle_filter_and_smooth(spec)
        .map_err(|error| LongitudinalCliError::Input(error.to_string()))?;
    publish_json(&output_path, &result)
}

fn run_phylogenetic_association(
    input_path: PathBuf,
    output_path: PathBuf,
) -> Result<(), LongitudinalCliError> {
    let bytes = read_input(input_path)?;
    let spec: PhylogeneticSpatialAssociationSpec = serde_json::from_slice(&bytes)?;
    let result = phylogenetic_spatial_association(spec)
        .map_err(|error| LongitudinalCliError::Input(error.to_string()))?;
    publish_json(&output_path, &result)
}

fn read_input(input_path: PathBuf) -> Result<Vec<u8>, LongitudinalCliError> {
    let metadata = fs::metadata(&input_path).map_err(|source| LongitudinalCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(LongitudinalCliError::Input(
            "input must be a regular file within 16 MiB".into(),
        ));
    }
    fs::read(&input_path).map_err(|source| LongitudinalCliError::Io {
        path: input_path,
        source,
    })
}

fn publish_json(path: &Path, result: &impl Serialize) -> Result<(), LongitudinalCliError> {
    publish_pretty_json(path, result, "longitudinal").map_err(|error| match error {
        ExclusiveJsonOutputError::OutputExists => {
            LongitudinalCliError::Input(format!("output already exists: {}", path.display()))
        }
        ExclusiveJsonOutputError::OutputMustNameFile => {
            LongitudinalCliError::Input("output must name a file".into())
        }
        ExclusiveJsonOutputError::Io { path, source } => LongitudinalCliError::Io { path, source },
        ExclusiveJsonOutputError::Json(error) => LongitudinalCliError::Json(error),
    })
}

pub(crate) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<LongitudinalCli>(|| run_cli().map_err(into_marklab_error))
}
