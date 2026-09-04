use std::path::PathBuf;

use clap::{Parser, Subcommand};
use marklab_bayes::{
    FitState, NormalMeanDiagnostics, NutsSamplingSpec, SamplingSummary, WorkerBackend,
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
struct QuadratureSensitivityCli {
    #[command(subcommand)]
    command: QuadratureSensitivityTopLevel,
}

#[derive(Debug, Subcommand)]
enum QuadratureSensitivityTopLevel {
    Bayes {
        #[command(subcommand)]
        command: QuadratureSensitivityCommand,
    },
}

#[derive(Debug, Subcommand)]
enum QuadratureSensitivityCommand {
    ArbitraryWindowIppQuadratureSensitivity(Box<Arguments>),
}

#[derive(Debug, clap::Args)]
struct Arguments {
    #[arg(long)]
    events: PathBuf,
    #[arg(long)]
    window: PathBuf,
    #[arg(long)]
    coarse_quadrature: PathBuf,
    #[arg(long)]
    baseline_quadrature: PathBuf,
    #[arg(long)]
    fine_quadrature: PathBuf,
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
    maximum_total_draw_node_work: u64,
    #[arg(long)]
    material_standardized_shift: f64,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Debug, Serialize)]
struct ResolutionResult {
    name: String,
    quadrature_node_count: usize,
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
    total_count_relative_shift: f64,
    maximum_standardized_shift: f64,
}

#[derive(Debug, Serialize)]
struct QuadratureSensitivityResult {
    format: String,
    version: u32,
    resolutions: Vec<ResolutionResult>,
    all_fits_complete: bool,
    maximum_standardized_shift: f64,
    material_standardized_shift: f64,
    material_change: bool,
    total_draw_node_work: u64,
    statistical_unit: String,
    claim_status: String,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let QuadratureSensitivityTopLevel::Bayes { command } =
        QuadratureSensitivityCli::parse_from(std::env::args_os()).command;
    let QuadratureSensitivityCommand::ArbitraryWindowIppQuadratureSensitivity(arguments) = command;
    if !arguments.material_standardized_shift.is_finite()
        || arguments.material_standardized_shift <= 0.0
        || arguments.maximum_total_draw_node_work == 0
        || arguments.maximum_total_draw_node_work > 100_000_000
    {
        return Err(BayesCliError::Input(
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
            arbitrary_window_ipp_fit::prepare(
                arguments.events.clone(),
                quadrature,
                arguments.window.clone(),
                arguments.intercept_prior_mean,
                arguments.intercept_prior_sd,
                arguments.coefficient_prior_mean,
                arguments.coefficient_prior_sd,
                sampling.clone(),
                arguments.maximum_events,
                arguments.maximum_quadrature_nodes,
                arguments.maximum_total_draw_node_work,
                arguments.timeout_seconds,
            )?,
        ));
    }
    let completed_draws = u64::from(arguments.chains) * u64::from(arguments.draws);
    let total_draw_node_work = prepared.iter().try_fold(0_u64, |work, (_, fit)| {
        work.checked_add(completed_draws * fit.input.spec.quadrature.len() as u64)
    });
    let total_draw_node_work = total_draw_node_work
        .ok_or_else(|| BayesCliError::Input("quadrature sensitivity work overflows".into()))?;
    if total_draw_node_work > arguments.maximum_total_draw_node_work {
        return Err(BayesCliError::Input(format!(
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
            statistical_unit: "one_observed_point_pattern".into(),
            claim_status: "experimental_quadrature_sensitivity".into(),
        },
    )
}

pub(super) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<QuadratureSensitivityCli>(|| run_cli().map_err(super::into_marklab_error))
}
