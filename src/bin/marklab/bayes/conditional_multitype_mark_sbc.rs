use std::{collections::BTreeSet, fs, path::PathBuf};

use clap::{Parser, Subcommand};
use marklab_bayes::{sha256_hex, BackendContract, FitState, NutsSamplingSpec, WorkerBackend};
use serde::{Deserialize, Serialize};

use super::{
    conditional_multitype_mark::{self, Args as FitArgs},
    publish_json,
    replicated_arbitrary_window_lgcp_agreement::backend_matches,
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
    ConditionalMultitypeMarkSbc(Box<Args>),
}

#[derive(Debug, clap::Args)]
struct Args {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    reference_type: String,
    #[arg(long)]
    radius_um: f64,
    #[arg(long)]
    intercept_prior_sd: f64,
    #[arg(long)]
    interaction_prior_sd: f64,
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
    maximum_points: usize,
    #[arg(long)]
    maximum_types: usize,
    #[arg(long)]
    maximum_neighbor_visits: u64,
    #[arg(long)]
    maximum_edges: usize,
    #[arg(long)]
    maximum_draw_parameter_work: u64,
    #[arg(long)]
    maximum_working_bytes: usize,
    #[arg(long)]
    maximum_tree_depth: u32,
    #[arg(long)]
    maximum_states: u64,
    #[arg(long)]
    maximum_enumeration_work: u64,
    #[arg(long)]
    maximum_enumeration_bytes: u64,
    #[arg(long)]
    maximum_total_iterations: u64,
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
    minimum_rank_uniformity_p_value: f64,
    minimum_coverage: f64,
    maximum_coverage: f64,
}

#[derive(Clone, Debug, Serialize)]
struct Resources {
    maximum_states: u64,
    state_count: u64,
    maximum_enumeration_work: u64,
    enumeration_work: u64,
    maximum_enumeration_bytes: u64,
    enumeration_bytes: u64,
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
struct ReplicateParameter {
    parameter: String,
    truth: f64,
    rank: u64,
    covered: bool,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Replicate {
    replicate: u32,
    simulated_type_counts: Vec<u64>,
    parameters: Vec<ReplicateParameter>,
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
struct Diagnostic {
    parameter: String,
    rank_histogram: Vec<u64>,
    rank_uniformity_p_value: f64,
    coverage_90: f64,
    mean_normalized_rank: f64,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EnumerationOracle {
    state_count: u64,
    zero_parameter_log_normalizer: f64,
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
    enumeration_oracle: EnumerationOracle,
    replicates: Vec<Replicate>,
    failures: Vec<Failure>,
    diagnostics: Vec<Diagnostic>,
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
    enumeration_oracle: EnumerationOracle,
    replicates: Vec<Replicate>,
    failures: Vec<Failure>,
    diagnostics: Vec<Diagnostic>,
    statistical_unit: &'static str,
    generative_model: &'static str,
    fitted_model: &'static str,
    assumptions: [&'static str; 4],
    finite_result_policy: &'static str,
    claim_status: &'static str,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let Top::Bayes { command } = Cli::parse_from(std::env::args_os()).command;
    let Command::ConditionalMultitypeMarkSbc(args) = command;
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
        || args.maximum_states == 0
        || args.maximum_enumeration_work == 0
        || args.maximum_enumeration_bytes == 0
    {
        return Err(BayesCliError::Input(
            "conditional multitype SBC controls are invalid".into(),
        ));
    }
    let sampling = NutsSamplingSpec {
        chains: args.chains,
        tune_per_chain: args.tune,
        draws_per_chain: args.draws,
        target_accept: args.target_accept,
        seed: args.seed,
    };
    let prepared = conditional_multitype_mark::prepare(FitArgs {
        input: args.input,
        reference_type: args.reference_type,
        radius_um: args.radius_um,
        intercept_prior_sd: args.intercept_prior_sd,
        interaction_prior_sd: args.interaction_prior_sd,
        chains: args.chains,
        tune: args.tune,
        draws: args.draws,
        target_accept: args.target_accept,
        seed: args.seed,
        maximum_points: args.maximum_points,
        maximum_types: args.maximum_types,
        maximum_neighbor_visits: args.maximum_neighbor_visits,
        maximum_edges: args.maximum_edges,
        maximum_draw_parameter_work: args.maximum_draw_parameter_work,
        maximum_working_bytes: args.maximum_working_bytes,
        maximum_tree_depth: args.maximum_tree_depth,
        timeout_seconds: args.timeout_seconds,
        out: PathBuf::new(),
    })?;
    let source: serde_json::Value = serde_json::from_slice(prepared.request_bytes())?;
    let point_count = source["points"]
        .as_array()
        .ok_or_else(|| BayesCliError::Input("prepared points differ".into()))?
        .len();
    let type_ids = source["type_ids"]
        .as_array()
        .ok_or_else(|| BayesCliError::Input("prepared types differ".into()))?
        .iter()
        .map(|value| value.as_str().unwrap_or_default().to_owned())
        .collect::<Vec<_>>();
    let edge_count = source["edges"]
        .as_array()
        .ok_or_else(|| BayesCliError::Input("prepared edges differ".into()))?
        .len();
    if point_count > 10 || type_ids.len() > 4 || type_ids.iter().any(String::is_empty) {
        return Err(BayesCliError::Input(
            "exact conditional multitype SBC requires at most ten sites and four types".into(),
        ));
    }
    let state_count = (type_ids.len() as u64)
        .checked_pow(point_count as u32)
        .ok_or_else(|| BayesCliError::Input("SBC state count overflows".into()))?;
    let enumeration_work = state_count
        .checked_mul((point_count + edge_count) as u64)
        .ok_or_else(|| BayesCliError::Input("SBC enumeration work overflows".into()))?;
    let enumeration_bytes = state_count
        .checked_mul(point_count as u64)
        .and_then(|value| value.checked_mul(2))
        .and_then(|value| value.checked_add(state_count.checked_mul(16)?))
        .ok_or_else(|| BayesCliError::Input("SBC enumeration bytes overflow".into()))?;
    let total_iterations = u64::from(args.replicates)
        .checked_mul(u64::from(args.chains))
        .and_then(|value| {
            u64::from(args.tune)
                .checked_add(u64::from(args.draws))
                .and_then(|iterations| value.checked_mul(iterations))
        })
        .ok_or_else(|| BayesCliError::Input("SBC total iterations overflow".into()))?;
    if state_count > args.maximum_states
        || enumeration_work > args.maximum_enumeration_work
        || enumeration_bytes > args.maximum_enumeration_bytes
        || total_iterations > args.maximum_total_iterations
    {
        return Err(BayesCliError::Input(
            "conditional multitype SBC resource ceiling exceeded".into(),
        ));
    }
    let calibration = Calibration {
        replicates: args.replicates,
        rank_bins: 10,
        interval_probability: 0.9,
        minimum_rank_uniformity_p_value: args.minimum_rank_uniformity_p_value,
        minimum_coverage: args.minimum_coverage_90,
        maximum_coverage: args.maximum_coverage_90,
    };
    let resources = Resources {
        maximum_states: args.maximum_states,
        state_count,
        maximum_enumeration_work: args.maximum_enumeration_work,
        enumeration_work,
        maximum_enumeration_bytes: args.maximum_enumeration_bytes,
        enumeration_bytes,
        maximum_total_iterations: args.maximum_total_iterations,
        maximum_output_bytes: 2 * 1_048_576,
        timeout_seconds: args.timeout_seconds,
    };
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let directory = repository.join("workers/python");
    let backend = BackendContract {
        name: "numpyro",
        version: NUMPYRO_VERSION,
        python_version: "3.12",
        environment_lock_sha256: sha256_hex(&read(&directory.join("uv.lock"))?),
        worker_sha256: sha256_hex(&read(
            &directory.join("marklab_numpyro_conditional_multitype_mark_sbc_worker.py"),
        )?),
    };
    let request = WorkerRequest {
        format: "marklab.numpyro_conditional_multitype_mark_sbc_request",
        version: 1,
        backend: backend.clone(),
        jax_version: JAX_VERSION,
        source_request_sha256: prepared.request_sha256(),
        source_request: source,
        calibration: calibration.clone(),
        resources: resources.clone(),
    };
    let bytes = serde_json::to_vec(&request)?;
    let request_sha = sha256_hex(&bytes);
    let output = run_worker(
        repository,
        "marklab_numpyro_conditional_multitype_mark_sbc_worker.py",
        &bytes,
        args.timeout_seconds,
    )?;
    let result: WorkerResult = serde_json::from_slice(&output)?;
    validate_result(
        &result,
        &backend,
        &request_sha,
        prepared.request_sha256(),
        &type_ids,
        point_count,
        &sampling,
        &calibration,
    )?;
    publish_json(
        &args.out,
        &Output {
            format: "marklab.conditional_multitype_mark_sbc",
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
            enumeration_oracle: result.enumeration_oracle,
            replicates: result.replicates,
            failures: result.failures,
            diagnostics: result.diagnostics,
            statistical_unit: "one_fixed_location_pattern",
            generative_model: "exact_normalized_finite_state_symmetric_categorical_gibbs",
            fitted_model: "single_site_conditional_composite_pseudoposterior",
            assumptions: [
                "all_finite_joint_states_are_enumerated_exactly",
                "truths_are_drawn_from_the_declared_fit_priors",
                "every_failed_replicate_is_retained_with_its_reason",
                "calibration_failure_is_not_repaired_by_threshold_or_seed_search",
            ],
            finite_result_policy: "accept_only_exact_dispositions_finite_diagnostics_and_declared_rank_coverage_bounds",
            claim_status: "experimental_simulation_calibration",
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_result(
    result: &WorkerResult,
    backend: &BackendContract,
    request_sha: &str,
    source_sha: &str,
    type_ids: &[String],
    point_count: usize,
    sampling: &NutsSamplingSpec,
    calibration: &Calibration,
) -> Result<(), BayesCliError> {
    let expected_names = parameter_names(type_ids);
    let completed_draws = u64::from(sampling.chains) * u64::from(sampling.draws_per_chain);
    let indices = result
        .replicates
        .iter()
        .map(|row| row.replicate)
        .chain(result.failures.iter().map(|row| row.replicate))
        .collect::<BTreeSet<_>>();
    let dispositions_valid = result.replicates.len() + result.failures.len()
        == calibration.replicates as usize
        && indices == (0..calibration.replicates).collect::<BTreeSet<_>>()
        && result.failures.iter().all(|row| !row.reason.is_empty());
    let completed = result.replicates.len();
    let rows_valid = result.replicates.iter().all(|row| {
        row.simulated_type_counts.len() == type_ids.len()
            && row.simulated_type_counts.iter().sum::<u64>() == point_count as u64
            && row.parameters.len() == expected_names.len()
            && row
                .parameters
                .iter()
                .zip(&expected_names)
                .all(|(parameter, expected)| {
                    parameter.parameter == *expected
                        && parameter.truth.is_finite()
                        && parameter.rank <= completed_draws
                })
            && [row.r_hat, row.ess_bulk, row.ess_tail, row.minimum_ebfmi]
                .into_iter()
                .all(f64::is_finite)
    });
    let diagnostics_valid = result.diagnostics.len() == expected_names.len()
        && result
            .diagnostics
            .iter()
            .zip(&expected_names)
            .all(|(row, expected)| {
                row.parameter == *expected
                    && row.rank_histogram.len() == calibration.rank_bins as usize
                    && row.rank_histogram.iter().sum::<u64>() == completed as u64
                    && [
                        row.rank_uniformity_p_value,
                        row.coverage_90,
                        row.mean_normalized_rank,
                    ]
                    .into_iter()
                    .all(|value| value.is_finite() && (0.0..=1.0).contains(&value))
            });
    let passes = result.failures.is_empty()
        && result.diagnostics.iter().all(|row| {
            row.rank_uniformity_p_value >= calibration.minimum_rank_uniformity_p_value
                && (calibration.minimum_coverage..=calibration.maximum_coverage)
                    .contains(&row.coverage_90)
        });
    if result.format != "marklab.numpyro_conditional_multitype_mark_sbc_result"
        || result.version != 1
        || !backend_matches(&result.backend, backend)
        || result.jax_version != JAX_VERSION
        || result.request_sha256 != request_sha
        || result.source_request_sha256 != source_sha
        || result.enumeration_oracle.state_count != (type_ids.len() as u64).pow(point_count as u32)
        || !result
            .enumeration_oracle
            .zero_parameter_log_normalizer
            .is_finite()
        || !dispositions_valid
        || !rows_valid
        || !diagnostics_valid
        || (result.fit_state == FitState::Complete) != passes
    {
        return Err(BayesCliError::Backend(
            "conditional multitype SBC result differs".into(),
        ));
    }
    Ok(())
}

fn parameter_names(type_ids: &[String]) -> Vec<String> {
    let mut names = type_ids[1..]
        .iter()
        .map(|identity| format!("intercept:{identity}"))
        .collect::<Vec<_>>();
    for left in 0..type_ids.len() {
        for right in left..type_ids.len() {
            if left != 0 || right != 0 {
                names.push(format!("potential:{}|{}", type_ids[left], type_ids[right]));
            }
        }
    }
    for left in 0..type_ids.len() {
        for right in left + 1..type_ids.len() {
            names.push(format!("affinity:{}|{}", type_ids[left], type_ids[right]));
        }
    }
    names
}

fn read(path: &std::path::Path) -> Result<Vec<u8>, BayesCliError> {
    fs::read(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })
}

pub(super) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<Cli>(|| run_cli().map_err(super::into_marklab_error))
}
