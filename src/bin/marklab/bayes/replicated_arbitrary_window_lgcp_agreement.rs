pub(crate) use marklab_bayes::protocol::replicated_arbitrary_window_lgcp_agreement::{
    compare, AgreementResult, NumpyroRequest, NumpyroResult,
};

use crate::input_file::read_file;
use std::path::PathBuf;

use marklab_bayes::json_sha256;
use marklab_bayes::sha256_hex;
use marklab_bayes::BackendContract;
use marklab_bayes::FitState;
use marklab_bayes::NutsSamplingSpec;

use super::publish_json;
use super::replicated_arbitrary_window_lgcp_fit;
use super::replicated_arbitrary_window_lgcp_fit::ResultDocument;
use super::replicated_arbitrary_window_lgcp_fit::WorkerRequest;
use super::run_worker;
use super::CliError;
use marklab_bayes::backend_versions::JAX_VERSION;
use marklab_bayes::backend_versions::NUMPYRO_VERSION;
use marklab_bayes::backend_versions::PYTHON_VERSION;
use marklab_bayes::AgreementStatus;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;

#[derive(Debug, clap::Args)]
pub(crate) struct Arguments {
    /// Per-node quadrature CSV (pattern_id, patient_id, group, node_id, x_um, y_um, count, ...).
    #[arg(long)]
    input: PathBuf,
    /// Group level used as the regression reference.
    #[arg(long)]
    reference_group: String,
    /// Group level whose effect is compared with the reference group.
    #[arg(long)]
    comparison_group: String,
    /// Prior mean for intercept.
    #[arg(long, allow_hyphen_values = true)]
    intercept_prior_mean: f64,
    /// Prior scale for intercept.
    #[arg(long)]
    intercept_prior_sd: f64,
    /// Prior scale for group effect.
    #[arg(long)]
    group_effect_prior_sd: f64,
    /// Prior scale for covariate effect.
    #[arg(long)]
    covariate_effect_prior_sd: f64,
    /// Prior scale for patient standard deviation.
    #[arg(long)]
    patient_sd_prior_scale: f64,
    /// Prior scale for pattern standard deviation.
    #[arg(long)]
    pattern_sd_prior_scale: f64,
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
    /// Upper bound on patients.
    #[arg(long)]
    maximum_patients: usize,
    /// Upper bound on patterns.
    #[arg(long)]
    maximum_patterns: usize,
    /// Upper bound on nodes per pattern.
    #[arg(long)]
    maximum_nodes_per_pattern: usize,
    /// Upper bound on total nodes.
    #[arg(long)]
    maximum_total_nodes: usize,
    /// Upper bound on total events.
    #[arg(long)]
    maximum_total_events: u64,
    /// Upper bound on draw node work.
    #[arg(long)]
    maximum_draw_node_work: u64,
    /// Largest standardized difference accepted between backend estimates.
    #[arg(long)]
    maximum_standardized_difference: f64,
    /// Minimum absolute comparison tolerance for parameter.
    #[arg(long)]
    minimum_parameter_tolerance: f64,
    /// Minimum absolute comparison tolerance for field.
    #[arg(long)]
    minimum_field_tolerance: f64,
    /// Maximum NUTS tree depth for the NumPyro fit.
    #[arg(long)]
    numpyro_maximum_tree_depth: u32,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    out: PathBuf,
}

pub(super) fn run(arguments: Arguments) -> Result<(), CliError> {
    for value in [
        arguments.maximum_standardized_difference,
        arguments.minimum_parameter_tolerance,
        arguments.minimum_field_tolerance,
    ] {
        if !value.is_finite() || value <= 0.0 {
            return Err(CliError::Input(
                "replicated LGCP agreement controls are invalid".into(),
            ));
        }
    }
    if !(10..=14).contains(&arguments.numpyro_maximum_tree_depth) {
        return Err(CliError::Input(
            "NumPyro replicated LGCP tree-depth bound is invalid".into(),
        ));
    }
    let sampling = NutsSamplingSpec {
        chains: arguments.chains,
        tune_per_chain: arguments.tune,
        draws_per_chain: arguments.draws,
        target_accept: arguments.target_accept,
        seed: arguments.seed,
    };
    let prepared = replicated_arbitrary_window_lgcp_fit::prepare(
        crate::bayes::ReplicatedArbitraryWindowLgcpFitArgs {
            input: arguments.input,
            reference_group: arguments.reference_group,
            comparison_group: arguments.comparison_group,
            intercept_prior_mean: arguments.intercept_prior_mean,
            intercept_prior_sd: arguments.intercept_prior_sd,
            group_effect_prior_sd: arguments.group_effect_prior_sd,
            covariate_effect_prior_sd: arguments.covariate_effect_prior_sd,
            patient_sd_prior_scale: arguments.patient_sd_prior_scale,
            pattern_sd_prior_scale: arguments.pattern_sd_prior_scale,
            field_amplitude: arguments.field_amplitude,
            field_length_scale_um: arguments.field_length_scale_um,
            jitter: arguments.jitter,
            chains: sampling.chains,
            tune: sampling.tune_per_chain,
            draws: sampling.draws_per_chain,
            target_accept: sampling.target_accept,
            seed: sampling.seed,
            maximum_patients: arguments.maximum_patients,
            maximum_patterns: arguments.maximum_patterns,
            maximum_nodes_per_pattern: arguments.maximum_nodes_per_pattern,
            maximum_total_nodes: arguments.maximum_total_nodes,
            maximum_total_events: arguments.maximum_total_events,
            maximum_draw_node_work: arguments.maximum_draw_node_work,
            timeout_seconds: arguments.timeout_seconds,
            out: std::path::PathBuf::new(),
        },
    )?;
    let pymc = replicated_arbitrary_window_lgcp_fit::execute(&prepared)?;
    let numpyro = execute_numpyro(
        &prepared.request,
        &prepared.request_sha256,
        arguments.numpyro_maximum_tree_depth,
        arguments.timeout_seconds,
    )?;
    validate_numpyro(
        &numpyro,
        &prepared.request,
        &prepared.request_sha256,
        arguments.numpyro_maximum_tree_depth,
        &pymc,
    )?;
    let comparison = compare(
        &pymc,
        &numpyro,
        arguments.maximum_standardized_difference,
        arguments.minimum_parameter_tolerance,
        arguments.minimum_field_tolerance,
    );
    let complete = pymc.fit_state == FitState::Complete
        && numpyro.fit_state == FitState::Complete
        && comparison.all_pass;
    publish_json(
        &arguments.out,
        &AgreementResult {
            format: "marklab.replicated_arbitrary_window_lgcp_backend_agreement".into(),
            version: 1,
            pymc,
            numpyro,
            comparison,
            fit_state: if complete {
                FitState::Complete
            } else {
                FitState::Nonconverged
            },
            agreement_status: if complete {
                AgreementStatus::AgreeWithinMonteCarloError
            } else {
                AgreementStatus::DiagnosticOnlyDisagreementOrNonconvergence
            },
            statistical_unit: StatisticalUnit::Patient,
            assumptions: vec![
                "same_exact_patients_patterns_windows_nodes_priors_draws_target_accept_and_seed"
                    .into(),
                "numpyro_tree_depth_capacity_is_explicit_and_sampler_specific".into(),
                "independent_pymc_and_numpyro_nuts_implementations".into(),
                "patients_not_patterns_nodes_or_cells_are_population_replicates".into(),
            ],
            finite_result_policy: "both_backends_finite_complete_and_monte_carlo_compatible".into(),
            claim_status: ClaimStatus::ExperimentalCrossBackendValidation,
        },
    )
}

fn execute_numpyro(
    source: &WorkerRequest,
    source_sha256: &str,
    maximum_tree_depth: u32,
    timeout_seconds: u64,
) -> Result<NumpyroResult, CliError> {
    let repository = &marklab::backend::python_backend_assets_root()?;
    let directory = repository.join("workers/python");
    let lock = read_file(&directory.join("uv.lock"))?;
    let worker =
        read_file(&directory.join("marklab_numpyro_replicated_arbitrary_window_lgcp_worker.py"))?;
    let request = NumpyroRequest {
        format: "marklab.numpyro_replicated_arbitrary_window_lgcp_request",
        version: 1,
        backend: BackendContract {
            name: "numpyro",
            version: NUMPYRO_VERSION,
            python_version: PYTHON_VERSION,
            environment_lock_sha256: sha256_hex(&lock),
            worker_sha256: sha256_hex(&worker),
        },
        jax_version: JAX_VERSION,
        source_request_sha256: source_sha256,
        source_request: source,
        maximum_tree_depth,
    };
    let bytes = serde_json::to_vec(&request)?;
    let output = run_worker(
        repository,
        "marklab_numpyro_replicated_arbitrary_window_lgcp_worker.py",
        &bytes,
        timeout_seconds,
    )?;
    serde_json::from_slice(&output).map_err(Into::into)
}

fn validate_numpyro(
    result: &NumpyroResult,
    source: &WorkerRequest,
    source_sha256: &str,
    maximum_tree_depth: u32,
    pymc: &ResultDocument,
) -> Result<(), CliError> {
    let directory = marklab::backend::python_backend_assets_root()?.join("workers/python");
    let lock = read_file(&directory.join("uv.lock"))?;
    let worker =
        read_file(&directory.join("marklab_numpyro_replicated_arbitrary_window_lgcp_worker.py"))?;
    let backend = BackendContract {
        name: "numpyro",
        version: NUMPYRO_VERSION,
        python_version: PYTHON_VERSION,
        environment_lock_sha256: sha256_hex(&lock),
        worker_sha256: sha256_hex(&worker),
    };
    let request = NumpyroRequest {
        format: "marklab.numpyro_replicated_arbitrary_window_lgcp_request",
        version: 1,
        backend: backend.clone(),
        jax_version: JAX_VERSION,
        source_request_sha256: source_sha256,
        source_request: source,
        maximum_tree_depth,
    };
    let request_sha256 = json_sha256(&request)?;
    marklab_bayes::protocol::replicated_arbitrary_window_lgcp_agreement::validate_numpyro(
        result,
        source,
        source_sha256,
        maximum_tree_depth,
        pymc,
        &backend,
        &request_sha256,
    )
    .map_err(super::protocol_error)
}
