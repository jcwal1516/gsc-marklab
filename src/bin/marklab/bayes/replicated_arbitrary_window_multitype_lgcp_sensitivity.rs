use std::{path::PathBuf, thread};

use clap::{Parser, Subcommand};
use marklab_bayes::{
    BackendContract, DiagnosticPolicy, FitState, NormalMeanDiagnostics, SarScalarSummary,
    WorkerBackend,
};
use serde::{Deserialize, Serialize};

use super::{
    publish_json,
    replicated_arbitrary_window_lgcp_agreement::{backend_matches, summary_valid},
    replicated_arbitrary_window_multitype_lgcp::{
        self, Args as FitArgs, Output as FitOutput, Prepared,
    },
    replicated_arbitrary_window_multitype_lgcp_numpyro::{self, JAX_VERSION},
    BayesCliError,
};

const MAXIMUM_RESULT_BYTES: u64 = 16 * 1_048_576;

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
    ReplicatedArbitraryWindowMultitypeLgcpSensitivity(Box<Args>),
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
    comparison_group: String,
    #[arg(long)]
    reference_type: String,
    #[arg(long, allow_hyphen_values = true)]
    intercept_prior_mean: f64,
    #[arg(long)]
    intercept_prior_sd: f64,
    #[arg(long)]
    group_effect_prior_sd: f64,
    #[arg(long)]
    covariate_effect_prior_sd: f64,
    #[arg(long)]
    patient_sd_prior_scale: f64,
    #[arg(long)]
    pattern_sd_prior_scale: f64,
    #[arg(long)]
    field_amplitude: f64,
    #[arg(long)]
    field_length_scale_um: f64,
    #[arg(long)]
    jitter: f64,
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
    maximum_types: usize,
    #[arg(long)]
    maximum_nodes_per_pattern: usize,
    #[arg(long)]
    maximum_total_nodes: usize,
    #[arg(long)]
    maximum_total_node_type_rows: usize,
    #[arg(long)]
    maximum_total_events: u64,
    #[arg(long)]
    maximum_draw_node_type_work: u64,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    numpyro_maximum_tree_depth: u32,
    #[arg(long)]
    maximum_processes: usize,
    #[arg(long)]
    maximum_total_draw_node_type_work: u64,
    #[arg(long)]
    materiality_standard_deviations: f64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Debug, Deserialize)]
struct TypeRow {
    type_id: String,
    intercept: SarScalarSummary,
    group_effect: SarScalarSummary,
    covariate_effect: SarScalarSummary,
    patient_sd: SarScalarSummary,
    pattern_sd: SarScalarSummary,
}

#[derive(Debug, Deserialize)]
struct DifferenceRow {
    type_a: String,
    type_b: String,
    difference: SarScalarSummary,
}

#[derive(Debug, Deserialize)]
struct EffectRow {
    owner_id: String,
    type_id: String,
    effect: SarScalarSummary,
}

#[derive(Debug, Deserialize)]
struct NodeRow {
    pattern_id: String,
    node_id: String,
    type_id: String,
    latent_effect: SarScalarSummary,
    expected_count: SarScalarSummary,
}

#[derive(Debug, Deserialize)]
struct BaselineView {
    input_sha256: String,
    request_sha256: String,
    fit_state: FitState,
    type_posteriors: Vec<TypeRow>,
    group_effect_differences: Vec<DifferenceRow>,
    patient_type_effects: Vec<EffectRow>,
    pattern_type_effects: Vec<EffectRow>,
    node_type_posteriors: Vec<NodeRow>,
}

#[derive(Debug, Deserialize)]
struct ScenarioView {
    format: String,
    version: u32,
    backend: WorkerBackend,
    jax_version: String,
    input_sha256: String,
    request_sha256: String,
    source_request_sha256: String,
    maximum_tree_depth: u32,
    fit_state: FitState,
    type_posteriors: Vec<TypeRow>,
    group_effect_differences: Vec<DifferenceRow>,
    patient_type_effects: Vec<EffectRow>,
    pattern_type_effects: Vec<EffectRow>,
    node_type_posteriors: Vec<NodeRow>,
    diagnostics: NormalMeanDiagnostics,
}

struct ScenarioSpec {
    name: &'static str,
    patient_scale: f64,
    pattern_scale: f64,
    field_amplitude: f64,
    field_length: f64,
    prepared: Prepared,
}

#[derive(Debug, Serialize)]
struct ScenarioOutput {
    scenario: &'static str,
    patient_sd_prior_scale: f64,
    pattern_sd_prior_scale: f64,
    field_amplitude: f64,
    field_length_scale_um: f64,
    maximum_type_group_shift_sd: f64,
    maximum_pairwise_group_difference_shift_sd: f64,
    maximum_patient_effect_shift_sd: f64,
    maximum_pattern_effect_shift_sd: f64,
    maximum_hierarchy_scale_shift_sd: f64,
    maximum_expected_count_shift_sd: f64,
    material_sensitivity: bool,
    fit: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct Resources {
    scenario_count: usize,
    maximum_processes: usize,
    per_scenario_draw_node_type_work: u64,
    total_draw_node_type_work: u64,
    maximum_total_draw_node_type_work: u64,
    timeout_seconds_per_process: u64,
}

#[derive(Debug, Serialize)]
struct Output {
    format: &'static str,
    version: u32,
    baseline_request_sha256: String,
    scenarios: Vec<ScenarioOutput>,
    materiality_standard_deviations: f64,
    material_sensitivity: bool,
    fit_state: FitState,
    resources: Resources,
    statistical_unit: &'static str,
    assumptions: [&'static str; 5],
    finite_result_policy: &'static str,
    claim_status: &'static str,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let Top::Bayes { command } = Cli::parse_from(std::env::args_os()).command;
    let Command::ReplicatedArbitraryWindowMultitypeLgcpSensitivity(args) = command;
    run(*args)
}

fn run(args: Args) -> Result<(), BayesCliError> {
    if !(10..=14).contains(&args.numpyro_maximum_tree_depth)
        || !(1..=4).contains(&args.maximum_processes)
        || args.maximum_total_draw_node_type_work == 0
        || !args.materiality_standard_deviations.is_finite()
        || args.materiality_standard_deviations <= 0.0
    {
        return Err(BayesCliError::Input(
            "replicated multitype LGCP sensitivity controls are invalid".into(),
        ));
    }
    let baseline_prepared = prepare(
        &args,
        args.patient_sd_prior_scale,
        args.pattern_sd_prior_scale,
        args.field_amplitude,
        args.field_length_scale_um,
    )?;
    let pymc: FitOutput = serde_json::from_slice(&read_bounded(&args.pymc_result)?)?;
    pymc.validate_for(&baseline_prepared)?;
    let baseline: BaselineView = serde_json::from_value(serde_json::to_value(&pymc)?)?;
    if baseline.request_sha256 != baseline_prepared.request_sha256() {
        return Err(BayesCliError::Input(
            "PyMC baseline does not match exact sensitivity request".into(),
        ));
    }
    let per_scenario_work = baseline_prepared.draw_node_type_work();
    let total_work = per_scenario_work
        .checked_mul(8)
        .ok_or_else(|| BayesCliError::Input("total sensitivity work overflows".into()))?;
    if total_work > args.maximum_total_draw_node_type_work {
        return Err(BayesCliError::Input(format!(
            "total sensitivity draw-node-type work exceeds maximum: {total_work} > {}",
            args.maximum_total_draw_node_type_work
        )));
    }
    let values = [
        ("patient_scale_half", 0.5, 1.0, 1.0, 1.0),
        ("patient_scale_double", 2.0, 1.0, 1.0, 1.0),
        ("pattern_scale_half", 1.0, 0.5, 1.0, 1.0),
        ("pattern_scale_double", 1.0, 2.0, 1.0, 1.0),
        ("field_amplitude_half", 1.0, 1.0, 0.5, 1.0),
        ("field_amplitude_double", 1.0, 1.0, 2.0, 1.0),
        ("field_length_half", 1.0, 1.0, 1.0, 0.5),
        ("field_length_double", 1.0, 1.0, 1.0, 2.0),
    ];
    let scenarios = values
        .into_iter()
        .map(|(name, patient, pattern, amplitude, length)| {
            let patient_scale = args.patient_sd_prior_scale * patient;
            let pattern_scale = args.pattern_sd_prior_scale * pattern;
            let field_amplitude = args.field_amplitude * amplitude;
            let field_length = args.field_length_scale_um * length;
            Ok(ScenarioSpec {
                name,
                patient_scale,
                pattern_scale,
                field_amplitude,
                field_length,
                prepared: prepare(
                    &args,
                    patient_scale,
                    pattern_scale,
                    field_amplitude,
                    field_length,
                )?,
            })
        })
        .collect::<Result<Vec<_>, BayesCliError>>()?;
    let mut outputs = Vec::with_capacity(8);
    for wave in scenarios.chunks(args.maximum_processes) {
        let executions = thread::scope(|scope| {
            let handles = wave
                .iter()
                .map(|scenario| {
                    scope.spawn(|| {
                        replicated_arbitrary_window_multitype_lgcp_numpyro::execute(
                            &scenario.prepared,
                            args.numpyro_maximum_tree_depth,
                            args.timeout_seconds,
                        )
                    })
                })
                .collect::<Vec<_>>();
            handles
                .into_iter()
                .map(|handle| {
                    handle.join().map_err(|_| {
                        BayesCliError::Backend(
                            "replicated multitype LGCP sensitivity worker panicked".into(),
                        )
                    })?
                })
                .collect::<Result<Vec<_>, BayesCliError>>()
        })?;
        for (scenario, execution) in wave.iter().zip(executions) {
            let view: ScenarioView = serde_json::from_value(execution.result.clone())?;
            validate_scenario(
                &view,
                &baseline,
                &scenario.prepared,
                &execution.request_sha256,
                &execution.backend,
                args.numpyro_maximum_tree_depth,
            )?;
            let type_group = maximum_shift(
                baseline.type_posteriors.iter().map(|row| &row.group_effect),
                view.type_posteriors.iter().map(|row| &row.group_effect),
            );
            let pair = maximum_shift(
                baseline
                    .group_effect_differences
                    .iter()
                    .map(|row| &row.difference),
                view.group_effect_differences
                    .iter()
                    .map(|row| &row.difference),
            );
            let patient = maximum_shift(
                baseline.patient_type_effects.iter().map(|row| &row.effect),
                view.patient_type_effects.iter().map(|row| &row.effect),
            );
            let hierarchy = maximum_shift(
                baseline
                    .type_posteriors
                    .iter()
                    .flat_map(|row| [&row.patient_sd, &row.pattern_sd]),
                view.type_posteriors
                    .iter()
                    .flat_map(|row| [&row.patient_sd, &row.pattern_sd]),
            );
            let pattern_effect = maximum_shift(
                baseline.pattern_type_effects.iter().map(|row| &row.effect),
                view.pattern_type_effects.iter().map(|row| &row.effect),
            );
            let expected = maximum_shift(
                baseline
                    .node_type_posteriors
                    .iter()
                    .map(|row| &row.expected_count),
                view.node_type_posteriors
                    .iter()
                    .map(|row| &row.expected_count),
            );
            let material = [
                type_group,
                pair,
                patient,
                pattern_effect,
                hierarchy,
                expected,
            ]
            .into_iter()
            .any(|value| value >= args.materiality_standard_deviations);
            outputs.push(ScenarioOutput {
                scenario: scenario.name,
                patient_sd_prior_scale: scenario.patient_scale,
                pattern_sd_prior_scale: scenario.pattern_scale,
                field_amplitude: scenario.field_amplitude,
                field_length_scale_um: scenario.field_length,
                maximum_type_group_shift_sd: type_group,
                maximum_pairwise_group_difference_shift_sd: pair,
                maximum_patient_effect_shift_sd: patient,
                maximum_pattern_effect_shift_sd: pattern_effect,
                maximum_hierarchy_scale_shift_sd: hierarchy,
                maximum_expected_count_shift_sd: expected,
                material_sensitivity: material,
                fit: execution.result,
            });
        }
    }
    let complete = baseline.fit_state == FitState::Complete
        && outputs.iter().all(|row| row.fit["fit_state"] == "complete");
    let material = outputs.iter().any(|row| row.material_sensitivity);
    publish_json(
        &args.out,
        &Output {
            format: "marklab.replicated_arbitrary_window_multitype_lgcp_prior_kernel_sensitivity",
            version: 1,
            baseline_request_sha256: baseline.request_sha256,
            scenarios: outputs,
            materiality_standard_deviations: args.materiality_standard_deviations,
            material_sensitivity: material,
            fit_state: if complete {
                FitState::Complete
            } else {
                FitState::Nonconverged
            },
            resources: Resources {
                scenario_count: 8,
                maximum_processes: args.maximum_processes,
                per_scenario_draw_node_type_work: per_scenario_work,
                total_draw_node_type_work: total_work,
                maximum_total_draw_node_type_work: args.maximum_total_draw_node_type_work,
                timeout_seconds_per_process: args.timeout_seconds,
            },
            statistical_unit: "patient",
            assumptions: [
                "same_exact_data_types_windows_seed_likelihood_and_diagnostic_thresholds",
                "patient_pattern_and_fixed_kernel_controls_vary_one_at_a_time",
                "baseline_is_the_exact_typed_pymc_result",
                "scenario_refits_use_the_independent_numpyro_implementation",
                "materiality_is_standardized_by_baseline_pymc_posterior_sd",
            ],
            finite_result_policy: "every_scenario_summary_and_diagnostic_must_be_finite",
            claim_status: "experimental_fixed_hierarchy_kernel_sensitivity",
        },
    )
}

fn prepare(
    args: &Args,
    patient_scale: f64,
    pattern_scale: f64,
    field_amplitude: f64,
    field_length: f64,
) -> Result<Prepared, BayesCliError> {
    replicated_arbitrary_window_multitype_lgcp::prepare(FitArgs {
        input: args.input.clone(),
        reference_group: args.reference_group.clone(),
        comparison_group: args.comparison_group.clone(),
        reference_type: args.reference_type.clone(),
        intercept_prior_mean: args.intercept_prior_mean,
        intercept_prior_sd: args.intercept_prior_sd,
        group_effect_prior_sd: args.group_effect_prior_sd,
        covariate_effect_prior_sd: args.covariate_effect_prior_sd,
        patient_sd_prior_scale: patient_scale,
        pattern_sd_prior_scale: pattern_scale,
        field_amplitude,
        field_length_scale_um: field_length,
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
    })
}

fn validate_scenario(
    result: &ScenarioView,
    baseline: &BaselineView,
    prepared: &Prepared,
    request_sha: &str,
    backend: &BackendContract,
    depth: u32,
) -> Result<(), BayesCliError> {
    let policy = DiagnosticPolicy::default();
    let complete = diagnostics_pass(&result.diagnostics, &policy);
    let identities = row_identities_match(baseline, result);
    if result.format != "marklab.numpyro_replicated_arbitrary_window_multitype_lgcp_result"
        || result.version != 1
        || !backend_matches(&result.backend, backend)
        || result.jax_version != JAX_VERSION
        || result.input_sha256 != baseline.input_sha256
        || result.request_sha256 != request_sha
        || result.source_request_sha256 != prepared.request_sha256()
        || result.maximum_tree_depth != depth
        || !identities
        || !scenario_summaries_valid(result)
        || (result.fit_state == FitState::Complete) != complete
    {
        return Err(BayesCliError::Backend(
            "replicated multitype LGCP sensitivity result differs".into(),
        ));
    }
    Ok(())
}

fn scenario_summaries_valid(result: &ScenarioView) -> bool {
    result
        .type_posteriors
        .iter()
        .flat_map(|row| {
            [
                &row.intercept,
                &row.group_effect,
                &row.covariate_effect,
                &row.patient_sd,
                &row.pattern_sd,
            ]
        })
        .chain(
            result
                .group_effect_differences
                .iter()
                .map(|row| &row.difference),
        )
        .chain(result.patient_type_effects.iter().map(|row| &row.effect))
        .chain(result.pattern_type_effects.iter().map(|row| &row.effect))
        .chain(
            result
                .node_type_posteriors
                .iter()
                .flat_map(|row| [&row.latent_effect, &row.expected_count]),
        )
        .all(summary_valid)
}

fn row_identities_match(left: &BaselineView, right: &ScenarioView) -> bool {
    left.type_posteriors.len() == right.type_posteriors.len()
        && left
            .type_posteriors
            .iter()
            .zip(&right.type_posteriors)
            .all(|(a, b)| a.type_id == b.type_id)
        && left.group_effect_differences.len() == right.group_effect_differences.len()
        && left
            .group_effect_differences
            .iter()
            .zip(&right.group_effect_differences)
            .all(|(a, b)| a.type_a == b.type_a && a.type_b == b.type_b)
        && left.patient_type_effects.len() == right.patient_type_effects.len()
        && left
            .patient_type_effects
            .iter()
            .zip(&right.patient_type_effects)
            .all(|(a, b)| a.owner_id == b.owner_id && a.type_id == b.type_id)
        && left.pattern_type_effects.len() == right.pattern_type_effects.len()
        && left
            .pattern_type_effects
            .iter()
            .zip(&right.pattern_type_effects)
            .all(|(a, b)| a.owner_id == b.owner_id && a.type_id == b.type_id)
        && left.node_type_posteriors.len() == right.node_type_posteriors.len()
        && left
            .node_type_posteriors
            .iter()
            .zip(&right.node_type_posteriors)
            .all(|(a, b)| {
                a.pattern_id == b.pattern_id && a.node_id == b.node_id && a.type_id == b.type_id
            })
}

fn maximum_shift<'a>(
    baseline: impl Iterator<Item = &'a SarScalarSummary>,
    scenario: impl Iterator<Item = &'a SarScalarSummary>,
) -> f64 {
    baseline
        .zip(scenario)
        .map(|(baseline, scenario)| {
            (baseline.mean - scenario.mean).abs() / baseline.sd.max(f64::MIN_POSITIVE)
        })
        .fold(0.0_f64, f64::max)
}

fn diagnostics_pass(value: &NormalMeanDiagnostics, policy: &DiagnosticPolicy) -> bool {
    value.prior_predictive_finite
        && value.posterior_finite
        && value.constraints_valid
        && value.identifiability_checks_passed
        && value.r_hat <= policy.maximum_r_hat
        && value.ess_bulk >= policy.minimum_bulk_ess
        && value.ess_tail >= policy.minimum_tail_ess
        && value.minimum_ebfmi >= policy.minimum_ebfmi
        && value.divergences <= policy.maximum_divergences
        && value.max_tree_depth_hits <= policy.maximum_tree_depth_hits
}

fn read_bounded(path: &std::path::Path) -> Result<Vec<u8>, BayesCliError> {
    super::input_file::read_regular_file(
        path,
        MAXIMUM_RESULT_BYTES,
        "PyMC multitype LGCP result is absent or oversized",
    )
}
