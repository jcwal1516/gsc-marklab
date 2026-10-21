use std::path::{Path, PathBuf};

use clap::Subcommand;
use marklab_spatial3d::{
    build_spatial_graph3d, directed_cross_k3d, homogeneous_k3d, inhomogeneous_k3d,
    voxel_window_k3d, DirectedCrossK3dSpec, HomogeneousK3dSpec, InhomogeneousK3dSpec,
    SpatialGraph3dSpec, VoxelWindowK3dSpec,
};
use serde::Serialize;

use super::exclusive_json_output::{publish_pretty_json, ExclusiveJsonOutputError};
use super::spatial3d_registered;
use crate::cli_error::{into_marklab_error, CliError};

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Subcommand)]
pub(crate) enum Spatial3dCommand {
    /// Estimate a three-dimensional K function in a declared window.
    KFunction {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Estimate a three-dimensional K function within a voxel window.
    VoxelKFunction {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Propagate serial-section registration into a voxel-window K analysis.
    RegisteredSerialVoxelK {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Compare longitudinal voxel-window K curves under registration uncertainty.
    RegisteredLongitudinalVoxelKChange {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Estimate an intensity-reweighted three-dimensional K function.
    InhomogeneousK {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Estimate a directed cross-type three-dimensional K function.
    CrossK {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Build a three-dimensional spatial neighbor graph.
    SpatialGraph {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
}

pub(crate) fn run_cli(command: Spatial3dCommand) -> Result<(), CliError> {
    match command {
        Spatial3dCommand::KFunction { input, out } => run(input, out),
        Spatial3dCommand::VoxelKFunction { input, out } => run_voxel(input, out),
        Spatial3dCommand::RegisteredSerialVoxelK { input, out } => {
            run_registered_serial(input, out)
        }
        Spatial3dCommand::RegisteredLongitudinalVoxelKChange { input, out } => {
            run_registered_longitudinal(input, out)
        }
        Spatial3dCommand::InhomogeneousK { input, out } => run_inhomogeneous(input, out),
        Spatial3dCommand::CrossK { input, out } => run_cross(input, out),
        Spatial3dCommand::SpatialGraph { input, out } => run_graph(input, out),
    }
}

fn run_voxel(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(input)?;
    let spec: VoxelWindowK3dSpec = serde_json::from_slice(&bytes)?;
    let result = voxel_window_k3d(spec).map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_registered_serial(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(input)?;
    let prepared = spatial3d_registered::prepare(&bytes)
        .map_err(|error| CliError::Input(error.to_string()))?;
    let result = spatial3d_registered::execute(&prepared)
        .map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_registered_longitudinal(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(input)?;
    let prepared = spatial3d_registered::longitudinal::prepare(&bytes)
        .map_err(|error| CliError::Input(error.to_string()))?;
    let result = spatial3d_registered::longitudinal::execute(&prepared)
        .map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_inhomogeneous(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(input)?;
    let spec: InhomogeneousK3dSpec = serde_json::from_slice(&bytes)?;
    let result = inhomogeneous_k3d(spec).map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_cross(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(input)?;
    let spec: DirectedCrossK3dSpec = serde_json::from_slice(&bytes)?;
    let result = directed_cross_k3d(spec).map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_graph(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(input)?;
    let spec: SpatialGraph3dSpec = serde_json::from_slice(&bytes)?;
    let result = build_spatial_graph3d(spec).map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn read_input(input: PathBuf) -> Result<Vec<u8>, CliError> {
    crate::input_file::read_regular_file(
        &input,
        MAXIMUM_INPUT_BYTES,
        "input must be a regular file within 16 MiB",
    )
}

fn run(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(input)?;
    let spec: HomogeneousK3dSpec = serde_json::from_slice(&bytes)?;
    let result = homogeneous_k3d(spec).map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn publish_json(path: &Path, result: &impl Serialize) -> Result<(), CliError> {
    publish_pretty_json(path, result, "spatial3d").map_err(|error| match error {
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

pub(crate) fn execute(command: Spatial3dCommand) -> marklab::Result<()> {
    run_cli(command).map_err(into_marklab_error)
}
