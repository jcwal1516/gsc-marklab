use std::path::PathBuf;

use clap::{Parser, Subcommand};
use marklab_bayes::{FitState, NutsSamplingSpec, SarScalarSummary};
use serde::Serialize;

use super::{
    publish_json,
    replicated_arbitrary_window_lgcp_fit::{self, ResultDocument},
    BayesCliError,
};

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct SensitivityCli {
    #[command(subcommand)]
    command: TopLevel,
}

#[derive(Debug, Subcommand)]
enum TopLevel {
    Bayes {
        #[command(subcommand)]
        command: Command,
    },
}

#[derive(Debug, Subcommand)]
enum Command {
    ReplicatedArbitraryWindowLgcpSensitivity(Box<Arguments>),
}

#[derive(Debug, clap::Args)]
struct Arguments {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    reference_group: String,
    #[arg(long)]
    comparison_group: String,
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
    maximum_nodes_per_pattern: usize,
    #[arg(long)]
    maximum_total_nodes: usize,
    #[arg(long)]
    maximum_total_events: u64,
    #[arg(long)]
    maximum_total_draw_node_work: u64,
    #[arg(long)]
    material_standardized_shift: f64,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Clone, Copy)]
struct Definition {
    name: &'static str,
    patient_scale_multiplier: f64,
    pattern_scale_multiplier: f64,
    amplitude_multiplier: f64,
    length_multiplier: f64,
}

#[derive(Debug, Serialize)]
struct Scenario {
    name: String,
    patient_sd_prior_scale: f64,
    pattern_sd_prior_scale: f64,
    field_amplitude: f64,
    field_length_scale_um: f64,
    fit: ResultDocument,
    intercept_standardized_shift: f64,
    group_effect_standardized_shift: f64,
    covariate_effect_standardized_shift: f64,
    patient_sd_standardized_shift: f64,
    pattern_sd_standardized_shift: f64,
    patient_effect_rms_standardized_shift: f64,
    pattern_effect_rms_standardized_shift: f64,
    maximum_standardized_shift: f64,
    material_change: bool,
}

#[derive(Debug, Serialize)]
struct SensitivityResult {
    format: String,
    version: u32,
    scenarios: Vec<Scenario>,
    all_fits_complete: bool,
    maximum_standardized_shift: f64,
    material_standardized_shift: f64,
    material_scenarios: Vec<String>,
    total_draw_node_work: u64,
    statistical_unit: String,
    assumptions: Vec<String>,
    finite_result_policy: String,
    claim_status: String,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let TopLevel::Bayes { command } = SensitivityCli::parse_from(std::env::args_os()).command;
    let Command::ReplicatedArbitraryWindowLgcpSensitivity(arguments) = command;
    run(*arguments)
}

fn run(arguments: Arguments) -> Result<(), BayesCliError> {
    if !arguments.material_standardized_shift.is_finite()
        || arguments.material_standardized_shift <= 0.0
        || arguments.maximum_total_draw_node_work == 0
        || arguments.maximum_total_draw_node_work > 1_000_000_000
    {
        return Err(BayesCliError::Input(
            "replicated LGCP sensitivity controls are invalid".into(),
        ));
    }
    let definitions = [
        Definition::baseline(),
        Definition::patient_scale("patient_sd_prior_lower", 0.5),
        Definition::patient_scale("patient_sd_prior_upper", 2.0),
        Definition::pattern_scale("pattern_sd_prior_lower", 0.5),
        Definition::pattern_scale("pattern_sd_prior_upper", 2.0),
        Definition::amplitude("field_amplitude_lower", 0.5),
        Definition::amplitude("field_amplitude_upper", 2.0),
        Definition::length("field_length_scale_lower", 0.5),
        Definition::length("field_length_scale_upper", 2.0),
    ];
    let per_fit_work = u64::from(arguments.chains)
        .checked_mul(u64::from(arguments.draws))
        .and_then(|draws| draws.checked_mul(arguments.maximum_total_nodes as u64))
        .ok_or_else(|| BayesCliError::Input("replicated LGCP sensitivity work overflows".into()))?;
    let total_draw_node_work = per_fit_work
        .checked_mul(definitions.len() as u64)
        .ok_or_else(|| {
            BayesCliError::Input("replicated LGCP total sensitivity work overflows".into())
        })?;
    if total_draw_node_work > arguments.maximum_total_draw_node_work {
        return Err(BayesCliError::Input(format!(
            "replicated LGCP sensitivity work exceeds maximum: {total_draw_node_work} > {}",
            arguments.maximum_total_draw_node_work
        )));
    }
    let sampling = NutsSamplingSpec {
        chains: arguments.chains,
        tune_per_chain: arguments.tune,
        draws_per_chain: arguments.draws,
        target_accept: arguments.target_accept,
        seed: arguments.seed,
    };
    let mut fits = Vec::with_capacity(definitions.len());
    for definition in definitions {
        let patient_scale = arguments.patient_sd_prior_scale * definition.patient_scale_multiplier;
        let pattern_scale = arguments.pattern_sd_prior_scale * definition.pattern_scale_multiplier;
        let amplitude = arguments.field_amplitude * definition.amplitude_multiplier;
        let length = arguments.field_length_scale_um * definition.length_multiplier;
        let prepared = replicated_arbitrary_window_lgcp_fit::prepare(
            arguments.input.clone(),
            arguments.reference_group.clone(),
            arguments.comparison_group.clone(),
            arguments.intercept_prior_mean,
            arguments.intercept_prior_sd,
            arguments.group_effect_prior_sd,
            arguments.covariate_effect_prior_sd,
            patient_scale,
            pattern_scale,
            amplitude,
            length,
            arguments.jitter,
            sampling.clone(),
            arguments.maximum_patients,
            arguments.maximum_patterns,
            arguments.maximum_nodes_per_pattern,
            arguments.maximum_total_nodes,
            arguments.maximum_total_events,
            per_fit_work,
            arguments.timeout_seconds,
        )?;
        fits.push((
            definition,
            patient_scale,
            pattern_scale,
            amplitude,
            length,
            replicated_arbitrary_window_lgcp_fit::execute(&prepared)?,
        ));
    }
    let baseline = &fits[0].5;
    if fits
        .iter()
        .any(|fit| fit.5.input_sha256 != baseline.input_sha256)
    {
        return Err(BayesCliError::Input(
            "replicated LGCP input changed across sensitivity scenarios".into(),
        ));
    }
    let shifts = fits
        .iter()
        .map(|fit| standardized_shifts(&fit.5, baseline))
        .collect::<Vec<_>>();
    let scenarios = fits
        .into_iter()
        .zip(shifts)
        .map(
            |((definition, patient_scale, pattern_scale, amplitude, length, fit), shifts)| {
                scenario(
                    definition.name,
                    patient_scale,
                    pattern_scale,
                    amplitude,
                    length,
                    fit,
                    shifts,
                    arguments.material_standardized_shift,
                )
            },
        )
        .collect::<Vec<_>>();
    let all_fits_complete = scenarios
        .iter()
        .all(|scenario| scenario.fit.fit_state == FitState::Complete);
    let maximum_standardized_shift = scenarios
        .iter()
        .map(|scenario| scenario.maximum_standardized_shift)
        .fold(0.0_f64, f64::max);
    let material_scenarios = scenarios
        .iter()
        .filter(|scenario| scenario.material_change)
        .map(|scenario| scenario.name.clone())
        .collect();
    publish_json(
        &arguments.out,
        &SensitivityResult {
            format: "marklab.replicated_arbitrary_window_lgcp_sensitivity".into(),
            version: 1,
            scenarios,
            all_fits_complete,
            maximum_standardized_shift,
            material_standardized_shift: arguments.material_standardized_shift,
            material_scenarios,
            total_draw_node_work,
            statistical_unit: "patient".into(),
            assumptions: vec![
                "fixed_prespecified_half_and_double_scale_grid".into(),
                "same_patients_patterns_windows_and_sampling_seed_in_every_scenario".into(),
                "patients_not_patterns_nodes_or_cells_are_population_replicates".into(),
            ],
            finite_result_policy: "all_scenarios_must_be_finite_and_diagnostically_complete".into(),
            claim_status: "experimental_hierarchy_and_kernel_sensitivity".into(),
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn scenario(
    name: &str,
    patient_sd_prior_scale: f64,
    pattern_sd_prior_scale: f64,
    field_amplitude: f64,
    field_length_scale_um: f64,
    fit: ResultDocument,
    shifts: [f64; 7],
    threshold: f64,
) -> Scenario {
    let maximum_standardized_shift = shifts.into_iter().fold(0.0_f64, f64::max);
    Scenario {
        name: name.into(),
        patient_sd_prior_scale,
        pattern_sd_prior_scale,
        field_amplitude,
        field_length_scale_um,
        fit,
        intercept_standardized_shift: shifts[0],
        group_effect_standardized_shift: shifts[1],
        covariate_effect_standardized_shift: shifts[2],
        patient_sd_standardized_shift: shifts[3],
        pattern_sd_standardized_shift: shifts[4],
        patient_effect_rms_standardized_shift: shifts[5],
        pattern_effect_rms_standardized_shift: shifts[6],
        maximum_standardized_shift,
        material_change: maximum_standardized_shift >= threshold,
    }
}

fn standardized_shifts(fit: &ResultDocument, baseline: &ResultDocument) -> [f64; 7] {
    [
        scalar_shift(&fit.posterior.intercept, &baseline.posterior.intercept),
        scalar_shift(
            &fit.posterior.group_effect,
            &baseline.posterior.group_effect,
        ),
        scalar_shift(
            &fit.posterior.covariate_effect,
            &baseline.posterior.covariate_effect,
        ),
        scalar_shift(&fit.posterior.patient_sd, &baseline.posterior.patient_sd),
        scalar_shift(&fit.posterior.pattern_sd, &baseline.posterior.pattern_sd),
        effect_rms_shift(
            fit.patient_effects.iter().map(|value| &value.effect),
            baseline.patient_effects.iter().map(|value| &value.effect),
        ),
        effect_rms_shift(
            fit.pattern_effects.iter().map(|value| &value.effect),
            baseline.pattern_effects.iter().map(|value| &value.effect),
        ),
    ]
}

fn scalar_shift(value: &SarScalarSummary, baseline: &SarScalarSummary) -> f64 {
    (value.mean - baseline.mean).abs() / baseline.sd.max(f64::EPSILON)
}

fn effect_rms_shift<'a>(
    values: impl Iterator<Item = &'a SarScalarSummary>,
    baselines: impl Iterator<Item = &'a SarScalarSummary>,
) -> f64 {
    let pairs = values.zip(baselines).collect::<Vec<_>>();
    let difference = (pairs
        .iter()
        .map(|(value, baseline)| (value.mean - baseline.mean).powi(2))
        .sum::<f64>()
        / pairs.len() as f64)
        .sqrt();
    let scale = (pairs
        .iter()
        .map(|(_, baseline)| baseline.sd.powi(2))
        .sum::<f64>()
        / pairs.len() as f64)
        .sqrt()
        .max(f64::EPSILON);
    difference / scale
}

impl Definition {
    const fn baseline() -> Self {
        Self {
            name: "baseline",
            patient_scale_multiplier: 1.0,
            pattern_scale_multiplier: 1.0,
            amplitude_multiplier: 1.0,
            length_multiplier: 1.0,
        }
    }

    const fn patient_scale(name: &'static str, multiplier: f64) -> Self {
        Self {
            name,
            patient_scale_multiplier: multiplier,
            ..Self::baseline()
        }
    }

    const fn pattern_scale(name: &'static str, multiplier: f64) -> Self {
        Self {
            name,
            pattern_scale_multiplier: multiplier,
            ..Self::baseline()
        }
    }

    const fn amplitude(name: &'static str, multiplier: f64) -> Self {
        Self {
            name,
            amplitude_multiplier: multiplier,
            ..Self::baseline()
        }
    }

    const fn length(name: &'static str, multiplier: f64) -> Self {
        Self {
            name,
            length_multiplier: multiplier,
            ..Self::baseline()
        }
    }
}

pub(super) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<SensitivityCli>(|| {
        run_cli().map_err(super::into_marklab_error)
    })
}
