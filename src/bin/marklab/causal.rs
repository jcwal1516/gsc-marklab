use std::{
    fs,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use marklab_causal::{
    binary_confounder_bias_sensitivity, compute_spatial_exposure_mapping,
    estimate_gaussian_expected_information_gain, manski_bounded_outcome_ate,
    randomized_binary_interference, rosenbaum_sign_sensitivity, BiasSensitivitySpec,
    ExposureMappingSpec, GaussianEigSpec, ManskiBoundedOutcomeSpec, RandomizedInterferenceSpec,
    RosenbaumSignSensitivitySpec,
};
use serde::Serialize;
use thiserror::Error;

use super::exclusive_json_output::{publish_pretty_json, ExclusiveJsonOutputError};

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct CausalCli {
    #[command(subcommand)]
    command: CausalTopLevel,
}

#[derive(Debug, Subcommand)]
enum CausalTopLevel {
    Causal {
        #[command(subcommand)]
        command: CausalCommand,
    },
}

#[derive(Debug, Subcommand)]
enum CausalCommand {
    RandomizedInterference {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    ExposureMapping {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    GaussianEig {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    BiasSensitivity {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    ManskiBounds {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    RosenbaumSignSensitivity {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Debug, Error)]
pub(crate) enum CausalCliError {
    #[error("invalid causal input: {0}")]
    Input(String),
    #[error("failed to access {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid causal JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub(crate) fn run_cli() -> Result<(), CausalCliError> {
    match CausalCli::parse().command {
        CausalTopLevel::Causal {
            command: CausalCommand::RandomizedInterference { input, out },
        } => run(input, out),
        CausalTopLevel::Causal {
            command: CausalCommand::ExposureMapping { input, out },
        } => run_exposure_mapping(input, out),
        CausalTopLevel::Causal {
            command: CausalCommand::GaussianEig { input, out },
        } => run_gaussian_eig(input, out),
        CausalTopLevel::Causal {
            command: CausalCommand::BiasSensitivity { input, out },
        } => run_bias_sensitivity(input, out),
        CausalTopLevel::Causal {
            command: CausalCommand::ManskiBounds { input, out },
        } => run_manski_bounds(input, out),
        CausalTopLevel::Causal {
            command: CausalCommand::RosenbaumSignSensitivity { input, out },
        } => run_rosenbaum(input, out),
    }
}

fn run_rosenbaum(input: PathBuf, out: PathBuf) -> Result<(), CausalCliError> {
    let bytes = read_input(input)?;
    let spec: RosenbaumSignSensitivitySpec = serde_json::from_slice(&bytes)?;
    let result = rosenbaum_sign_sensitivity(spec)
        .map_err(|error| CausalCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_manski_bounds(input: PathBuf, out: PathBuf) -> Result<(), CausalCliError> {
    let bytes = read_input(input)?;
    let spec: ManskiBoundedOutcomeSpec = serde_json::from_slice(&bytes)?;
    let result = manski_bounded_outcome_ate(spec)
        .map_err(|error| CausalCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_bias_sensitivity(input: PathBuf, out: PathBuf) -> Result<(), CausalCliError> {
    let bytes = read_input(input)?;
    let spec: BiasSensitivitySpec = serde_json::from_slice(&bytes)?;
    let result = binary_confounder_bias_sensitivity(spec)
        .map_err(|error| CausalCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_gaussian_eig(input: PathBuf, out: PathBuf) -> Result<(), CausalCliError> {
    let bytes = read_input(input)?;
    let spec: GaussianEigSpec = serde_json::from_slice(&bytes)?;
    let result = estimate_gaussian_expected_information_gain(spec)
        .map_err(|error| CausalCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_exposure_mapping(input: PathBuf, out: PathBuf) -> Result<(), CausalCliError> {
    let bytes = read_input(input)?;
    let spec: ExposureMappingSpec = serde_json::from_slice(&bytes)?;
    let result = compute_spatial_exposure_mapping(spec)
        .map_err(|error| CausalCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

pub(crate) fn into_marklab_error(error: CausalCliError) -> marklab::MarklabError {
    marklab::MarklabError::Validation(error.to_string())
}

fn run(input: PathBuf, out: PathBuf) -> Result<(), CausalCliError> {
    let bytes = read_input(input)?;
    let spec: RandomizedInterferenceSpec = serde_json::from_slice(&bytes)?;
    let result = randomized_binary_interference(spec)
        .map_err(|error| CausalCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn read_input(input: PathBuf) -> Result<Vec<u8>, CausalCliError> {
    let metadata = fs::metadata(&input).map_err(|source| CausalCliError::Io {
        path: input.clone(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(CausalCliError::Input(
            "input must be a regular file within 16 MiB".into(),
        ));
    }
    fs::read(&input).map_err(|source| CausalCliError::Io {
        path: input,
        source,
    })
}

fn publish_json(path: &Path, result: &impl Serialize) -> Result<(), CausalCliError> {
    publish_pretty_json(path, result, "causal").map_err(|error| match error {
        ExclusiveJsonOutputError::OutputExists => {
            CausalCliError::Input(format!("output already exists: {}", path.display()))
        }
        ExclusiveJsonOutputError::OutputMustNameFile => {
            CausalCliError::Input("output must name a file".into())
        }
        ExclusiveJsonOutputError::Io { path, source } => CausalCliError::Io { path, source },
        ExclusiveJsonOutputError::Json(error) => CausalCliError::Json(error),
    })
}

pub(crate) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<CausalCli>(|| run_cli().map_err(into_marklab_error))
}
