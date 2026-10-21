pub(crate) use marklab_bayes::protocol::arbitrary_window_ipp_sbc::{
    validate, SbcRequest, SbcResult,
};

use std::fs;
use std::path::PathBuf;

use marklab_bayes::sha256_hex;
use marklab_bayes::BackendContract;
use marklab_bayes::NutsSamplingSpec;

use super::arbitrary_window_ipp_fit;
use super::arbitrary_window_ipp_fit::PreparedArbitraryWindowIppFit;
use super::publish_json;
use super::run_worker;
use super::CliError;
use marklab_bayes::backend_versions::JAX_VERSION;
use marklab_bayes::backend_versions::NUMPYRO_VERSION;
use marklab_bayes::backend_versions::PYTHON_VERSION;

#[derive(Debug, clap::Args)]
pub(crate) struct SbcArguments {
    /// Observed point events with coordinates and model covariates.
    #[arg(long)]
    events: PathBuf,
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
    /// Number of simulated datasets used for posterior calibration.
    #[arg(long)]
    replicates: u32,
    /// Upper bound on events.
    #[arg(long)]
    maximum_events: usize,
    /// Upper bound on quadrature nodes.
    #[arg(long)]
    maximum_quadrature_nodes: usize,
    /// Upper bound on generated count.
    #[arg(long)]
    maximum_generated_count: u64,
    /// Upper bound on total work.
    #[arg(long)]
    maximum_total_work: u64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    out: PathBuf,
}

pub(super) fn run(arguments: SbcArguments) -> Result<(), CliError> {
    if !(20..=100).contains(&arguments.replicates)
        || arguments.maximum_generated_count == 0
        || arguments.maximum_generated_count > 10_000_000
        || arguments.maximum_total_work == 0
        || arguments.maximum_total_work > 100_000_000
    {
        return Err(CliError::Input(
            "weighted IPP SBC replicates, generated-count ceiling, or work ceiling is invalid"
                .into(),
        ));
    }
    let prepared =
        arbitrary_window_ipp_fit::prepare(crate::bayes::ArbitraryWindowIppFitParameters {
            events_path: arguments.events,
            quadrature_path: arguments.quadrature,
            window_path: arguments.window,
            intercept_prior_mean: arguments.intercept_prior_mean,
            intercept_prior_sd: arguments.intercept_prior_sd,
            coefficient_prior_mean: arguments.coefficient_prior_mean,
            coefficient_prior_sd: arguments.coefficient_prior_sd,
            sampling: NutsSamplingSpec {
                chains: arguments.chains,
                tune_per_chain: arguments.tune,
                draws_per_chain: arguments.draws,
                target_accept: arguments.target_accept,
                seed: arguments.seed,
            },
            maximum_events: arguments.maximum_events,
            maximum_quadrature_nodes: arguments.maximum_quadrature_nodes,
            maximum_draw_node_work: arguments.maximum_total_work,
            timeout_seconds: arguments.timeout_seconds,
        })?;
    let result = execute(
        &prepared,
        arguments.replicates,
        arguments.maximum_generated_count,
        arguments.maximum_total_work,
        arguments.timeout_seconds,
    )?;
    publish_json(&arguments.out, &result)
}

fn execute(
    prepared: &PreparedArbitraryWindowIppFit,
    replicates: u32,
    maximum_generated_count: u64,
    maximum_total_work: u64,
    timeout_seconds: u64,
) -> Result<SbcResult, CliError> {
    let repository = &marklab::backend::python_backend_assets_root()?;
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path =
        repository.join("workers/python/marklab_numpyro_arbitrary_window_ipp_sbc_worker.py");
    let lock = fs::read(&lock_path).map_err(|source| CliError::Io {
        path: lock_path,
        source,
    })?;
    let worker = fs::read(&worker_path).map_err(|source| CliError::Io {
        path: worker_path,
        source,
    })?;
    let request = SbcRequest {
        format: "marklab.numpyro_arbitrary_window_ipp_sbc_request",
        version: 1,
        backend: BackendContract {
            name: "numpyro",
            version: NUMPYRO_VERSION,
            python_version: PYTHON_VERSION,
            environment_lock_sha256: sha256_hex(&lock),
            worker_sha256: sha256_hex(&worker),
        },
        jax_version: JAX_VERSION,
        source_request_sha256: &prepared.request_sha256,
        source_request: &prepared.request,
        replicates,
        maximum_generated_count,
        maximum_total_work,
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let bytes = run_worker(
        repository,
        "marklab_numpyro_arbitrary_window_ipp_sbc_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: SbcResult = serde_json::from_slice(&bytes)?;
    validate(
        &result,
        &request,
        &request_sha256,
        prepared.input.spec.quadrature.len(),
    )
    .map_err(super::protocol_error)?;
    Ok(result)
}
