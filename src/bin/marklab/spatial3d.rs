use std::{
    fs,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use marklab_spatial3d::{
    build_spatial_graph3d, directed_cross_k3d, homogeneous_k3d, inhomogeneous_k3d,
    voxel_window_k3d, DirectedCrossK3dSpec, HomogeneousK3dSpec, InhomogeneousK3dSpec,
    SpatialGraph3dSpec, VoxelWindowK3dSpec,
};
use serde::Serialize;
use thiserror::Error;

use super::exclusive_json_output::{publish_pretty_json, ExclusiveJsonOutputError};
use super::spatial3d_registered;

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct Spatial3dCli {
    #[command(subcommand)]
    command: Spatial3dTopLevel,
}

#[derive(Debug, Subcommand)]
enum Spatial3dTopLevel {
    Spatial3d {
        #[command(subcommand)]
        command: Spatial3dCommand,
    },
}

#[derive(Debug, Subcommand)]
enum Spatial3dCommand {
    KFunction {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    VoxelKFunction {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    RegisteredSerialVoxelK {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    RegisteredLongitudinalVoxelKChange {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    InhomogeneousK {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    CrossK {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SpatialGraph {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Debug, Error)]
pub(crate) enum Spatial3dCliError {
    #[error("invalid spatial3d input: {0}")]
    Input(String),
    #[error("failed to access {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid spatial3d JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub(crate) fn run_cli() -> Result<(), Spatial3dCliError> {
    match Spatial3dCli::parse().command {
        Spatial3dTopLevel::Spatial3d {
            command: Spatial3dCommand::KFunction { input, out },
        } => run(input, out),
        Spatial3dTopLevel::Spatial3d {
            command: Spatial3dCommand::VoxelKFunction { input, out },
        } => run_voxel(input, out),
        Spatial3dTopLevel::Spatial3d {
            command: Spatial3dCommand::RegisteredSerialVoxelK { input, out },
        } => run_registered_serial(input, out),
        Spatial3dTopLevel::Spatial3d {
            command: Spatial3dCommand::RegisteredLongitudinalVoxelKChange { input, out },
        } => run_registered_longitudinal(input, out),
        Spatial3dTopLevel::Spatial3d {
            command: Spatial3dCommand::InhomogeneousK { input, out },
        } => run_inhomogeneous(input, out),
        Spatial3dTopLevel::Spatial3d {
            command: Spatial3dCommand::CrossK { input, out },
        } => run_cross(input, out),
        Spatial3dTopLevel::Spatial3d {
            command: Spatial3dCommand::SpatialGraph { input, out },
        } => run_graph(input, out),
    }
}

fn run_voxel(input: PathBuf, out: PathBuf) -> Result<(), Spatial3dCliError> {
    let bytes = read_input(input)?;
    let spec: VoxelWindowK3dSpec = serde_json::from_slice(&bytes)?;
    let result =
        voxel_window_k3d(spec).map_err(|error| Spatial3dCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_registered_serial(input: PathBuf, out: PathBuf) -> Result<(), Spatial3dCliError> {
    let bytes = read_input(input)?;
    let prepared = spatial3d_registered::prepare(&bytes)
        .map_err(|error| Spatial3dCliError::Input(error.to_string()))?;
    let result = spatial3d_registered::execute(&prepared)
        .map_err(|error| Spatial3dCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_registered_longitudinal(input: PathBuf, out: PathBuf) -> Result<(), Spatial3dCliError> {
    let bytes = read_input(input)?;
    let prepared = spatial3d_registered::longitudinal::prepare(&bytes)
        .map_err(|error| Spatial3dCliError::Input(error.to_string()))?;
    let result = spatial3d_registered::longitudinal::execute(&prepared)
        .map_err(|error| Spatial3dCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_inhomogeneous(input: PathBuf, out: PathBuf) -> Result<(), Spatial3dCliError> {
    let bytes = read_input(input)?;
    let spec: InhomogeneousK3dSpec = serde_json::from_slice(&bytes)?;
    let result =
        inhomogeneous_k3d(spec).map_err(|error| Spatial3dCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_cross(input: PathBuf, out: PathBuf) -> Result<(), Spatial3dCliError> {
    let bytes = read_input(input)?;
    let spec: DirectedCrossK3dSpec = serde_json::from_slice(&bytes)?;
    let result =
        directed_cross_k3d(spec).map_err(|error| Spatial3dCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_graph(input: PathBuf, out: PathBuf) -> Result<(), Spatial3dCliError> {
    let bytes = read_input(input)?;
    let spec: SpatialGraph3dSpec = serde_json::from_slice(&bytes)?;
    let result =
        build_spatial_graph3d(spec).map_err(|error| Spatial3dCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn read_input(input: PathBuf) -> Result<Vec<u8>, Spatial3dCliError> {
    let metadata = fs::metadata(&input).map_err(|source| Spatial3dCliError::Io {
        path: input.clone(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(Spatial3dCliError::Input(
            "input must be a regular file within 16 MiB".into(),
        ));
    }
    fs::read(&input).map_err(|source| Spatial3dCliError::Io {
        path: input,
        source,
    })
}

pub(crate) fn into_marklab_error(error: Spatial3dCliError) -> marklab::MarklabError {
    marklab::MarklabError::Validation(error.to_string())
}

fn run(input: PathBuf, out: PathBuf) -> Result<(), Spatial3dCliError> {
    let bytes = read_input(input)?;
    let spec: HomogeneousK3dSpec = serde_json::from_slice(&bytes)?;
    let result =
        homogeneous_k3d(spec).map_err(|error| Spatial3dCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn publish_json(path: &Path, result: &impl Serialize) -> Result<(), Spatial3dCliError> {
    publish_pretty_json(path, result, "spatial3d").map_err(|error| match error {
        ExclusiveJsonOutputError::OutputExists => {
            Spatial3dCliError::Input(format!("output already exists: {}", path.display()))
        }
        ExclusiveJsonOutputError::OutputMustNameFile => {
            Spatial3dCliError::Input("output must name a file".into())
        }
        ExclusiveJsonOutputError::Io { path, source } => Spatial3dCliError::Io { path, source },
        ExclusiveJsonOutputError::Json(error) => Spatial3dCliError::Json(error),
    })
}
