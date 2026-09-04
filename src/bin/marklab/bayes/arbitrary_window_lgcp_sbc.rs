use std::{fs, path::PathBuf};

use clap::{Parser, Subcommand};
use marklab_bayes::{
    sha256_hex, BackendContract, FitState, GriddedLgcpSbcCalibrationPolicy,
    GriddedLgcpSbcDiagnostics, GriddedLgcpSbcFailure, GriddedLgcpSbcReplicate,
    GriddedLgcpSbcResourceLimits, NumpyroGriddedLgcpSbcWorkerRequest,
    NumpyroGriddedLgcpSbcWorkerResult, NutsSamplingSpec, WorkerBackend,
};
use serde::{Deserialize, Serialize};

use super::{
    arbitrary_window_lgcp_fit::{self, InputIdentity},
    publish_json, run_worker, BayesCliError,
};

const NUMPYRO_VERSION: &str = "0.21.0";
const JAX_VERSION: &str = "0.11.1";

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct SbcCli {
    #[command(subcommand)]
    command: TopLevel,
}

#[derive(Debug, Subcommand)]
enum TopLevel {
    Bayes {
        #[command(subcommand)]
        command: Command,
    },
}

#[derive(Debug, Subcommand)]
enum Command {
    ArbitraryWindowLgcpSbc(Box<Arguments>),
}

#[derive(Debug, clap::Args)]
struct Arguments {
    #[arg(long)]
    events: PathBuf,
    #[arg(long)]
    event_membership: PathBuf,
    #[arg(long)]
    quadrature: PathBuf,
    #[arg(long)]
    window: PathBuf,
    #[arg(long, allow_hyphen_values = true)]
    intercept_prior_mean: f64,
    #[arg(long)]
    intercept_prior_sd: f64,
    #[arg(long, allow_hyphen_values = true)]
    coefficient_prior_mean: f64,
    #[arg(long)]
    coefficient_prior_sd: f64,
    #[arg(long)]
    field_amplitude: f64,
    #[arg(long)]
    field_length_scale_um: f64,
    #[arg(long)]
    jitter: f64,
    #[arg(long)]
    replicates: u32,
    #[arg(long)]
    chains: u32,
    #[arg(long)]
    tune: u32,
    #[arg(long)]
    draws: u32,
    #[arg(long)]
    target_accept: f64,
    #[arg(long)]
    seed: u64,
    #[arg(long)]
    maximum_events: usize,
    #[arg(long)]
    maximum_quadrature_nodes: usize,
    #[arg(long)]
    maximum_draw_node_work: u64,
    #[arg(long)]
    prediction_replicates: u32,
    #[arg(long)]
    prediction_seed: u64,
    #[arg(long)]
    maximum_predictive_points: u64,
    #[arg(long)]
    neighbor_radius_um: f64,
    #[arg(long)]
    maximum_neighbor_pairs: usize,
    #[arg(long)]
    minimum_rank_uniformity_p_value: f64,
    #[arg(long)]
    minimum_coverage_90: f64,
    #[arg(long)]
    maximum_coverage_90: f64,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Serialize)]
struct WorkerRequest<'a> {
    format: &'static str,
    version: u32,
    backend: BackendContract,
    source_backend: BackendContract,
    jax_version: &'static str,
    source_request_sha256: &'a str,
    source_request: &'a arbitrary_window_lgcp_fit::WorkerRequest,
    source_sbc_request_sha256: &'a str,
    source_sbc_request: &'a NumpyroGriddedLgcpSbcWorkerRequest,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerResult {
    format: String,
    version: u32,
    backend: WorkerBackend,
    jax_version: String,
    input: InputIdentity,
    covariance_sha256: String,
    physical_latent_node_id: String,
    request_sha256: String,
    source_request_sha256: String,
    source_sbc_request_sha256: String,
    source_result: NumpyroGriddedLgcpSbcWorkerResult,
}

#[derive(Debug, Serialize)]
struct SbcResult {
    format: String,
    version: u32,
    backend: WorkerBackend,
    source_backend: WorkerBackend,
    jax_version: String,
    input: InputIdentity,
    covariance_sha256: String,
    physical_latent_node_id: String,
    sampling: NutsSamplingSpec,
    calibration: GriddedLgcpSbcCalibrationPolicy,
    resources: GriddedLgcpSbcResourceLimits,
    fit_state: FitState,
    replicates: Vec<GriddedLgcpSbcReplicate>,
    failures: Vec<GriddedLgcpSbcFailure>,
    diagnostics: GriddedLgcpSbcDiagnostics,
    request_sha256: String,
    source_request_sha256: String,
    source_sbc_request_sha256: String,
    statistical_unit: String,
    claim_status: String,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let TopLevel::Bayes { command } = SbcCli::parse_from(std::env::args_os()).command;
    let Command::ArbitraryWindowLgcpSbc(arguments) = command;
    let sampling = NutsSamplingSpec {
        chains: arguments.chains,
        tune_per_chain: arguments.tune,
        draws_per_chain: arguments.draws,
        target_accept: arguments.target_accept,
        seed: arguments.seed,
    };
    let prepared = arbitrary_window_lgcp_fit::prepare(
        arguments.events,
        arguments.event_membership,
        arguments.quadrature,
        arguments.window,
        arguments.intercept_prior_mean,
        arguments.intercept_prior_sd,
        arguments.coefficient_prior_mean,
        arguments.coefficient_prior_sd,
        arguments.field_amplitude,
        arguments.field_length_scale_um,
        arguments.jitter,
        sampling,
        arguments.maximum_events,
        arguments.maximum_quadrature_nodes,
        arguments.maximum_draw_node_work,
        arguments.prediction_replicates,
        arguments.prediction_seed,
        arguments.maximum_predictive_points,
        arguments.neighbor_radius_um,
        arguments.maximum_neighbor_pairs,
        arguments.timeout_seconds,
    )?;
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let directory = repository.join("workers/python");
    let lock = read(&directory.join("uv.lock"))?;
    let worker = read(&directory.join("marklab_numpyro_arbitrary_window_lgcp_sbc_worker.py"))?;
    let source_worker = read(&directory.join("marklab_numpyro_gridded_lgcp_sbc_worker.py"))?;
    let source_sbc_request = NumpyroGriddedLgcpSbcWorkerRequest::new(
        prepared.request.source_request.clone(),
        arguments.replicates,
        arguments.minimum_rank_uniformity_p_value,
        arguments.minimum_coverage_90,
        arguments.maximum_coverage_90,
        sha256_hex(&lock),
        sha256_hex(&source_worker),
        arguments.timeout_seconds,
    )?;
    let source_sbc_request_bytes = serde_json::to_vec(&source_sbc_request)?;
    let source_sbc_request_sha256 = sha256_hex(&source_sbc_request_bytes);
    let request = WorkerRequest {
        format: "marklab.numpyro_arbitrary_window_lgcp_sbc_request",
        version: 1,
        backend: BackendContract {
            name: "numpyro",
            version: NUMPYRO_VERSION,
            python_version: "3.12",
            environment_lock_sha256: sha256_hex(&lock),
            worker_sha256: sha256_hex(&worker),
        },
        source_backend: source_sbc_request.backend.clone(),
        jax_version: JAX_VERSION,
        source_request_sha256: &prepared.request_sha256,
        source_request: &prepared.request,
        source_sbc_request_sha256: &source_sbc_request_sha256,
        source_sbc_request: &source_sbc_request,
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let bytes = run_worker(
        repository,
        "marklab_numpyro_arbitrary_window_lgcp_sbc_worker.py",
        &request_bytes,
        arguments.timeout_seconds,
    )?;
    let worker_result: WorkerResult = serde_json::from_slice(&bytes)?;
    validate_outer(
        &worker_result,
        &request,
        &request_sha256,
        &prepared,
        &source_sbc_request_sha256,
    )?;
    worker_result
        .source_result
        .validate(&source_sbc_request, &source_sbc_request_sha256)?;
    let source = worker_result.source_result.into_result(source_sbc_request);
    publish_json(
        &arguments.out,
        &SbcResult {
            format: "marklab.arbitrary_window_lgcp_sbc".into(),
            version: 1,
            backend: worker_result.backend,
            source_backend: source.backend,
            jax_version: worker_result.jax_version,
            input: worker_result.input,
            covariance_sha256: worker_result.covariance_sha256,
            physical_latent_node_id: worker_result.physical_latent_node_id,
            sampling: source.sampling,
            calibration: source.calibration,
            resources: source.resources,
            fit_state: source.fit_state,
            replicates: source.replicates,
            failures: source.failures,
            diagnostics: source.diagnostics,
            request_sha256: worker_result.request_sha256,
            source_request_sha256: worker_result.source_request_sha256,
            source_sbc_request_sha256: worker_result.source_sbc_request_sha256,
            statistical_unit: "prior_generative_point_pattern".into(),
            claim_status: source.claim_status.into(),
        },
    )
}

fn validate_outer(
    result: &WorkerResult,
    request: &WorkerRequest<'_>,
    request_sha256: &str,
    prepared: &arbitrary_window_lgcp_fit::PreparedArbitraryWindowLgcpFit,
    source_sbc_request_sha256: &str,
) -> Result<(), BayesCliError> {
    let latent_index = request.source_sbc_request.calibration.latent_cell_index as usize;
    if result.format != "marklab.numpyro_arbitrary_window_lgcp_sbc_result"
        || result.version != 1
        || !backend_matches(&result.backend, &request.backend)
        || result.jax_version != JAX_VERSION
        || result.input != prepared.request.input
        || result.covariance_sha256 != prepared.request.covariance_sha256
        || result.physical_latent_node_id != prepared.request.nodes[latent_index].node_id
        || result.request_sha256 != request_sha256
        || result.source_request_sha256 != prepared.request_sha256
        || result.source_sbc_request_sha256 != source_sbc_request_sha256
    {
        return Err(BayesCliError::Backend(
            "NumPyro arbitrary-window LGCP SBC outer identity differs".into(),
        ));
    }
    Ok(())
}

fn backend_matches(result: &WorkerBackend, request: &BackendContract) -> bool {
    result.name == request.name
        && result.version == request.version
        && result.python_version == request.python_version
        && result.environment_lock_sha256 == request.environment_lock_sha256
        && result.worker_sha256 == request.worker_sha256
}

fn read(path: &std::path::Path) -> Result<Vec<u8>, BayesCliError> {
    fs::read(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })
}

pub(super) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<SbcCli>(|| run_cli().map_err(super::into_marklab_error))
}
