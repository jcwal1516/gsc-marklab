use std::{fs, path::PathBuf};

use clap::{Parser, Subcommand};
use marklab_bayes::{sha256_hex, BackendContract, FitState, NutsSamplingSpec, WorkerBackend};
use serde::{Deserialize, Serialize};

use super::{
    publish_json,
    replicated_arbitrary_window_lgcp_fit::{self, WorkerRequest as FitRequest},
    run_worker, BayesCliError,
};

const NUMPYRO_VERSION: &str = "0.21.0";
const JAX_VERSION: &str = "0.11.1";

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct SbcCli {
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
    ReplicatedArbitraryWindowLgcpSbc(Box<Arguments>),
}

#[derive(Debug, clap::Args)]
struct Arguments {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    reference_group: String,
    #[arg(long)]
    comparison_group: String,
    #[arg(long, allow_hyphen_values = true)]
    intercept_prior_mean: f64,
    #[arg(long)]
    intercept_prior_sd: f64,
    #[arg(long)]
    group_effect_prior_sd: f64,
    #[arg(long)]
    covariate_effect_prior_sd: f64,
    #[arg(long)]
    patient_sd_prior_scale: f64,
    #[arg(long)]
    pattern_sd_prior_scale: f64,
    #[arg(long)]
    field_amplitude: f64,
    #[arg(long)]
    field_length_scale_um: f64,
    #[arg(long)]
    jitter: f64,
    #[arg(long)]
    replicates: u32,
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
    maximum_patients: usize,
    #[arg(long)]
    maximum_patterns: usize,
    #[arg(long)]
    maximum_nodes_per_pattern: usize,
    #[arg(long)]
    maximum_total_nodes: usize,
    #[arg(long)]
    maximum_total_events: u64,
    #[arg(long)]
    maximum_draw_node_work: u64,
    #[arg(long)]
    minimum_rank_uniformity_p_value: f64,
    #[arg(long)]
    minimum_coverage_90: f64,
    #[arg(long)]
    maximum_coverage_90: f64,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Clone, Debug, Serialize)]
struct Calibration {
    replicates: u32,
    rank_bins: u32,
    interval_probability: f64,
    latent_node_index: usize,
    minimum_rank_uniformity_p_value: f64,
    minimum_coverage: f64,
    maximum_coverage: f64,
}

#[derive(Clone, Debug, Serialize)]
struct Resources {
    maximum_replicates: u32,
    maximum_simulated_nodes: u64,
    maximum_total_iterations: u64,
    maximum_output_bytes: usize,
    timeout_seconds: u64,
}

#[derive(Serialize)]
struct WorkerRequest<'a> {
    format: &'static str,
    version: u32,
    backend: BackendContract,
    jax_version: &'static str,
    source_request_sha256: &'a str,
    source_request: &'a FitRequest,
    calibration: Calibration,
    resources: Resources,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Replicate {
    replicate: u32,
    simulated_total_events: u64,
    true_group_effect: f64,
    group_effect_rank: u64,
    group_effect_covered: bool,
    true_patient_sd: f64,
    patient_sd_rank: u64,
    patient_sd_covered: bool,
    true_pattern_sd: f64,
    pattern_sd_rank: u64,
    pattern_sd_covered: bool,
    true_latent_node: f64,
    latent_node_rank: u64,
    latent_node_covered: bool,
    r_hat: f64,
    ess_bulk: f64,
    ess_tail: f64,
    minimum_ebfmi: f64,
    divergences: u64,
    max_tree_depth_hits: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Failure {
    replicate: u32,
    reason: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ParameterDiagnostics {
    rank_histogram: Vec<u64>,
    rank_uniformity_p_value: f64,
    coverage_90: f64,
    mean_normalized_rank: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Diagnostics {
    group_effect: ParameterDiagnostics,
    patient_sd: ParameterDiagnostics,
    pattern_sd: ParameterDiagnostics,
    latent_node: ParameterDiagnostics,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerResult {
    format: String,
    version: u32,
    backend: WorkerBackend,
    jax_version: String,
    input_sha256: String,
    request_sha256: String,
    source_request_sha256: String,
    fit_state: FitState,
    replicates: Vec<Replicate>,
    failures: Vec<Failure>,
    diagnostics: Diagnostics,
    physical_latent_pattern_id: String,
    physical_latent_node_id: String,
}

#[derive(Debug, Serialize)]
struct SbcResult {
    format: String,
    version: u32,
    backend: WorkerBackend,
    jax_version: String,
    input_sha256: String,
    request_sha256: String,
    source_request_sha256: String,
    fit_state: FitState,
    sampling: NutsSamplingSpec,
    calibration: Calibration,
    resources: Resources,
    replicates: Vec<Replicate>,
    failures: Vec<Failure>,
    diagnostics: Diagnostics,
    physical_latent_pattern_id: String,
    physical_latent_node_id: String,
    statistical_unit: String,
    assumptions: Vec<String>,
    finite_result_policy: String,
    claim_status: String,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let TopLevel::Bayes { command } = SbcCli::parse_from(std::env::args_os()).command;
    let Command::ReplicatedArbitraryWindowLgcpSbc(arguments) = command;
    run(*arguments)
}

fn run(arguments: Arguments) -> Result<(), BayesCliError> {
    if !(20..=100).contains(&arguments.replicates)
        || !arguments.minimum_rank_uniformity_p_value.is_finite()
        || !(0.0..=1.0).contains(&arguments.minimum_rank_uniformity_p_value)
        || !arguments.minimum_coverage_90.is_finite()
        || !arguments.maximum_coverage_90.is_finite()
        || !(0.0..=arguments.maximum_coverage_90).contains(&arguments.minimum_coverage_90)
        || arguments.maximum_coverage_90 > 1.0
    {
        return Err(BayesCliError::Input(
            "replicated LGCP SBC controls are invalid".into(),
        ));
    }
    let sampling = NutsSamplingSpec {
        chains: arguments.chains,
        tune_per_chain: arguments.tune,
        draws_per_chain: arguments.draws,
        target_accept: arguments.target_accept,
        seed: arguments.seed,
    };
    let prepared = replicated_arbitrary_window_lgcp_fit::prepare(
        arguments.input,
        arguments.reference_group,
        arguments.comparison_group,
        arguments.intercept_prior_mean,
        arguments.intercept_prior_sd,
        arguments.group_effect_prior_sd,
        arguments.covariate_effect_prior_sd,
        arguments.patient_sd_prior_scale,
        arguments.pattern_sd_prior_scale,
        arguments.field_amplitude,
        arguments.field_length_scale_um,
        arguments.jitter,
        sampling.clone(),
        arguments.maximum_patients,
        arguments.maximum_patterns,
        arguments.maximum_nodes_per_pattern,
        arguments.maximum_total_nodes,
        arguments.maximum_total_events,
        arguments.maximum_draw_node_work,
        arguments.timeout_seconds,
    )?;
    let maximum_simulated_nodes = u64::from(arguments.replicates)
        .checked_mul(arguments.maximum_total_nodes as u64)
        .ok_or_else(|| BayesCliError::Input("replicated LGCP SBC node work overflows".into()))?;
    let maximum_total_iterations = u64::from(arguments.replicates)
        .checked_mul(u64::from(arguments.chains))
        .and_then(|value| value.checked_mul(u64::from(arguments.tune + arguments.draws)))
        .ok_or_else(|| BayesCliError::Input("replicated LGCP SBC iterations overflow".into()))?;
    let calibration = Calibration {
        replicates: arguments.replicates,
        rank_bins: 10,
        interval_probability: 0.9,
        latent_node_index: 0,
        minimum_rank_uniformity_p_value: arguments.minimum_rank_uniformity_p_value,
        minimum_coverage: arguments.minimum_coverage_90,
        maximum_coverage: arguments.maximum_coverage_90,
    };
    let resources = Resources {
        maximum_replicates: arguments.replicates,
        maximum_simulated_nodes,
        maximum_total_iterations,
        maximum_output_bytes: 2 * 1_048_576,
        timeout_seconds: arguments.timeout_seconds,
    };
    let repository = &marklab::python_backend_assets_root()?;
    let directory = repository.join("workers/python");
    let lock = read(&directory.join("uv.lock"))?;
    let worker =
        read(&directory.join("marklab_numpyro_replicated_arbitrary_window_lgcp_sbc_worker.py"))?;
    let backend = BackendContract {
        name: "numpyro",
        version: NUMPYRO_VERSION,
        python_version: "3.12",
        environment_lock_sha256: sha256_hex(&lock),
        worker_sha256: sha256_hex(&worker),
    };
    let request = WorkerRequest {
        format: "marklab.numpyro_replicated_arbitrary_window_lgcp_sbc_request",
        version: 1,
        backend: backend.clone(),
        jax_version: JAX_VERSION,
        source_request_sha256: &prepared.request_sha256,
        source_request: &prepared.request,
        calibration: calibration.clone(),
        resources: resources.clone(),
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let output = run_worker(
        repository,
        "marklab_numpyro_replicated_arbitrary_window_lgcp_sbc_worker.py",
        &request_bytes,
        arguments.timeout_seconds,
    )?;
    let worker_result: WorkerResult = serde_json::from_slice(&output)?;
    validate_result(
        &worker_result,
        &backend,
        &request_sha256,
        &prepared.request,
        &prepared.request_sha256,
        &calibration,
    )?;
    publish_json(
        &arguments.out,
        &SbcResult {
            format: "marklab.replicated_arbitrary_window_lgcp_sbc".into(),
            version: 1,
            backend: worker_result.backend,
            jax_version: worker_result.jax_version,
            input_sha256: worker_result.input_sha256,
            request_sha256: worker_result.request_sha256,
            source_request_sha256: worker_result.source_request_sha256,
            fit_state: worker_result.fit_state,
            sampling,
            calibration,
            resources,
            replicates: worker_result.replicates,
            failures: worker_result.failures,
            diagnostics: worker_result.diagnostics,
            physical_latent_pattern_id: worker_result.physical_latent_pattern_id,
            physical_latent_node_id: worker_result.physical_latent_node_id,
            statistical_unit: "patient".into(),
            assumptions: vec![
                "prior_generative_counts_use_the_exact_replicated_hierarchy".into(),
                "every_failed_replicate_is_retained_with_its_reason".into(),
                "patients_not_patterns_nodes_or_cells_are_population_replicates".into(),
            ],
            finite_result_policy:
                "accept_only_complete_finite_replicates_and_declared_calibration_bounds".into(),
            claim_status: "experimental_simulation_calibration".into(),
        },
    )
}

fn validate_result(
    result: &WorkerResult,
    backend: &BackendContract,
    request_sha256: &str,
    source: &FitRequest,
    source_sha256: &str,
    calibration: &Calibration,
) -> Result<(), BayesCliError> {
    let completed_draws = source.completed_draws();
    let (pattern_id, node_id) = source.first_node_identity();
    let completed = result.replicates.len();
    let dispositions_complete =
        completed + result.failures.len() == calibration.replicates as usize;
    let disposition_indices = result
        .replicates
        .iter()
        .map(|row| row.replicate)
        .chain(result.failures.iter().map(|row| row.replicate))
        .collect::<std::collections::BTreeSet<_>>();
    let dispositions_exact = disposition_indices
        == (0..calibration.replicates).collect::<std::collections::BTreeSet<_>>()
        && result
            .failures
            .iter()
            .all(|failure| !failure.reason.is_empty());
    let replicate_rows_valid = result.replicates.iter().all(|row| {
        let finite = [
            row.true_group_effect,
            row.true_patient_sd,
            row.true_pattern_sd,
            row.true_latent_node,
            row.r_hat,
            row.ess_bulk,
            row.ess_tail,
            row.minimum_ebfmi,
        ]
        .into_iter()
        .all(f64::is_finite);
        finite
            && row.replicate < calibration.replicates
            && row.group_effect_rank <= completed_draws
            && row.patient_sd_rank <= completed_draws
            && row.pattern_sd_rank <= completed_draws
            && row.latent_node_rank <= completed_draws
    });
    let parameters = [
        &result.diagnostics.group_effect,
        &result.diagnostics.patient_sd,
        &result.diagnostics.pattern_sd,
        &result.diagnostics.latent_node,
    ];
    let diagnostic_rows_valid = parameters.iter().all(|diagnostic| {
        diagnostic.rank_histogram.len() == calibration.rank_bins as usize
            && diagnostic.rank_histogram.iter().sum::<u64>() == completed as u64
            && [
                diagnostic.rank_uniformity_p_value,
                diagnostic.coverage_90,
                diagnostic.mean_normalized_rank,
            ]
            .into_iter()
            .all(f64::is_finite)
            && (0.0..=1.0).contains(&diagnostic.rank_uniformity_p_value)
            && (0.0..=1.0).contains(&diagnostic.coverage_90)
            && (0.0..=1.0).contains(&diagnostic.mean_normalized_rank)
    });
    let passes = result.failures.is_empty()
        && parameters.iter().all(|diagnostic| {
            diagnostic.rank_uniformity_p_value >= calibration.minimum_rank_uniformity_p_value
                && (calibration.minimum_coverage..=calibration.maximum_coverage)
                    .contains(&diagnostic.coverage_90)
        });
    if result.format != "marklab.numpyro_replicated_arbitrary_window_lgcp_sbc_result"
        || result.version != 1
        || !backend_matches(&result.backend, backend)
        || result.jax_version != JAX_VERSION
        || result.input_sha256 != source.input_sha256()
        || result.request_sha256 != request_sha256
        || result.source_request_sha256 != source_sha256
        || result.physical_latent_pattern_id != pattern_id
        || result.physical_latent_node_id != node_id
        || !dispositions_complete
        || !dispositions_exact
        || !replicate_rows_valid
        || !diagnostic_rows_valid
        || (result.fit_state == FitState::Complete) != passes
    {
        return Err(BayesCliError::Backend(
            "replicated LGCP SBC result differs".into(),
        ));
    }
    Ok(())
}

fn backend_matches(result: &WorkerBackend, expected: &BackendContract) -> bool {
    result.name == expected.name
        && result.version == expected.version
        && result.python_version == expected.python_version
        && result.environment_lock_sha256 == expected.environment_lock_sha256
        && result.worker_sha256 == expected.worker_sha256
}

fn read(path: &std::path::Path) -> Result<Vec<u8>, BayesCliError> {
    fs::read(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })
}

pub(super) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<SbcCli>(|| run_cli().map_err(super::into_marklab_error))
}
