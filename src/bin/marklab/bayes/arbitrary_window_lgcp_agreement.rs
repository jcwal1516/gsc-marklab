pub(crate) use marklab_bayes::protocol::arbitrary_window_lgcp_agreement::{
    compare, validate_numpyro, AgreementResult, NumpyroRequest, NumpyroResult,
};

use crate::input_file::read_file;
use std::path::PathBuf;

use marklab_bayes::sha256_hex;
use marklab_bayes::BackendContract;
use marklab_bayes::FitState;
use marklab_core::backend::ClaimStatus;

use super::arbitrary_window_lgcp_fit;
use super::arbitrary_window_lgcp_fit::FitResult;
use super::arbitrary_window_lgcp_fit::PreparedArbitraryWindowLgcpFit;
use super::publish_json;
use super::run_worker;
use super::CliError;
use marklab_bayes::backend_versions::JAX_VERSION;
use marklab_bayes::backend_versions::NUMPYRO_VERSION;
use marklab_bayes::backend_versions::PYTHON_VERSION;
use marklab_bayes::AgreementStatus;
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
    /// Largest standardized difference accepted between backend estimates.
    #[arg(long)]
    maximum_standardized_difference: f64,
    /// Minimum absolute comparison tolerance for parameter.
    #[arg(long)]
    minimum_parameter_tolerance: f64,
    /// Minimum absolute comparison tolerance for field.
    #[arg(long)]
    minimum_field_tolerance: f64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    out: PathBuf,
}

pub(super) fn run(arguments: Box<Arguments>) -> Result<(), CliError> {
    validate_controls(&arguments)?;
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
        chains: arguments.chains,
        tune: arguments.tune,
        draws: arguments.draws,
        target_accept: arguments.target_accept,
        seed: arguments.seed,
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
    let pymc = arbitrary_window_lgcp_fit::execute(&prepared)?;
    let numpyro = execute_numpyro(&prepared, &pymc, arguments.timeout_seconds)?;
    let comparison = compare(
        &pymc,
        &numpyro,
        arguments.maximum_standardized_difference,
        arguments.minimum_parameter_tolerance,
        arguments.minimum_field_tolerance,
    );
    let fits_complete =
        pymc.fit_state == FitState::Complete && numpyro.fit_state == FitState::Complete;
    let agrees = comparison.intercept.passes
        && comparison.coefficient.passes
        && comparison.latent_effect.passes
        && comparison.expected_count.passes;
    let available = fits_complete && agrees;
    let result = AgreementResult {
        format: "marklab.arbitrary_window_lgcp_backend_agreement".into(),
        version: 1,
        input: pymc.input.clone(),
        covariance_sha256: prepared.request.covariance_sha256.clone(),
        fit_state: if fits_complete {
            FitState::Complete
        } else {
            FitState::Nonconverged
        },
        agreement_status: if available {
            AgreementStatus::AgreeWithinMonteCarloError
        } else if fits_complete {
            AgreementStatus::DiagnosticOnlyBackendDisagreement
        } else {
            AgreementStatus::DiagnosticOnlyNonconverged
        },
        pymc,
        numpyro,
        comparison,
        statistical_unit: StatisticalUnit::OneObservedPointPattern,
        claim_status: if available {
            ClaimStatus::ExperimentalCrossBackendValidation
        } else {
            ClaimStatus::DiagnosticOnlyCrossBackendValidation
        },
    };
    publish_json(&arguments.out, &result)
}

fn validate_controls(arguments: &Arguments) -> Result<(), CliError> {
    if !arguments.maximum_standardized_difference.is_finite()
        || !(1.0..=10.0).contains(&arguments.maximum_standardized_difference)
        || !arguments.minimum_parameter_tolerance.is_finite()
        || arguments.minimum_parameter_tolerance <= 0.0
        || !arguments.minimum_field_tolerance.is_finite()
        || arguments.minimum_field_tolerance <= 0.0
    {
        return Err(CliError::Input(
            "arbitrary-window LGCP agreement controls are invalid".into(),
        ));
    }
    Ok(())
}

fn execute_numpyro(
    prepared: &PreparedArbitraryWindowLgcpFit,
    pymc: &FitResult,
    timeout_seconds: u64,
) -> Result<NumpyroResult, CliError> {
    let repository = &marklab::backend::python_backend_assets_root()?;
    let directory = repository.join("workers/python");
    let lock = read_file(&directory.join("uv.lock"))?;
    let worker = read_file(&directory.join("marklab_numpyro_arbitrary_window_lgcp_worker.py"))?;
    let source_worker = read_file(&directory.join("marklab_numpyro_gridded_lgcp_worker.py"))?;
    let request = NumpyroRequest {
        format: "marklab.numpyro_arbitrary_window_lgcp_request",
        version: 1,
        backend: BackendContract {
            name: "numpyro",
            version: NUMPYRO_VERSION,
            python_version: PYTHON_VERSION,
            environment_lock_sha256: sha256_hex(&lock),
            worker_sha256: sha256_hex(&worker),
        },
        source_backend: BackendContract {
            name: "numpyro",
            version: NUMPYRO_VERSION,
            python_version: PYTHON_VERSION,
            environment_lock_sha256: sha256_hex(&lock),
            worker_sha256: sha256_hex(&source_worker),
        },
        jax_version: JAX_VERSION,
        source_request_sha256: &prepared.request_sha256,
        source_request: &prepared.request,
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let bytes = run_worker(
        repository,
        "marklab_numpyro_arbitrary_window_lgcp_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: NumpyroResult = serde_json::from_slice(&bytes)?;
    validate_numpyro(&result, &request, &request_sha256, pymc).map_err(super::protocol_error)?;
    Ok(result)
}
