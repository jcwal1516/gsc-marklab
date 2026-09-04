use std::{collections::BTreeMap, fs, path::PathBuf};

use clap::{Parser, Subcommand};
use marklab_bayes::{sha256_hex, BackendContract, NutsSamplingSpec, WorkerBackend};
use serde::{Deserialize, Serialize};

use super::{
    arbitrary_window_ipp_fit::{self, PreparedArbitraryWindowIppFit},
    publish_json, run_worker, BayesCliError,
};

const NUMPYRO_VERSION: &str = "0.21.0";
const JAX_VERSION: &str = "0.11.1";

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct SbcCli {
    #[command(subcommand)]
    command: SbcTopLevel,
}

#[derive(Debug, Subcommand)]
enum SbcTopLevel {
    Bayes {
        #[command(subcommand)]
        command: SbcCommand,
    },
}

#[derive(Debug, Subcommand)]
enum SbcCommand {
    ArbitraryWindowIppSbc(Box<SbcArguments>),
}

#[derive(Debug, clap::Args)]
struct SbcArguments {
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
    replicates: u32,
    #[arg(long)]
    maximum_events: usize,
    #[arg(long)]
    maximum_quadrature_nodes: usize,
    #[arg(long)]
    maximum_generated_count: u64,
    #[arg(long)]
    maximum_total_work: u64,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let SbcTopLevel::Bayes { command } = SbcCli::parse_from(std::env::args_os()).command;
    let SbcCommand::ArbitraryWindowIppSbc(arguments) = command;
    if !(20..=100).contains(&arguments.replicates)
        || arguments.maximum_generated_count == 0
        || arguments.maximum_generated_count > 10_000_000
        || arguments.maximum_total_work == 0
        || arguments.maximum_total_work > 100_000_000
    {
        return Err(BayesCliError::Input(
            "weighted IPP SBC replicates, generated-count ceiling, or work ceiling is invalid"
                .into(),
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
        arguments.maximum_total_work,
        arguments.timeout_seconds,
    )?;
    let result = execute(
        &prepared,
        arguments.replicates,
        arguments.maximum_generated_count,
        arguments.maximum_total_work,
        arguments.timeout_seconds,
    )?;
    publish_json(&arguments.out, &result)
}

#[derive(Serialize)]
struct SbcRequest<'a> {
    format: &'static str,
    version: u32,
    backend: BackendContract,
    jax_version: &'static str,
    source_request_sha256: &'a str,
    source_request: &'a marklab_bayes::InhomogeneousPoissonFitWorkerRequest,
    replicates: u32,
    maximum_generated_count: u64,
    maximum_total_work: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CalibrationSummary {
    replicates: usize,
    posterior_draws_per_replicate: u64,
    normalized_mean_rank: f64,
    coverage_90: f64,
    accepted: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SbcReplicate {
    replicate: u32,
    true_intercept: f64,
    true_coefficient: f64,
    true_total_expected_count: f64,
    realized_total_count: u64,
    intercept_rank: u64,
    coefficient_rank: u64,
    total_expected_count_rank: u64,
    intercept_covered_90: bool,
    coefficient_covered_90: bool,
    total_expected_count_covered_90: bool,
    r_hat: f64,
    divergences: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SbcFailure {
    replicate: u32,
    reason: String,
    true_total_expected_count: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SbcResult {
    format: String,
    version: u32,
    backend: WorkerBackend,
    jax_version: String,
    request_sha256: String,
    source_request_sha256: String,
    requested_replicates: usize,
    completed_replicates: usize,
    failed_replicates: usize,
    replicates: Vec<SbcReplicate>,
    failures: Vec<SbcFailure>,
    calibration: BTreeMap<String, CalibrationSummary>,
    calibration_status: String,
    total_work: u64,
    maximum_generated_count: u64,
    claim_status: String,
}

fn execute(
    prepared: &PreparedArbitraryWindowIppFit,
    replicates: u32,
    maximum_generated_count: u64,
    maximum_total_work: u64,
    timeout_seconds: u64,
) -> Result<SbcResult, BayesCliError> {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path =
        repository.join("workers/python/marklab_numpyro_arbitrary_window_ipp_sbc_worker.py");
    let lock = fs::read(&lock_path).map_err(|source| BayesCliError::Io {
        path: lock_path,
        source,
    })?;
    let worker = fs::read(&worker_path).map_err(|source| BayesCliError::Io {
        path: worker_path,
        source,
    })?;
    let request = SbcRequest {
        format: "marklab.numpyro_arbitrary_window_ipp_sbc_request",
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
        replicates,
        maximum_generated_count,
        maximum_total_work,
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let bytes = run_worker(
        repository,
        "marklab_numpyro_arbitrary_window_ipp_sbc_worker.py",
        &request_bytes,
        timeout_seconds,
    )?;
    let result: SbcResult = serde_json::from_slice(&bytes)?;
    validate(&result, &request, &request_sha256, prepared)?;
    Ok(result)
}

fn validate(
    result: &SbcResult,
    request: &SbcRequest<'_>,
    request_sha256: &str,
    prepared: &PreparedArbitraryWindowIppFit,
) -> Result<(), BayesCliError> {
    let names = ["intercept", "coefficient", "total_expected_count"];
    let expected_draws = u64::from(prepared.request.sampling.chains)
        * u64::from(prepared.request.sampling.draws_per_chain);
    let expected_work = u64::from(request.replicates)
        * u64::from(prepared.request.sampling.chains)
        * u64::from(
            prepared.request.sampling.tune_per_chain + prepared.request.sampling.draws_per_chain,
        )
        * prepared.input.spec.quadrature.len() as u64;
    let calibration_valid = names.iter().all(|name| {
        result.calibration.get(*name).is_some_and(|value| {
            value.replicates == result.completed_replicates
                && value.posterior_draws_per_replicate == expected_draws
                && value.normalized_mean_rank.is_finite()
                && (0.0..=1.0).contains(&value.normalized_mean_rank)
                && value.coverage_90.is_finite()
                && (0.0..=1.0).contains(&value.coverage_90)
                && value.accepted
                    == ((0.25..=0.75).contains(&value.normalized_mean_rank)
                        && (0.65..=1.0).contains(&value.coverage_90))
        })
    });
    let replicates_valid = result.replicates.iter().all(|value| {
        [
            value.true_intercept,
            value.true_coefficient,
            value.true_total_expected_count,
            value.r_hat,
        ]
        .into_iter()
        .all(f64::is_finite)
            && value.true_total_expected_count > 0.0
            && value.intercept_rank <= expected_draws
            && value.coefficient_rank <= expected_draws
            && value.total_expected_count_rank <= expected_draws
    });
    let failures_valid = result.failures.iter().all(|value| {
        matches!(
            value.reason.as_str(),
            "generated_count_resource_ceiling"
                | "realized_count_resource_ceiling"
                | "non_finite_posterior"
        ) && value
            .true_total_expected_count
            .is_none_or(|count| count.is_finite() && count > 0.0)
    });
    let accepted = result.failed_replicates == 0
        && result.completed_replicates == result.requested_replicates
        && names.iter().all(|name| result.calibration[*name].accepted)
        && result
            .replicates
            .iter()
            .all(|value| value.divergences == 0 && value.r_hat <= 1.05);
    if result.format != "marklab.arbitrary_window_ipp_sbc"
        || result.version != 1
        || result.backend.name != request.backend.name
        || result.backend.version != request.backend.version
        || result.backend.python_version != request.backend.python_version
        || result.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
        || result.backend.worker_sha256 != request.backend.worker_sha256
        || result.jax_version != JAX_VERSION
        || result.request_sha256 != request_sha256
        || result.source_request_sha256 != prepared.request_sha256
        || result.requested_replicates != request.replicates as usize
        || result.completed_replicates != result.replicates.len()
        || result.failed_replicates != result.failures.len()
        || result.completed_replicates + result.failed_replicates != result.requested_replicates
        || result.calibration.len() != names.len()
        || !calibration_valid
        || !replicates_valid
        || !failures_valid
        || result.total_work != expected_work
        || result.total_work > request.maximum_total_work
        || result.maximum_generated_count != request.maximum_generated_count
        || (result.calibration_status == "accepted") != accepted
        || !matches!(
            result.calibration_status.as_str(),
            "accepted" | "not_accepted"
        )
        || result.claim_status != "synthetic_prior_generative_calibration_only"
    {
        return Err(BayesCliError::Backend(
            "weighted IPP SBC identity, bounds, or calibration contract differs".into(),
        ));
    }
    Ok(())
}

pub(super) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<SbcCli>(|| run_cli().map_err(super::into_marklab_error))
}
