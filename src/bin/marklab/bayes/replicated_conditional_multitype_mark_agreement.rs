pub(crate) use marklab_bayes::protocol::replicated_conditional_multitype_mark_agreement::{
    compare, validate_numpyro, FitView, NumpyroResult, Output,
};

use std::path::PathBuf;

use marklab_bayes::sha256_hex;
use marklab_bayes::FitState;

use super::publish_json;
use super::replicated_conditional_multitype_mark;
use super::replicated_conditional_multitype_mark::Args as FitArgs;
use super::replicated_conditional_multitype_mark::Output as FitOutput;
use super::replicated_conditional_multitype_mark_numpyro;
use super::CliError;
use marklab_bayes::AgreementStatus;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;

const MAXIMUM_RESULT_BYTES: u64 = 8 * 1_048_576;

#[derive(Debug, clap::Args)]
pub(crate) struct Args {
    /// Previously fitted PyMC result to use in the comparison.
    #[arg(long)]
    pymc_result: PathBuf,
    /// CSV of points with pattern_id, patient_id, group, point_id, x_um, y_um and type_id.
    #[arg(long)]
    input: PathBuf,
    /// Group level used as the regression reference.
    #[arg(long)]
    reference_group: String,
    /// Point-type label used as the model reference.
    #[arg(long)]
    reference_type: String,
    /// Spatial radius in micrometers.
    #[arg(long)]
    radius_um: f64,
    /// Prior scale for intercept.
    #[arg(long)]
    intercept_prior_sd: f64,
    /// Prior scale for interaction.
    #[arg(long)]
    interaction_prior_sd: f64,
    /// Prior scale for group effect.
    #[arg(long)]
    group_effect_prior_sd: f64,
    /// Prior scale for patient standard deviation.
    #[arg(long)]
    patient_sd_prior_scale: f64,
    /// Prior scale for pattern standard deviation.
    #[arg(long)]
    pattern_sd_prior_scale: f64,
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
    /// Upper bound on points.
    #[arg(long)]
    maximum_points: usize,
    /// Upper bound on types.
    #[arg(long)]
    maximum_types: usize,
    /// Upper bound on neighbor visits.
    #[arg(long)]
    maximum_neighbor_visits: u64,
    /// Upper bound on edges.
    #[arg(long)]
    maximum_edges: usize,
    /// Upper bound on draw parameter work.
    #[arg(long)]
    maximum_draw_parameter_work: u64,
    /// Upper bound on working bytes.
    #[arg(long)]
    maximum_working_bytes: usize,
    /// Maximum NUTS tree depth for the PyMC fit.
    #[arg(long)]
    pymc_maximum_tree_depth: u32,
    /// Maximum NUTS tree depth for the NumPyro fit.
    #[arg(long)]
    numpyro_maximum_tree_depth: u32,
    /// Largest standardized difference accepted between backend estimates.
    #[arg(long)]
    maximum_standardized_difference: f64,
    /// Minimum absolute comparison tolerance for absolute.
    #[arg(long)]
    minimum_absolute_tolerance: f64,
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
        || !args.maximum_standardized_difference.is_finite()
        || args.maximum_standardized_difference <= 0.0
        || !args.minimum_absolute_tolerance.is_finite()
        || args.minimum_absolute_tolerance <= 0.0
    {
        return Err(CliError::Input(
            "replicated conditional-mark agreement controls are invalid".into(),
        ));
    }
    let input_bytes = read_bounded(&args.input, 64 * 1_048_576)?;
    let input_sha256 = sha256_hex(&input_bytes);
    let backend = replicated_conditional_multitype_mark::backend_contract()?;
    let prepared = replicated_conditional_multitype_mark::prepare(FitArgs {
        input: args.input,
        reference_group: args.reference_group.clone(),
        reference_type: args.reference_type.clone(),
        radius_um: args.radius_um,
        intercept_prior_sd: args.intercept_prior_sd,
        interaction_prior_sd: args.interaction_prior_sd,
        group_effect_prior_sd: args.group_effect_prior_sd,
        patient_sd_prior_scale: args.patient_sd_prior_scale,
        pattern_sd_prior_scale: args.pattern_sd_prior_scale,
        chains: args.chains,
        tune: args.tune,
        draws: args.draws,
        target_accept: args.target_accept,
        seed: args.seed,
        maximum_patients: args.maximum_patients,
        maximum_patterns: args.maximum_patterns,
        maximum_points: args.maximum_points,
        maximum_types: args.maximum_types,
        maximum_neighbor_visits: args.maximum_neighbor_visits,
        maximum_edges: args.maximum_edges,
        maximum_draw_parameter_work: args.maximum_draw_parameter_work,
        maximum_working_bytes: args.maximum_working_bytes,
        maximum_tree_depth: args.pymc_maximum_tree_depth,
        timeout_seconds: args.timeout_seconds,
        out: PathBuf::new(),
    })?;
    let pymc_bytes = read_bounded(&args.pymc_result, MAXIMUM_RESULT_BYTES)?;
    let pymc: FitOutput = serde_json::from_slice(&pymc_bytes)?;
    pymc.validate_for(
        &input_sha256,
        &args.reference_group,
        &args.reference_type,
        args.radius_um,
        &backend,
    )
    .map_err(super::protocol_error)?;
    let pymc_view: FitView = serde_json::from_value(serde_json::to_value(&pymc)?)?;
    if pymc_view.request_sha256 != prepared.request_sha256() {
        return Err(CliError::Input(
            "PyMC result does not match the exact agreement request".into(),
        ));
    }
    let execution = replicated_conditional_multitype_mark_numpyro::execute(
        &prepared,
        args.numpyro_maximum_tree_depth,
        args.timeout_seconds,
    )?;
    let numpyro: NumpyroResult = serde_json::from_value(execution.result)?;
    validate_numpyro(
        &numpyro,
        &pymc_view,
        prepared.request_sha256(),
        &execution.request_sha256,
        &execution.backend,
        args.numpyro_maximum_tree_depth,
    )
    .map_err(super::protocol_error)?;
    let comparison = compare(
        &pymc_view,
        &numpyro,
        args.maximum_standardized_difference,
        args.minimum_absolute_tolerance,
    );
    let complete = pymc_view.fit_state == FitState::Complete
        && numpyro.fit_state == FitState::Complete
        && comparison.all_pass;
    publish_json(
        &args.out,
        &Output {
            format: "marklab.replicated_conditional_multitype_mark_backend_agreement",
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
                "same_fixed_pattern_graphs_hierarchy_priors_draws_and_seed",
                "independent_pymc_and_numpyro_hierarchical_conditional_implementations",
                "patients_are_independent_and_patterns_are_nested_within_patient",
                "composition_and_spatial_pair_effects_are_separate",
                "composite_pseudolikelihood_is_not_a_normalized_joint_likelihood",
            ],
            finite_result_policy: "both_backends_finite_complete_and_monte_carlo_compatible",
            claim_status: ClaimStatus::ExperimentalPatientHierarchyCrossBackendValidation,
        },
    )
}

fn read_bounded(path: &std::path::Path, maximum: u64) -> Result<Vec<u8>, CliError> {
    crate::input_file::read_regular_file(
        path,
        maximum,
        &format!("agreement input is absent or exceeds {maximum} bytes"),
    )
}
