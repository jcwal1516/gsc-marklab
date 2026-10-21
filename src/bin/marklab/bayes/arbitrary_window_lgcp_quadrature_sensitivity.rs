pub(crate) use marklab_bayes::protocol::arbitrary_window_lgcp_quadrature_sensitivity::{
    Resolution, SensitivityResult,
};

use std::path::PathBuf;

use marklab_bayes::FitState;
use marklab_bayes::NutsSamplingSpec;

use super::arbitrary_window_lgcp_fit;
use super::publish_json;
use super::CliError;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;

#[derive(Debug, clap::Args)]
pub(crate) struct Arguments {
    /// Observed point events with coordinates and model covariates.
    #[arg(long)]
    events: PathBuf,
    /// Observation-window geometry used for spatial integration or edge correction.
    #[arg(long)]
    window: PathBuf,
    /// Coarse version of: table binding observed events to their declared components or quadrature cells.
    #[arg(long)]
    coarse_event_membership: PathBuf,
    /// Coarse version of: quadrature nodes, weights and covariates for spatial integration.
    #[arg(long)]
    coarse_quadrature: PathBuf,
    /// Intermediate version of: table binding observed events to their declared components or quadrature cells.
    #[arg(long)]
    intermediate_event_membership: PathBuf,
    /// Intermediate version of: quadrature nodes, weights and covariates for spatial integration.
    #[arg(long)]
    intermediate_quadrature: PathBuf,
    /// Baseline version of: table binding observed events to their declared components or quadrature cells.
    #[arg(long)]
    baseline_event_membership: PathBuf,
    /// Baseline version of: quadrature nodes, weights and covariates for spatial integration.
    #[arg(long)]
    baseline_quadrature: PathBuf,
    /// Prior mean for intercept.
    #[arg(long, allow_hyphen_values = true)]
    intercept_prior_mean: f64,
    /// Prior scale for intercept.
    #[arg(long)]
    intercept_prior_sd: f64,
    /// Prior mean for coefficient.
    #[arg(long, allow_hyphen_values = true)]
    coefficient_prior_mean: f64,
    /// Prior scale for coefficient.
    #[arg(long)]
    coefficient_prior_sd: f64,
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
    /// Upper bound on events.
    #[arg(long)]
    maximum_events: usize,
    /// Upper bound on quadrature nodes.
    #[arg(long)]
    maximum_quadrature_nodes: usize,
    /// Upper bound on total draw node work.
    #[arg(long)]
    maximum_total_draw_node_work: u64,
    /// Number of posterior predictive simulations.
    #[arg(long)]
    prediction_replicates: u32,
    /// Random seed for posterior predictive simulation.
    #[arg(long)]
    prediction_seed: u64,
    /// Upper bound on predictive points.
    #[arg(long)]
    maximum_predictive_points: u64,
    /// Neighborhood radius in micrometers.
    #[arg(long)]
    neighbor_radius_um: f64,
    /// Upper bound on neighbor pairs.
    #[arg(long)]
    maximum_neighbor_pairs: usize,
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

pub(super) fn run(arguments: Box<Arguments>) -> Result<(), CliError> {
    if !arguments.material_standardized_shift.is_finite()
        || arguments.material_standardized_shift <= 0.0
        || arguments.maximum_total_draw_node_work == 0
        || arguments.maximum_total_draw_node_work > 100_000_000
    {
        return Err(CliError::Input(
            "arbitrary-window LGCP quadrature sensitivity controls are invalid".into(),
        ));
    }
    let sampling = NutsSamplingSpec {
        chains: arguments.chains,
        tune_per_chain: arguments.tune,
        draws_per_chain: arguments.draws,
        target_accept: arguments.target_accept,
        seed: arguments.seed,
    };
    let definitions = [
        (
            "coarse",
            arguments.coarse_event_membership,
            arguments.coarse_quadrature,
        ),
        (
            "intermediate",
            arguments.intermediate_event_membership,
            arguments.intermediate_quadrature,
        ),
        (
            "baseline",
            arguments.baseline_event_membership,
            arguments.baseline_quadrature,
        ),
    ];
    let mut prepared = Vec::with_capacity(definitions.len());
    for (name, membership, quadrature) in definitions {
        prepared.push((
            name,
            arbitrary_window_lgcp_fit::prepare(crate::bayes::ArbitraryWindowLgcpFitArgs {
                events: arguments.events.clone(),
                event_membership: membership,
                quadrature,
                window: arguments.window.clone(),
                intercept_prior_mean: arguments.intercept_prior_mean,
                intercept_prior_sd: arguments.intercept_prior_sd,
                coefficient_prior_mean: arguments.coefficient_prior_mean,
                coefficient_prior_sd: arguments.coefficient_prior_sd,
                field_amplitude: arguments.field_amplitude,
                field_length_scale_um: arguments.field_length_scale_um,
                jitter: arguments.jitter,
                chains: sampling.chains,
                tune: sampling.tune_per_chain,
                draws: sampling.draws_per_chain,
                target_accept: sampling.target_accept,
                seed: sampling.seed,
                maximum_events: arguments.maximum_events,
                maximum_quadrature_nodes: arguments.maximum_quadrature_nodes,
                maximum_draw_node_work: arguments.maximum_total_draw_node_work,
                prediction_replicates: arguments.prediction_replicates,
                prediction_seed: arguments.prediction_seed,
                maximum_predictive_points: arguments.maximum_predictive_points,
                neighbor_radius_um: arguments.neighbor_radius_um,
                maximum_neighbor_pairs: arguments.maximum_neighbor_pairs,
                timeout_seconds: arguments.timeout_seconds,
                out: std::path::PathBuf::new(),
            })?,
        ));
    }
    let first = &prepared[0].1.request.input;
    if prepared.iter().any(|(_, prepared)| {
        prepared.request.input.events_digest != first.events_digest
            || prepared.request.input.window_logical_digest != first.window_logical_digest
    }) {
        return Err(CliError::Input(
            "LGCP quadrature sensitivity events or exact window changed across resolutions".into(),
        ));
    }
    let completed_draws = u64::from(arguments.chains) * u64::from(arguments.draws);
    let total_draw_node_work = prepared.iter().try_fold(0_u64, |work, (_, prepared)| {
        work.checked_add(completed_draws * prepared.request.nodes.len() as u64)
    });
    let total_draw_node_work = total_draw_node_work
        .ok_or_else(|| CliError::Input("LGCP quadrature work overflows".into()))?;
    if total_draw_node_work > arguments.maximum_total_draw_node_work {
        return Err(CliError::Input(format!(
            "LGCP quadrature work exceeds maximum: {total_draw_node_work} > {}",
            arguments.maximum_total_draw_node_work
        )));
    }
    let mut fits = Vec::with_capacity(prepared.len());
    for (name, prepared) in &prepared {
        fits.push((*name, arbitrary_window_lgcp_fit::execute(prepared)?));
    }
    let baseline = &fits[2].1;
    let baseline_intercept = baseline.posterior.intercept.mean;
    let baseline_intercept_sd = baseline.posterior.intercept.sd.max(f64::EPSILON);
    let baseline_coefficient = baseline.posterior.coefficient.mean;
    let baseline_coefficient_sd = baseline.posterior.coefficient.sd.max(f64::EPSILON);
    let baseline_total = baseline
        .nodes
        .iter()
        .map(|node| node.expected_count.mean)
        .sum::<f64>();
    let resolutions = fits
        .into_iter()
        .map(|(name, fit)| {
            let intercept_shift =
                (fit.posterior.intercept.mean - baseline_intercept).abs() / baseline_intercept_sd;
            let coefficient_shift = (fit.posterior.coefficient.mean - baseline_coefficient).abs()
                / baseline_coefficient_sd;
            let total = fit
                .nodes
                .iter()
                .map(|node| node.expected_count.mean)
                .sum::<f64>();
            Resolution {
                name: name.into(),
                quadrature_node_count: fit.nodes.len(),
                backend: fit.backend,
                source_backend: fit.source_backend,
                input: fit.input,
                request_sha256: fit.request_sha256,
                fit_state: fit.fit_state,
                posterior: fit.posterior,
                diagnostics: fit.diagnostics,
                spatial_posterior_predictive: fit.spatial_posterior_predictive,
                total_expected_count_mean: total,
                intercept_standardized_shift: intercept_shift,
                coefficient_standardized_shift: coefficient_shift,
                total_count_relative_shift: (total - baseline_total).abs() / baseline_total,
                maximum_standardized_shift: intercept_shift.max(coefficient_shift),
            }
        })
        .collect::<Vec<_>>();
    let all_fits_complete = resolutions
        .iter()
        .all(|resolution| resolution.fit_state == FitState::Complete);
    let maximum_standardized_shift = resolutions
        .iter()
        .map(|resolution| resolution.maximum_standardized_shift)
        .fold(0.0_f64, f64::max);
    publish_json(
        &arguments.out,
        &SensitivityResult {
            format: "marklab.arbitrary_window_lgcp_quadrature_sensitivity".into(),
            version: 1,
            resolutions,
            all_fits_complete,
            maximum_standardized_shift,
            material_standardized_shift: arguments.material_standardized_shift,
            material_change: maximum_standardized_shift >= arguments.material_standardized_shift,
            total_draw_node_work,
            statistical_unit: StatisticalUnit::OneObservedPointPattern,
            claim_status: ClaimStatus::ExperimentalQuadratureSensitivity,
        },
    )
}
