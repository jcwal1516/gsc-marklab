use std::path::PathBuf;

use clap::{Parser, Subcommand};
use marklab_topology::sha256_hex;
use serde_json::Value;

use super::topology::{publish_json, read_input, read_required, run_worker, TopologyCliError};

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct CausalModelCli {
    #[command(subcommand)]
    command: CausalTopLevel,
}

#[derive(Debug, Subcommand)]
enum CausalTopLevel {
    Causal {
        #[command(subcommand)]
        command: CausalModelCommand,
    },
}

#[derive(Debug, Subcommand)]
enum CausalModelCommand {
    Observational {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Perturbation {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    ActiveDesign {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    ValidateActive {
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
}

pub(crate) fn run_cli() -> Result<(), TopologyCliError> {
    match CausalModelCli::parse().command {
        CausalTopLevel::Causal {
            command: CausalModelCommand::Observational { input, out },
        } => run_model("observational", input, out),
        CausalTopLevel::Causal {
            command: CausalModelCommand::Perturbation { input, out },
        } => run_model("perturbation", input, out),
        CausalTopLevel::Causal {
            command: CausalModelCommand::ActiveDesign { input, out },
        } => run_model("active_design", input, out),
        CausalTopLevel::Causal {
            command:
                CausalModelCommand::ValidateActive {
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_validation(seed, timeout_seconds, out),
    }
}

fn backend(
    repository: &std::path::Path,
    worker_path: &std::path::Path,
) -> Result<Value, TopologyCliError> {
    let lock = read_required(&repository.join("workers/python/uv.lock"))?;
    let worker = read_required(worker_path)?;
    Ok(serde_json::json!({
        "name":"numpy_scipy_causal_active_design",
        "scipy_version":"1.18.1", "numpy_version":"2.4.6", "python_version":"3.12",
        "license":"BSD-3-Clause", "environment_lock_sha256":sha256_hex(&lock),
        "worker_sha256":sha256_hex(&worker)
    }))
}

fn run_model(mode: &str, input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let bytes = read_input(&input)?;
    let spec: Value = serde_json::from_slice(&bytes)?;
    let timeout = spec
        .get("timeout_seconds")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            TopologyCliError::Input("causal model input requires integer timeout_seconds".into())
        })?;
    if !(1..=3_600).contains(&timeout) || !spec.is_object() {
        return Err(TopologyCliError::Input(
            "causal model timeout must be between 1 and 3600 seconds".into(),
        ));
    }
    let repository = marklab::python_backend_assets_root()?;
    let worker_path = repository.join("workers/python/marklab_scipy_causal_active_worker.py");
    let request = serde_json::json!({
        "format":"marklab.scipy_causal_active_request", "version":1, "mode":mode,
        "backend":backend(&repository, &worker_path)?, "spec":spec,
        "input_sha256":sha256_hex(&bytes)
    });
    publish_checked(&repository, &worker_path, request, timeout, out, mode)
}

fn run_validation(seed: u64, timeout_seconds: u64, out: PathBuf) -> Result<(), TopologyCliError> {
    if !(1..=3_600).contains(&timeout_seconds) {
        return Err(TopologyCliError::Input(
            "causal validation timeout must be between 1 and 3600 seconds".into(),
        ));
    }
    let repository = marklab::python_backend_assets_root()?;
    let worker_path = repository.join("workers/python/marklab_scipy_causal_active_worker.py");
    let request = serde_json::json!({
        "format":"marklab.scipy_causal_active_request", "version":1, "mode":"validation",
        "backend":backend(&repository, &worker_path)?, "seed":seed
    });
    publish_checked(
        &repository,
        &worker_path,
        request,
        timeout_seconds,
        out,
        "validation",
    )
}

fn publish_checked(
    repository: &std::path::Path,
    worker_path: &std::path::Path,
    request: Value,
    timeout: u64,
    out: PathBuf,
    mode: &str,
) -> Result<(), TopologyCliError> {
    let bytes = serde_json::to_vec(&request)?;
    let response = run_worker(repository, worker_path, &bytes, timeout)?;
    let result: Value = serde_json::from_slice(&response)?;
    let expected = match mode {
        "observational" => "marklab.synthetic_observational_causal_estimators",
        "perturbation" => "marklab.synthetic_spatial_perturbation_analysis",
        "active_design" => "marklab.synthetic_active_design",
        _ => "marklab.causal_active_validation_suite",
    };
    if result["format"] != expected
        || result["backend"] != request["backend"]
        || result["request_sha256"] != sha256_hex(&bytes)
    {
        return Err(TopologyCliError::Backend(
            "causal/active result identity mismatch".into(),
        ));
    }
    publish_json(&out, &result)
}

pub(crate) fn into_marklab_error(error: TopologyCliError) -> marklab::MarklabError {
    marklab::MarklabError::Validation(error.to_string())
}

pub(crate) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<CausalModelCli>(|| run_cli().map_err(into_marklab_error))
}
