use std::{collections::BTreeSet, fs, path::PathBuf};

use clap::{Parser, Subcommand};
use marklab_bayes::{sha256_hex, BackendContract, FitState, NutsSamplingSpec, WorkerBackend};
use serde::{Deserialize, Serialize};

use super::{
    publish_json,
    replicated_arbitrary_window_lgcp_agreement::backend_matches,
    replicated_arbitrary_window_lgcp_inferred_kernel::{
        self, PreparedReplicatedArbitraryWindowLgcpInferredKernel,
    },
    run_worker, BayesCliError,
};

const NUMPYRO_VERSION: &str = "0.21.0";
const JAX_VERSION: &str = "0.11.1";

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct Cli {
    #[command(subcommand)]
    command: Top,
}

#[derive(Debug, Subcommand)]
enum Top {
    Bayes {
        #[command(subcommand)]
        command: Command,
    },
}

#[derive(Debug, Subcommand)]
enum Command {
    ReplicatedArbitraryWindowLgcpInferredKernelSbc(Box<Args>),
}

#[derive(Debug, clap::Args)]
struct Args {
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
    field_amplitude_prior_scale: f64,
    #[arg(long)]
    field_length_scale_prior_scale_um: f64,
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
    maximum_kernel_cube_work: u64,
    #[arg(long)]
    maximum_tree_depth: u32,
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
    source_request: serde_json::Value,
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
    true_field_amplitude: f64,
    field_amplitude_rank: u64,
    field_amplitude_covered: bool,
    true_field_length_scale_um: f64,
    field_length_scale_um_rank: u64,
    field_length_scale_um_covered: bool,
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
    field_amplitude: ParameterDiagnostics,
    field_length_scale_um: ParameterDiagnostics,
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
struct Output {
    format: &'static str,
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
    statistical_unit: &'static str,
    assumptions: [&'static str; 4],
    finite_result_policy: &'static str,
    claim_status: &'static str,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let Top::Bayes { command } = Cli::parse_from(std::env::args_os()).command;
    let Command::ReplicatedArbitraryWindowLgcpInferredKernelSbc(args) = command;
    run(*args)
}

fn run(args: Args) -> Result<(), BayesCliError> {
    if !(20..=100).contains(&args.replicates)
        || !args.minimum_rank_uniformity_p_value.is_finite()
        || !(0.0..=1.0).contains(&args.minimum_rank_uniformity_p_value)
        || !args.minimum_coverage_90.is_finite()
        || !args.maximum_coverage_90.is_finite()
        || !(0.0..=args.maximum_coverage_90).contains(&args.minimum_coverage_90)
        || args.maximum_coverage_90 > 1.0
    {
        return Err(BayesCliError::Input(
            "inferred-kernel replicated LGCP SBC controls are invalid".into(),
        ));
    }
    let sampling = NutsSamplingSpec {
        chains: args.chains,
        tune_per_chain: args.tune,
        draws_per_chain: args.draws,
        target_accept: args.target_accept,
        seed: args.seed,
    };
    let prepared = replicated_arbitrary_window_lgcp_inferred_kernel::prepare(
        args.input,
        args.reference_group,
        args.comparison_group,
        args.intercept_prior_mean,
        args.intercept_prior_sd,
        args.group_effect_prior_sd,
        args.covariate_effect_prior_sd,
        args.patient_sd_prior_scale,
        args.pattern_sd_prior_scale,
        args.field_amplitude_prior_scale,
        args.field_length_scale_prior_scale_um,
        args.jitter,
        sampling.clone(),
        args.maximum_patients,
        args.maximum_patterns,
        args.maximum_nodes_per_pattern,
        args.maximum_total_nodes,
        args.maximum_total_events,
        args.maximum_draw_node_work,
        args.maximum_kernel_cube_work,
        args.maximum_tree_depth,
        args.timeout_seconds,
    )?;
    let maximum_simulated_nodes = u64::from(args.replicates)
        .checked_mul(args.maximum_total_nodes as u64)
        .ok_or_else(|| BayesCliError::Input("inferred-kernel SBC node work overflows".into()))?;
    let maximum_total_iterations = u64::from(args.replicates)
        .checked_mul(u64::from(args.chains))
        .and_then(|value| {
            u64::from(args.tune)
                .checked_add(u64::from(args.draws))
                .and_then(|iterations| value.checked_mul(iterations))
        })
        .ok_or_else(|| BayesCliError::Input("inferred-kernel SBC iterations overflow".into()))?;
    let calibration = Calibration {
        replicates: args.replicates,
        rank_bins: 10,
        interval_probability: 0.9,
        latent_node_index: 0,
        minimum_rank_uniformity_p_value: args.minimum_rank_uniformity_p_value,
        minimum_coverage: args.minimum_coverage_90,
        maximum_coverage: args.maximum_coverage_90,
    };
    let resources = Resources {
        maximum_replicates: args.replicates,
        maximum_simulated_nodes,
        maximum_total_iterations,
        maximum_output_bytes: 2 * 1_048_576,
        timeout_seconds: args.timeout_seconds,
    };
    execute(
        prepared,
        sampling,
        calibration,
        resources,
        args.timeout_seconds,
        &args.out,
    )
}

fn execute(
    prepared: PreparedReplicatedArbitraryWindowLgcpInferredKernel,
    sampling: NutsSamplingSpec,
    calibration: Calibration,
    resources: Resources,
    timeout_seconds: u64,
    out: &std::path::Path,
) -> Result<(), BayesCliError> {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let directory = repository.join("workers/python");
    let backend = BackendContract {
        name: "numpyro",
        version: NUMPYRO_VERSION,
        python_version: "3.12",
        environment_lock_sha256: sha256_hex(&read(&directory.join("uv.lock"))?),
        worker_sha256: sha256_hex(&read(&directory.join(
            "marklab_numpyro_replicated_arbitrary_window_lgcp_inferred_kernel_sbc_worker.py",
        ))?),
    };
    let request = WorkerRequest {
        format: "marklab.numpyro_replicated_arbitrary_window_lgcp_inferred_kernel_sbc_request",
        version: 1,
        backend: backend.clone(),
        jax_version: JAX_VERSION,
        source_request_sha256: prepared.request_sha256(),
        source_request: serde_json::from_slice(prepared.request_bytes())?,
        calibration: calibration.clone(),
        resources: resources.clone(),
    };
    let bytes = serde_json::to_vec(&request)?;
    let request_sha = sha256_hex(&bytes);
    let worker_bytes = run_worker(
        repository,
        "marklab_numpyro_replicated_arbitrary_window_lgcp_inferred_kernel_sbc_worker.py",
        &bytes,
        timeout_seconds,
    )?;
    let result: WorkerResult = serde_json::from_slice(&worker_bytes)?;
    validate_result(&result, &prepared, &backend, &request_sha, &calibration)?;
    publish_json(
        out,
        &Output {
            format: "marklab.replicated_arbitrary_window_lgcp_inferred_kernel_sbc",
            version: 1,
            backend: result.backend,
            jax_version: result.jax_version,
            input_sha256: result.input_sha256,
            request_sha256: result.request_sha256,
            source_request_sha256: result.source_request_sha256,
            fit_state: result.fit_state,
            sampling,
            calibration,
            resources,
            replicates: result.replicates,
            failures: result.failures,
            diagnostics: result.diagnostics,
            physical_latent_pattern_id: result.physical_latent_pattern_id,
            physical_latent_node_id: result.physical_latent_node_id,
            statistical_unit: "patient",
            assumptions: [
                "prior_generative_counts_use_the_exact_replicated_hierarchy",
                "kernel_amplitude_and_physical_length_are_drawn_from_declared_priors",
                "every_failed_replicate_is_retained_with_its_reason",
                "patients_not_patterns_nodes_or_cells_are_population_replicates",
            ],
            finite_result_policy:
                "accept_only_complete_finite_replicates_and_declared_calibration_bounds",
            claim_status: "experimental_simulation_calibration",
        },
    )
}

fn validate_result(
    result: &WorkerResult,
    prepared: &PreparedReplicatedArbitraryWindowLgcpInferredKernel,
    backend: &BackendContract,
    request_sha: &str,
    calibration: &Calibration,
) -> Result<(), BayesCliError> {
    let source = prepared.source_request();
    let completed_draws = source.completed_draws();
    let (pattern_id, node_id) = source.first_node_identity();
    let completed = result.replicates.len();
    let indices = result
        .replicates
        .iter()
        .map(|row| row.replicate)
        .chain(result.failures.iter().map(|row| row.replicate))
        .collect::<BTreeSet<_>>();
    let exact_dispositions = result.replicates.len() + result.failures.len()
        == calibration.replicates as usize
        && indices == (0..calibration.replicates).collect::<BTreeSet<_>>()
        && result
            .failures
            .iter()
            .all(|failure| !failure.reason.is_empty());
    let rows_valid = result.replicates.iter().all(|row| {
        [
            row.true_group_effect,
            row.true_patient_sd,
            row.true_pattern_sd,
            row.true_field_amplitude,
            row.true_field_length_scale_um,
            row.true_latent_node,
            row.r_hat,
            row.ess_bulk,
            row.ess_tail,
            row.minimum_ebfmi,
        ]
        .into_iter()
        .all(f64::is_finite)
            && row.replicate < calibration.replicates
            && [
                row.group_effect_rank,
                row.patient_sd_rank,
                row.pattern_sd_rank,
                row.field_amplitude_rank,
                row.field_length_scale_um_rank,
                row.latent_node_rank,
            ]
            .into_iter()
            .all(|rank| rank <= completed_draws)
    });
    let parameters = [
        &result.diagnostics.group_effect,
        &result.diagnostics.patient_sd,
        &result.diagnostics.pattern_sd,
        &result.diagnostics.field_amplitude,
        &result.diagnostics.field_length_scale_um,
        &result.diagnostics.latent_node,
    ];
    let diagnostics_valid = parameters.iter().all(|diagnostic| {
        diagnostic.rank_histogram.len() == calibration.rank_bins as usize
            && diagnostic.rank_histogram.iter().sum::<u64>() == completed as u64
            && [
                diagnostic.rank_uniformity_p_value,
                diagnostic.coverage_90,
                diagnostic.mean_normalized_rank,
            ]
            .into_iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(&value))
    });
    let passes = result.failures.is_empty()
        && parameters.iter().all(|diagnostic| {
            diagnostic.rank_uniformity_p_value >= calibration.minimum_rank_uniformity_p_value
                && (calibration.minimum_coverage..=calibration.maximum_coverage)
                    .contains(&diagnostic.coverage_90)
        });
    if result.format
        != "marklab.numpyro_replicated_arbitrary_window_lgcp_inferred_kernel_sbc_result"
        || result.version != 1
        || !backend_matches(&result.backend, backend)
        || result.jax_version != JAX_VERSION
        || result.input_sha256 != source.input_sha256()
        || result.request_sha256 != request_sha
        || result.source_request_sha256 != prepared.request_sha256()
        || result.physical_latent_pattern_id != pattern_id
        || result.physical_latent_node_id != node_id
        || !exact_dispositions
        || !rows_valid
        || !diagnostics_valid
        || (result.fit_state == FitState::Complete) != passes
    {
        return Err(BayesCliError::Backend(
            "inferred-kernel replicated LGCP SBC result differs".into(),
        ));
    }
    Ok(())
}

fn read(path: &std::path::Path) -> Result<Vec<u8>, BayesCliError> {
    fs::read(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })
}
