pub(crate) use marklab_bayes::protocol::replicated_arbitrary_window_lgcp_sensitivity::{
    scenario, standardized_shifts, ScenarioPriors, SensitivityResult,
};

use std::path::PathBuf;

use marklab_bayes::FitState;
use marklab_bayes::NutsSamplingSpec;

use super::publish_json;
use super::replicated_arbitrary_window_lgcp_fit;
use super::CliError;
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
    /// Upper bound on total draw node work.
    #[arg(long)]
    maximum_total_draw_node_work: u64,
    /// Smallest standardized posterior change considered material.
    #[arg(long)]
    material_standardized_shift: f64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    timeout_seconds: u64,
    /// Destination for the analysis output.
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

pub(super) fn run(arguments: Arguments) -> Result<(), CliError> {
    if !arguments.material_standardized_shift.is_finite()
        || arguments.material_standardized_shift <= 0.0
        || arguments.maximum_total_draw_node_work == 0
        || arguments.maximum_total_draw_node_work > 1_000_000_000
    {
        return Err(CliError::Input(
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
        .ok_or_else(|| CliError::Input("replicated LGCP sensitivity work overflows".into()))?;
    let total_draw_node_work = per_fit_work
        .checked_mul(definitions.len() as u64)
        .ok_or_else(|| {
            CliError::Input("replicated LGCP total sensitivity work overflows".into())
        })?;
    if total_draw_node_work > arguments.maximum_total_draw_node_work {
        return Err(CliError::Input(format!(
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
            crate::bayes::ReplicatedArbitraryWindowLgcpFitArgs {
                input: arguments.input.clone(),
                reference_group: arguments.reference_group.clone(),
                comparison_group: arguments.comparison_group.clone(),
                intercept_prior_mean: arguments.intercept_prior_mean,
                intercept_prior_sd: arguments.intercept_prior_sd,
                group_effect_prior_sd: arguments.group_effect_prior_sd,
                covariate_effect_prior_sd: arguments.covariate_effect_prior_sd,
                patient_sd_prior_scale: patient_scale,
                pattern_sd_prior_scale: pattern_scale,
                field_amplitude: amplitude,
                field_length_scale_um: length,
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
                maximum_draw_node_work: per_fit_work,
                timeout_seconds: arguments.timeout_seconds,
                out: std::path::PathBuf::new(),
            },
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
        return Err(CliError::Input(
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
                    ScenarioPriors {
                        patient_sd_prior_scale: patient_scale,
                        pattern_sd_prior_scale: pattern_scale,
                        field_amplitude: amplitude,
                        field_length_scale_um: length,
                    },
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
            statistical_unit: StatisticalUnit::Patient,
            assumptions: vec![
                "fixed_prespecified_half_and_double_scale_grid".into(),
                "same_patients_patterns_windows_and_sampling_seed_in_every_scenario".into(),
                "patients_not_patterns_nodes_or_cells_are_population_replicates".into(),
            ],
            finite_result_policy: "all_scenarios_must_be_finite_and_diagnostically_complete".into(),
            claim_status: ClaimStatus::ExperimentalHierarchyAndKernelSensitivity,
        },
    )
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
