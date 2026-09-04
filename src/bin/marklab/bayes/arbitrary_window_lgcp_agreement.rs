use std::{fs, path::PathBuf};

use clap::{Parser, Subcommand};
use marklab_bayes::{
    sha256_hex, BackendContract, FitState, GriddedLgcpPosterior, GriddedLgcpPosteriorPredictive,
    NormalMeanDiagnostics, NutsSamplingSpec, SamplingSummary, SarScalarSummary, WorkerBackend,
};
use serde::{Deserialize, Serialize};

use super::{
    arbitrary_window_lgcp_fit::{self, FitResult, InputIdentity, PreparedArbitraryWindowLgcpFit},
    posterior_validation::scalar_valid,
    publish_json, run_worker, BayesCliError,
};

const NUMPYRO_VERSION: &str = "0.21.0";
const JAX_VERSION: &str = "0.11.1";

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct AgreementCli {
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
    ArbitraryWindowLgcpAgreement(Box<Arguments>),
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
    maximum_draw_node_work: u64,
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
    maximum_standardized_difference: f64,
    #[arg(long)]
    minimum_parameter_tolerance: f64,
    #[arg(long)]
    minimum_field_tolerance: f64,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Serialize)]
struct NumpyroRequest<'a> {
    format: &'static str,
    version: u32,
    backend: BackendContract,
    source_backend: BackendContract,
    jax_version: &'static str,
    source_request_sha256: &'a str,
    source_request: &'a arbitrary_window_lgcp_fit::WorkerRequest,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NumpyroNode {
    node_id: String,
    latent_effect: SarScalarSummary,
    expected_count: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NumpyroResult {
    format: String,
    version: u32,
    backend: WorkerBackend,
    source_backend: WorkerBackend,
    jax_version: String,
    input: InputIdentity,
    request_sha256: String,
    source_request_sha256: String,
    covariance_sha256: String,
    fit_state: FitState,
    sampling: SamplingSummary,
    posterior: GriddedLgcpPosterior,
    nodes: Vec<NumpyroNode>,
    diagnostics: NormalMeanDiagnostics,
    posterior_predictive: GriddedLgcpPosteriorPredictive,
}

#[derive(Debug, Serialize)]
struct ScalarAgreement {
    pymc_mean: f64,
    numpyro_mean: f64,
    absolute_difference: f64,
    combined_mcse: f64,
    standardized_difference: f64,
    tolerance: f64,
    intervals_overlap: bool,
    passes: bool,
}

#[derive(Debug, Serialize)]
struct FieldAgreement {
    root_mean_square_difference: f64,
    maximum_absolute_difference: f64,
    maximum_standardized_difference: f64,
    all_intervals_overlap: bool,
    passes: bool,
}

#[derive(Debug, Serialize)]
struct Comparison {
    intercept: ScalarAgreement,
    coefficient: ScalarAgreement,
    latent_effect: FieldAgreement,
    expected_count: FieldAgreement,
    node_count: usize,
    maximum_standardized_difference: f64,
    minimum_parameter_tolerance: f64,
    minimum_field_tolerance: f64,
}

#[derive(Debug, Serialize)]
struct AgreementResult {
    format: String,
    version: u32,
    input: InputIdentity,
    covariance_sha256: String,
    fit_state: FitState,
    agreement_status: String,
    pymc: FitResult,
    numpyro: NumpyroResult,
    comparison: Comparison,
    statistical_unit: String,
    claim_status: String,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let TopLevel::Bayes { command } = AgreementCli::parse_from(std::env::args_os()).command;
    let Command::ArbitraryWindowLgcpAgreement(arguments) = command;
    validate_controls(&arguments)?;
    let prepared = arbitrary_window_lgcp_fit::prepare(
        arguments.events,
        arguments.event_membership,
        arguments.quadrature,
        arguments.window,
        arguments.intercept_prior_mean,
        arguments.intercept_prior_sd,
        arguments.coefficient_prior_mean,
        arguments.coefficient_prior_sd,
        arguments.field_amplitude,
        arguments.field_length_scale_um,
        arguments.jitter,
        NutsSamplingSpec {
            chains: arguments.chains,
            tune_per_chain: arguments.tune,
            draws_per_chain: arguments.draws,
            target_accept: arguments.target_accept,
            seed: arguments.seed,
        },
        arguments.maximum_events,
        arguments.maximum_quadrature_nodes,
        arguments.maximum_draw_node_work,
        arguments.prediction_replicates,
        arguments.prediction_seed,
        arguments.maximum_predictive_points,
        arguments.neighbor_radius_um,
        arguments.maximum_neighbor_pairs,
        arguments.timeout_seconds,
    )?;
    let pymc = arbitrary_window_lgcp_fit::execute(&prepared)?;
    let numpyro = execute_numpyro(&prepared, &pymc, arguments.timeout_seconds)?;
    let comparison = compare(
        &pymc,
        &numpyro,
        arguments.maximum_standardized_difference,
        arguments.minimum_parameter_tolerance,
        arguments.minimum_field_tolerance,
    );
    let fits_complete =
        pymc.fit_state == FitState::Complete && numpyro.fit_state == FitState::Complete;
    let agrees = comparison.intercept.passes
        && comparison.coefficient.passes
        && comparison.latent_effect.passes
        && comparison.expected_count.passes;
    let available = fits_complete && agrees;
    let result = AgreementResult {
        format: "marklab.arbitrary_window_lgcp_backend_agreement".into(),
        version: 1,
        input: pymc.input.clone(),
        covariance_sha256: prepared.request.covariance_sha256.clone(),
        fit_state: if fits_complete {
            FitState::Complete
        } else {
            FitState::Nonconverged
        },
        agreement_status: if available {
            "agree_within_monte_carlo_error"
        } else if fits_complete {
            "diagnostic_only_backend_disagreement"
        } else {
            "diagnostic_only_nonconverged"
        }
        .into(),
        pymc,
        numpyro,
        comparison,
        statistical_unit: "one_observed_point_pattern".into(),
        claim_status: if available {
            "experimental_cross_backend_validation"
        } else {
            "diagnostic_only_cross_backend_validation"
        }
        .into(),
    };
    publish_json(&arguments.out, &result)
}

fn validate_controls(arguments: &Arguments) -> Result<(), BayesCliError> {
    if !arguments.maximum_standardized_difference.is_finite()
        || !(1.0..=10.0).contains(&arguments.maximum_standardized_difference)
        || !arguments.minimum_parameter_tolerance.is_finite()
        || arguments.minimum_parameter_tolerance <= 0.0
        || !arguments.minimum_field_tolerance.is_finite()
        || arguments.minimum_field_tolerance <= 0.0
    {
        return Err(BayesCliError::Input(
            "arbitrary-window LGCP agreement controls are invalid".into(),
        ));
    }
    Ok(())
}

fn execute_numpyro(
    prepared: &PreparedArbitraryWindowLgcpFit,
    pymc: &FitResult,
    timeout_seconds: u64,
) -> Result<NumpyroResult, BayesCliError> {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let directory = repository.join("workers/python");
    let lock = read(&directory.join("uv.lock"))?;
    let worker = read(&directory.join("marklab_numpyro_arbitrary_window_lgcp_worker.py"))?;
    let source_worker = read(&directory.join("marklab_numpyro_gridded_lgcp_worker.py"))?;
    let request = NumpyroRequest {
        format: "marklab.numpyro_arbitrary_window_lgcp_request",
        version: 1,
        backend: BackendContract {
            name: "numpyro",
            version: NUMPYRO_VERSION,
            python_version: "3.12",
            environment_lock_sha256: sha256_hex(&lock),
            worker_sha256: sha256_hex(&worker),
        },
        source_backend: BackendContract {
            name: "numpyro",
            version: NUMPYRO_VERSION,
            python_version: "3.12",
            environment_lock_sha256: sha256_hex(&lock),
            worker_sha256: sha256_hex(&source_worker),
        },
        jax_version: JAX_VERSION,
        source_request_sha256: &prepared.request_sha256,
        source_request: &prepared.request,
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let bytes = run_worker(
        repository,
        "marklab_numpyro_arbitrary_window_lgcp_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: NumpyroResult = serde_json::from_slice(&bytes)?;
    validate_numpyro(&result, &request, &request_sha256, prepared, pymc)?;
    Ok(result)
}

fn validate_numpyro(
    result: &NumpyroResult,
    request: &NumpyroRequest<'_>,
    request_sha256: &str,
    prepared: &PreparedArbitraryWindowLgcpFit,
    pymc: &FitResult,
) -> Result<(), BayesCliError> {
    let policy = &prepared.request.source_request.diagnostic_policy;
    let diagnostics_pass = result.diagnostics.prior_predictive_finite
        && result.diagnostics.posterior_finite
        && result.diagnostics.constraints_valid
        && result.diagnostics.identifiability_checks_passed
        && result.diagnostics.r_hat <= policy.maximum_r_hat
        && result.diagnostics.ess_bulk >= policy.minimum_bulk_ess
        && result.diagnostics.ess_tail >= policy.minimum_tail_ess
        && result.diagnostics.minimum_ebfmi >= policy.minimum_ebfmi
        && result.diagnostics.divergences <= policy.maximum_divergences
        && result.diagnostics.max_tree_depth_hits <= policy.maximum_tree_depth_hits;
    let nodes_valid = result.nodes.len() == pymc.nodes.len()
        && result
            .nodes
            .iter()
            .zip(&pymc.nodes)
            .all(|(result, source)| {
                result.node_id == source.node_id
                    && scalar_valid(&result.latent_effect, false)
                    && scalar_valid(&result.expected_count, true)
            });
    let draws = u64::from(prepared.request.source_request.sampling.chains)
        * u64::from(prepared.request.source_request.sampling.draws_per_chain);
    let predictive = &result.posterior_predictive;
    if result.format != "marklab.numpyro_arbitrary_window_lgcp_result"
        || result.version != 1
        || !backend_matches(&result.backend, &request.backend)
        || !backend_matches(&result.source_backend, &request.source_backend)
        || result.jax_version != JAX_VERSION
        || result.input != pymc.input
        || result.request_sha256 != request_sha256
        || result.source_request_sha256 != prepared.request_sha256
        || result.covariance_sha256 != prepared.request.covariance_sha256
        || result.sampling.chains != prepared.request.source_request.sampling.chains
        || result.sampling.tune_per_chain != prepared.request.source_request.sampling.tune_per_chain
        || result.sampling.draws_per_chain
            != prepared.request.source_request.sampling.draws_per_chain
        || result.sampling.completed_draws != draws
        || !scalar_valid(&result.posterior.intercept, false)
        || !scalar_valid(&result.posterior.coefficient, false)
        || !nodes_valid
        || predictive.observed_total_count
            != prepared
                .request
                .nodes
                .iter()
                .map(|node| node.count)
                .sum::<u64>()
        || ![
            predictive.replicated_total_mean,
            predictive.replicated_total_sd,
            predictive.replicated_zero_cells_mean,
        ]
        .into_iter()
        .all(f64::is_finite)
        || predictive.replicated_total_mean < 0.0
        || predictive.replicated_total_sd < 0.0
        || predictive.replicated_zero_cells_mean < 0.0
        || (result.fit_state == FitState::Complete) != diagnostics_pass
    {
        return Err(BayesCliError::Backend(
            "NumPyro arbitrary-window LGCP result identity or diagnostics differ".into(),
        ));
    }
    Ok(())
}

fn compare(
    pymc: &FitResult,
    numpyro: &NumpyroResult,
    maximum_standardized_difference: f64,
    minimum_parameter_tolerance: f64,
    minimum_field_tolerance: f64,
) -> Comparison {
    let intercept = compare_scalar(
        &pymc.posterior.intercept,
        pymc.diagnostics.mcse_mean,
        &numpyro.posterior.intercept,
        numpyro.diagnostics.mcse_mean,
        maximum_standardized_difference,
        minimum_parameter_tolerance,
    );
    let coefficient = compare_scalar(
        &pymc.posterior.coefficient,
        pymc.diagnostics.mcse_mean,
        &numpyro.posterior.coefficient,
        numpyro.diagnostics.mcse_mean,
        maximum_standardized_difference,
        minimum_parameter_tolerance,
    );
    let latent_effect = compare_field(
        pymc.nodes.iter().map(|node| &node.latent_effect),
        numpyro.nodes.iter().map(|node| &node.latent_effect),
        pymc.diagnostics.ess_bulk,
        numpyro.diagnostics.ess_bulk,
        maximum_standardized_difference,
        minimum_field_tolerance,
    );
    let expected_count = compare_field(
        pymc.nodes.iter().map(|node| &node.expected_count),
        numpyro.nodes.iter().map(|node| &node.expected_count),
        pymc.diagnostics.ess_bulk,
        numpyro.diagnostics.ess_bulk,
        maximum_standardized_difference,
        minimum_field_tolerance,
    );
    Comparison {
        intercept,
        coefficient,
        latent_effect,
        expected_count,
        node_count: pymc.nodes.len(),
        maximum_standardized_difference,
        minimum_parameter_tolerance,
        minimum_field_tolerance,
    }
}

fn compare_scalar(
    pymc: &SarScalarSummary,
    pymc_mcse: f64,
    numpyro: &SarScalarSummary,
    numpyro_mcse: f64,
    maximum_standardized_difference: f64,
    minimum_tolerance: f64,
) -> ScalarAgreement {
    let absolute_difference = (pymc.mean - numpyro.mean).abs();
    let combined_mcse = pymc_mcse.hypot(numpyro_mcse);
    let standardized_difference = standardized(absolute_difference, combined_mcse);
    let tolerance = minimum_tolerance.max(maximum_standardized_difference * combined_mcse);
    let intervals_overlap = overlap(pymc, numpyro);
    ScalarAgreement {
        pymc_mean: pymc.mean,
        numpyro_mean: numpyro.mean,
        absolute_difference,
        combined_mcse,
        standardized_difference,
        tolerance,
        intervals_overlap,
        passes: intervals_overlap && absolute_difference <= tolerance,
    }
}

fn compare_field<'a>(
    pymc: impl Iterator<Item = &'a SarScalarSummary>,
    numpyro: impl Iterator<Item = &'a SarScalarSummary>,
    pymc_ess: f64,
    numpyro_ess: f64,
    maximum_standardized_difference: f64,
    minimum_tolerance: f64,
) -> FieldAgreement {
    let mut squared = 0.0;
    let mut count = 0_usize;
    let mut maximum_absolute = 0.0_f64;
    let mut maximum_standardized = 0.0_f64;
    let mut all_intervals_overlap = true;
    let mut passes = true;
    for (pymc, numpyro) in pymc.zip(numpyro) {
        let difference = (pymc.mean - numpyro.mean).abs();
        let combined_mcse = (pymc.sd / pymc_ess.sqrt()).hypot(numpyro.sd / numpyro_ess.sqrt());
        let standardized_difference = standardized(difference, combined_mcse);
        let tolerance = minimum_tolerance.max(maximum_standardized_difference * combined_mcse);
        let intervals_overlap = overlap(pymc, numpyro);
        squared += difference.powi(2);
        count += 1;
        maximum_absolute = maximum_absolute.max(difference);
        maximum_standardized = maximum_standardized.max(standardized_difference);
        all_intervals_overlap &= intervals_overlap;
        passes &= intervals_overlap && difference <= tolerance;
    }
    FieldAgreement {
        root_mean_square_difference: (squared / count as f64).sqrt(),
        maximum_absolute_difference: maximum_absolute,
        maximum_standardized_difference: maximum_standardized,
        all_intervals_overlap,
        passes,
    }
}

fn standardized(difference: f64, combined_mcse: f64) -> f64 {
    if combined_mcse > 0.0 {
        difference / combined_mcse
    } else if difference == 0.0 {
        0.0
    } else {
        f64::MAX
    }
}

fn overlap(left: &SarScalarSummary, right: &SarScalarSummary) -> bool {
    left.interval_lower <= right.interval_upper && right.interval_lower <= left.interval_upper
}

fn backend_matches(result: &WorkerBackend, request: &BackendContract) -> bool {
    result.name == request.name
        && result.version == request.version
        && result.python_version == request.python_version
        && result.environment_lock_sha256 == request.environment_lock_sha256
        && result.worker_sha256 == request.worker_sha256
}

fn read(path: &std::path::Path) -> Result<Vec<u8>, BayesCliError> {
    fs::read(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })
}

pub(super) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<AgreementCli>(|| run_cli().map_err(super::into_marklab_error))
}
