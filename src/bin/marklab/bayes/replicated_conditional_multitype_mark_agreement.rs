use std::path::PathBuf;

use clap::{Parser, Subcommand};
use marklab_bayes::{
    sha256_hex, BackendContract, FitState, NormalMeanDiagnostics, SamplingSummary,
    SarScalarSummary, WorkerBackend,
};
use serde::{Deserialize, Serialize};

use super::{
    publish_json,
    replicated_arbitrary_window_lgcp_agreement::{
        backend_matches, compare_scalar, compare_vector, summary_valid, ScalarAgreement,
        VectorAgreement,
    },
    replicated_conditional_multitype_mark::{self, Args as FitArgs, Output as FitOutput},
    replicated_conditional_multitype_mark_numpyro::{self, JAX_VERSION},
    BayesCliError,
};

const MAXIMUM_RESULT_BYTES: u64 = 8 * 1_048_576;

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct Cli {
    #[command(subcommand)]
    command: Top,
}

#[derive(Debug, Subcommand)]
enum Top {
    Bayes {
        #[command(subcommand)]
        command: Command,
    },
}

#[derive(Debug, Subcommand)]
enum Command {
    ReplicatedConditionalMultitypeMarkAgreement(Box<Args>),
}

#[derive(Debug, clap::Args)]
struct Args {
    #[arg(long)]
    pymc_result: PathBuf,
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    reference_group: String,
    #[arg(long)]
    reference_type: String,
    #[arg(long)]
    radius_um: f64,
    #[arg(long)]
    intercept_prior_sd: f64,
    #[arg(long)]
    interaction_prior_sd: f64,
    #[arg(long)]
    group_effect_prior_sd: f64,
    #[arg(long)]
    patient_sd_prior_scale: f64,
    #[arg(long)]
    pattern_sd_prior_scale: f64,
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
    maximum_patients: usize,
    #[arg(long)]
    maximum_patterns: usize,
    #[arg(long)]
    maximum_points: usize,
    #[arg(long)]
    maximum_types: usize,
    #[arg(long)]
    maximum_neighbor_visits: u64,
    #[arg(long)]
    maximum_edges: usize,
    #[arg(long)]
    maximum_draw_parameter_work: u64,
    #[arg(long)]
    maximum_working_bytes: usize,
    #[arg(long)]
    pymc_maximum_tree_depth: u32,
    #[arg(long)]
    numpyro_maximum_tree_depth: u32,
    #[arg(long)]
    maximum_standardized_difference: f64,
    #[arg(long)]
    minimum_absolute_tolerance: f64,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PairRow {
    type_a: String,
    type_b: String,
    contrast: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PatientPairRow {
    patient_id: String,
    group: String,
    type_a: String,
    type_b: String,
    contrast: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScaleRow {
    level: String,
    component: String,
    scale: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PatternRow {
    pattern_id: String,
    observed_same_type_edges: u64,
    expected_same_type_edges: SarScalarSummary,
    observed_type_counts: Vec<u64>,
    expected_type_counts: Vec<SarScalarSummary>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScoreRows {
    independent_pattern_label_composite_log_score: f64,
    spatial_hierarchical_composite_log_score: SarScalarSummary,
    mean_composite_log_score_improvement: f64,
}

#[derive(Debug, Deserialize)]
struct FitView {
    input_sha256: String,
    request_sha256: String,
    fit_state: FitState,
    sampling: SamplingSummary,
    baseline_affinities: Vec<PairRow>,
    group_affinity_shifts: Vec<PairRow>,
    patient_affinities: Vec<PatientPairRow>,
    hierarchy_scales: Vec<ScaleRow>,
    comparison: ScoreRows,
    pattern_checks: Vec<PatternRow>,
    diagnostics: NormalMeanDiagnostics,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NumpyroResult {
    format: String,
    version: u32,
    backend: WorkerBackend,
    jax_version: String,
    input_sha256: String,
    request_sha256: String,
    source_request_sha256: String,
    maximum_tree_depth: u32,
    fit_state: FitState,
    sampling: SamplingSummary,
    baseline_affinities: Vec<PairRow>,
    group_affinity_shifts: Vec<PairRow>,
    patient_affinities: Vec<PatientPairRow>,
    hierarchy_scales: Vec<ScaleRow>,
    comparison: ScoreRows,
    pattern_checks: Vec<PatternRow>,
    diagnostics: NormalMeanDiagnostics,
}

#[derive(Debug, Serialize)]
struct Comparison {
    baseline_affinities: VectorAgreement,
    group_affinity_shifts: VectorAgreement,
    patient_affinities: VectorAgreement,
    hierarchy_scales: VectorAgreement,
    composite_log_score: ScalarAgreement,
    pattern_same_type_edges: VectorAgreement,
    pattern_type_counts: VectorAgreement,
    all_pass: bool,
}

#[derive(Debug, Serialize)]
struct Output {
    format: &'static str,
    version: u32,
    pymc: FitOutput,
    numpyro: NumpyroResult,
    comparison: Comparison,
    fit_state: FitState,
    agreement_status: &'static str,
    statistical_unit: &'static str,
    assumptions: [&'static str; 5],
    finite_result_policy: &'static str,
    claim_status: &'static str,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let Top::Bayes { command } = Cli::parse_from(std::env::args_os()).command;
    let Command::ReplicatedConditionalMultitypeMarkAgreement(args) = command;
    run(*args)
}

fn run(args: Args) -> Result<(), BayesCliError> {
    if !(10..=14).contains(&args.pymc_maximum_tree_depth)
        || !(10..=14).contains(&args.numpyro_maximum_tree_depth)
        || !args.maximum_standardized_difference.is_finite()
        || args.maximum_standardized_difference <= 0.0
        || !args.minimum_absolute_tolerance.is_finite()
        || args.minimum_absolute_tolerance <= 0.0
    {
        return Err(BayesCliError::Input(
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
    )?;
    let pymc_view: FitView = serde_json::from_value(serde_json::to_value(&pymc)?)?;
    if pymc_view.request_sha256 != prepared.request_sha256() {
        return Err(BayesCliError::Input(
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
        &prepared,
        &execution.request_sha256,
        &execution.backend,
        args.numpyro_maximum_tree_depth,
    )?;
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
                "agree_within_monte_carlo_error"
            } else {
                "diagnostic_only_disagreement_or_nonconvergence"
            },
            statistical_unit: "patient",
            assumptions: [
                "same_fixed_pattern_graphs_hierarchy_priors_draws_and_seed",
                "independent_pymc_and_numpyro_hierarchical_conditional_implementations",
                "patients_are_independent_and_patterns_are_nested_within_patient",
                "composition_and_spatial_pair_effects_are_separate",
                "composite_pseudolikelihood_is_not_a_normalized_joint_likelihood",
            ],
            finite_result_policy: "both_backends_finite_complete_and_monte_carlo_compatible",
            claim_status: "experimental_patient_hierarchy_cross_backend_validation",
        },
    )
}

fn validate_numpyro(
    result: &NumpyroResult,
    pymc: &FitView,
    prepared: &replicated_conditional_multitype_mark::Prepared,
    request_sha256: &str,
    backend: &BackendContract,
    depth: u32,
) -> Result<(), BayesCliError> {
    let rows_valid = result
        .baseline_affinities
        .iter()
        .chain(&result.group_affinity_shifts)
        .map(|row| &row.contrast)
        .chain(result.patient_affinities.iter().map(|row| &row.contrast))
        .chain(result.hierarchy_scales.iter().map(|row| &row.scale))
        .chain(result.pattern_checks.iter().flat_map(|row| {
            std::iter::once(&row.expected_same_type_edges).chain(row.expected_type_counts.iter())
        }))
        .chain(std::iter::once(
            &result.comparison.spatial_hierarchical_composite_log_score,
        ))
        .all(summary_valid);
    let identities_valid = rows_match(&result.baseline_affinities, &pymc.baseline_affinities)
        && rows_match(&result.group_affinity_shifts, &pymc.group_affinity_shifts)
        && result
            .patient_affinities
            .iter()
            .zip(&pymc.patient_affinities)
            .all(|(left, right)| {
                left.patient_id == right.patient_id
                    && left.group == right.group
                    && left.type_a == right.type_a
                    && left.type_b == right.type_b
            })
        && result
            .hierarchy_scales
            .iter()
            .zip(&pymc.hierarchy_scales)
            .all(|(left, right)| left.level == right.level && left.component == right.component)
        && result
            .pattern_checks
            .iter()
            .zip(&pymc.pattern_checks)
            .all(|(left, right)| {
                left.pattern_id == right.pattern_id
                    && left.observed_same_type_edges == right.observed_same_type_edges
                    && left.observed_type_counts == right.observed_type_counts
                    && left.expected_type_counts.len() == right.expected_type_counts.len()
            });
    let complete = result.diagnostics.prior_predictive_finite
        && result.diagnostics.posterior_finite
        && result.diagnostics.constraints_valid
        && result.diagnostics.identifiability_checks_passed
        && result.diagnostics.r_hat <= 1.01
        && result.diagnostics.ess_bulk >= 400.0
        && result.diagnostics.ess_tail >= 400.0
        && result.diagnostics.minimum_ebfmi >= 0.2
        && result.diagnostics.divergences == 0
        && result.diagnostics.max_tree_depth_hits == 0;
    if result.format != "marklab.numpyro_replicated_conditional_multitype_mark_result"
        || result.version != 1
        || !backend_matches(&result.backend, backend)
        || result.jax_version != JAX_VERSION
        || result.input_sha256 != pymc.input_sha256
        || result.request_sha256 != request_sha256
        || result.source_request_sha256 != prepared.request_sha256()
        || result.maximum_tree_depth != depth
        || result.sampling.chains != pymc.sampling.chains
        || result.sampling.tune_per_chain != pymc.sampling.tune_per_chain
        || result.sampling.draws_per_chain != pymc.sampling.draws_per_chain
        || result.sampling.completed_draws != pymc.sampling.completed_draws
        || result.baseline_affinities.len() != pymc.baseline_affinities.len()
        || result.group_affinity_shifts.len() != pymc.group_affinity_shifts.len()
        || result.patient_affinities.len() != pymc.patient_affinities.len()
        || result.hierarchy_scales.len() != pymc.hierarchy_scales.len()
        || result.pattern_checks.len() != pymc.pattern_checks.len()
        || result
            .comparison
            .independent_pattern_label_composite_log_score
            .to_bits()
            != pymc
                .comparison
                .independent_pattern_label_composite_log_score
                .to_bits()
        || !rows_valid
        || !identities_valid
        || (result.fit_state == FitState::Complete) != complete
    {
        return Err(BayesCliError::Backend(
            "replicated conditional-mark NumPyro result differs".into(),
        ));
    }
    Ok(())
}

fn rows_match(left: &[PairRow], right: &[PairRow]) -> bool {
    left.iter()
        .zip(right)
        .all(|(left, right)| left.type_a == right.type_a && left.type_b == right.type_b)
}

fn compare(left: &FitView, right: &NumpyroResult, standardized: f64, tolerance: f64) -> Comparison {
    let left_ess = left.diagnostics.ess_bulk;
    let right_ess = right.diagnostics.ess_bulk;
    let baseline_affinities = compare_vector(
        left.baseline_affinities.iter().map(|row| &row.contrast),
        right.baseline_affinities.iter().map(|row| &row.contrast),
        left_ess,
        right_ess,
        standardized,
        tolerance,
    );
    let group_affinity_shifts = compare_vector(
        left.group_affinity_shifts.iter().map(|row| &row.contrast),
        right.group_affinity_shifts.iter().map(|row| &row.contrast),
        left_ess,
        right_ess,
        standardized,
        tolerance,
    );
    let patient_affinities = compare_vector(
        left.patient_affinities.iter().map(|row| &row.contrast),
        right.patient_affinities.iter().map(|row| &row.contrast),
        left_ess,
        right_ess,
        standardized,
        tolerance,
    );
    let hierarchy_scales = compare_vector(
        left.hierarchy_scales.iter().map(|row| &row.scale),
        right.hierarchy_scales.iter().map(|row| &row.scale),
        left_ess,
        right_ess,
        standardized,
        tolerance,
    );
    let composite_log_score = compare_scalar(
        &left.comparison.spatial_hierarchical_composite_log_score,
        &right.comparison.spatial_hierarchical_composite_log_score,
        left_ess,
        right_ess,
        standardized,
        tolerance,
    );
    let pattern_same_type_edges = compare_vector(
        left.pattern_checks
            .iter()
            .map(|row| &row.expected_same_type_edges),
        right
            .pattern_checks
            .iter()
            .map(|row| &row.expected_same_type_edges),
        left_ess,
        right_ess,
        standardized,
        tolerance,
    );
    let pattern_type_counts = compare_vector(
        left.pattern_checks
            .iter()
            .flat_map(|row| row.expected_type_counts.iter()),
        right
            .pattern_checks
            .iter()
            .flat_map(|row| row.expected_type_counts.iter()),
        left_ess,
        right_ess,
        standardized,
        tolerance,
    );
    let all_pass = baseline_affinities.passes
        && group_affinity_shifts.passes
        && patient_affinities.passes
        && hierarchy_scales.passes
        && composite_log_score.passes
        && pattern_same_type_edges.passes
        && pattern_type_counts.passes;
    Comparison {
        baseline_affinities,
        group_affinity_shifts,
        patient_affinities,
        hierarchy_scales,
        composite_log_score,
        pattern_same_type_edges,
        pattern_type_counts,
        all_pass,
    }
}

fn read_bounded(path: &std::path::Path, maximum: u64) -> Result<Vec<u8>, BayesCliError> {
    super::input_file::read_regular_file(
        path,
        maximum,
        &format!("agreement input is absent or exceeds {maximum} bytes"),
    )
}
