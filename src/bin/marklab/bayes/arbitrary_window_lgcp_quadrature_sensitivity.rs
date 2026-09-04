use std::path::PathBuf;

use clap::{Parser, Subcommand};
use marklab_bayes::{
    FitState, GriddedLgcpPosterior, NormalMeanDiagnostics, NutsSamplingSpec, WorkerBackend,
};
use serde::Serialize;

use super::{
    arbitrary_window_lgcp_fit::{self, InputIdentity, SpatialPpc},
    publish_json, BayesCliError,
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
    ArbitraryWindowLgcpQuadratureSensitivity(Box<Arguments>),
}

#[derive(Debug, clap::Args)]
struct Arguments {
    #[arg(long)]
    events: PathBuf,
    #[arg(long)]
    window: PathBuf,
    #[arg(long)]
    coarse_event_membership: PathBuf,
    #[arg(long)]
    coarse_quadrature: PathBuf,
    #[arg(long)]
    intermediate_event_membership: PathBuf,
    #[arg(long)]
    intermediate_quadrature: PathBuf,
    #[arg(long)]
    baseline_event_membership: PathBuf,
    #[arg(long)]
    baseline_quadrature: PathBuf,
    #[arg(long, allow_hyphen_values = true)]
    intercept_prior_mean: f64,
    #[arg(long)]
    intercept_prior_sd: f64,
    #[arg(long, allow_hyphen_values = true)]
    coefficient_prior_mean: f64,
    #[arg(long)]
    coefficient_prior_sd: f64,
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
    maximum_events: usize,
    #[arg(long)]
    maximum_quadrature_nodes: usize,
    #[arg(long)]
    maximum_total_draw_node_work: u64,
    #[arg(long)]
    prediction_replicates: u32,
    #[arg(long)]
    prediction_seed: u64,
    #[arg(long)]
    maximum_predictive_points: u64,
    #[arg(long)]
    neighbor_radius_um: f64,
    #[arg(long)]
    maximum_neighbor_pairs: usize,
    #[arg(long)]
    material_standardized_shift: f64,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Debug, Serialize)]
struct Resolution {
    name: String,
    quadrature_node_count: usize,
    backend: WorkerBackend,
    source_backend: WorkerBackend,
    input: InputIdentity,
    request_sha256: String,
    fit_state: FitState,
    posterior: GriddedLgcpPosterior,
    diagnostics: NormalMeanDiagnostics,
    spatial_posterior_predictive: Option<SpatialPpc>,
    total_expected_count_mean: f64,
    intercept_standardized_shift: f64,
    coefficient_standardized_shift: f64,
    total_count_relative_shift: f64,
    maximum_standardized_shift: f64,
}

#[derive(Debug, Serialize)]
struct SensitivityResult {
    format: String,
    version: u32,
    resolutions: Vec<Resolution>,
    all_fits_complete: bool,
    maximum_standardized_shift: f64,
    material_standardized_shift: f64,
    material_change: bool,
    total_draw_node_work: u64,
    statistical_unit: String,
    claim_status: String,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let TopLevel::Bayes { command } = SensitivityCli::parse_from(std::env::args_os()).command;
    let Command::ArbitraryWindowLgcpQuadratureSensitivity(arguments) = command;
    if !arguments.material_standardized_shift.is_finite()
        || arguments.material_standardized_shift <= 0.0
        || arguments.maximum_total_draw_node_work == 0
        || arguments.maximum_total_draw_node_work > 100_000_000
    {
        return Err(BayesCliError::Input(
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
            arbitrary_window_lgcp_fit::prepare(
                arguments.events.clone(),
                membership,
                quadrature,
                arguments.window.clone(),
                arguments.intercept_prior_mean,
                arguments.intercept_prior_sd,
                arguments.coefficient_prior_mean,
                arguments.coefficient_prior_sd,
                arguments.field_amplitude,
                arguments.field_length_scale_um,
                arguments.jitter,
                sampling.clone(),
                arguments.maximum_events,
                arguments.maximum_quadrature_nodes,
                arguments.maximum_total_draw_node_work,
                arguments.prediction_replicates,
                arguments.prediction_seed,
                arguments.maximum_predictive_points,
                arguments.neighbor_radius_um,
                arguments.maximum_neighbor_pairs,
                arguments.timeout_seconds,
            )?,
        ));
    }
    let first = &prepared[0].1.request.input;
    if prepared.iter().any(|(_, prepared)| {
        prepared.request.input.events_digest != first.events_digest
            || prepared.request.input.window_logical_digest != first.window_logical_digest
    }) {
        return Err(BayesCliError::Input(
            "LGCP quadrature sensitivity events or exact window changed across resolutions".into(),
        ));
    }
    let completed_draws = u64::from(arguments.chains) * u64::from(arguments.draws);
    let total_draw_node_work = prepared.iter().try_fold(0_u64, |work, (_, prepared)| {
        work.checked_add(completed_draws * prepared.request.nodes.len() as u64)
    });
    let total_draw_node_work = total_draw_node_work
        .ok_or_else(|| BayesCliError::Input("LGCP quadrature work overflows".into()))?;
    if total_draw_node_work > arguments.maximum_total_draw_node_work {
        return Err(BayesCliError::Input(format!(
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
            statistical_unit: "one_observed_point_pattern".into(),
            claim_status: "experimental_quadrature_sensitivity".into(),
        },
    )
}

pub(super) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<SensitivityCli>(|| run_cli().map_err(super::into_marklab_error))
}
