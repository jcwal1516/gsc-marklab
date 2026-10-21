pub(crate) use marklab_bayes::protocol::arbitrary_window_lgcp_sbc::{
    validate_outer, SbcResult, WorkerRequest, WorkerResult,
};

use crate::input_file::read_file;
use std::path::PathBuf;

use marklab_bayes::sha256_hex;
use marklab_bayes::BackendContract;
use marklab_bayes::NumpyroGriddedLgcpSbcWorkerRequest;
use marklab_bayes::NutsSamplingSpec;

use super::arbitrary_window_lgcp_fit;
use super::publish_json;
use super::run_worker;
use super::CliError;
use marklab_bayes::backend_versions::JAX_VERSION;
use marklab_bayes::backend_versions::NUMPYRO_VERSION;
use marklab_bayes::backend_versions::PYTHON_VERSION;
use marklab_core::vocabulary::StatisticalUnit;

#[derive(Debug, clap::Args)]
pub(crate) struct Arguments {
    /// Observed point events with coordinates and model covariates.
    #[arg(long)]
    events: PathBuf,
    /// Table binding observed events to their declared components or quadrature cells.
    #[arg(long)]
    event_membership: PathBuf,
    /// Quadrature nodes, weights and covariates for spatial integration.
    #[arg(long)]
    quadrature: PathBuf,
    /// Observation-window geometry used for spatial integration or edge correction.
    #[arg(long)]
    window: PathBuf,
    /// Prior mean for intercept.
    #[arg(long, allow_hyphen_values = true)]
    intercept_prior_mean: f64,
    /// Prior scale for intercept.
    #[arg(long)]
    intercept_prior_sd: f64,
    /// Prior mean for coefficient.
    #[arg(long, allow_hyphen_values = true)]
    coefficient_prior_mean: f64,
    /// Prior scale for coefficient.
    #[arg(long)]
    coefficient_prior_sd: f64,
    /// Amplitude of the latent spatial field covariance.
    #[arg(long)]
    field_amplitude: f64,
    /// Spatial covariance length scale in micrometers.
    #[arg(long)]
    field_length_scale_um: f64,
    /// Diagonal stabilization added to covariance matrices.
    #[arg(long)]
    jitter: f64,
    /// Number of simulated datasets used for posterior calibration.
    #[arg(long)]
    replicates: u32,
    /// Number of independent posterior sampling chains.
    #[arg(long)]
    chains: u32,
    /// Warmup iterations per posterior sampling chain.
    #[arg(long)]
    tune: u32,
    /// Posterior draws retained per chain after warmup.
    #[arg(long)]
    draws: u32,
    /// Target NUTS acceptance probability.
    #[arg(long)]
    target_accept: f64,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    seed: u64,
    /// Upper bound on events.
    #[arg(long)]
    maximum_events: usize,
    /// Upper bound on quadrature nodes.
    #[arg(long)]
    maximum_quadrature_nodes: usize,
    /// Upper bound on draw node work.
    #[arg(long)]
    maximum_draw_node_work: u64,
    /// Number of posterior predictive simulations.
    #[arg(long)]
    prediction_replicates: u32,
    /// Random seed for posterior predictive simulation.
    #[arg(long)]
    prediction_seed: u64,
    /// Upper bound on predictive points.
    #[arg(long)]
    maximum_predictive_points: u64,
    /// Neighborhood radius in micrometers.
    #[arg(long)]
    neighbor_radius_um: f64,
    /// Upper bound on neighbor pairs.
    #[arg(long)]
    maximum_neighbor_pairs: usize,
    /// Smallest admitted p-value for simulation-based rank uniformity.
    #[arg(long)]
    minimum_rank_uniformity_p_value: f64,
    /// Smallest admitted empirical coverage of nominal 90% intervals.
    #[arg(long)]
    minimum_coverage_90: f64,
    /// Largest admitted empirical coverage of nominal 90% intervals.
    #[arg(long)]
    maximum_coverage_90: f64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    out: PathBuf,
}

pub(super) fn run(arguments: Box<Arguments>) -> Result<(), CliError> {
    let sampling = NutsSamplingSpec {
        chains: arguments.chains,
        tune_per_chain: arguments.tune,
        draws_per_chain: arguments.draws,
        target_accept: arguments.target_accept,
        seed: arguments.seed,
    };
    let prepared = arbitrary_window_lgcp_fit::prepare(crate::bayes::ArbitraryWindowLgcpFitArgs {
        events: arguments.events,
        event_membership: arguments.event_membership,
        quadrature: arguments.quadrature,
        window: arguments.window,
        intercept_prior_mean: arguments.intercept_prior_mean,
        intercept_prior_sd: arguments.intercept_prior_sd,
        coefficient_prior_mean: arguments.coefficient_prior_mean,
        coefficient_prior_sd: arguments.coefficient_prior_sd,
        field_amplitude: arguments.field_amplitude,
        field_length_scale_um: arguments.field_length_scale_um,
        jitter: arguments.jitter,
        chains: sampling.chains,
        tune: sampling.tune_per_chain,
        draws: sampling.draws_per_chain,
        target_accept: sampling.target_accept,
        seed: sampling.seed,
        maximum_events: arguments.maximum_events,
        maximum_quadrature_nodes: arguments.maximum_quadrature_nodes,
        maximum_draw_node_work: arguments.maximum_draw_node_work,
        prediction_replicates: arguments.prediction_replicates,
        prediction_seed: arguments.prediction_seed,
        maximum_predictive_points: arguments.maximum_predictive_points,
        neighbor_radius_um: arguments.neighbor_radius_um,
        maximum_neighbor_pairs: arguments.maximum_neighbor_pairs,
        timeout_seconds: arguments.timeout_seconds,
        out: std::path::PathBuf::new(),
    })?;
    let repository = &marklab::backend::python_backend_assets_root()?;
    let directory = repository.join("workers/python");
    let lock = read_file(&directory.join("uv.lock"))?;
    let worker = read_file(&directory.join("marklab_numpyro_arbitrary_window_lgcp_sbc_worker.py"))?;
    let source_worker = read_file(&directory.join("marklab_numpyro_gridded_lgcp_sbc_worker.py"))?;
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
            python_version: PYTHON_VERSION,
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
        &source_sbc_request_sha256,
    )
    .map_err(super::protocol_error)?;
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
            statistical_unit: StatisticalUnit::PriorGenerativePointPattern,
            claim_status: source.claim_status,
        },
    )
}
