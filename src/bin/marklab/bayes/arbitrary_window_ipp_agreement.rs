pub(crate) use marklab_bayes::protocol::arbitrary_window_ipp_agreement::{
    compare, validate_numpyro, AgreementComparison, AgreementResult, NumpyroRequest, NumpyroResult,
};

use std::fs;
use std::path::PathBuf;

use marklab_bayes::sha256_hex;
use marklab_bayes::BackendContract;
use marklab_bayes::FitState;
use marklab_bayes::NutsSamplingSpec;

use super::arbitrary_window_ipp_fit;
use super::arbitrary_window_ipp_fit::PreparedArbitraryWindowIppFit;
use super::publish_json;
use super::run_worker;
use super::CliError;
use marklab_bayes::backend_versions::JAX_VERSION;
use marklab_bayes::backend_versions::NUMPYRO_VERSION;
use marklab_bayes::backend_versions::PYTHON_VERSION;
use marklab_core::backend::ClaimStatus;

#[derive(Debug, clap::Args)]
pub(crate) struct AgreementArguments {
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
    /// Upper bound on events.
    #[arg(long)]
    maximum_events: usize,
    /// Upper bound on quadrature nodes.
    #[arg(long)]
    maximum_quadrature_nodes: usize,
    /// Upper bound on draw node work.
    #[arg(long)]
    maximum_draw_node_work: u64,
    /// Absolute tolerance for comparing fitted parameter estimates.
    #[arg(long)]
    parameter_absolute_tolerance: f64,
    /// Absolute tolerance for differences in total event counts.
    #[arg(long)]
    total_count_absolute_tolerance: f64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    out: PathBuf,
}

pub(super) fn run(arguments: Box<AgreementArguments>) -> Result<(), CliError> {
    if !arguments.parameter_absolute_tolerance.is_finite()
        || arguments.parameter_absolute_tolerance <= 0.0
        || !arguments.total_count_absolute_tolerance.is_finite()
        || arguments.total_count_absolute_tolerance <= 0.0
    {
        return Err(CliError::Input(
            "arbitrary-window IPP agreement tolerances must be positive and finite".into(),
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
            maximum_draw_node_work: arguments.maximum_draw_node_work,
            timeout_seconds: arguments.timeout_seconds,
        })?;
    let pymc = arbitrary_window_ipp_fit::execute(&prepared)?;
    let numpyro = execute_numpyro(&prepared, arguments.timeout_seconds)?;
    let comparison = compare(
        &pymc,
        &numpyro,
        arguments.parameter_absolute_tolerance,
        arguments.total_count_absolute_tolerance,
    );
    let all_within_tolerance = pymc.fit_state == FitState::Complete
        && numpyro.fit_state == FitState::Complete
        && comparison.intercept.passes
        && comparison.coefficient.passes
        && comparison.total_expected_count.passes;
    let result = AgreementResult {
        format: "marklab.arbitrary_window_ipp_backend_agreement".into(),
        version: 1,
        input: prepared.input_identity.clone(),
        pymc,
        numpyro,
        comparison: AgreementComparison {
            all_within_tolerance,
            ..comparison
        },
        claim_status: ClaimStatus::ExperimentalCrossBackendAgreement,
    };
    publish_json(&arguments.out, &result)
}

fn execute_numpyro(
    prepared: &PreparedArbitraryWindowIppFit,
    timeout_seconds: u64,
) -> Result<NumpyroResult, CliError> {
    let repository = &marklab::backend::python_backend_assets_root()?;
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path =
        repository.join("workers/python/marklab_numpyro_arbitrary_window_ipp_worker.py");
    let lock = fs::read(&lock_path).map_err(|source| CliError::Io {
        path: lock_path,
        source,
    })?;
    let worker = fs::read(&worker_path).map_err(|source| CliError::Io {
        path: worker_path,
        source,
    })?;
    let request = NumpyroRequest {
        format: "marklab.numpyro_arbitrary_window_ipp_request",
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
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let bytes = run_worker(
        repository,
        "marklab_numpyro_arbitrary_window_ipp_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: NumpyroResult = serde_json::from_slice(&bytes)?;
    validate_numpyro(
        &result,
        &request,
        &request_sha256,
        &prepared.request,
        &prepared.request_sha256,
    )
    .map_err(super::protocol_error)?;
    Ok(result)
}
