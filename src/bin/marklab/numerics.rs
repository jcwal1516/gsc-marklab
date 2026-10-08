use std::{
    fs,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use marklab_numerics::{evaluate_stable_primitives, StablePrimitivesSpec};
use serde::Serialize;
use thiserror::Error;

use super::exclusive_json_output::{publish_pretty_json, ExclusiveJsonOutputError};

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
    LocalMultivariateMoran {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        window: PathBuf,
        #[arg(long)]
        radius_um: f64,
        #[arg(long)]
        permutations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        maximum_points: usize,
        #[arg(long)]
        maximum_dimension: usize,
        #[arg(long)]
        maximum_directed_edges: usize,
        #[arg(long)]
        maximum_permutation_edge_evaluations: usize,
        #[arg(long)]
        memory_budget_mib: usize,
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
        NumericsTopLevel::Numerics {
            command:
                NumericsCommand::LocalMultivariateMoran {
                    input,
                    window,
                    radius_um,
                    permutations,
                    seed,
                    maximum_points,
                    maximum_dimension,
                    maximum_directed_edges,
                    maximum_permutation_edge_evaluations,
                    memory_budget_mib,
                    out,
                },
        } => super::local_multivariate::run_direct(
            input,
            window,
            radius_um,
            permutations,
            seed,
            maximum_points,
            maximum_dimension,
            maximum_directed_edges,
            maximum_permutation_edge_evaluations,
            memory_budget_mib,
            out,
        ),
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
    publish_pretty_json(path, result, "numerics").map_err(|error| match error {
        ExclusiveJsonOutputError::OutputExists => {
            NumericsCliError::Input(format!("output already exists: {}", path.display()))
        }
        ExclusiveJsonOutputError::OutputMustNameFile => {
            NumericsCliError::Input("output must name a file".into())
        }
        ExclusiveJsonOutputError::Io { path, source } => NumericsCliError::Io { path, source },
        ExclusiveJsonOutputError::Json(error) => NumericsCliError::Json(error),
    })
}

pub(crate) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<NumericsCli>(|| run_cli().map_err(into_marklab_error))
}
