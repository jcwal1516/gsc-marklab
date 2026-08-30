use std::{path::PathBuf, thread};

use clap::{Parser, Subcommand};
use marklab_bayes::{
    BackendContract, FitState, NormalMeanDiagnostics, SamplingSummary, SarScalarSummary,
    WorkerBackend,
};
use serde::{Deserialize, Serialize};

use super::{
    publish_json,
    replicated_arbitrary_window_lgcp_agreement::{backend_matches, summary_valid},
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
    ReplicatedConditionalMultitypeMarkSensitivity(Box<Args>),
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
    maximum_processes: usize,
    #[arg(long)]
    maximum_total_draw_parameter_work: u64,
    #[arg(long)]
    materiality_standard_deviations: f64,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Debug, Deserialize)]
struct SummaryRow {
    contrast: SarScalarSummary,
}

#[derive(Debug, Deserialize)]
struct ScaleRow {
    scale: SarScalarSummary,
}

#[derive(Debug, Deserialize)]
struct BaselineView {
    input_sha256: String,
    request_sha256: String,
    fit_state: FitState,
    sampling: SamplingSummary,
    patient_count: usize,
    pattern_count: usize,
    type_count: usize,
    group_affinity_shifts: Vec<SummaryRow>,
    patient_affinities: Vec<SummaryRow>,
    hierarchy_scales: Vec<ScaleRow>,
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
    sampling: SamplingSummary,
    group_affinity_shifts: Vec<SummaryRow>,
    patient_affinities: Vec<SummaryRow>,
    hierarchy_scales: Vec<ScaleRow>,
    diagnostics: NormalMeanDiagnostics,
}

struct ScenarioSpec {
    name: &'static str,
    patient_scale: f64,
    pattern_scale: f64,
    prepared: replicated_conditional_multitype_mark::Prepared,
}

#[derive(Debug, Serialize)]
struct ScenarioOutput {
    scenario: &'static str,
    patient_sd_prior_scale: f64,
    pattern_sd_prior_scale: f64,
    maximum_group_affinity_shift_sd: f64,
    maximum_patient_affinity_shift_sd: f64,
    maximum_hierarchy_scale_shift_sd: f64,
    material_prior_sensitivity: bool,
    fit: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct Resources {
    scenario_count: usize,
    maximum_processes: usize,
    per_scenario_draw_parameter_work: u64,
    total_draw_parameter_work: u64,
    maximum_total_draw_parameter_work: u64,
    timeout_seconds_per_process: u64,
}

#[derive(Debug, Serialize)]
struct Output {
    format: &'static str,
    version: u32,
    baseline_request_sha256: String,
    scenarios: Vec<ScenarioOutput>,
    materiality_standard_deviations: f64,
    material_prior_sensitivity: bool,
    fit_state: FitState,
    resources: Resources,
    statistical_unit: &'static str,
    assumptions: [&'static str; 5],
    finite_result_policy: &'static str,
    claim_status: &'static str,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let Top::Bayes { command } = Cli::parse_from(std::env::args_os()).command;
    let Command::ReplicatedConditionalMultitypeMarkSensitivity(args) = command;
    run(*args)
}

fn run(args: Args) -> Result<(), BayesCliError> {
    if !(10..=14).contains(&args.pymc_maximum_tree_depth)
        || !(10..=14).contains(&args.numpyro_maximum_tree_depth)
        || !(1..=4).contains(&args.maximum_processes)
        || args.maximum_total_draw_parameter_work == 0
        || !args.materiality_standard_deviations.is_finite()
        || args.materiality_standard_deviations <= 0.0
    {
        return Err(BayesCliError::Input(
            "replicated conditional-mark sensitivity controls are invalid".into(),
        ));
    }
    let input_bytes = read_bounded(&args.input, 64 * 1_048_576)?;
    let input_sha256 = marklab_bayes::sha256_hex(&input_bytes);
    let backend = replicated_conditional_multitype_mark::backend_contract()?;
    let baseline_prepared = prepare(
        &args,
        args.patient_sd_prior_scale,
        args.pattern_sd_prior_scale,
    )?;
    let pymc_bytes = read_bounded(&args.pymc_result, MAXIMUM_RESULT_BYTES)?;
    let pymc: FitOutput = serde_json::from_slice(&pymc_bytes)?;
    pymc.validate_for(
        &input_sha256,
        &args.reference_group,
        &args.reference_type,
        args.radius_um,
        &backend,
    )?;
    let baseline: BaselineView = serde_json::from_value(serde_json::to_value(&pymc)?)?;
    if baseline.request_sha256 != baseline_prepared.request_sha256() {
        return Err(BayesCliError::Input(
            "PyMC baseline does not match the exact sensitivity request".into(),
        ));
    }
    let free = baseline.type_count - 1 + baseline.type_count * (baseline.type_count + 1) / 2 - 1;
    let parameters = 2_usize
        .checked_mul(free)
        .and_then(|value| {
            value.checked_add((baseline.patient_count + baseline.pattern_count) * free)
        })
        .and_then(|value| value.checked_add(4))
        .ok_or_else(|| BayesCliError::Input("sensitivity parameter work overflows".into()))?;
    let per_scenario_work = u64::from(args.chains)
        .checked_mul(u64::from(args.draws))
        .and_then(|value| value.checked_mul(parameters as u64))
        .ok_or_else(|| BayesCliError::Input("sensitivity draw work overflows".into()))?;
    let total_work = per_scenario_work
        .checked_mul(4)
        .ok_or_else(|| BayesCliError::Input("total sensitivity work overflows".into()))?;
    if total_work > args.maximum_total_draw_parameter_work {
        return Err(BayesCliError::Input(format!(
            "total sensitivity draw work exceeds maximum: {total_work} > {}",
            args.maximum_total_draw_parameter_work
        )));
    }

    let scenario_values = [
        (
            "patient_scale_half",
            args.patient_sd_prior_scale * 0.5,
            args.pattern_sd_prior_scale,
        ),
        (
            "patient_scale_double",
            args.patient_sd_prior_scale * 2.0,
            args.pattern_sd_prior_scale,
        ),
        (
            "pattern_scale_half",
            args.patient_sd_prior_scale,
            args.pattern_sd_prior_scale * 0.5,
        ),
        (
            "pattern_scale_double",
            args.patient_sd_prior_scale,
            args.pattern_sd_prior_scale * 2.0,
        ),
    ];
    let scenarios = scenario_values
        .into_iter()
        .map(|(name, patient_scale, pattern_scale)| {
            Ok(ScenarioSpec {
                name,
                patient_scale,
                pattern_scale,
                prepared: prepare(&args, patient_scale, pattern_scale)?,
            })
        })
        .collect::<Result<Vec<_>, BayesCliError>>()?;
    let mut outputs = Vec::with_capacity(scenarios.len());
    for wave in scenarios.chunks(args.maximum_processes) {
        let executions = thread::scope(|scope| {
            let handles = wave
                .iter()
                .map(|scenario| {
                    scope.spawn(|| {
                        replicated_conditional_multitype_mark_numpyro::execute(
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
                            "replicated conditional-mark sensitivity worker panicked".into(),
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
            let group_shift = maximum_shift(
                baseline
                    .group_affinity_shifts
                    .iter()
                    .map(|row| &row.contrast),
                view.group_affinity_shifts.iter().map(|row| &row.contrast),
            );
            let patient_shift = maximum_shift(
                baseline.patient_affinities.iter().map(|row| &row.contrast),
                view.patient_affinities.iter().map(|row| &row.contrast),
            );
            let hierarchy_shift = maximum_shift(
                baseline.hierarchy_scales.iter().map(|row| &row.scale),
                view.hierarchy_scales.iter().map(|row| &row.scale),
            );
            outputs.push(ScenarioOutput {
                scenario: scenario.name,
                patient_sd_prior_scale: scenario.patient_scale,
                pattern_sd_prior_scale: scenario.pattern_scale,
                maximum_group_affinity_shift_sd: group_shift,
                maximum_patient_affinity_shift_sd: patient_shift,
                maximum_hierarchy_scale_shift_sd: hierarchy_shift,
                material_prior_sensitivity: [group_shift, patient_shift, hierarchy_shift]
                    .into_iter()
                    .any(|value| value >= args.materiality_standard_deviations),
                fit: execution.result,
            });
        }
    }
    let material = outputs.iter().any(|row| row.material_prior_sensitivity);
    let complete = baseline.fit_state == FitState::Complete
        && outputs.iter().all(|row| row.fit["fit_state"] == "complete");
    publish_json(
        &args.out,
        &Output {
            format: "marklab.replicated_conditional_multitype_mark_prior_sensitivity",
            version: 1,
            baseline_request_sha256: baseline.request_sha256,
            scenarios: outputs,
            materiality_standard_deviations: args.materiality_standard_deviations,
            material_prior_sensitivity: material,
            fit_state: if complete {
                FitState::Complete
            } else {
                FitState::Nonconverged
            },
            resources: Resources {
                scenario_count: 4,
                maximum_processes: args.maximum_processes,
                per_scenario_draw_parameter_work: per_scenario_work,
                total_draw_parameter_work: total_work,
                maximum_total_draw_parameter_work: args.maximum_total_draw_parameter_work,
                timeout_seconds_per_process: args.timeout_seconds,
            },
            statistical_unit: "patient",
            assumptions: [
                "same_fixed_graphs_data_seed_likelihood_and_diagnostic_thresholds",
                "patient_and_pattern_scale_priors_vary_one_at_a_time",
                "baseline_is_the_exact_typed_pymc_result",
                "scenario_refits_use_the_independent_numpyro_implementation",
                "materiality_is_standardized_by_baseline_pymc_pseudoposterior_sd",
            ],
            finite_result_policy: "every_scenario_summary_and_diagnostic_must_be_finite",
            claim_status: "experimental_fixed_hierarchy_prior_sensitivity",
        },
    )
}

fn prepare(
    args: &Args,
    patient_scale: f64,
    pattern_scale: f64,
) -> Result<replicated_conditional_multitype_mark::Prepared, BayesCliError> {
    replicated_conditional_multitype_mark::prepare(FitArgs {
        input: args.input.clone(),
        reference_group: args.reference_group.clone(),
        reference_type: args.reference_type.clone(),
        radius_um: args.radius_um,
        intercept_prior_sd: args.intercept_prior_sd,
        interaction_prior_sd: args.interaction_prior_sd,
        group_effect_prior_sd: args.group_effect_prior_sd,
        patient_sd_prior_scale: patient_scale,
        pattern_sd_prior_scale: pattern_scale,
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
    })
}

fn validate_scenario(
    result: &ScenarioView,
    baseline: &BaselineView,
    prepared: &replicated_conditional_multitype_mark::Prepared,
    request_sha256: &str,
    backend: &BackendContract,
    depth: u32,
) -> Result<(), BayesCliError> {
    let finite = result
        .group_affinity_shifts
        .iter()
        .chain(&result.patient_affinities)
        .map(|row| &row.contrast)
        .chain(result.hierarchy_scales.iter().map(|row| &row.scale))
        .all(summary_valid);
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
        || result.input_sha256 != baseline.input_sha256
        || result.request_sha256 != request_sha256
        || result.source_request_sha256 != prepared.request_sha256()
        || result.maximum_tree_depth != depth
        || result.sampling.chains != baseline.sampling.chains
        || result.sampling.tune_per_chain != baseline.sampling.tune_per_chain
        || result.sampling.draws_per_chain != baseline.sampling.draws_per_chain
        || result.sampling.completed_draws != baseline.sampling.completed_draws
        || result.group_affinity_shifts.len() != baseline.group_affinity_shifts.len()
        || result.patient_affinities.len() != baseline.patient_affinities.len()
        || result.hierarchy_scales.len() != baseline.hierarchy_scales.len()
        || !finite
        || (result.fit_state == FitState::Complete) != complete
    {
        return Err(BayesCliError::Backend(
            "replicated conditional-mark sensitivity scenario differs".into(),
        ));
    }
    Ok(())
}

fn maximum_shift<'a>(
    baseline: impl Iterator<Item = &'a SarScalarSummary>,
    scenario: impl Iterator<Item = &'a SarScalarSummary>,
) -> f64 {
    baseline
        .zip(scenario)
        .map(|(baseline, scenario)| {
            (scenario.mean - baseline.mean).abs() / baseline.sd.max(f64::MIN_POSITIVE)
        })
        .fold(0.0, f64::max)
}

fn read_bounded(path: &std::path::Path, maximum: u64) -> Result<Vec<u8>, BayesCliError> {
    super::input_file::read_regular_file(path, maximum, "sensitivity input is absent or oversized")
}
