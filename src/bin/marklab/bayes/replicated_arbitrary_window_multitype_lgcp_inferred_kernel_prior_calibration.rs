use std::path::PathBuf;

use clap::{Parser, Subcommand};
use marklab_bayes::{sha256_hex, BackendContract, FitState, WorkerBackend};
use serde::{Deserialize, Serialize};

use super::{
    publish_json,
    replicated_arbitrary_window_lgcp_agreement::backend_matches,
    replicated_arbitrary_window_multitype_lgcp_inferred_kernel::{self, Args as FitArgs},
    run_worker, BayesCliError,
};

const NUMPY_VERSION: &str = "2.4.6";
const MAXIMUM_ADAPTER_BYTES: u64 = 8 * 1_048_576;

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
    ReplicatedArbitraryWindowMultitypeLgcpInferredKernelPriorCalibration(Box<Args>),
}

#[derive(Debug, clap::Args)]
struct Args {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    reference_group: String,
    #[arg(long)]
    comparison_group: String,
    #[arg(long)]
    reference_type: String,
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
    maximum_types: usize,
    #[arg(long)]
    maximum_nodes_per_pattern: usize,
    #[arg(long)]
    maximum_total_nodes: usize,
    #[arg(long)]
    maximum_total_node_type_rows: usize,
    #[arg(long)]
    maximum_total_events: u64,
    #[arg(long)]
    maximum_draw_node_type_work: u64,
    #[arg(long)]
    maximum_kernel_cube_work: u64,
    #[arg(long)]
    maximum_tree_depth: u32,
    #[arg(long)]
    timeout_seconds: u64,
    #[arg(long)]
    replicates: u32,
    #[arg(long)]
    maximum_simulation_node_type_work: u64,
    #[arg(long)]
    maximum_kernel_simulation_work: u64,
    #[arg(long)]
    maximum_working_bytes: u64,
    #[arg(long)]
    maximum_mean_error_sd: f64,
    #[arg(long)]
    maximum_relative_sd_error: f64,
    #[arg(long)]
    maximum_field_whitened_moment_error: f64,
    #[arg(long)]
    maximum_poisson_residual_mean: f64,
    #[arg(long)]
    maximum_poisson_residual_second_moment_error: f64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Clone, Debug, Serialize)]
struct Calibration {
    replicates: u32,
    maximum_mean_error_sd: f64,
    maximum_relative_sd_error: f64,
    maximum_field_whitened_moment_error: f64,
    maximum_poisson_residual_mean: f64,
    maximum_poisson_residual_second_moment_error: f64,
}

#[derive(Clone, Debug, Serialize)]
struct Resources {
    node_type_rows: usize,
    simulation_node_type_work: u64,
    maximum_simulation_node_type_work: u64,
    kernel_cube_work_per_replicate: u64,
    kernel_simulation_work: u64,
    maximum_kernel_simulation_work: u64,
    estimated_working_bytes: u64,
    maximum_working_bytes: u64,
    timeout_seconds: u64,
}

#[derive(Serialize)]
struct WorkerRequest {
    format: &'static str,
    version: u32,
    backend: BackendContract,
    numpy_version: &'static str,
    source_request_sha256: String,
    source_request: serde_json::Value,
    calibration: Calibration,
    resources: Resources,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MomentCheck {
    component: String,
    expected_mean: f64,
    observed_mean: f64,
    mean_error_sd: f64,
    expected_sd: f64,
    observed_sd: f64,
    relative_sd_error: f64,
    passes: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FieldOracle {
    maximum_centering_error: f64,
    whitened_mean: f64,
    whitened_second_moment: f64,
    passes: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PoissonOracle {
    standardized_residual_count: u64,
    standardized_residual_mean: f64,
    standardized_residual_second_moment: f64,
    passes: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerResult {
    format: String,
    version: u32,
    backend: WorkerBackend,
    numpy_version: String,
    input_sha256: String,
    request_sha256: String,
    source_request_sha256: String,
    fit_state: FitState,
    moment_checks: Vec<MomentCheck>,
    field_oracle: FieldOracle,
    poisson_oracle: PoissonOracle,
}

#[derive(Debug, Serialize)]
struct Output {
    format: &'static str,
    version: u32,
    backend: WorkerBackend,
    numpy_version: String,
    input_sha256: String,
    request_sha256: String,
    source_request_sha256: String,
    fit_state: FitState,
    calibration: Calibration,
    resources: Resources,
    moment_checks: Vec<MomentCheck>,
    field_oracle: FieldOracle,
    poisson_oracle: PoissonOracle,
    statistical_unit: &'static str,
    generative_model: &'static str,
    analytic_oracles: [&'static str; 3],
    assumptions: [&'static str; 4],
    finite_result_policy: &'static str,
    claim_status: &'static str,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let Top::Bayes { command } = Cli::parse_from(std::env::args_os()).command;
    let Command::ReplicatedArbitraryWindowMultitypeLgcpInferredKernelPriorCalibration(args) =
        command;
    run(*args)
}

fn run(args: Args) -> Result<(), BayesCliError> {
    validate_controls(&args)?;
    let prepared = replicated_arbitrary_window_multitype_lgcp_inferred_kernel::prepare(FitArgs {
        input: args.input,
        reference_group: args.reference_group,
        comparison_group: args.comparison_group,
        reference_type: args.reference_type,
        intercept_prior_mean: args.intercept_prior_mean,
        intercept_prior_sd: args.intercept_prior_sd,
        group_effect_prior_sd: args.group_effect_prior_sd,
        covariate_effect_prior_sd: args.covariate_effect_prior_sd,
        patient_sd_prior_scale: args.patient_sd_prior_scale,
        pattern_sd_prior_scale: args.pattern_sd_prior_scale,
        field_amplitude_prior_scale: args.field_amplitude_prior_scale,
        field_length_scale_prior_scale_um: args.field_length_scale_prior_scale_um,
        jitter: args.jitter,
        chains: args.chains,
        tune: args.tune,
        draws: args.draws,
        target_accept: args.target_accept,
        seed: args.seed,
        maximum_patients: args.maximum_patients,
        maximum_patterns: args.maximum_patterns,
        maximum_types: args.maximum_types,
        maximum_nodes_per_pattern: args.maximum_nodes_per_pattern,
        maximum_total_nodes: args.maximum_total_nodes,
        maximum_total_node_type_rows: args.maximum_total_node_type_rows,
        maximum_total_events: args.maximum_total_events,
        maximum_draw_node_type_work: args.maximum_draw_node_type_work,
        maximum_kernel_cube_work: args.maximum_kernel_cube_work,
        maximum_tree_depth: args.maximum_tree_depth,
        timeout_seconds: args.timeout_seconds,
        out: PathBuf::new(),
    })?;
    let source: serde_json::Value = serde_json::from_slice(prepared.request_bytes())?;
    let fixed = &source["source_request"];
    let input_sha = fixed["input_sha256"]
        .as_str()
        .filter(|value| value.len() == 64)
        .ok_or_else(|| BayesCliError::Input("prepared calibration input digest differs".into()))?
        .to_owned();
    let rows = fixed["node_type_counts"]
        .as_array()
        .ok_or_else(|| BayesCliError::Input("prepared calibration rows differ".into()))?
        .len();
    let nodes = fixed["nodes"]
        .as_array()
        .ok_or_else(|| BayesCliError::Input("prepared calibration nodes differ".into()))?
        .len();
    let patients = fixed["patients"]
        .as_array()
        .map(Vec::len)
        .ok_or_else(|| BayesCliError::Input("prepared calibration patients differ".into()))?;
    let patterns = fixed["patterns"]
        .as_array()
        .map(Vec::len)
        .ok_or_else(|| BayesCliError::Input("prepared calibration patterns differ".into()))?;
    let types = fixed["type_ids"]
        .as_array()
        .map(Vec::len)
        .ok_or_else(|| BayesCliError::Input("prepared calibration types differ".into()))?;
    let replicates = u64::from(args.replicates);
    let work = replicates
        .checked_mul(rows as u64)
        .ok_or_else(|| BayesCliError::Input("calibration work overflows".into()))?;
    let kernel_cube_work = source["resources"]["kernel_cube_work"]
        .as_u64()
        .ok_or_else(|| BayesCliError::Input("prepared kernel work differs".into()))?;
    let kernel_simulation_work = replicates
        .checked_mul(kernel_cube_work)
        .ok_or_else(|| BayesCliError::Input("calibration kernel work overflows".into()))?;
    let estimated_bytes = replicates
        .checked_mul(types as u64)
        .and_then(|value| value.checked_mul(nodes as u64))
        .and_then(|value| value.checked_mul(112))
        .and_then(|arrays| {
            replicates
                .checked_mul(types as u64)
                .and_then(|value| value.checked_mul((patients + patterns) as u64))
                .and_then(|value| value.checked_mul(32))
                .and_then(|hierarchy| arrays.checked_add(hierarchy))
        })
        .and_then(|value| value.checked_add(384 * 1_048_576))
        .ok_or_else(|| BayesCliError::Input("calibration memory estimate overflows".into()))?;
    if work > args.maximum_simulation_node_type_work
        || kernel_simulation_work > args.maximum_kernel_simulation_work
        || estimated_bytes > args.maximum_working_bytes
    {
        return Err(BayesCliError::Input(
            "inferred multitype calibration resource ceiling exceeded".into(),
        ));
    }
    let calibration = Calibration {
        replicates: args.replicates,
        maximum_mean_error_sd: args.maximum_mean_error_sd,
        maximum_relative_sd_error: args.maximum_relative_sd_error,
        maximum_field_whitened_moment_error: args.maximum_field_whitened_moment_error,
        maximum_poisson_residual_mean: args.maximum_poisson_residual_mean,
        maximum_poisson_residual_second_moment_error: args
            .maximum_poisson_residual_second_moment_error,
    };
    let resources = Resources {
        node_type_rows: rows,
        simulation_node_type_work: work,
        maximum_simulation_node_type_work: args.maximum_simulation_node_type_work,
        kernel_cube_work_per_replicate: kernel_cube_work,
        kernel_simulation_work,
        maximum_kernel_simulation_work: args.maximum_kernel_simulation_work,
        estimated_working_bytes: estimated_bytes,
        maximum_working_bytes: args.maximum_working_bytes,
        timeout_seconds: args.timeout_seconds,
    };
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workers = repository.join("workers/python");
    let worker_name = "marklab_numpy_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_prior_calibration_worker.py";
    let backend = BackendContract {
        name: "numpy",
        version: NUMPY_VERSION,
        python_version: "3.12",
        environment_lock_sha256: sha256_hex(&read_bounded(&workers.join("uv.lock"))?),
        worker_sha256: sha256_hex(&read_bounded(&workers.join(worker_name))?),
    };
    let request = WorkerRequest {
        format: "marklab.numpy_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_prior_calibration_request",
        version: 1, backend: backend.clone(), numpy_version: NUMPY_VERSION,
        source_request_sha256: sha256_hex(prepared.request_bytes()),
        source_request: source, calibration: calibration.clone(), resources: resources.clone(),
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha = sha256_hex(&request_bytes);
    let output = run_worker(
        repository,
        worker_name,
        &request_bytes,
        args.timeout_seconds,
    )?;
    let result: WorkerResult = serde_json::from_slice(&output)?;
    validate_result(
        &result,
        &backend,
        &request_sha,
        &request.source_request_sha256,
        &input_sha,
        &calibration,
        work,
    )?;
    publish_json(&args.out, &Output {
        format: "marklab.replicated_arbitrary_window_multitype_lgcp_inferred_kernel_prior_calibration",
        version: 1, backend: result.backend, numpy_version: result.numpy_version,
        input_sha256: result.input_sha256, request_sha256: result.request_sha256,
        source_request_sha256: result.source_request_sha256, fit_state: result.fit_state,
        calibration, resources, moment_checks: result.moment_checks,
        field_oracle: result.field_oracle, poisson_oracle: result.poisson_oracle,
        statistical_unit: "patient",
        generative_model: "patient_pattern_hierarchical_inferred_shared_matern_multitype_poisson_intensity",
        analytic_oracles: ["gaussian_and_half_normal_prior_moments",
            "conditional_cholesky_whitened_standard_normal_moments",
            "conditional_poisson_standardized_residual_moments"],
        assumptions: ["patients_are_population_units_and_patterns_remain_nested",
            "kernel_scales_are_drawn_from_the_declared_inferred_model_priors",
            "type_fields_are_conditionally_independent_given_shared_kernel_scales",
            "thresholds_and_seed_are_declared_before_simulation"],
        finite_result_policy: "every_metric_must_be_finite_and_every_analytic_check_must_pass",
        claim_status: "experimental_inferred_kernel_prior_generative_calibration",
    })
}

fn validate_controls(args: &Args) -> Result<(), BayesCliError> {
    let values = [
        args.maximum_mean_error_sd,
        args.maximum_relative_sd_error,
        args.maximum_field_whitened_moment_error,
        args.maximum_poisson_residual_mean,
        args.maximum_poisson_residual_second_moment_error,
    ];
    if !(1_024..=65_536).contains(&args.replicates)
        || values
            .into_iter()
            .any(|value| !value.is_finite() || value <= 0.0 || value > 1.0)
        || args.maximum_simulation_node_type_work == 0
        || args.maximum_kernel_simulation_work == 0
        || args.maximum_working_bytes == 0
    {
        return Err(BayesCliError::Input(
            "inferred multitype calibration controls are invalid".into(),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_result(
    result: &WorkerResult,
    backend: &BackendContract,
    request_sha: &str,
    source_sha: &str,
    input_sha: &str,
    calibration: &Calibration,
    work: u64,
) -> Result<(), BayesCliError> {
    let names = [
        "intercept",
        "group_effect",
        "covariate_effect",
        "patient_sd",
        "pattern_sd",
        "field_amplitude",
        "field_length_scale_um",
    ];
    let moments = result.moment_checks.len() == names.len()
        && result.moment_checks.iter().zip(names).all(|(row, name)| {
            row.component == name
                && [
                    row.expected_mean,
                    row.observed_mean,
                    row.mean_error_sd,
                    row.expected_sd,
                    row.observed_sd,
                    row.relative_sd_error,
                ]
                .into_iter()
                .all(f64::is_finite)
                && row.expected_sd > 0.0
                && row.observed_sd >= 0.0
                && row.passes
                    == (row.mean_error_sd <= calibration.maximum_mean_error_sd
                        && row.relative_sd_error <= calibration.maximum_relative_sd_error)
        });
    let field = [
        result.field_oracle.maximum_centering_error,
        result.field_oracle.whitened_mean,
        result.field_oracle.whitened_second_moment,
    ]
    .into_iter()
    .all(f64::is_finite)
        && result.field_oracle.passes
            == (result.field_oracle.maximum_centering_error <= 1e-12
                && result.field_oracle.whitened_mean.abs()
                    <= calibration.maximum_field_whitened_moment_error
                && (result.field_oracle.whitened_second_moment - 1.0).abs()
                    <= calibration.maximum_field_whitened_moment_error);
    let poisson = [
        result.poisson_oracle.standardized_residual_mean,
        result.poisson_oracle.standardized_residual_second_moment,
    ]
    .into_iter()
    .all(f64::is_finite)
        && result.poisson_oracle.standardized_residual_count == work
        && result.poisson_oracle.passes
            == (result.poisson_oracle.standardized_residual_mean.abs()
                <= calibration.maximum_poisson_residual_mean
                && (result.poisson_oracle.standardized_residual_second_moment - 1.0).abs()
                    <= calibration.maximum_poisson_residual_second_moment_error);
    let complete = moments
        && result.moment_checks.iter().all(|row| row.passes)
        && field
        && result.field_oracle.passes
        && poisson
        && result.poisson_oracle.passes;
    if result.format != "marklab.numpy_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_prior_calibration_result"
        || result.version != 1 || !backend_matches(&result.backend, backend)
        || result.numpy_version != NUMPY_VERSION || result.request_sha256 != request_sha
        || result.source_request_sha256 != source_sha || result.input_sha256 != input_sha
        || !moments || !field || !poisson || (result.fit_state == FitState::Complete) != complete {
        return Err(BayesCliError::Backend("inferred multitype calibration result differs".into()));
    }
    Ok(())
}

fn read_bounded(path: &std::path::Path) -> Result<Vec<u8>, BayesCliError> {
    super::input_file::read_regular_file(
        path,
        MAXIMUM_ADAPTER_BYTES,
        "inferred multitype calibration adapter is absent or oversized",
    )
}
