use std::{
    fs,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use marklab_policy::{
    determine_result_maturity, evaluate_validation_ladder, run_runtime_validation,
    select_execution_mode, BayesianHmcFitDiagnosticSpec, ExecutionModeSelectionSpec,
    ResultMaturitySpec, RuntimeValidationSpec, ValidationLadderSpec,
};
use serde::Serialize;
use thiserror::Error;

use super::exclusive_json_output::{publish_pretty_json, ExclusiveJsonOutputError};

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct PolicyCli {
    #[command(subcommand)]
    command: PolicyTopLevel,
}

#[derive(Debug, Subcommand)]
enum PolicyTopLevel {
    Policy {
        #[command(subcommand)]
        command: PolicyCommand,
    },
}

#[derive(Debug, Subcommand)]
enum PolicyCommand {
    DetermineMaturity {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SelectMode {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    ValidationLadder {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    RuntimeValidation {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        fit: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Debug, Error)]
pub(crate) enum PolicyCliError {
    #[error("invalid policy input: {0}")]
    Input(String),
    #[error("failed to access {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid policy JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub(crate) fn run_cli() -> Result<(), PolicyCliError> {
    match PolicyCli::parse().command {
        PolicyTopLevel::Policy {
            command: PolicyCommand::DetermineMaturity { input, out },
        } => run(input, out),
        PolicyTopLevel::Policy {
            command: PolicyCommand::SelectMode { input, out },
        } => run_mode(input, out),
        PolicyTopLevel::Policy {
            command: PolicyCommand::ValidationLadder { input, out },
        } => run_validation_ladder(input, out),
        PolicyTopLevel::Policy {
            command: PolicyCommand::RuntimeValidation { input, fit, out },
        } => run_runtime(input, fit, out),
    }
}

fn run_runtime(input: PathBuf, fit: PathBuf, out: PathBuf) -> Result<(), PolicyCliError> {
    let input_bytes = read_bounded(&input)?;
    let fit_bytes = read_bounded(&fit)?;
    let spec: RuntimeValidationSpec = serde_json::from_slice(&input_bytes)?;
    let fit_value: serde_json::Value = serde_json::from_slice(&fit_bytes)?;
    if fit_value["format"] != "marklab.fixed_step_hmc_normal_mean" {
        return Err(PolicyCliError::Input(
            "runtime validation currently requires a fixed-step HMC fit artifact".into(),
        ));
    }
    let diagnostic = BayesianHmcFitDiagnosticSpec {
        posterior_mean: required_f64(&fit_value, &["posterior", "mean"])?,
        analytic_mean: required_f64(&fit_value, &["analytic_posterior", "mean"])?,
        posterior_standard_deviation: required_f64(
            &fit_value,
            &["posterior", "standard_deviation"],
        )?,
        draw_count: fit_value["posterior"]["draws"]
            .as_array()
            .map(Vec::len)
            .ok_or_else(|| PolicyCliError::Input("HMC fit lacks posterior draws".into()))?,
        acceptance_rate: required_f64(&fit_value, &["diagnostics", "acceptance_rate"])?,
        maximum_absolute_energy_error: required_f64(
            &fit_value,
            &["diagnostics", "maximum_absolute_energy_error"],
        )?,
    };
    let result = run_runtime_validation(spec, diagnostic)
        .map_err(|error| PolicyCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn required_f64(value: &serde_json::Value, path: &[&str]) -> Result<f64, PolicyCliError> {
    let mut current = value;
    for component in path {
        current = &current[*component];
    }
    current
        .as_f64()
        .filter(|number| number.is_finite())
        .ok_or_else(|| PolicyCliError::Input(format!("HMC fit lacks finite {}", path.join("."))))
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, PolicyCliError> {
    let metadata = fs::metadata(path).map_err(|source| PolicyCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(PolicyCliError::Input(
            "input must be a regular file within 16 MiB".into(),
        ));
    }
    fs::read(path).map_err(|source| PolicyCliError::Io {
        path: path.to_owned(),
        source,
    })
}

fn run_validation_ladder(input: PathBuf, out: PathBuf) -> Result<(), PolicyCliError> {
    let bytes = read_bounded(&input)?;
    let spec: ValidationLadderSpec = serde_json::from_slice(&bytes)?;
    let result = evaluate_validation_ladder(spec)
        .map_err(|error| PolicyCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_mode(input: PathBuf, out: PathBuf) -> Result<(), PolicyCliError> {
    let bytes = read_bounded(&input)?;
    let spec: ExecutionModeSelectionSpec = serde_json::from_slice(&bytes)?;
    let result =
        select_execution_mode(spec).map_err(|error| PolicyCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

pub(crate) fn into_marklab_error(error: PolicyCliError) -> marklab::MarklabError {
    marklab::MarklabError::Validation(error.to_string())
}

fn run(input: PathBuf, out: PathBuf) -> Result<(), PolicyCliError> {
    let bytes = read_bounded(&input)?;
    let spec: ResultMaturitySpec = serde_json::from_slice(&bytes)?;
    let result = determine_result_maturity(spec)
        .map_err(|error| PolicyCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn publish_json(path: &Path, result: &impl Serialize) -> Result<(), PolicyCliError> {
    publish_pretty_json(path, result, "policy").map_err(|error| match error {
        ExclusiveJsonOutputError::OutputExists => {
            PolicyCliError::Input(format!("output already exists: {}", path.display()))
        }
        ExclusiveJsonOutputError::OutputMustNameFile => {
            PolicyCliError::Input("output must name a file".into())
        }
        ExclusiveJsonOutputError::Io { path, source } => PolicyCliError::Io { path, source },
        ExclusiveJsonOutputError::Json(error) => PolicyCliError::Json(error),
    })
}
