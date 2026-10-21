pub(crate) use marklab_bayes::protocol::conditional_multitype_mark_sensitivity::{
    Output, Scenario, View,
};

use std::path::PathBuf;

use marklab_bayes::FitState;

use super::conditional_multitype_mark;
use super::conditional_multitype_mark::Args as FitArgs;
use super::publish_json;
use super::CliError;
use marklab_bayes::SensitivityStatus;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;

#[derive(Debug, clap::Args)]
pub(crate) struct Args {
    /// CSV of points: point_id or cell_id, x_um, y_um, and type_id or histologic_compartment.
    #[arg(long)]
    input: PathBuf,
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
    /// Upper bound on total draw parameter work.
    #[arg(long)]
    maximum_total_draw_parameter_work: u64,
    /// Upper bound on working bytes.
    #[arg(long)]
    maximum_working_bytes: usize,
    /// Upper bound on tree depth.
    #[arg(long)]
    maximum_tree_depth: u32,
    /// Posterior change, in standard-deviation units, considered material.
    #[arg(long)]
    materiality_threshold_sd: f64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    out: PathBuf,
}

pub(super) fn run(args: Args) -> Result<(), CliError> {
    if !args.materiality_threshold_sd.is_finite() || args.materiality_threshold_sd <= 0.0 {
        return Err(CliError::Input(
            "conditional multitype sensitivity threshold is invalid".into(),
        ));
    }
    let conservative_parameters = args
        .maximum_types
        .checked_sub(1)
        .and_then(|value| {
            args.maximum_types
                .checked_mul(args.maximum_types + 1)
                .and_then(|pairs| pairs.checked_div(2))
                .and_then(|pairs| value.checked_add(pairs.checked_sub(1)?))
        })
        .ok_or_else(|| CliError::Input("sensitivity parameter work overflows".into()))?;
    let total_work = u64::from(args.chains)
        .checked_mul(u64::from(args.draws))
        .and_then(|value| value.checked_mul(conservative_parameters as u64))
        .and_then(|value| value.checked_mul(5))
        .ok_or_else(|| CliError::Input("sensitivity total work overflows".into()))?;
    if total_work > args.maximum_total_draw_parameter_work {
        return Err(CliError::Input(format!(
            "sensitivity total work exceeds maximum: {total_work} > {}",
            args.maximum_total_draw_parameter_work
        )));
    }
    let specifications = [
        (
            "baseline",
            args.intercept_prior_sd,
            args.interaction_prior_sd,
        ),
        (
            "intercept_sd_lower",
            args.intercept_prior_sd * 0.5,
            args.interaction_prior_sd,
        ),
        (
            "intercept_sd_upper",
            args.intercept_prior_sd * 2.0,
            args.interaction_prior_sd,
        ),
        (
            "interaction_sd_lower",
            args.intercept_prior_sd,
            args.interaction_prior_sd * 0.5,
        ),
        (
            "interaction_sd_upper",
            args.intercept_prior_sd,
            args.interaction_prior_sd * 2.0,
        ),
    ];
    let mut fitted = Vec::with_capacity(specifications.len());
    for (name, intercept_sd, interaction_sd) in specifications {
        let result = conditional_multitype_mark::execute(FitArgs {
            input: args.input.clone(),
            reference_type: args.reference_type.clone(),
            radius_um: args.radius_um,
            intercept_prior_sd: intercept_sd,
            interaction_prior_sd: interaction_sd,
            chains: args.chains,
            tune: args.tune,
            draws: args.draws,
            target_accept: args.target_accept,
            seed: args.seed,
            maximum_points: args.maximum_points,
            maximum_types: args.maximum_types,
            maximum_neighbor_visits: args.maximum_neighbor_visits,
            maximum_edges: args.maximum_edges,
            maximum_draw_parameter_work: args.maximum_draw_parameter_work,
            maximum_working_bytes: args.maximum_working_bytes,
            maximum_tree_depth: args.maximum_tree_depth,
            timeout_seconds: args.timeout_seconds,
            out: PathBuf::new(),
        })?;
        let view: View = serde_json::from_value(serde_json::to_value(&result)?)?;
        fitted.push((name, intercept_sd, interaction_sd, result, view));
    }
    let baseline_affinities = fitted[0]
        .4
        .pair_affinity_contrasts
        .iter()
        .map(|row| {
            (
                row.type_a.clone(),
                row.type_b.clone(),
                row.contrast.mean,
                row.contrast.sd,
            )
        })
        .collect::<Vec<_>>();
    let baseline_score_mean = fitted[0].4.comparison.spatial_composite_log_score.mean;
    let baseline_score_sd = fitted[0].4.comparison.spatial_composite_log_score.sd;
    let mut scenarios = Vec::with_capacity(fitted.len());
    let mut any_material = false;
    let mut all_complete = true;
    for (name, intercept_sd, interaction_sd, result, view) in fitted {
        let maximum_affinity_shift_sd = baseline_affinities
            .iter()
            .zip(&view.pair_affinity_contrasts)
            .map(|((type_a, type_b, mean, sd), current)| {
                if type_a != &current.type_a || type_b != &current.type_b {
                    f64::INFINITY
                } else {
                    (current.contrast.mean - *mean).abs() / sd.max(1e-12)
                }
            })
            .fold(0.0_f64, f64::max);
        let composite_log_score_shift_sd =
            (view.comparison.spatial_composite_log_score.mean - baseline_score_mean).abs()
                / baseline_score_sd.max(1e-12);
        let material = maximum_affinity_shift_sd > args.materiality_threshold_sd
            || composite_log_score_shift_sd > args.materiality_threshold_sd;
        any_material |= material;
        all_complete &= view.fit_state == FitState::Complete;
        scenarios.push(Scenario {
            scenario: name,
            intercept_prior_sd: intercept_sd,
            interaction_prior_sd: interaction_sd,
            maximum_affinity_shift_sd,
            composite_log_score_shift_sd,
            material,
            result,
        });
    }
    let complete = all_complete
        && scenarios.iter().all(|row| {
            row.maximum_affinity_shift_sd.is_finite()
                && row.composite_log_score_shift_sd.is_finite()
        });
    publish_json(
        &args.out,
        &Output {
            format: "marklab.conditional_multitype_mark_prior_sensitivity",
            version: 1,
            baseline_scenario: "baseline",
            materiality_threshold_sd: args.materiality_threshold_sd,
            maximum_total_draw_parameter_work: args.maximum_total_draw_parameter_work,
            scenarios,
            fit_state: if complete {
                FitState::Complete
            } else {
                FitState::Nonconverged
            },
            sensitivity_status: if !complete {
                SensitivityStatus::DiagnosticOnlyNonconverged
            } else if any_material {
                SensitivityStatus::MaterialPriorSensitivity
            } else {
                SensitivityStatus::StableBelowDeclaredThreshold
            },
            statistical_unit: StatisticalUnit::OneFixedLocationPattern,
            assumptions: [
                "fixed_half_base_double_one_at_a_time_prior_grid",
                "same_graph_seed_sampling_and_diagnostic_thresholds",
                "posterior_sd_standardization_uses_baseline_pseudoposterior",
                "one_pattern_does_not_support_patient_population_inference",
            ],
            finite_result_policy: "retain_every_scenario_and_require_finite_complete_fits",
            claim_status: ClaimStatus::ExperimentalPriorSensitivity,
        },
    )
}
