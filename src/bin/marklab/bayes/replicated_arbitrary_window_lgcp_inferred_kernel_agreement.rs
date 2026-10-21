pub(crate) use marklab_bayes::protocol::replicated_arbitrary_window_lgcp_inferred_kernel_agreement::{NumpyroResult, Output, validate_numpyro, compare};

use crate::input_file::read_file;
use std::path::PathBuf;

use marklab_bayes::sha256_hex;
use marklab_bayes::BackendContract;
use marklab_bayes::FitState;

use super::publish_json;
use super::replicated_arbitrary_window_lgcp_inferred_kernel;
use super::run_worker;
use super::CliError;
use marklab_bayes::backend_versions::JAX_VERSION;
use marklab_bayes::backend_versions::NUMPYRO_VERSION;
use marklab_bayes::backend_versions::PYTHON_VERSION;
use marklab_bayes::AgreementStatus;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;

#[derive(Debug, clap::Args)]
pub(crate) struct Args {
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
    /// Prior scale for field amplitude.
    #[arg(long)]
    field_amplitude_prior_scale: f64,
    /// Prior scale, in micrometers, for field length scale.
    #[arg(long)]
    field_length_scale_prior_scale_um: f64,
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
    /// Upper bound on kernel cube work.
    #[arg(long)]
    maximum_kernel_cube_work: u64,
    /// Maximum NUTS tree depth for the PyMC fit.
    #[arg(long)]
    pymc_maximum_tree_depth: u32,
    /// Maximum NUTS tree depth for the NumPyro fit.
    #[arg(long)]
    numpyro_maximum_tree_depth: u32,
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

pub(super) fn run(args: Args) -> Result<(), CliError> {
    if !(10..=14).contains(&args.pymc_maximum_tree_depth)
        || !(10..=14).contains(&args.numpyro_maximum_tree_depth)
        || [
            args.maximum_standardized_difference,
            args.minimum_parameter_tolerance,
            args.minimum_field_tolerance,
        ]
        .into_iter()
        .any(|value| !value.is_finite() || value <= 0.0)
    {
        return Err(CliError::Input(
            "inferred-kernel agreement controls are invalid".into(),
        ));
    }
    let prepared = replicated_arbitrary_window_lgcp_inferred_kernel::prepare(
        crate::bayes::ReplicatedArbitraryWindowLgcpInferredKernelArgs {
            input: args.input,
            reference_group: args.reference_group,
            comparison_group: args.comparison_group,
            intercept_prior_mean: args.intercept_prior_mean,
            intercept_prior_sd: args.intercept_prior_sd,
            group_effect_prior_sd: args.group_effect_prior_sd,
            covariate_effect_prior_sd: args.covariate_effect_prior_sd,
            patient_sd_prior_scale: args.patient_sd_prior_scale,
            pattern_sd_prior_scale: args.pattern_sd_prior_scale,
            field_amplitude_prior_scale: args.field_amplitude_prior_scale,
            field_length_scale_prior_scale_um: args.field_length_scale_prior_scale_um,
            jitter: args.jitter,
            chains: args.chains,
            tune: args.tune,
            draws: args.draws,
            target_accept: args.target_accept,
            seed: args.seed,
            maximum_patients: args.maximum_patients,
            maximum_patterns: args.maximum_patterns,
            maximum_nodes_per_pattern: args.maximum_nodes_per_pattern,
            maximum_total_nodes: args.maximum_total_nodes,
            maximum_total_events: args.maximum_total_events,
            maximum_draw_node_work: args.maximum_draw_node_work,
            maximum_kernel_cube_work: args.maximum_kernel_cube_work,
            maximum_tree_depth: args.pymc_maximum_tree_depth,
            timeout_seconds: args.timeout_seconds,
            out: std::path::PathBuf::new(),
        },
    )?;
    let pymc = replicated_arbitrary_window_lgcp_inferred_kernel::execute(&prepared)?;
    let (numpyro, request_sha, expected_backend) = execute_numpyro(
        &prepared,
        args.numpyro_maximum_tree_depth,
        args.timeout_seconds,
    )?;
    validate_numpyro(
        &numpyro,
        prepared.source_request(),
        prepared.request_sha256(),
        &request_sha,
        &expected_backend,
        args.numpyro_maximum_tree_depth,
    )
    .map_err(super::protocol_error)?;
    let comparison = compare(
        &pymc,
        &numpyro,
        args.maximum_standardized_difference,
        args.minimum_parameter_tolerance,
        args.minimum_field_tolerance,
    );
    let complete = pymc.fit_state == FitState::Complete
        && numpyro.fit_state == FitState::Complete
        && comparison.all_pass;
    publish_json(
        &args.out,
        &Output {
            format: "marklab.replicated_arbitrary_window_lgcp_inferred_kernel_backend_agreement",
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
            assumptions: [
                "same_exact_data_priors_draws_target_accept_and_seed",
                "independent_pymc_and_numpyro_dynamic_kernel_implementations",
                "sampler_tree_depth_capacities_are_explicit",
            ],
            finite_result_policy: "both_backends_finite_complete_and_monte_carlo_compatible",
            claim_status: ClaimStatus::ExperimentalCrossBackendValidation,
        },
    )
}

fn request_value(
    prepared: &replicated_arbitrary_window_lgcp_inferred_kernel::PreparedReplicatedArbitraryWindowLgcpInferredKernel,
    backend: BackendContract,
    depth: u32,
) -> Result<serde_json::Value, CliError> {
    Ok(serde_json::json!({
        "format": "marklab.numpyro_replicated_arbitrary_window_lgcp_inferred_kernel_request",
        "version": 1,
        "backend": backend,
        "jax_version": JAX_VERSION,
        "source_request_sha256": prepared.request_sha256(),
        "source_request": serde_json::from_slice::<serde_json::Value>(prepared.request_bytes())?,
        "maximum_tree_depth": depth,
    }))
}

fn execute_numpyro(
    prepared: &replicated_arbitrary_window_lgcp_inferred_kernel::PreparedReplicatedArbitraryWindowLgcpInferredKernel,
    depth: u32,
    timeout: u64,
) -> Result<(NumpyroResult, String, BackendContract), CliError> {
    let repository = &marklab::backend::python_backend_assets_root()?;
    let directory = repository.join("workers/python");
    let backend = BackendContract {
        name: "numpyro",
        version: NUMPYRO_VERSION,
        python_version: PYTHON_VERSION,
        environment_lock_sha256: sha256_hex(&read_file(&directory.join("uv.lock"))?),
        worker_sha256: sha256_hex(&read_file(&directory.join(
            "marklab_numpyro_replicated_arbitrary_window_lgcp_inferred_kernel_worker.py",
        ))?),
    };
    let bytes = serde_json::to_vec(&request_value(prepared, backend.clone(), depth)?)?;
    let request_sha = sha256_hex(&bytes);
    let output = run_worker(
        repository,
        "marklab_numpyro_replicated_arbitrary_window_lgcp_inferred_kernel_worker.py",
        &bytes,
        timeout,
    )?;
    Ok((serde_json::from_slice(&output)?, request_sha, backend))
}
