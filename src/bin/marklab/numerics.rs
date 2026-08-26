use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use marklab_numerics::{evaluate_stable_primitives, StablePrimitivesSpec};
use serde::Serialize;
use thiserror::Error;

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct NumericsCli {
    #[command(subcommand)]
    command: NumericsTopLevel,
}

#[derive(Debug, Subcommand)]
enum NumericsTopLevel {
    Numerics {
        #[command(subcommand)]
        command: NumericsCommand,
    },
}

#[derive(Debug, Subcommand)]
enum NumericsCommand {
    StablePrimitives {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Debug, Error)]
pub(crate) enum NumericsCliError {
    #[error("invalid numerics input: {0}")]
    Input(String),
    #[error("failed to access {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid numerics JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub(crate) fn run_cli() -> Result<(), NumericsCliError> {
    match NumericsCli::parse().command {
        NumericsTopLevel::Numerics {
            command: NumericsCommand::StablePrimitives { input, out },
        } => run(input, out),
    }
}

pub(crate) fn into_marklab_error(error: NumericsCliError) -> marklab::MarklabError {
    marklab::MarklabError::Validation(error.to_string())
}

fn run(input: PathBuf, out: PathBuf) -> Result<(), NumericsCliError> {
    let metadata = fs::metadata(&input).map_err(|source| NumericsCliError::Io {
        path: input.clone(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(NumericsCliError::Input(
            "input must be a regular file within 16 MiB".into(),
        ));
    }
    let bytes = fs::read(&input).map_err(|source| NumericsCliError::Io {
        path: input,
        source,
    })?;
    let spec: StablePrimitivesSpec = serde_json::from_slice(&bytes)?;
    let result = evaluate_stable_primitives(spec)
        .map_err(|error| NumericsCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn publish_json(path: &Path, result: &impl Serialize) -> Result<(), NumericsCliError> {
    match fs::symlink_metadata(path) {
        Ok(_) => {
            return Err(NumericsCliError::Input(format!(
                "output already exists: {}",
                path.display()
            )))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(NumericsCliError::Io {
                path: path.to_owned(),
                source,
            })
        }
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| NumericsCliError::Io {
        path: parent.to_owned(),
        source,
    })?;
    let file_name = path
        .file_name()
        .ok_or_else(|| NumericsCliError::Input("output must name a file".into()))?;
    let mut staging_name = OsString::from(".");
    staging_name.push(file_name);
    staging_name.push(format!(".marklab-numerics-{}.tmp", std::process::id()));
    let staging = parent.join(staging_name);
    let bytes = serde_json::to_vec_pretty(result)?;
    let publication = (|| -> Result<(), NumericsCliError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)
            .map_err(|source| NumericsCliError::Io {
                path: staging.clone(),
                source,
            })?;
        file.write_all(&bytes)
            .and_then(|()| file.write_all(b"\n"))
            .and_then(|()| file.sync_all())
            .map_err(|source| NumericsCliError::Io {
                path: staging.clone(),
                source,
            })?;
        fs::rename(&staging, path).map_err(|source| NumericsCliError::Io {
            path: path.to_owned(),
            source,
        })
    })();
    if publication.is_err() {
        let _ = fs::remove_file(&staging);
    }
    publication
}
