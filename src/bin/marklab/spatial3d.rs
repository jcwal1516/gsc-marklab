use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use marklab_spatial3d::{
    build_spatial_graph3d, directed_cross_k3d, homogeneous_k3d, inhomogeneous_k3d,
    DirectedCrossK3dSpec, HomogeneousK3dSpec, InhomogeneousK3dSpec, SpatialGraph3dSpec,
};
use serde::Serialize;
use thiserror::Error;

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
    match fs::symlink_metadata(path) {
        Ok(_) => {
            return Err(Spatial3dCliError::Input(format!(
                "output already exists: {}",
                path.display()
            )))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(Spatial3dCliError::Io {
                path: path.to_owned(),
                source,
            })
        }
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| Spatial3dCliError::Io {
        path: parent.to_owned(),
        source,
    })?;
    let file_name = path
        .file_name()
        .ok_or_else(|| Spatial3dCliError::Input("output must name a file".into()))?;
    let mut staging_name = OsString::from(".");
    staging_name.push(file_name);
    staging_name.push(format!(".marklab-spatial3d-{}.tmp", std::process::id()));
    let staging = parent.join(staging_name);
    let bytes = serde_json::to_vec_pretty(result)?;
    let publication = (|| -> Result<(), Spatial3dCliError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)
            .map_err(|source| Spatial3dCliError::Io {
                path: staging.clone(),
                source,
            })?;
        file.write_all(&bytes)
            .and_then(|()| file.write_all(b"\n"))
            .and_then(|()| file.sync_all())
            .map_err(|source| Spatial3dCliError::Io {
                path: staging.clone(),
                source,
            })?;
        fs::rename(&staging, path).map_err(|source| Spatial3dCliError::Io {
            path: path.to_owned(),
            source,
        })
    })();
    if publication.is_err() {
        let _ = fs::remove_file(&staging);
    }
    publication
}
