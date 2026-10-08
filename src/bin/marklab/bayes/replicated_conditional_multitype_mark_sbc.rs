use std::{collections::BTreeSet, path::PathBuf};

use clap::{Parser, Subcommand};
use marklab_bayes::{sha256_hex, BackendContract, FitState, NutsSamplingSpec, WorkerBackend};
use serde::{Deserialize, Serialize};

use super::{
    publish_json,
    replicated_arbitrary_window_lgcp_agreement::backend_matches,
    replicated_conditional_multitype_mark::{self, Args as FitArgs},
    run_worker, BayesCliError,
};

const NUMPYRO_VERSION: &str = "0.21.0";
const JAX_VERSION: &str = "0.11.1";
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
    ReplicatedConditionalMultitypeMarkSbc(Box<Args>),
}

#[derive(Debug, clap::Args)]
struct Args {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    reference_group: String,
    #[arg(long)]
    reference_type: String,
    #[arg(long)]
    radius_um: f64,
    #[arg(long)]
    intercept_prior_sd: f64,
    #[arg(long)]
    interaction_prior_sd: f64,
    #[arg(long)]
    group_effect_prior_sd: f64,
    #[arg(long)]
    patient_sd_prior_scale: f64,
    #[arg(long)]
    pattern_sd_prior_scale: f64,
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
    maximum_states_per_pattern: u64,
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
    maximum_states_per_pattern: u64,
    states_per_pattern: u64,
    points_per_pattern: usize,
    pattern_count: usize,
    maximum_enumeration_work: u64,
    enumeration_work: u64,
    maximum_enumeration_bytes: u64,
    enumeration_bytes: u64,
    maximum_total_iterations: u64,
    total_iterations: u64,
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
    simulated_pattern_type_counts: Vec<Vec<u64>>,
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
    states_per_pattern: u64,
    zero_parameter_log_normalizer_per_pattern: f64,
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
    assumptions: [&'static str; 5],
    finite_result_policy: &'static str,
    claim_status: &'static str,
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let Top::Bayes { command } = Cli::parse_from(std::env::args_os()).command;
    let Command::ReplicatedConditionalMultitypeMarkSbc(args) = command;
    run(*args)
}

fn run(args: Args) -> Result<(), BayesCliError> {
    validate_controls(&args)?;
    let sampling = NutsSamplingSpec {
        chains: args.chains,
        tune_per_chain: args.tune,
        draws_per_chain: args.draws,
        target_accept: args.target_accept,
        seed: args.seed,
    };
    let prepared = replicated_conditional_multitype_mark::prepare(FitArgs {
        input: args.input,
        reference_group: args.reference_group,
        reference_type: args.reference_type,
        radius_um: args.radius_um,
        intercept_prior_sd: args.intercept_prior_sd,
        interaction_prior_sd: args.interaction_prior_sd,
        group_effect_prior_sd: args.group_effect_prior_sd,
        patient_sd_prior_scale: args.patient_sd_prior_scale,
        pattern_sd_prior_scale: args.pattern_sd_prior_scale,
        chains: args.chains,
        tune: args.tune,
        draws: args.draws,
        target_accept: args.target_accept,
        seed: args.seed,
        maximum_patients: args.maximum_patients,
        maximum_patterns: args.maximum_patterns,
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
    let input_sha = source["input_sha256"]
        .as_str()
        .filter(|value| value.len() == 64)
        .ok_or_else(|| BayesCliError::Input("prepared input digest differs".into()))?
        .to_owned();
    let types = string_array(&source, "type_ids")?;
    let patterns = source["patterns"]
        .as_array()
        .ok_or_else(|| BayesCliError::Input("prepared patterns differ".into()))?;
    let pattern_count = patterns.len();
    let point_counts = patterns
        .iter()
        .map(|row| row["point_count"].as_u64().map(|value| value as usize))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| BayesCliError::Input("prepared pattern point counts differ".into()))?;
    let points_per_pattern = point_counts[0];
    if !(6..=8).contains(&points_per_pattern)
        || point_counts
            .iter()
            .any(|count| *count != points_per_pattern)
        || types.len() > 4
    {
        return Err(BayesCliError::Input(
            "exact replicated conditional-mark SBC requires six to eight sites per pattern, equal pattern sizes, and at most four types"
                .into(),
        ));
    }
    let states_per_pattern = (types.len() as u64)
        .checked_pow(points_per_pattern as u32)
        .ok_or_else(|| BayesCliError::Input("SBC state count overflows".into()))?;
    let per_replicate_enumeration = patterns.iter().try_fold(0_u64, |total, row| {
        let points = row["point_count"].as_u64()?;
        let edges = row["edge_count"].as_u64()?;
        total.checked_add(states_per_pattern.checked_mul(points.checked_add(edges)?)?)
    });
    let enumeration_work = per_replicate_enumeration
        .and_then(|work| work.checked_mul(u64::from(args.replicates)))
        .ok_or_else(|| BayesCliError::Input("SBC enumeration work overflows".into()))?;
    let enumeration_bytes = states_per_pattern
        .checked_mul(points_per_pattern as u64)
        .and_then(|value| value.checked_mul(2))
        .and_then(|value| value.checked_add(states_per_pattern.checked_mul(16)?))
        .ok_or_else(|| BayesCliError::Input("SBC enumeration bytes overflow".into()))?;
    let total_iterations = u64::from(args.replicates)
        .checked_mul(u64::from(args.chains))
        .and_then(|value| {
            u64::from(args.tune)
                .checked_add(u64::from(args.draws))
                .and_then(|iterations| value.checked_mul(iterations))
        })
        .ok_or_else(|| BayesCliError::Input("SBC iteration work overflows".into()))?;
    if states_per_pattern > args.maximum_states_per_pattern
        || enumeration_work > args.maximum_enumeration_work
        || enumeration_bytes > args.maximum_enumeration_bytes
        || total_iterations > args.maximum_total_iterations
    {
        return Err(BayesCliError::Input(
            "replicated conditional-mark SBC resource ceiling exceeded".into(),
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
        maximum_states_per_pattern: args.maximum_states_per_pattern,
        states_per_pattern,
        points_per_pattern,
        pattern_count,
        maximum_enumeration_work: args.maximum_enumeration_work,
        enumeration_work,
        maximum_enumeration_bytes: args.maximum_enumeration_bytes,
        enumeration_bytes,
        maximum_total_iterations: args.maximum_total_iterations,
        total_iterations,
        maximum_output_bytes: 4 * 1_048_576,
        timeout_seconds: args.timeout_seconds,
    };
    let repository = &marklab::python_backend_assets_root()?;
    let workers = repository.join("workers/python");
    let backend = BackendContract {
        name: "numpyro",
        version: NUMPYRO_VERSION,
        python_version: "3.12",
        environment_lock_sha256: sha256_hex(&read_bounded(
            &workers.join("uv.lock"),
            MAXIMUM_ADAPTER_BYTES,
        )?),
        worker_sha256: sha256_hex(&read_bounded(
            &workers.join("marklab_numpyro_replicated_conditional_multitype_mark_sbc_worker.py"),
            MAXIMUM_ADAPTER_BYTES,
        )?),
    };
    let request = WorkerRequest {
        format: "marklab.numpyro_replicated_conditional_multitype_mark_sbc_request",
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
    let worker_output = run_worker(
        repository,
        "marklab_numpyro_replicated_conditional_multitype_mark_sbc_worker.py",
        &bytes,
        args.timeout_seconds,
    )?;
    let result: WorkerResult = serde_json::from_slice(&worker_output)?;
    validate_result(
        &result,
        &backend,
        &request_sha,
        prepared.request_sha256(),
        &input_sha,
        &types,
        pattern_count,
        points_per_pattern,
        &sampling,
        &calibration,
    )?;
    publish_json(
        &args.out,
        &Output {
            format: "marklab.replicated_conditional_multitype_mark_sbc",
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
            statistical_unit: "patient",
            generative_model:
                "exact_normalized_finite_state_patient_pattern_hierarchical_gibbs",
            fitted_model: "patient_pattern_hierarchical_single_site_composite_pseudoposterior",
            assumptions: [
                "patient_and_pattern_hierarchy_truths_are_drawn_from_the_declared_fit_priors",
                "every_pattern_state_is_enumerated_exactly_conditional_on_its_hierarchy_truth",
                "patients_are_the_population_replicates_and_patterns_remain_nested",
                "every_failed_replicate_is_retained_with_its_exact_reason",
                "calibration_failure_is_not_repaired_by_threshold_or_seed_search",
            ],
            finite_result_policy:
                "accept_only_complete_dispositions_finite_diagnostics_and_declared_rank_coverage_bounds",
            claim_status: "experimental_simulation_calibration",
        },
    )
}

fn validate_controls(args: &Args) -> Result<(), BayesCliError> {
    if !(20..=100).contains(&args.replicates)
        || !args.minimum_rank_uniformity_p_value.is_finite()
        || !(0.0..=1.0).contains(&args.minimum_rank_uniformity_p_value)
        || !args.minimum_coverage_90.is_finite()
        || !args.maximum_coverage_90.is_finite()
        || !(0.0..=args.maximum_coverage_90).contains(&args.minimum_coverage_90)
        || args.maximum_coverage_90 > 1.0
        || args.maximum_states_per_pattern == 0
        || args.maximum_enumeration_work == 0
        || args.maximum_enumeration_bytes == 0
        || args.maximum_total_iterations == 0
    {
        return Err(BayesCliError::Input(
            "replicated conditional-mark SBC controls are invalid".into(),
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
    types: &[String],
    pattern_count: usize,
    points_per_pattern: usize,
    sampling: &NutsSamplingSpec,
    calibration: &Calibration,
) -> Result<(), BayesCliError> {
    let names = parameter_names(types);
    let completed_draws = u64::from(sampling.chains) * u64::from(sampling.draws_per_chain);
    let dispositions = result
        .replicates
        .iter()
        .map(|row| row.replicate)
        .chain(result.failures.iter().map(|row| row.replicate))
        .collect::<BTreeSet<_>>();
    let disposition_valid = result.replicates.len() + result.failures.len()
        == calibration.replicates as usize
        && dispositions == (0..calibration.replicates).collect::<BTreeSet<_>>()
        && result.failures.iter().all(|row| !row.reason.is_empty());
    let rows_valid = result.replicates.iter().all(|row| {
        row.simulated_pattern_type_counts.len() == pattern_count
            && row.simulated_pattern_type_counts.iter().all(|counts| {
                counts.len() == types.len()
                    && counts.iter().sum::<u64>() == points_per_pattern as u64
            })
            && row.parameters.len() == names.len()
            && row.parameters.iter().zip(&names).all(|(parameter, name)| {
                parameter.parameter == *name
                    && parameter.truth.is_finite()
                    && parameter.rank <= completed_draws
            })
            && [row.r_hat, row.ess_bulk, row.ess_tail, row.minimum_ebfmi]
                .into_iter()
                .all(f64::is_finite)
    });
    let diagnostics_valid = result.diagnostics.len() == names.len()
        && result.diagnostics.iter().zip(&names).all(|(row, name)| {
            row.parameter == *name
                && row.rank_histogram.len() == calibration.rank_bins as usize
                && row.rank_histogram.iter().sum::<u64>() == result.replicates.len() as u64
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
    let expected_states = (types.len() as u64).pow(points_per_pattern as u32);
    let expected_zero_log_normalizer = points_per_pattern as f64 * (types.len() as f64).ln();
    if result.format != "marklab.numpyro_replicated_conditional_multitype_mark_sbc_result"
        || result.version != 1
        || !backend_matches(&result.backend, backend)
        || result.jax_version != JAX_VERSION
        || result.request_sha256 != request_sha
        || result.source_request_sha256 != source_sha
        || result.input_sha256 != input_sha
        || result.enumeration_oracle.states_per_pattern != expected_states
        || (result
            .enumeration_oracle
            .zero_parameter_log_normalizer_per_pattern
            - expected_zero_log_normalizer)
            .abs()
            > 1e-12
        || !disposition_valid
        || !rows_valid
        || !diagnostics_valid
        || (result.fit_state == FitState::Complete) != passes
    {
        return Err(BayesCliError::Backend(
            "replicated conditional-mark SBC result differs".into(),
        ));
    }
    Ok(())
}

fn parameter_names(types: &[String]) -> Vec<String> {
    let mut names = Vec::new();
    for prefix in ["baseline_affinity", "group_affinity_shift"] {
        for left in 0..types.len() {
            for right in left + 1..types.len() {
                names.push(format!("{prefix}:{}|{}", types[left], types[right]));
            }
        }
    }
    names.extend([
        "patient_intercept_sd".into(),
        "patient_potential_sd".into(),
        "pattern_intercept_sd".into(),
        "pattern_potential_sd".into(),
    ]);
    names
}

fn string_array(value: &serde_json::Value, name: &str) -> Result<Vec<String>, BayesCliError> {
    value[name]
        .as_array()
        .and_then(|rows| {
            rows.iter()
                .map(|row| row.as_str().map(ToOwned::to_owned))
                .collect::<Option<Vec<_>>>()
        })
        .filter(|rows| !rows.is_empty())
        .ok_or_else(|| BayesCliError::Input(format!("prepared {name} differ")))
}

fn read_bounded(path: &std::path::Path, maximum: u64) -> Result<Vec<u8>, BayesCliError> {
    super::input_file::read_regular_file(
        path,
        maximum,
        "replicated conditional-mark SBC adapter is absent or oversized",
    )
}

pub(super) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<Cli>(|| run_cli().map_err(super::into_marklab_error))
}
