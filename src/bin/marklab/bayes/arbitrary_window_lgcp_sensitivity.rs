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
    ArbitraryWindowLgcpSensitivity(Box<Arguments>),
}

#[derive(Debug, clap::Args)]
struct Arguments {
    #[arg(long)]
    events: PathBuf,
    #[arg(long)]
    event_membership: PathBuf,
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
struct Scenario {
    name: String,
    field_amplitude: f64,
    field_length_scale_um: f64,
    backend: WorkerBackend,
    source_backend: WorkerBackend,
    input: InputIdentity,
    request_sha256: String,
    fit_state: FitState,
    posterior: GriddedLgcpPosterior,
    diagnostics: NormalMeanDiagnostics,
    spatial_posterior_predictive: Option<SpatialPpc>,
    intercept_standardized_shift: f64,
    coefficient_standardized_shift: f64,
    latent_field_rms_standardized_shift: f64,
    expected_count_rms_relative_shift: f64,
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
    claim_status: String,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let TopLevel::Bayes { command } = SensitivityCli::parse_from(std::env::args_os()).command;
    let Command::ArbitraryWindowLgcpSensitivity(arguments) = command;
    if !arguments.material_standardized_shift.is_finite()
        || arguments.material_standardized_shift <= 0.0
        || arguments.maximum_total_draw_node_work == 0
        || arguments.maximum_total_draw_node_work > 100_000_000
    {
        return Err(BayesCliError::Input(
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
        .ok_or_else(|| BayesCliError::Input("LGCP sensitivity work overflows".into()))?;
    let total_draw_node_work = per_fit_work
        .checked_mul(definitions.len() as u64)
        .ok_or_else(|| BayesCliError::Input("LGCP sensitivity total work overflows".into()))?;
    if total_draw_node_work > arguments.maximum_total_draw_node_work {
        return Err(BayesCliError::Input(format!(
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
        let prepared = arbitrary_window_lgcp_fit::prepare(
            arguments.events.clone(),
            arguments.event_membership.clone(),
            arguments.quadrature.clone(),
            arguments.window.clone(),
            arguments.intercept_prior_mean,
            arguments.intercept_prior_sd,
            arguments.coefficient_prior_mean,
            arguments.coefficient_prior_sd,
            amplitude,
            length_scale,
            arguments.jitter,
            sampling.clone(),
            arguments.maximum_events,
            arguments.maximum_quadrature_nodes,
            per_fit_work,
            arguments.prediction_replicates,
            arguments.prediction_seed,
            arguments.maximum_predictive_points,
            arguments.neighbor_radius_um,
            arguments.maximum_neighbor_pairs,
            arguments.timeout_seconds,
        )?;
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
        return Err(BayesCliError::Input(
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
            statistical_unit: "one_observed_point_pattern".into(),
            claim_status: "experimental_fixed_kernel_sensitivity".into(),
        },
    )
}

fn rms_difference(left: impl Iterator<Item = f64>, right: impl Iterator<Item = f64>) -> f64 {
    let differences = left
        .zip(right)
        .map(|(left, right)| (left - right).powi(2))
        .collect::<Vec<_>>();
    (differences.iter().sum::<f64>() / differences.len() as f64).sqrt()
}
