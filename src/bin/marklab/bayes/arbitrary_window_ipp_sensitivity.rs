pub(crate) use marklab_bayes::protocol::arbitrary_window_ipp_sensitivity::{
    standardized, SensitivityResult, SensitivityScenario,
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
pub(crate) struct SensitivityArguments {
    /// Observed point events with coordinates and model covariates.
    #[arg(long)]
    events: PathBuf,
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
    /// Upper bound on draw node work.
    #[arg(long)]
    maximum_draw_node_work: u64,
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

pub(super) fn run(arguments: SensitivityArguments) -> Result<(), CliError> {
    if !arguments.intercept_prior_sd.is_finite()
        || arguments.intercept_prior_sd <= 0.0
        || !arguments.coefficient_prior_sd.is_finite()
        || arguments.coefficient_prior_sd <= 0.0
        || !arguments.material_standardized_shift.is_finite()
        || arguments.material_standardized_shift <= 0.0
    {
        return Err(CliError::Input(
            "arbitrary-window IPP sensitivity scales and threshold must be positive and finite"
                .into(),
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
            "baseline",
            arguments.intercept_prior_sd,
            arguments.coefficient_prior_sd,
        ),
        (
            "intercept_sd_lower",
            arguments.intercept_prior_sd * 0.5,
            arguments.coefficient_prior_sd,
        ),
        (
            "intercept_sd_upper",
            arguments.intercept_prior_sd * 2.0,
            arguments.coefficient_prior_sd,
        ),
        (
            "coefficient_sd_lower",
            arguments.intercept_prior_sd,
            arguments.coefficient_prior_sd * 0.5,
        ),
        (
            "coefficient_sd_upper",
            arguments.intercept_prior_sd,
            arguments.coefficient_prior_sd * 2.0,
        ),
    ];
    let total_draw_node_work = arguments
        .maximum_draw_node_work
        .checked_mul(definitions.len() as u64)
        .ok_or_else(|| CliError::Input("sensitivity work overflows".into()))?;
    if total_draw_node_work > 100_000_000 {
        return Err(CliError::Input(
            "arbitrary-window IPP sensitivity work exceeds 100000000".into(),
        ));
    }
    let mut fits = Vec::with_capacity(definitions.len());
    for (name, intercept_sd, coefficient_sd) in definitions {
        let prepared =
            arbitrary_window_ipp_fit::prepare(crate::bayes::ArbitraryWindowIppFitParameters {
                events_path: arguments.events.clone(),
                quadrature_path: arguments.quadrature.clone(),
                window_path: arguments.window.clone(),
                intercept_prior_mean: arguments.intercept_prior_mean,
                intercept_prior_sd: intercept_sd,
                coefficient_prior_mean: arguments.coefficient_prior_mean,
                coefficient_prior_sd: coefficient_sd,
                sampling: sampling.clone(),
                maximum_events: arguments.maximum_events,
                maximum_quadrature_nodes: arguments.maximum_quadrature_nodes,
                maximum_draw_node_work: arguments.maximum_draw_node_work,
                timeout_seconds: arguments.timeout_seconds,
            })?;
        fits.push((
            name,
            intercept_sd,
            coefficient_sd,
            arbitrary_window_ipp_fit::execute(&prepared)?,
        ));
    }
    let baseline_intercept = fits[0].3.posterior.intercept.mean;
    let baseline_intercept_sd = fits[0].3.posterior.intercept.sd;
    let baseline_coefficient = fits[0].3.posterior.coefficient.mean;
    let baseline_coefficient_sd = fits[0].3.posterior.coefficient.sd;
    let scenarios = fits
        .into_iter()
        .map(|(name, intercept_sd, coefficient_sd, fit)| {
            let intercept_shift = standardized(
                &fit.posterior.intercept,
                baseline_intercept,
                baseline_intercept_sd,
            );
            let coefficient_shift = standardized(
                &fit.posterior.coefficient,
                baseline_coefficient,
                baseline_coefficient_sd,
            );
            let maximum_shift = intercept_shift.max(coefficient_shift);
            SensitivityScenario {
                name: name.into(),
                intercept_prior_sd: intercept_sd,
                coefficient_prior_sd: coefficient_sd,
                backend: fit.backend,
                input: fit.input,
                request_sha256: fit.request_sha256,
                sampling: fit.sampling,
                fit_state: fit.fit_state,
                posterior: fit.posterior,
                diagnostics: fit.diagnostics,
                total_expected_count_mean: fit.posterior_predictive.total_expected_count_mean,
                intercept_standardized_shift: intercept_shift,
                coefficient_standardized_shift: coefficient_shift,
                maximum_standardized_shift: maximum_shift,
                material_change: name != "baseline"
                    && maximum_shift >= arguments.material_standardized_shift,
            }
        })
        .collect::<Vec<_>>();
    let all_fits_complete = scenarios
        .iter()
        .all(|scenario| scenario.fit_state == FitState::Complete);
    let material_scenarios = scenarios
        .iter()
        .filter(|scenario| scenario.material_change)
        .map(|scenario| scenario.name.clone())
        .collect();
    publish_json(
        &arguments.out,
        &SensitivityResult {
            format: "marklab.arbitrary_window_ipp_prior_sensitivity".into(),
            version: 1,
            fit_count: scenarios.len(),
            scenarios,
            all_fits_complete,
            material_standardized_shift: arguments.material_standardized_shift,
            material_scenarios,
            total_draw_node_work,
            statistical_unit: StatisticalUnit::OneObservedPointPattern,
            claim_status: ClaimStatus::ExperimentalPriorScaleSensitivity,
        },
    )
}
