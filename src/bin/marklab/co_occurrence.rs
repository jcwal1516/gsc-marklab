use std::path::{Path, PathBuf};

use marklab::marks::categorical_cooccurrence_from_json;

use super::{
    exclusive_json_output::{publish_pretty_json, ExclusiveJsonOutputError},
    input_file::read_regular_file,
};
use crate::cli_error::CliError;

const MAXIMUM_INPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, clap::Args)]
pub(crate) struct Arguments {
    /// Strict categorical co-occurrence JSON request.
    #[arg(long)]
    input: PathBuf,
    /// New JSON result file; it must not already exist.
    #[arg(long)]
    out: PathBuf,
}

pub(crate) fn run(args: Arguments) -> Result<(), CliError> {
    let bytes = read_regular_file(
        &args.input,
        MAXIMUM_INPUT_BYTES as u64,
        "categorical co-occurrence input must be a regular file within 16 MiB",
    )?;
    let result = categorical_cooccurrence_from_json(&bytes)
        .map_err(|error| CliError::Input(error.to_string()))?;
    drop(bytes);
    publish(&args.out, &result)
}

fn publish(path: &Path, result: &impl serde::Serialize) -> Result<(), CliError> {
    publish_pretty_json(path, result, "categorical-cooccurrence").map_err(|error| match error {
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
