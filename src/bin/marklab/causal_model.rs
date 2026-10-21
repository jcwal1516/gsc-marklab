use marklab_core::backend::is_worker_timeout;
use std::path::PathBuf;

use clap::Subcommand;
use marklab_causal::protocol::{Backend, ModelRequest, ModelResult, ModelSpec, ValidationRequest};
use marklab_core::backend::sha256_hex;
use serde::Serialize;

use super::topology::{publish_json, read_input, read_required, run_worker};
use crate::cli_error::{into_marklab_error, CliError};
use marklab_core::backend::versions::NUMPY_VERSION;
use marklab_core::backend::versions::PYTHON_VERSION;
use marklab_core::backend::versions::SCIPY_VERSION;

#[derive(Debug, Subcommand)]
pub(crate) enum CausalModelCommand {
    /// Fit synthetic observational causal estimators with cluster cross-fitting.
    Observational {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Analyze direct, spillover and mediation effects in a randomized perturbation.
    Perturbation {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Select budgeted acquisitions and compare replication and power designs.
    ActiveDesign {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Run causal and active-design checks with explicit unverified evidence.
    ValidateActive {
        /// Random seed for reproducible simulation or inference.
        #[arg(long)]
        seed: u64,
        /// Maximum worker execution time in seconds.
        #[arg(long)]
        timeout_seconds: u64,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
}

pub(crate) fn run_cli(command: CausalModelCommand) -> Result<(), CliError> {
    match command {
        CausalModelCommand::Observational { input, out } => run_model("observational", input, out),
        CausalModelCommand::Perturbation { input, out } => run_model("perturbation", input, out),
        CausalModelCommand::ActiveDesign { input, out } => run_model("active_design", input, out),
        CausalModelCommand::ValidateActive {
            seed,
            timeout_seconds,
            out,
        } => run_validation(seed, timeout_seconds, out),
    }
}

fn backend(
    repository: &std::path::Path,
    worker_path: &std::path::Path,
) -> Result<Backend, CliError> {
    let lock = read_required(&repository.join("workers/python/uv.lock"))?;
    let worker = read_required(worker_path)?;
    Ok(Backend {
        name: "numpy_scipy_causal_active_design".into(),
        scipy_version: SCIPY_VERSION.into(),
        numpy_version: NUMPY_VERSION.into(),
        python_version: PYTHON_VERSION.into(),
        license: "BSD-3-Clause".into(),
        environment_lock_sha256: sha256_hex(&lock),
        worker_sha256: sha256_hex(&worker),
    })
}

fn run_model(mode: &str, input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(&input)?;
    let spec = match mode {
        "observational" => serde_json::from_slice(&bytes).map(ModelSpec::Observational),
        "perturbation" => serde_json::from_slice(&bytes).map(ModelSpec::Perturbation),
        _ => serde_json::from_slice(&bytes).map(ModelSpec::ActiveDesign),
    }
    .map_err(|error| {
        if error
            .to_string()
            .contains("missing field `timeout_seconds`")
        {
            CliError::Input("causal model input requires integer timeout_seconds".into())
        } else {
            schema_error(&format!("causal {mode} input"), error)
        }
    })?;
    let timeout = spec
        .timeout_seconds()
        .map_err(|message| CliError::Input(message.into()))?;
    let repository = marklab::backend::python_backend_assets_root()?;
    let worker_path = repository.join("workers/python/marklab_scipy_causal_active_worker.py");
    let request = ModelRequest {
        format: "marklab.scipy_causal_active_request",
        version: 1,
        mode: spec.mode(),
        backend: backend(&repository, &worker_path)?,
        spec,
        input_sha256: sha256_hex(&bytes),
    };
    publish_checked(
        &repository,
        &worker_path,
        &request,
        &request.backend,
        timeout,
        out,
        mode,
    )
}

fn run_validation(seed: u64, timeout_seconds: u64, out: PathBuf) -> Result<(), CliError> {
    if !is_worker_timeout(timeout_seconds) {
        return Err(CliError::Input(
            "causal validation timeout must be between 1 and 3600 seconds".into(),
        ));
    }
    let repository = marklab::backend::python_backend_assets_root()?;
    let worker_path = repository.join("workers/python/marklab_scipy_causal_active_worker.py");
    let request = ValidationRequest {
        format: "marklab.scipy_causal_active_request",
        version: 1,
        mode: "validation",
        backend: backend(&repository, &worker_path)?,
        seed,
    };
    publish_checked(
        &repository,
        &worker_path,
        &request,
        &request.backend,
        timeout_seconds,
        out,
        "validation",
    )
}

fn publish_checked(
    repository: &std::path::Path,
    worker_path: &std::path::Path,
    request: &impl Serialize,
    backend: &Backend,
    timeout: u64,
    out: PathBuf,
    mode: &str,
) -> Result<(), CliError> {
    // Retain the sorted-key encoding used by the existing request identities.
    let bytes = serde_json::to_vec(&serde_json::to_value(request)?)?;
    let response = run_worker(repository, worker_path, &bytes, timeout)?;
    let result = ModelResult::decode(mode, &response)
        .map_err(|error| schema_error(&format!("causal {mode} result"), error))?;
    result
        .validate_identity(backend, &sha256_hex(&bytes))
        .map_err(|message| CliError::Backend(message.into()))?;
    publish_json(&out, &serde_json::to_value(&result)?)
}

fn schema_error(context: &str, error: serde_json::Error) -> CliError {
    // Keep serde's field and type detail; it names what to fix in the input file.
    CliError::Json(<serde_json::Error as serde::de::Error>::custom(format!(
        "{context} schema mismatch: {error}"
    )))
}

pub(crate) fn execute(command: CausalModelCommand) -> marklab::Result<()> {
    run_cli(command).map_err(into_marklab_error)
}
