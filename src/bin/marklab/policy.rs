use std::path::{Path, PathBuf};

use clap::Subcommand;
use marklab_policy::{
    determine_result_maturity, evaluate_validation_ladder, select_execution_mode,
    ExecutionModeSelectionSpec, ResultMaturitySpec, ValidationLadderSpec,
};
use serde::Serialize;

use super::exclusive_json_output::{publish_pretty_json, ExclusiveJsonOutputError};
use crate::cli_error::{into_marklab_error, CliError};

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Subcommand)]
pub(crate) enum PolicyCommand {
    /// Determine the supported claim level from declared validation evidence.
    DetermineMaturity {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Select an execution mode under declared data and resource constraints.
    SelectMode {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Evaluate the declared stages of a scientific validation ladder.
    ValidationLadder {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
}

pub(crate) fn run_cli(command: PolicyCommand) -> Result<(), CliError> {
    match command {
        PolicyCommand::DetermineMaturity { input, out } => run(input, out),
        PolicyCommand::SelectMode { input, out } => run_mode(input, out),
        PolicyCommand::ValidationLadder { input, out } => run_validation_ladder(input, out),
    }
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, CliError> {
    crate::input_file::read_regular_file(
        path,
        MAXIMUM_INPUT_BYTES,
        "input must be a regular file within 16 MiB",
    )
}

fn run_validation_ladder(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_bounded(&input)?;
    let spec: ValidationLadderSpec = serde_json::from_slice(&bytes)?;
    let result =
        evaluate_validation_ladder(spec).map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_mode(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_bounded(&input)?;
    let spec: ExecutionModeSelectionSpec = serde_json::from_slice(&bytes)?;
    let result = select_execution_mode(spec).map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_bounded(&input)?;
    let spec: ResultMaturitySpec = serde_json::from_slice(&bytes)?;
    let result =
        determine_result_maturity(spec).map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn publish_json(path: &Path, result: &impl Serialize) -> Result<(), CliError> {
    publish_pretty_json(path, result, "policy").map_err(|error| match error {
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

pub(crate) fn execute(command: PolicyCommand) -> marklab::Result<()> {
    run_cli(command).map_err(into_marklab_error)
}
