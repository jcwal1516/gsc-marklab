use std::path::PathBuf;

use clap::Subcommand;
use marklab::{ObservationWindow2D, ObservationWindowLimits};
use marklab_bayes::advanced::{
    AdaptiveWindowSpdeResult, AdvancedRequest, AdvancedResult, AdvancedSpec, Backend, Mode,
    NonstationaryAdaptiveWindowSpdeResult,
};
use marklab_core::backend::sha256_hex;

use super::topology::{publish_json, read_input, read_required, run_worker};
use crate::cli_error::{into_marklab_error, CliError};
use marklab_core::backend::versions::NUMPY_VERSION;
use marklab_core::backend::versions::PYTHON_VERSION;
use marklab_core::backend::versions::SCIPY_VERSION;

#[derive(Clone)]
pub(crate) struct PreparedAdvancedBayes {
    pub input_bytes: Vec<u8>,
    pub lock_bytes: Vec<u8>,
    pub worker_bytes: Vec<u8>,
    pub request_bytes: Vec<u8>,
    repository: PathBuf,
    worker_path: PathBuf,
    request: AdvancedRequest,
    timeout_seconds: u64,
}

#[derive(Debug, Subcommand)]
pub(crate) enum AdvancedBayesCommand {
    /// Sample a normal-mean posterior with fixed-step Hamiltonian Monte Carlo.
    HmcNormal {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Fit a Bayesian Thomas cluster process (birth-death MCMC, pooled across patients).
    AdvancedCluster {
        /// JSON specification containing patterns, priors, sampler and resource limits.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Run simulation-based calibration of the Bayesian Thomas cluster sampler.
    AdvancedClusterSbc {
        /// JSON specification containing the window, priors, sampler and calibration policy.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the calibration output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Fit rectangular-mesh SPDE intensity and spatial-factor models.
    SpdeSuite {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Fit a spatial factor on an adaptive mesh inside a polygonal window.
    AdaptiveWindowSpde {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Fit an adaptive SPDE with declared local scale and anisotropy.
    NonstationaryAdaptiveWindowSpde {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
}

pub(crate) fn run_cli(command: AdvancedBayesCommand) -> Result<(), CliError> {
    match command {
        AdvancedBayesCommand::HmcNormal { input, out } => run("hmc_normal", input, out),
        AdvancedBayesCommand::AdvancedCluster { input, out } => {
            crate::bayes_thomas_cluster::run_fit(&input, &out)
        }
        AdvancedBayesCommand::AdvancedClusterSbc { input, out } => {
            crate::bayes_thomas_cluster::run_sbc(&input, &out)
        }
        AdvancedBayesCommand::SpdeSuite { input, out } => run("spde_suite", input, out),
        AdvancedBayesCommand::AdaptiveWindowSpde { input, out } => {
            run("adaptive_window_spde", input, out)
        }
        AdvancedBayesCommand::NonstationaryAdaptiveWindowSpde { input, out } => {
            run("nonstationary_adaptive_window_spde", input, out)
        }
    }
}

fn run(mode: &str, input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let prepared = prepare(mode, &input)?;
    let result = execute(&prepared)?;
    publish_json(&out, &serde_json::to_value(&result)?)
}

impl PreparedAdvancedBayes {
    /// The worker environment lock the request was bound to.
    pub(crate) fn lock_path(&self) -> PathBuf {
        self.repository.join("workers/python/uv.lock")
    }

    /// The worker script the request was bound to.
    pub(crate) fn worker_path(&self) -> &std::path::Path {
        &self.worker_path
    }
}

pub(crate) fn prepare(
    mode: &str,
    input: &std::path::Path,
) -> Result<PreparedAdvancedBayes, CliError> {
    let mode = Mode::parse(mode)
        .ok_or_else(|| CliError::Input(format!("unknown advanced Bayesian mode {mode}")))?;
    let input_bytes = read_input(input)?;
    let spec = AdvancedSpec::decode(mode, &input_bytes).map_err(|error| {
        if error
            .to_string()
            .contains("missing field `timeout_seconds`")
        {
            CliError::Input("advanced Bayesian input requires integer timeout_seconds".into())
        } else {
            schema_error("advanced Bayesian input", error)
        }
    })?;
    let timeout = spec
        .timeout_seconds()
        .map_err(|message| CliError::Input(message.into()))?;
    let repository = marklab::backend::python_backend_assets_root()?;
    let worker_path = repository.join("workers/python/marklab_scipy_advanced_bayes_worker.py");
    let lock = read_required(&repository.join("workers/python/uv.lock"))?;
    let worker = read_required(&worker_path)?;
    let adaptive_window = if let Some(geometry) = spec.window() {
        let geometry_bytes = serde_json::to_vec(&serde_json::to_value(geometry)?)?;
        let geometry_text = std::str::from_utf8(&geometry_bytes).map_err(|_| {
            CliError::Input("adaptive SPDE window serialization is not UTF-8".into())
        })?;
        Some(
            ObservationWindow2D::from_geojson_str(
                geometry_text,
                ObservationWindowLimits::default(),
            )
            .map_err(|error| CliError::Input(error.to_string()))?,
        )
    } else {
        None
    };
    let request = AdvancedRequest {
        format: "marklab.scipy_advanced_bayes_request",
        version: 1,
        mode,
        backend: Backend {
            name: "numpy_scipy_advanced_bayes".into(),
            scipy_version: SCIPY_VERSION.into(),
            numpy_version: NUMPY_VERSION.into(),
            python_version: PYTHON_VERSION.into(),
            license: "BSD-3-Clause".into(),
            environment_lock_sha256: sha256_hex(&lock),
            worker_sha256: sha256_hex(&worker),
        },
        spec,
        input_sha256: sha256_hex(&input_bytes),
        canonical_window_sha256: adaptive_window
            .as_ref()
            .map(|window| window.descriptor().logical_digest.to_string()),
        canonical_window_area: adaptive_window.as_ref().map(ObservationWindow2D::area_um2),
    };
    // Preserve the sorted keys and raw numeric forms bound into existing request hashes.
    let request_bytes = serde_json::to_vec(&serde_json::to_value(&request)?)?;
    Ok(PreparedAdvancedBayes {
        input_bytes,
        lock_bytes: lock,
        worker_bytes: worker,
        request_bytes,
        repository,
        worker_path,
        request,
        timeout_seconds: timeout,
    })
}

pub(crate) fn execute(prepared: &PreparedAdvancedBayes) -> Result<AdvancedResult, CliError> {
    let response = run_worker(
        &prepared.repository,
        &prepared.worker_path,
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result = AdvancedResult::decode(prepared.request.mode, &response)
        .map_err(|error| schema_error("advanced Bayesian result", error))?;
    result
        .validate(&prepared.request, &sha256_hex(&prepared.request_bytes))
        .map_err(|message| CliError::Backend(message.into()))?;
    Ok(result)
}

pub(crate) fn validate_adaptive_result(
    prepared: &PreparedAdvancedBayes,
    result: &AdaptiveWindowSpdeResult,
) -> Result<(), CliError> {
    prepared
        .request
        .validate_adaptive(result, &sha256_hex(&prepared.request_bytes))
        .map_err(|message| CliError::Backend(message.into()))
}

pub(crate) fn validate_nonstationary_result(
    prepared: &PreparedAdvancedBayes,
    result: &NonstationaryAdaptiveWindowSpdeResult,
) -> Result<(), CliError> {
    prepared
        .request
        .validate_nonstationary(result, &sha256_hex(&prepared.request_bytes))
        .map_err(|message| CliError::Backend(message.into()))
}

fn schema_error(context: &str, error: serde_json::Error) -> CliError {
    // Keep serde's field and type detail; it names what to fix in the input file.
    CliError::Json(<serde_json::Error as serde::de::Error>::custom(format!(
        "{context} schema mismatch: {error}"
    )))
}

pub(crate) fn execute_command(command: AdvancedBayesCommand) -> marklab::Result<()> {
    run_cli(command).map_err(into_marklab_error)
}
