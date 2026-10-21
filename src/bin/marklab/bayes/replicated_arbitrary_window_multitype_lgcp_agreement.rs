pub(crate) use marklab_bayes::protocol::replicated_arbitrary_window_multitype_lgcp_agreement::{
    compare, validate_numpyro, FitView, NumpyroView, Output,
};

use std::path::PathBuf;

use marklab_bayes::FitState;

use super::publish_json;
use super::replicated_arbitrary_window_multitype_lgcp;
use super::replicated_arbitrary_window_multitype_lgcp::Args as FitArgs;
use super::replicated_arbitrary_window_multitype_lgcp::Output as FitOutput;
use super::replicated_arbitrary_window_multitype_lgcp_numpyro;
use super::CliError;
use marklab_bayes::AgreementStatus;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;

const MAXIMUM_RESULT_BYTES: u64 = 16 * 1_048_576;

#[derive(Debug, clap::Args)]
pub(crate) struct Args {
    /// Previously fitted PyMC result to use in the comparison.
    #[arg(long)]
    pymc_result: PathBuf,
    /// Per-node, per-type quadrature CSV (pattern_id, patient_id, group, type_id, count, ...).
    #[arg(long)]
    input: PathBuf,
    /// Group level used as the regression reference.
    #[arg(long)]
    reference_group: String,
    /// Group level whose effect is compared with the reference group.
    #[arg(long)]
    comparison_group: String,
    /// Point-type label used as the model reference.
    #[arg(long)]
    reference_type: String,
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
    /// Upper bound on types.
    #[arg(long)]
    maximum_types: usize,
    /// Upper bound on nodes per pattern.
    #[arg(long)]
    maximum_nodes_per_pattern: usize,
    /// Upper bound on total nodes.
    #[arg(long)]
    maximum_total_nodes: usize,
    /// Upper bound on total node type rows.
    #[arg(long)]
    maximum_total_node_type_rows: usize,
    /// Upper bound on total events.
    #[arg(long)]
    maximum_total_events: u64,
    /// Upper bound on draw node type work.
    #[arg(long)]
    maximum_draw_node_type_work: u64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    timeout_seconds: u64,
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
    /// Destination for the analysis output.
    #[arg(long)]
    out: PathBuf,
}

pub(super) fn run(args: Args) -> Result<(), CliError> {
    for value in [
        args.maximum_standardized_difference,
        args.minimum_parameter_tolerance,
        args.minimum_field_tolerance,
    ] {
        if !value.is_finite() || value <= 0.0 {
            return Err(CliError::Input(
                "replicated multitype LGCP agreement controls are invalid".into(),
            ));
        }
    }
    let prepared = replicated_arbitrary_window_multitype_lgcp::prepare(fit_args(&args))?;
    let pymc_bytes = read_bounded(&args.pymc_result)?;
    let pymc: FitOutput = serde_json::from_slice(&pymc_bytes)?;
    prepared.validate_result(&pymc)?;
    let pymc_view: FitView = serde_json::from_value(serde_json::to_value(&pymc)?)?;
    let execution = replicated_arbitrary_window_multitype_lgcp_numpyro::execute(
        &prepared,
        args.numpyro_maximum_tree_depth,
        args.timeout_seconds,
    )?;
    let numpyro: NumpyroView = serde_json::from_value(execution.result.clone())?;
    validate_numpyro(
        &numpyro,
        &pymc_view,
        &execution.backend,
        &execution.request_sha256,
        prepared.request_sha256(),
        args.numpyro_maximum_tree_depth,
    )
    .map_err(super::protocol_error)?;
    let comparison = compare(
        &pymc_view,
        &numpyro,
        args.maximum_standardized_difference,
        args.minimum_parameter_tolerance,
        args.minimum_field_tolerance,
    );
    let complete = pymc_view.fit_state == FitState::Complete
        && numpyro.fit_state == FitState::Complete
        && comparison.all_pass;
    publish_json(
        &args.out,
        &Output {
            format: "marklab.replicated_arbitrary_window_multitype_lgcp_backend_agreement",
            version: 1,
            pymc,
            numpyro: execution.result,
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
                "same_exact_patients_patterns_windows_types_nodes_priors_draws_target_accept_and_seed",
                "numpyro_tree_depth_capacity_is_explicit_and_sampler_specific",
                "independent_pymc_and_numpyro_hierarchy_and_likelihood_implementations",
                "patients_not_patterns_nodes_types_or_cells_are_population_replicates",
            ],
            finite_result_policy: "both_backends_finite_complete_and_monte_carlo_compatible",
            claim_status: ClaimStatus::ExperimentalCrossBackendValidation,
        },
    )
}

fn fit_args(args: &Args) -> FitArgs {
    FitArgs {
        input: args.input.clone(),
        reference_group: args.reference_group.clone(),
        comparison_group: args.comparison_group.clone(),
        reference_type: args.reference_type.clone(),
        intercept_prior_mean: args.intercept_prior_mean,
        intercept_prior_sd: args.intercept_prior_sd,
        group_effect_prior_sd: args.group_effect_prior_sd,
        covariate_effect_prior_sd: args.covariate_effect_prior_sd,
        patient_sd_prior_scale: args.patient_sd_prior_scale,
        pattern_sd_prior_scale: args.pattern_sd_prior_scale,
        field_amplitude: args.field_amplitude,
        field_length_scale_um: args.field_length_scale_um,
        jitter: args.jitter,
        chains: args.chains,
        tune: args.tune,
        draws: args.draws,
        target_accept: args.target_accept,
        seed: args.seed,
        maximum_patients: args.maximum_patients,
        maximum_patterns: args.maximum_patterns,
        maximum_types: args.maximum_types,
        maximum_nodes_per_pattern: args.maximum_nodes_per_pattern,
        maximum_total_nodes: args.maximum_total_nodes,
        maximum_total_node_type_rows: args.maximum_total_node_type_rows,
        maximum_total_events: args.maximum_total_events,
        maximum_draw_node_type_work: args.maximum_draw_node_type_work,
        timeout_seconds: args.timeout_seconds,
        out: PathBuf::new(),
    }
}

fn read_bounded(path: &std::path::Path) -> Result<Vec<u8>, CliError> {
    crate::input_file::read_regular_file(
        path,
        MAXIMUM_RESULT_BYTES,
        "PyMC replicated multitype LGCP result is absent or oversized",
    )
}
