use std::path::PathBuf;

use clap::{Parser, Subcommand};
use marklab_bayes::{
    FitState, NormalMeanDiagnostics, NutsSamplingSpec, SamplingSummary, SarScalarSummary,
    WorkerBackend,
};
use serde::Serialize;

use super::{
    arbitrary_window_ipp_fit::{
        self, ArbitraryWindowIppFitInputIdentity, ArbitraryWindowIppFitPosterior,
    },
    publish_json, BayesCliError,
};

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct SensitivityCli {
    #[command(subcommand)]
    command: SensitivityTopLevel,
}

#[derive(Debug, Subcommand)]
enum SensitivityTopLevel {
    Bayes {
        #[command(subcommand)]
        command: SensitivityCommand,
    },
}

#[derive(Debug, Subcommand)]
enum SensitivityCommand {
    ArbitraryWindowIppSensitivity(Box<SensitivityArguments>),
}

#[derive(Debug, clap::Args)]
struct SensitivityArguments {
    #[arg(long)]
    events: PathBuf,
    #[arg(long)]
    quadrature: PathBuf,
    #[arg(long)]
    window: PathBuf,
    #[arg(long, allow_hyphen_values = true)]
    intercept_prior_mean: f64,
    #[arg(long)]
    intercept_prior_sd: f64,
    #[arg(long, allow_hyphen_values = true)]
    coefficient_prior_mean: f64,
    #[arg(long)]
    coefficient_prior_sd: f64,
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
    maximum_events: usize,
    #[arg(long)]
    maximum_quadrature_nodes: usize,
    #[arg(long)]
    maximum_draw_node_work: u64,
    #[arg(long)]
    material_standardized_shift: f64,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Debug, Serialize)]
struct SensitivityScenario {
    name: String,
    intercept_prior_sd: f64,
    coefficient_prior_sd: f64,
    backend: WorkerBackend,
    input: ArbitraryWindowIppFitInputIdentity,
    request_sha256: String,
    sampling: SamplingSummary,
    fit_state: FitState,
    posterior: ArbitraryWindowIppFitPosterior,
    diagnostics: NormalMeanDiagnostics,
    total_expected_count_mean: f64,
    intercept_standardized_shift: f64,
    coefficient_standardized_shift: f64,
    maximum_standardized_shift: f64,
    material_change: bool,
}

#[derive(Debug, Serialize)]
struct SensitivityResult {
    format: String,
    version: u32,
    scenarios: Vec<SensitivityScenario>,
    all_fits_complete: bool,
    material_standardized_shift: f64,
    material_scenarios: Vec<String>,
    fit_count: usize,
    total_draw_node_work: u64,
    statistical_unit: String,
    claim_status: String,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let SensitivityTopLevel::Bayes { command } =
        SensitivityCli::parse_from(std::env::args_os()).command;
    let SensitivityCommand::ArbitraryWindowIppSensitivity(arguments) = command;
    if !arguments.intercept_prior_sd.is_finite()
        || arguments.intercept_prior_sd <= 0.0
        || !arguments.coefficient_prior_sd.is_finite()
        || arguments.coefficient_prior_sd <= 0.0
        || !arguments.material_standardized_shift.is_finite()
        || arguments.material_standardized_shift <= 0.0
    {
        return Err(BayesCliError::Input(
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
        .ok_or_else(|| BayesCliError::Input("sensitivity work overflows".into()))?;
    if total_draw_node_work > 100_000_000 {
        return Err(BayesCliError::Input(
            "arbitrary-window IPP sensitivity work exceeds 100000000".into(),
        ));
    }
    let mut fits = Vec::with_capacity(definitions.len());
    for (name, intercept_sd, coefficient_sd) in definitions {
        let prepared = arbitrary_window_ipp_fit::prepare(
            arguments.events.clone(),
            arguments.quadrature.clone(),
            arguments.window.clone(),
            arguments.intercept_prior_mean,
            intercept_sd,
            arguments.coefficient_prior_mean,
            coefficient_sd,
            sampling.clone(),
            arguments.maximum_events,
            arguments.maximum_quadrature_nodes,
            arguments.maximum_draw_node_work,
            arguments.timeout_seconds,
        )?;
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
            statistical_unit: "one_observed_point_pattern".into(),
            claim_status: "experimental_prior_scale_sensitivity".into(),
        },
    )
}

fn standardized(summary: &SarScalarSummary, baseline: f64, baseline_sd: f64) -> f64 {
    (summary.mean - baseline).abs() / baseline_sd.max(f64::EPSILON)
}
