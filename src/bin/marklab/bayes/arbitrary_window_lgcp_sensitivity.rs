pub(crate) use marklab_bayes::protocol::arbitrary_window_lgcp_sensitivity::{
    rms_difference, Scenario, SensitivityResult,
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
    /// Table binding observed events to their declared components or quadrature cells.
    #[arg(long)]
    event_membership: PathBuf,
    /// Quadrature nodes, weights and covariates for spatial integration.
    #[arg(long)]
    quadrature: PathBuf,
    /// Observation-window geometry used for spatial integration or edge correction.
    #[arg(long)]
    window: PathBuf,
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
            "arbitrary-window LGCP sensitivity threshold or work ceiling is invalid".into(),
        ));
    }
    let definitions = [
        (
            "baseline",
            arguments.field_amplitude,
            arguments.field_length_scale_um,
        ),
        (
            "field_amplitude_lower",
            arguments.field_amplitude * 0.5,
            arguments.field_length_scale_um,
        ),
        (
            "field_amplitude_upper",
            arguments.field_amplitude * 2.0,
            arguments.field_length_scale_um,
        ),
        (
            "field_length_scale_lower",
            arguments.field_amplitude,
            arguments.field_length_scale_um * 0.5,
        ),
        (
            "field_length_scale_upper",
            arguments.field_amplitude,
            arguments.field_length_scale_um * 2.0,
        ),
    ];
    let per_fit_work = u64::from(arguments.chains)
        .checked_mul(u64::from(arguments.draws))
        .and_then(|draws| draws.checked_mul(arguments.maximum_quadrature_nodes as u64))
        .ok_or_else(|| CliError::Input("LGCP sensitivity work overflows".into()))?;
    let total_draw_node_work = per_fit_work
        .checked_mul(definitions.len() as u64)
        .ok_or_else(|| CliError::Input("LGCP sensitivity total work overflows".into()))?;
    if total_draw_node_work > arguments.maximum_total_draw_node_work {
        return Err(CliError::Input(format!(
            "LGCP sensitivity draw-node work exceeds maximum: {total_draw_node_work} > {}",
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
    for (name, amplitude, length_scale) in definitions {
        let prepared =
            arbitrary_window_lgcp_fit::prepare(crate::bayes::ArbitraryWindowLgcpFitArgs {
                events: arguments.events.clone(),
                event_membership: arguments.event_membership.clone(),
                quadrature: arguments.quadrature.clone(),
                window: arguments.window.clone(),
                intercept_prior_mean: arguments.intercept_prior_mean,
                intercept_prior_sd: arguments.intercept_prior_sd,
                coefficient_prior_mean: arguments.coefficient_prior_mean,
                coefficient_prior_sd: arguments.coefficient_prior_sd,
                field_amplitude: amplitude,
                field_length_scale_um: length_scale,
                jitter: arguments.jitter,
                chains: sampling.chains,
                tune: sampling.tune_per_chain,
                draws: sampling.draws_per_chain,
                target_accept: sampling.target_accept,
                seed: sampling.seed,
                maximum_events: arguments.maximum_events,
                maximum_quadrature_nodes: arguments.maximum_quadrature_nodes,
                maximum_draw_node_work: per_fit_work,
                prediction_replicates: arguments.prediction_replicates,
                prediction_seed: arguments.prediction_seed,
                maximum_predictive_points: arguments.maximum_predictive_points,
                neighbor_radius_um: arguments.neighbor_radius_um,
                maximum_neighbor_pairs: arguments.maximum_neighbor_pairs,
                timeout_seconds: arguments.timeout_seconds,
                out: std::path::PathBuf::new(),
            })?;
        fits.push((
            name,
            amplitude,
            length_scale,
            arbitrary_window_lgcp_fit::execute(&prepared)?,
        ));
    }
    let baseline = &fits[0].3;
    if fits
        .iter()
        .any(|(_, _, _, fit)| fit.input != baseline.input)
    {
        return Err(CliError::Input(
            "arbitrary-window LGCP inputs changed across sensitivity scenarios".into(),
        ));
    }
    let baseline_intercept = baseline.posterior.intercept.mean;
    let baseline_intercept_sd = baseline.posterior.intercept.sd.max(f64::EPSILON);
    let baseline_coefficient = baseline.posterior.coefficient.mean;
    let baseline_coefficient_sd = baseline.posterior.coefficient.sd.max(f64::EPSILON);
    let baseline_latent = baseline
        .nodes
        .iter()
        .map(|node| node.latent_effect.mean)
        .collect::<Vec<_>>();
    let baseline_latent_scale = (baseline
        .nodes
        .iter()
        .map(|node| node.latent_effect.sd.powi(2))
        .sum::<f64>()
        / baseline.nodes.len() as f64)
        .sqrt()
        .max(f64::EPSILON);
    let baseline_expected = baseline
        .nodes
        .iter()
        .map(|node| node.expected_count.mean)
        .collect::<Vec<_>>();
    let baseline_expected_scale = (baseline_expected
        .iter()
        .map(|value| value.powi(2))
        .sum::<f64>()
        / baseline_expected.len() as f64)
        .sqrt()
        .max(f64::EPSILON);
    let scenarios = fits
        .into_iter()
        .map(|(name, amplitude, length_scale, fit)| {
            let intercept_shift =
                (fit.posterior.intercept.mean - baseline_intercept).abs() / baseline_intercept_sd;
            let coefficient_shift = (fit.posterior.coefficient.mean - baseline_coefficient).abs()
                / baseline_coefficient_sd;
            let latent_shift = rms_difference(
                fit.nodes.iter().map(|node| node.latent_effect.mean),
                baseline_latent.iter().copied(),
            ) / baseline_latent_scale;
            let expected_shift = rms_difference(
                fit.nodes.iter().map(|node| node.expected_count.mean),
                baseline_expected.iter().copied(),
            ) / baseline_expected_scale;
            let maximum_shift = intercept_shift.max(coefficient_shift).max(latent_shift);
            Scenario {
                name: name.into(),
                field_amplitude: amplitude,
                field_length_scale_um: length_scale,
                backend: fit.backend,
                source_backend: fit.source_backend,
                input: fit.input,
                request_sha256: fit.request_sha256,
                fit_state: fit.fit_state,
                posterior: fit.posterior,
                diagnostics: fit.diagnostics,
                spatial_posterior_predictive: fit.spatial_posterior_predictive,
                intercept_standardized_shift: intercept_shift,
                coefficient_standardized_shift: coefficient_shift,
                latent_field_rms_standardized_shift: latent_shift,
                expected_count_rms_relative_shift: expected_shift,
                maximum_standardized_shift: maximum_shift,
                material_change: name != "baseline"
                    && maximum_shift >= arguments.material_standardized_shift,
            }
        })
        .collect::<Vec<_>>();
    let all_fits_complete = scenarios
        .iter()
        .all(|scenario| scenario.fit_state == FitState::Complete);
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
            format: "marklab.arbitrary_window_lgcp_sensitivity".into(),
            version: 1,
            scenarios,
            all_fits_complete,
            maximum_standardized_shift,
            material_standardized_shift: arguments.material_standardized_shift,
            material_scenarios,
            total_draw_node_work,
            statistical_unit: StatisticalUnit::OneObservedPointPattern,
            claim_status: ClaimStatus::ExperimentalFixedKernelSensitivity,
        },
    )
}
