pub(crate) use marklab_bayes::protocol::replicated_conditional_multitype_mark_sensitivity::{
    maximum_shift, validate_scenario, BaselineView, Output, Resources, ScenarioOutput, ScenarioView,
};

use std::path::PathBuf;
use std::thread;

use marklab_bayes::FitState;

use super::publish_json;
use super::replicated_conditional_multitype_mark;
use super::replicated_conditional_multitype_mark::Args as FitArgs;
use super::replicated_conditional_multitype_mark::Output as FitOutput;
use super::replicated_conditional_multitype_mark_numpyro;
use super::CliError;
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
    /// Upper bound on processes.
    #[arg(long)]
    maximum_processes: usize,
    /// Upper bound on total draw parameter work.
    #[arg(long)]
    maximum_total_draw_parameter_work: u64,
    /// Posterior change, in standard-deviation units, considered material.
    #[arg(long)]
    materiality_standard_deviations: f64,
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
        || !(1..=4).contains(&args.maximum_processes)
        || args.maximum_total_draw_parameter_work == 0
        || !args.materiality_standard_deviations.is_finite()
        || args.materiality_standard_deviations <= 0.0
    {
        return Err(CliError::Input(
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
    )
    .map_err(super::protocol_error)?;
    let baseline: BaselineView = serde_json::from_value(serde_json::to_value(&pymc)?)?;
    if baseline.request_sha256 != baseline_prepared.request_sha256() {
        return Err(CliError::Input(
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
        .ok_or_else(|| CliError::Input("sensitivity parameter work overflows".into()))?;
    let per_scenario_work = u64::from(args.chains)
        .checked_mul(u64::from(args.draws))
        .and_then(|value| value.checked_mul(parameters as u64))
        .ok_or_else(|| CliError::Input("sensitivity draw work overflows".into()))?;
    let total_work = per_scenario_work
        .checked_mul(4)
        .ok_or_else(|| CliError::Input("total sensitivity work overflows".into()))?;
    if total_work > args.maximum_total_draw_parameter_work {
        return Err(CliError::Input(format!(
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
        .collect::<Result<Vec<_>, CliError>>()?;
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
                        CliError::Backend(
                            "replicated conditional-mark sensitivity worker panicked".into(),
                        )
                    })?
                })
                .collect::<Result<Vec<_>, CliError>>()
        })?;
        for (scenario, execution) in wave.iter().zip(executions) {
            let view: ScenarioView = serde_json::from_value(execution.result.clone())?;
            validate_scenario(
                &view,
                &baseline,
                scenario.prepared.request_sha256(),
                &execution.request_sha256,
                &execution.backend,
                args.numpyro_maximum_tree_depth,
            )
            .map_err(super::protocol_error)?;
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
            statistical_unit: StatisticalUnit::Patient,
            assumptions: [
                "same_fixed_graphs_data_seed_likelihood_and_diagnostic_thresholds",
                "patient_and_pattern_scale_priors_vary_one_at_a_time",
                "baseline_is_the_exact_typed_pymc_result",
                "scenario_refits_use_the_independent_numpyro_implementation",
                "materiality_is_standardized_by_baseline_pymc_pseudoposterior_sd",
            ],
            finite_result_policy: "every_scenario_summary_and_diagnostic_must_be_finite",
            claim_status: ClaimStatus::ExperimentalFixedHierarchyPriorSensitivity,
        },
    )
}

fn prepare(
    args: &Args,
    patient_scale: f64,
    pattern_scale: f64,
) -> Result<replicated_conditional_multitype_mark::Prepared, CliError> {
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

fn read_bounded(path: &std::path::Path, maximum: u64) -> Result<Vec<u8>, CliError> {
    crate::input_file::read_regular_file(path, maximum, "sensitivity input is absent or oversized")
}

struct ScenarioSpec {
    name: &'static str,
    patient_scale: f64,
    pattern_scale: f64,
    prepared: replicated_conditional_multitype_mark::Prepared,
}
