use std::{fs, path::PathBuf};

use clap::{Parser, Subcommand};
use marklab_bayes::{
    sha256_hex, BackendContract, FitState, NormalMeanDiagnostics, NutsSamplingSpec,
    SamplingSummary, WorkerBackend,
};
use serde::{Deserialize, Serialize};

use super::{
    arbitrary_window_ipp_fit::{
        self, ArbitraryWindowIppFitPosterior, ArbitraryWindowIppFitResult,
        PreparedArbitraryWindowIppFit,
    },
    publish_json, run_worker, BayesCliError,
};

const NUMPYRO_VERSION: &str = "0.21.0";
const JAX_VERSION: &str = "0.11.1";

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct AgreementCli {
    #[command(subcommand)]
    command: AgreementTopLevel,
}

#[derive(Debug, Subcommand)]
enum AgreementTopLevel {
    Bayes {
        #[command(subcommand)]
        command: AgreementCommand,
    },
}

#[derive(Debug, Subcommand)]
enum AgreementCommand {
    ArbitraryWindowIppAgreement(Box<AgreementArguments>),
}

#[derive(Debug, clap::Args)]
struct AgreementArguments {
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
    parameter_absolute_tolerance: f64,
    #[arg(long)]
    total_count_absolute_tolerance: f64,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let AgreementTopLevel::Bayes { command } =
        AgreementCli::parse_from(std::env::args_os()).command;
    let AgreementCommand::ArbitraryWindowIppAgreement(arguments) = command;
    if !arguments.parameter_absolute_tolerance.is_finite()
        || arguments.parameter_absolute_tolerance <= 0.0
        || !arguments.total_count_absolute_tolerance.is_finite()
        || arguments.total_count_absolute_tolerance <= 0.0
    {
        return Err(BayesCliError::Input(
            "arbitrary-window IPP agreement tolerances must be positive and finite".into(),
        ));
    }
    let prepared = arbitrary_window_ipp_fit::prepare(
        arguments.events,
        arguments.quadrature,
        arguments.window,
        arguments.intercept_prior_mean,
        arguments.intercept_prior_sd,
        arguments.coefficient_prior_mean,
        arguments.coefficient_prior_sd,
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
        arguments.timeout_seconds,
    )?;
    let pymc = arbitrary_window_ipp_fit::execute(&prepared)?;
    let numpyro = execute_numpyro(&prepared, arguments.timeout_seconds)?;
    let comparison = compare(
        &pymc,
        &numpyro,
        arguments.parameter_absolute_tolerance,
        arguments.total_count_absolute_tolerance,
    );
    let all_within_tolerance = pymc.fit_state == FitState::Complete
        && numpyro.fit_state == FitState::Complete
        && comparison.intercept.passes
        && comparison.coefficient.passes
        && comparison.total_expected_count.passes;
    let result = AgreementResult {
        format: "marklab.arbitrary_window_ipp_backend_agreement".into(),
        version: 1,
        input: prepared.input_identity.clone(),
        pymc,
        numpyro,
        comparison: AgreementComparison {
            all_within_tolerance,
            ..comparison
        },
        claim_status: "experimental_cross_backend_agreement".into(),
    };
    publish_json(&arguments.out, &result)
}

#[derive(Serialize)]
struct NumpyroRequest<'a> {
    format: &'static str,
    version: u32,
    backend: BackendContract,
    jax_version: &'static str,
    source_request_sha256: &'a str,
    source_request: &'a marklab_bayes::InhomogeneousPoissonFitWorkerRequest,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NumpyroResult {
    format: String,
    version: u32,
    backend: WorkerBackend,
    jax_version: String,
    request_sha256: String,
    source_request_sha256: String,
    fit_state: FitState,
    sampling: SamplingSummary,
    posterior: ArbitraryWindowIppFitPosterior,
    diagnostics: NormalMeanDiagnostics,
    total_expected_count_mean: f64,
    replicated_total_mean: f64,
    replicated_total_sd: f64,
}

fn execute_numpyro(
    prepared: &PreparedArbitraryWindowIppFit,
    timeout_seconds: u64,
) -> Result<NumpyroResult, BayesCliError> {
    let repository = &marklab::python_backend_assets_root()?;
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path =
        repository.join("workers/python/marklab_numpyro_arbitrary_window_ipp_worker.py");
    let lock = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = NumpyroRequest {
        format: "marklab.numpyro_arbitrary_window_ipp_request",
        version: 1,
        backend: BackendContract {
            name: "numpyro",
            version: NUMPYRO_VERSION,
            python_version: "3.12",
            environment_lock_sha256: sha256_hex(&lock),
            worker_sha256: sha256_hex(&worker),
        },
        jax_version: JAX_VERSION,
        source_request_sha256: &prepared.request_sha256,
        source_request: &prepared.request,
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let bytes = run_worker(
        repository,
        "marklab_numpyro_arbitrary_window_ipp_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: NumpyroResult = serde_json::from_slice(&bytes)?;
    validate_numpyro(&result, &request, &request_sha256, prepared)?;
    Ok(result)
}

fn validate_numpyro(
    result: &NumpyroResult,
    request: &NumpyroRequest<'_>,
    request_sha256: &str,
    prepared: &PreparedArbitraryWindowIppFit,
) -> Result<(), BayesCliError> {
    let diagnostics_pass = result.diagnostics.prior_predictive_finite
        && result.diagnostics.posterior_finite
        && result.diagnostics.constraints_valid
        && result.diagnostics.identifiability_checks_passed
        && result.diagnostics.r_hat <= prepared.request.diagnostic_policy.maximum_r_hat
        && result.diagnostics.ess_bulk >= prepared.request.diagnostic_policy.minimum_bulk_ess
        && result.diagnostics.ess_tail >= prepared.request.diagnostic_policy.minimum_tail_ess
        && result.diagnostics.minimum_ebfmi >= prepared.request.diagnostic_policy.minimum_ebfmi
        && result.diagnostics.divergences <= prepared.request.diagnostic_policy.maximum_divergences
        && result.diagnostics.max_tree_depth_hits
            <= prepared.request.diagnostic_policy.maximum_tree_depth_hits;
    let finite = [
        result.posterior.intercept.mean,
        result.posterior.intercept.sd,
        result.posterior.intercept.interval_lower,
        result.posterior.intercept.interval_upper,
        result.posterior.coefficient.mean,
        result.posterior.coefficient.sd,
        result.posterior.coefficient.interval_lower,
        result.posterior.coefficient.interval_upper,
        result.total_expected_count_mean,
        result.replicated_total_mean,
        result.replicated_total_sd,
    ]
    .into_iter()
    .all(f64::is_finite);
    if result.format != "marklab.numpyro_arbitrary_window_ipp_result"
        || result.version != 1
        || result.backend.name != request.backend.name
        || result.backend.version != request.backend.version
        || result.backend.python_version != request.backend.python_version
        || result.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
        || result.backend.worker_sha256 != request.backend.worker_sha256
        || result.jax_version != JAX_VERSION
        || result.request_sha256 != request_sha256
        || result.source_request_sha256 != prepared.request_sha256
        || result.sampling.chains != prepared.request.sampling.chains
        || result.sampling.tune_per_chain != prepared.request.sampling.tune_per_chain
        || result.sampling.draws_per_chain != prepared.request.sampling.draws_per_chain
        || result.sampling.completed_draws
            != u64::from(prepared.request.sampling.chains)
                * u64::from(prepared.request.sampling.draws_per_chain)
        || !finite
        || result.posterior.intercept.sd <= 0.0
        || result.posterior.coefficient.sd <= 0.0
        || result.total_expected_count_mean <= 0.0
        || result.replicated_total_mean < 0.0
        || result.replicated_total_sd < 0.0
        || (result.fit_state == FitState::Complete) != diagnostics_pass
    {
        return Err(BayesCliError::Backend(
            "NumPyro arbitrary-window IPP result identity or diagnostics differ".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct ScalarComparison {
    pymc_mean: f64,
    numpyro_mean: f64,
    absolute_difference: f64,
    tolerance: f64,
    intervals_overlap: bool,
    passes: bool,
}

#[derive(Debug, Serialize)]
struct AgreementComparison {
    intercept: ScalarComparison,
    coefficient: ScalarComparison,
    total_expected_count: ScalarComparison,
    parameter_absolute_tolerance: f64,
    total_count_absolute_tolerance: f64,
    all_within_tolerance: bool,
}

fn compare(
    pymc: &ArbitraryWindowIppFitResult,
    numpyro: &NumpyroResult,
    parameter_tolerance: f64,
    total_tolerance: f64,
) -> AgreementComparison {
    AgreementComparison {
        intercept: scalar(
            &pymc.posterior.intercept,
            &numpyro.posterior.intercept,
            parameter_tolerance,
        ),
        coefficient: scalar(
            &pymc.posterior.coefficient,
            &numpyro.posterior.coefficient,
            parameter_tolerance,
        ),
        total_expected_count: ScalarComparison {
            pymc_mean: pymc.posterior_predictive.total_expected_count_mean,
            numpyro_mean: numpyro.total_expected_count_mean,
            absolute_difference: (pymc.posterior_predictive.total_expected_count_mean
                - numpyro.total_expected_count_mean)
                .abs(),
            tolerance: total_tolerance,
            intervals_overlap: true,
            passes: (pymc.posterior_predictive.total_expected_count_mean
                - numpyro.total_expected_count_mean)
                .abs()
                <= total_tolerance,
        },
        parameter_absolute_tolerance: parameter_tolerance,
        total_count_absolute_tolerance: total_tolerance,
        all_within_tolerance: false,
    }
}

fn scalar(
    pymc: &marklab_bayes::SarScalarSummary,
    numpyro: &marklab_bayes::SarScalarSummary,
    tolerance: f64,
) -> ScalarComparison {
    let absolute_difference = (pymc.mean - numpyro.mean).abs();
    let intervals_overlap = pymc.interval_lower <= numpyro.interval_upper
        && numpyro.interval_lower <= pymc.interval_upper;
    ScalarComparison {
        pymc_mean: pymc.mean,
        numpyro_mean: numpyro.mean,
        absolute_difference,
        tolerance,
        intervals_overlap,
        passes: absolute_difference <= tolerance && intervals_overlap,
    }
}

#[derive(Debug, Serialize)]
struct AgreementResult {
    format: String,
    version: u32,
    input: super::arbitrary_window_ipp_fit::ArbitraryWindowIppFitInputIdentity,
    pymc: ArbitraryWindowIppFitResult,
    numpyro: NumpyroResult,
    comparison: AgreementComparison,
    claim_status: String,
}

pub(super) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<AgreementCli>(|| run_cli().map_err(super::into_marklab_error))
}
