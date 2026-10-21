use std::path::{Path, PathBuf};

use clap::Subcommand;
use marklab_causal::{
    binary_confounder_bias_sensitivity, compute_spatial_exposure_mapping,
    estimate_gaussian_expected_information_gain, manski_bounded_outcome_ate,
    randomized_binary_interference, rosenbaum_sign_sensitivity, BiasSensitivitySpec,
    ExposureMappingSpec, GaussianEigSpec, ManskiBoundedOutcomeSpec, RandomizedInterferenceSpec,
    RosenbaumSignSensitivitySpec,
};
use serde::Serialize;

use super::exclusive_json_output::{publish_pretty_json, ExclusiveJsonOutputError};
use crate::cli_error::{into_marklab_error, CliError};

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Subcommand)]
pub(crate) enum CausalCommand {
    /// Estimate randomized treatment effects under declared spatial interference.
    RandomizedInterference {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Compute neighborhood exposure under a declared interference graph.
    ExposureMapping {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Estimate expected information gain for a Gaussian observation design.
    GaussianEig {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Evaluate effect sensitivity to a declared unmeasured confounder.
    BiasSensitivity {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Compute treatment-effect bounds from bounded outcomes.
    ManskiBounds {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Bound matched-pair sign-test evidence under hidden assignment bias.
    RosenbaumSignSensitivity {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
}

pub(crate) fn run_cli(command: CausalCommand) -> Result<(), CliError> {
    match command {
        CausalCommand::RandomizedInterference { input, out } => run(input, out),
        CausalCommand::ExposureMapping { input, out } => run_exposure_mapping(input, out),
        CausalCommand::GaussianEig { input, out } => run_gaussian_eig(input, out),
        CausalCommand::BiasSensitivity { input, out } => run_bias_sensitivity(input, out),
        CausalCommand::ManskiBounds { input, out } => run_manski_bounds(input, out),
        CausalCommand::RosenbaumSignSensitivity { input, out } => run_rosenbaum(input, out),
    }
}

fn run_rosenbaum(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(input)?;
    let spec: RosenbaumSignSensitivitySpec = serde_json::from_slice(&bytes)?;
    let result =
        rosenbaum_sign_sensitivity(spec).map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_manski_bounds(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(input)?;
    let spec: ManskiBoundedOutcomeSpec = serde_json::from_slice(&bytes)?;
    let result =
        manski_bounded_outcome_ate(spec).map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_bias_sensitivity(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(input)?;
    let spec: BiasSensitivitySpec = serde_json::from_slice(&bytes)?;
    let result = binary_confounder_bias_sensitivity(spec)
        .map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_gaussian_eig(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(input)?;
    let spec: GaussianEigSpec = serde_json::from_slice(&bytes)?;
    let result = estimate_gaussian_expected_information_gain(spec)
        .map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_exposure_mapping(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(input)?;
    let spec: ExposureMappingSpec = serde_json::from_slice(&bytes)?;
    let result = compute_spatial_exposure_mapping(spec)
        .map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(input)?;
    let spec: RandomizedInterferenceSpec = serde_json::from_slice(&bytes)?;
    let result =
        randomized_binary_interference(spec).map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn read_input(input: PathBuf) -> Result<Vec<u8>, CliError> {
    crate::input_file::read_regular_file(
        &input,
        MAXIMUM_INPUT_BYTES,
        "input must be a regular file within 16 MiB",
    )
}

fn publish_json(path: &Path, result: &impl Serialize) -> Result<(), CliError> {
    publish_pretty_json(path, result, "causal").map_err(|error| match error {
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

pub(crate) fn execute(command: CausalCommand) -> marklab::Result<()> {
    run_cli(command).map_err(into_marklab_error)
}
