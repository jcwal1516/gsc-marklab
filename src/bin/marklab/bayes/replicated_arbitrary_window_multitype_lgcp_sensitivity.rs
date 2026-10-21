pub(crate) use marklab_bayes::protocol::replicated_arbitrary_window_multitype_lgcp_sensitivity::{
    maximum_shift, validate_scenario, BaselineView, Output, Resources, ScenarioOutput, ScenarioView,
};

use std::path::PathBuf;
use std::thread;

use marklab_bayes::FitState;

use super::publish_json;
use super::replicated_arbitrary_window_multitype_lgcp;
use super::replicated_arbitrary_window_multitype_lgcp::Args as FitArgs;
use super::replicated_arbitrary_window_multitype_lgcp::Output as FitOutput;
use super::replicated_arbitrary_window_multitype_lgcp::Prepared;
use super::replicated_arbitrary_window_multitype_lgcp_numpyro;
use super::CliError;
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
    /// Maximum NUTS tree depth for the NumPyro fit.
    #[arg(long)]
    numpyro_maximum_tree_depth: u32,
    /// Upper bound on processes.
    #[arg(long)]
    maximum_processes: usize,
    /// Upper bound on total draw node type work.
    #[arg(long)]
    maximum_total_draw_node_type_work: u64,
    /// Posterior change, in standard-deviation units, considered material.
    #[arg(long)]
    materiality_standard_deviations: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    out: PathBuf,
}

pub(super) fn run(args: Args) -> Result<(), CliError> {
    if !(10..=14).contains(&args.numpyro_maximum_tree_depth)
        || !(1..=4).contains(&args.maximum_processes)
        || args.maximum_total_draw_node_type_work == 0
        || !args.materiality_standard_deviations.is_finite()
        || args.materiality_standard_deviations <= 0.0
    {
        return Err(CliError::Input(
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
    baseline_prepared.validate_result(&pymc)?;
    let baseline: BaselineView = serde_json::from_value(serde_json::to_value(&pymc)?)?;
    if baseline.request_sha256 != baseline_prepared.request_sha256() {
        return Err(CliError::Input(
            "PyMC baseline does not match exact sensitivity request".into(),
        ));
    }
    let per_scenario_work = baseline_prepared.draw_node_type_work();
    let total_work = per_scenario_work
        .checked_mul(8)
        .ok_or_else(|| CliError::Input("total sensitivity work overflows".into()))?;
    if total_work > args.maximum_total_draw_node_type_work {
        return Err(CliError::Input(format!(
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
        .collect::<Result<Vec<_>, CliError>>()?;
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
                        CliError::Backend(
                            "replicated multitype LGCP sensitivity worker panicked".into(),
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
            statistical_unit: StatisticalUnit::Patient,
            assumptions: [
                "same_exact_data_types_windows_seed_likelihood_and_diagnostic_thresholds",
                "patient_pattern_and_fixed_kernel_controls_vary_one_at_a_time",
                "baseline_is_the_exact_typed_pymc_result",
                "scenario_refits_use_the_independent_numpyro_implementation",
                "materiality_is_standardized_by_baseline_pymc_posterior_sd",
            ],
            finite_result_policy: "every_scenario_summary_and_diagnostic_must_be_finite",
            claim_status: ClaimStatus::ExperimentalFixedHierarchyKernelSensitivity,
        },
    )
}

fn prepare(
    args: &Args,
    patient_scale: f64,
    pattern_scale: f64,
    field_amplitude: f64,
    field_length: f64,
) -> Result<Prepared, CliError> {
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

fn read_bounded(path: &std::path::Path) -> Result<Vec<u8>, CliError> {
    crate::input_file::read_regular_file(
        path,
        MAXIMUM_RESULT_BYTES,
        "PyMC multitype LGCP result is absent or oversized",
    )
}

struct ScenarioSpec {
    name: &'static str,
    patient_scale: f64,
    pattern_scale: f64,
    field_amplitude: f64,
    field_length: f64,
    prepared: Prepared,
}
