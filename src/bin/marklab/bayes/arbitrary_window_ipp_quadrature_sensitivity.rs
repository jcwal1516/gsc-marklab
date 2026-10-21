pub(crate) use marklab_bayes::protocol::arbitrary_window_ipp_quadrature_sensitivity::{
    QuadratureSensitivityResult, ResolutionResult,
};

use std::path::PathBuf;

use marklab_bayes::FitState;
use marklab_bayes::NutsSamplingSpec;

use super::arbitrary_window_ipp_fit;
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
    /// Coarse version of: quadrature nodes, weights and covariates for spatial integration.
    #[arg(long)]
    coarse_quadrature: PathBuf,
    /// Baseline version of: quadrature nodes, weights and covariates for spatial integration.
    #[arg(long)]
    baseline_quadrature: PathBuf,
    /// Fine version of: quadrature nodes, weights and covariates for spatial integration.
    #[arg(long)]
    fine_quadrature: PathBuf,
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
            "quadrature sensitivity threshold or total work bound is invalid".into(),
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
        ("coarse", arguments.coarse_quadrature),
        ("baseline", arguments.baseline_quadrature),
        ("fine", arguments.fine_quadrature),
    ];
    let mut prepared = Vec::with_capacity(definitions.len());
    for (name, quadrature) in definitions {
        prepared.push((
            name,
            arbitrary_window_ipp_fit::prepare(crate::bayes::ArbitraryWindowIppFitParameters {
                events_path: arguments.events.clone(),
                quadrature_path: quadrature,
                window_path: arguments.window.clone(),
                intercept_prior_mean: arguments.intercept_prior_mean,
                intercept_prior_sd: arguments.intercept_prior_sd,
                coefficient_prior_mean: arguments.coefficient_prior_mean,
                coefficient_prior_sd: arguments.coefficient_prior_sd,
                sampling: sampling.clone(),
                maximum_events: arguments.maximum_events,
                maximum_quadrature_nodes: arguments.maximum_quadrature_nodes,
                maximum_draw_node_work: arguments.maximum_total_draw_node_work,
                timeout_seconds: arguments.timeout_seconds,
            })?,
        ));
    }
    let completed_draws = u64::from(arguments.chains) * u64::from(arguments.draws);
    let total_draw_node_work = prepared.iter().try_fold(0_u64, |work, (_, fit)| {
        work.checked_add(completed_draws * fit.input.spec.quadrature.len() as u64)
    });
    let total_draw_node_work = total_draw_node_work
        .ok_or_else(|| CliError::Input("quadrature sensitivity work overflows".into()))?;
    if total_draw_node_work > arguments.maximum_total_draw_node_work {
        return Err(CliError::Input(format!(
            "quadrature sensitivity work exceeds maximum: {total_draw_node_work} > {}",
            arguments.maximum_total_draw_node_work
        )));
    }
    let mut fits = Vec::with_capacity(prepared.len());
    for (name, fit) in &prepared {
        fits.push((*name, arbitrary_window_ipp_fit::execute(fit)?));
    }
    let baseline = &fits[1].1;
    let baseline_intercept = baseline.posterior.intercept.mean;
    let baseline_intercept_sd = baseline.posterior.intercept.sd.max(f64::EPSILON);
    let baseline_coefficient = baseline.posterior.coefficient.mean;
    let baseline_coefficient_sd = baseline.posterior.coefficient.sd.max(f64::EPSILON);
    let baseline_total = baseline.posterior_predictive.total_expected_count_mean;
    let resolutions = fits
        .into_iter()
        .map(|(name, fit)| {
            let intercept_shift =
                (fit.posterior.intercept.mean - baseline_intercept).abs() / baseline_intercept_sd;
            let coefficient_shift = (fit.posterior.coefficient.mean - baseline_coefficient).abs()
                / baseline_coefficient_sd;
            ResolutionResult {
                name: name.into(),
                quadrature_node_count: fit.quadrature_node_count,
                backend: fit.backend,
                input: fit.input,
                request_sha256: fit.request_sha256,
                sampling: fit.sampling,
                fit_state: fit.fit_state,
                total_expected_count_mean: fit.posterior_predictive.total_expected_count_mean,
                total_count_relative_shift: (fit.posterior_predictive.total_expected_count_mean
                    - baseline_total)
                    .abs()
                    / baseline_total,
                maximum_standardized_shift: intercept_shift.max(coefficient_shift),
                intercept_standardized_shift: intercept_shift,
                coefficient_standardized_shift: coefficient_shift,
                posterior: fit.posterior,
                diagnostics: fit.diagnostics,
            }
        })
        .collect::<Vec<_>>();
    let maximum_standardized_shift = resolutions
        .iter()
        .map(|resolution| resolution.maximum_standardized_shift)
        .fold(0.0_f64, f64::max);
    let all_fits_complete = resolutions
        .iter()
        .all(|resolution| resolution.fit_state == FitState::Complete);
    publish_json(
        &arguments.out,
        &QuadratureSensitivityResult {
            format: "marklab.arbitrary_window_ipp_quadrature_sensitivity".into(),
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
