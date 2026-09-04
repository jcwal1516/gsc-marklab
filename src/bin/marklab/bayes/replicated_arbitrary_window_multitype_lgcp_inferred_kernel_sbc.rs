use std::{collections::BTreeMap, fs, path::PathBuf};

use clap::{Parser, Subcommand};
use marklab_bayes::{sha256_hex, BackendContract, FitState, WorkerBackend};
use serde::{Deserialize, Serialize};

use super::{
    publish_json,
    replicated_arbitrary_window_lgcp_agreement::backend_matches,
    replicated_arbitrary_window_multitype_lgcp_inferred_kernel::{self, Args as FitArgs},
    run_worker, BayesCliError,
};

const NUMPYRO_VERSION: &str = "0.21.0";
const JAX_VERSION: &str = "0.11.1";
const SCENARIOS: [&str; 5] = [
    "positive",
    "null",
    "weak_identification",
    "boundary",
    "misspecified",
];

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
    ReplicatedArbitraryWindowMultitypeLgcpInferredKernelSbc(Box<Args>),
}

#[derive(Clone, Debug, clap::Args)]
pub(crate) struct Args {
    #[arg(long)]
    pub(crate) input: PathBuf,
    #[arg(long)]
    pub(crate) reference_group: String,
    #[arg(long)]
    pub(crate) comparison_group: String,
    #[arg(long)]
    pub(crate) reference_type: String,
    #[arg(long, allow_hyphen_values = true)]
    pub(crate) intercept_prior_mean: f64,
    #[arg(long)]
    pub(crate) intercept_prior_sd: f64,
    #[arg(long)]
    pub(crate) group_effect_prior_sd: f64,
    #[arg(long)]
    pub(crate) covariate_effect_prior_sd: f64,
    #[arg(long)]
    pub(crate) patient_sd_prior_scale: f64,
    #[arg(long)]
    pub(crate) pattern_sd_prior_scale: f64,
    #[arg(long)]
    pub(crate) field_amplitude_prior_scale: f64,
    #[arg(long)]
    pub(crate) field_length_scale_prior_scale_um: f64,
    #[arg(long)]
    pub(crate) jitter: f64,
    #[arg(long)]
    pub(crate) replicates_per_scenario: u32,
    #[arg(long)]
    pub(crate) chains: u32,
    #[arg(long)]
    pub(crate) tune: u32,
    #[arg(long)]
    pub(crate) draws: u32,
    #[arg(long)]
    pub(crate) target_accept: f64,
    #[arg(long)]
    pub(crate) seed: u64,
    #[arg(long)]
    pub(crate) maximum_patients: usize,
    #[arg(long)]
    pub(crate) maximum_patterns: usize,
    #[arg(long)]
    pub(crate) maximum_types: usize,
    #[arg(long)]
    pub(crate) maximum_nodes_per_pattern: usize,
    #[arg(long)]
    pub(crate) maximum_total_nodes: usize,
    #[arg(long)]
    pub(crate) maximum_total_node_type_rows: usize,
    #[arg(long)]
    pub(crate) maximum_total_events: u64,
    #[arg(long)]
    pub(crate) maximum_draw_node_type_work: u64,
    #[arg(long)]
    pub(crate) maximum_kernel_cube_work: u64,
    #[arg(long)]
    pub(crate) maximum_tree_depth: u32,
    #[arg(long)]
    pub(crate) maximum_simulated_node_type_work: u64,
    #[arg(long)]
    pub(crate) maximum_total_iterations: u64,
    #[arg(long)]
    pub(crate) maximum_ppc_pair_work: u64,
    #[arg(long)]
    pub(crate) maximum_working_bytes: u64,
    #[arg(long)]
    pub(crate) timeout_seconds: u64,
    #[arg(long)]
    pub(crate) out: PathBuf,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Calibration {
    scenarios: Vec<String>,
    replicates_per_scenario: u32,
    rank_bins: u32,
    interval_probability: f64,
    latent_node_index: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Resources {
    scenario_replicates: u64,
    simulated_node_type_work: u64,
    maximum_simulated_node_type_work: u64,
    total_iterations: u64,
    maximum_total_iterations: u64,
    ppc_pair_work: u64,
    maximum_ppc_pair_work: u64,
    estimated_working_bytes: u64,
    maximum_working_bytes: u64,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Truth {
    intercept_type_a: f64,
    intercept_type_b: f64,
    group_effect_difference: f64,
    patient_sd_type_a: f64,
    pattern_sd_type_a: f64,
    field_amplitude: f64,
    field_length_scale_um: f64,
    latent_node_type_a: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Ranks {
    intercept_type_a: u64,
    group_effect_difference: u64,
    patient_sd_type_a: u64,
    pattern_sd_type_a: u64,
    field_amplitude: u64,
    field_length_scale_um: u64,
    latent_node_type_a: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Coverage {
    intercept_type_a: bool,
    group_effect_difference: bool,
    patient_sd_type_a: bool,
    pattern_sd_type_a: bool,
    field_amplitude: bool,
    field_length_scale_um: bool,
    latent_node_type_a: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PpcMetric {
    observed: f64,
    replicated_mean: f64,
    two_sided_tail_probability: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PosteriorPredictive {
    counts: PpcMetric,
    cross_type_enrichment: PpcMetric,
    mark_proportions: PpcMetric,
    clustering: PpcMetric,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScenarioDisposition {
    scenario: String,
    replicate: u32,
    status: String,
    failure_reason: Option<String>,
    simulated_total_events: u64,
    truth: Truth,
    ranks: Option<Ranks>,
    coverage_90: Option<Coverage>,
    posterior_predictive: Option<PosteriorPredictive>,
    r_hat: Option<f64>,
    ess_bulk: Option<f64>,
    ess_tail: Option<f64>,
    minimum_ebfmi: Option<f64>,
    divergences: Option<u64>,
    max_tree_depth_hits: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ParameterDiagnostics {
    completed_replicates: u64,
    rank_histogram: Vec<u64>,
    coverage_90: f64,
    mean_normalized_rank: f64,
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
    scenario_dispositions: Vec<ScenarioDisposition>,
    rank_diagnostics: BTreeMap<String, ParameterDiagnostics>,
    pattern_representation: String,
    cross_type_dependence_estimand: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Output {
    format: String,
    version: u32,
    backend: WorkerBackend,
    jax_version: String,
    input_sha256: String,
    request_sha256: String,
    source_request_sha256: String,
    fit_state: FitState,
    calibration: Calibration,
    resources: Resources,
    scenario_dispositions: Vec<ScenarioDisposition>,
    rank_diagnostics: BTreeMap<String, ParameterDiagnostics>,
    pattern_representation: String,
    cross_type_dependence_estimand: String,
    statistical_unit: String,
    null_model: String,
    assumptions: Vec<String>,
    failure_policy: String,
    finite_result_policy: String,
    claim_status: String,
}

pub(crate) struct Prepared {
    fit: replicated_arbitrary_window_multitype_lgcp_inferred_kernel::Prepared,
    backend: BackendContract,
    request_bytes: Vec<u8>,
    request_sha256: String,
    input_sha256: String,
    calibration: Calibration,
    resources: Resources,
    timeout_seconds: u64,
}

impl Prepared {
    pub(crate) fn request_bytes(&self) -> &[u8] {
        &self.request_bytes
    }
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let Top::Bayes { command } = Cli::parse_from(std::env::args_os()).command;
    let Command::ReplicatedArbitraryWindowMultitypeLgcpInferredKernelSbc(args) = command;
    let out = args.out.clone();
    let prepared = prepare(*args)?;
    publish_json(&out, &execute(&prepared)?)
}

pub(crate) fn prepare(args: Args) -> Result<Prepared, BayesCliError> {
    if !(1..=20).contains(&args.replicates_per_scenario)
        || args.maximum_simulated_node_type_work == 0
        || args.maximum_total_iterations == 0
        || args.maximum_ppc_pair_work == 0
        || args.maximum_working_bytes == 0
        || args.maximum_working_bytes > 8 * 1024 * 1024 * 1024
    {
        return Err(BayesCliError::Input(
            "multitype SBC controls are invalid".into(),
        ));
    }
    let fit = replicated_arbitrary_window_multitype_lgcp_inferred_kernel::prepare(FitArgs {
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
    let source: serde_json::Value = serde_json::from_slice(fit.request_bytes())?;
    let fixed = &source["source_request"];
    let rows = fixed["node_type_counts"]
        .as_array()
        .ok_or_else(|| BayesCliError::Input("prepared SBC rows differ".into()))?
        .len() as u64;
    let nodes = fixed["nodes"]
        .as_array()
        .ok_or_else(|| BayesCliError::Input("prepared SBC nodes differ".into()))?
        .len() as u64;
    let input_sha256 = fixed["input_sha256"]
        .as_str()
        .ok_or_else(|| BayesCliError::Input("prepared SBC input digest differs".into()))?
        .to_owned();
    let scenario_replicates = u64::from(args.replicates_per_scenario) * SCENARIOS.len() as u64;
    let simulated_node_type_work = scenario_replicates
        .checked_mul(rows)
        .ok_or_else(|| BayesCliError::Input("SBC simulation work overflows".into()))?;
    let total_iterations = scenario_replicates
        .checked_mul(u64::from(args.chains))
        .and_then(|value| value.checked_mul(u64::from(args.tune) + u64::from(args.draws)))
        .ok_or_else(|| BayesCliError::Input("SBC iteration work overflows".into()))?;
    let completed_draws = u64::from(args.chains) * u64::from(args.draws);
    let ppc_pair_work = scenario_replicates
        .checked_mul(nodes)
        .and_then(|value| value.checked_mul(nodes))
        .and_then(|value| value.checked_mul(completed_draws))
        .ok_or_else(|| BayesCliError::Input("SBC PPC work overflows".into()))?;
    let estimated_working_bytes = scenario_replicates
        .checked_mul(rows)
        .and_then(|value| value.checked_mul(256))
        .and_then(|value| value.checked_add(384 * 1024 * 1024))
        .ok_or_else(|| BayesCliError::Input("SBC memory estimate overflows".into()))?;
    if simulated_node_type_work > args.maximum_simulated_node_type_work
        || total_iterations > args.maximum_total_iterations
        || ppc_pair_work > args.maximum_ppc_pair_work
        || estimated_working_bytes > args.maximum_working_bytes
    {
        return Err(BayesCliError::Input(
            "multitype SBC resource ceiling exceeded".into(),
        ));
    }
    let calibration = Calibration {
        scenarios: SCENARIOS.into_iter().map(str::to_owned).collect(),
        replicates_per_scenario: args.replicates_per_scenario,
        rank_bins: 10,
        interval_probability: 0.9,
        latent_node_index: 0,
    };
    let resources = Resources {
        scenario_replicates,
        simulated_node_type_work,
        maximum_simulated_node_type_work: args.maximum_simulated_node_type_work,
        total_iterations,
        maximum_total_iterations: args.maximum_total_iterations,
        ppc_pair_work,
        maximum_ppc_pair_work: args.maximum_ppc_pair_work,
        estimated_working_bytes,
        maximum_working_bytes: args.maximum_working_bytes,
        maximum_output_bytes: 4 * 1_048_576,
        timeout_seconds: args.timeout_seconds,
    };
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let directory = repository.join("workers/python");
    let backend = BackendContract {
        name: "numpyro",
        version: NUMPYRO_VERSION,
        python_version: "3.12",
        environment_lock_sha256: sha256_hex(&read(&directory.join("uv.lock"))?),
        worker_sha256: sha256_hex(&read(&directory.join(
            "marklab_numpyro_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc_worker.py",
        ))?),
    };
    let source_request_sha256 = sha256_hex(fit.request_bytes());
    let request = WorkerRequest {
        format:
            "marklab.numpyro_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc_request",
        version: 1,
        backend: backend.clone(),
        jax_version: JAX_VERSION,
        source_request_sha256: &source_request_sha256,
        source_request: source,
        calibration: calibration.clone(),
        resources: resources.clone(),
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    Ok(Prepared {
        fit,
        backend,
        request_bytes,
        request_sha256,
        input_sha256,
        calibration,
        resources,
        timeout_seconds: args.timeout_seconds,
    })
}

pub(crate) fn execute(prepared: &Prepared) -> Result<Output, BayesCliError> {
    let bytes = run_worker(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
        "marklab_numpyro_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: WorkerResult = serde_json::from_slice(&bytes)?;
    validate_worker(&result, prepared)?;
    Ok(Output {
        format: "marklab.replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc".into(),
        version: 1,
        backend: result.backend,
        jax_version: result.jax_version,
        input_sha256: result.input_sha256,
        request_sha256: result.request_sha256,
        source_request_sha256: result.source_request_sha256,
        fit_state: result.fit_state,
        calibration: prepared.calibration.clone(),
        resources: prepared.resources.clone(),
        scenario_dispositions: result.scenario_dispositions,
        rank_diagnostics: result.rank_diagnostics,
        pattern_representation: "complete_quadrature_resolved_counting_measure_on_exact_window"
            .into(),
        cross_type_dependence_estimand:
            "not_estimated_current_model_uses_conditionally_independent_type_fields".into(),
        statistical_unit: "patient".into(),
        null_model:
            "prior_generative_or_declared_stress_scenario_under_the_same_quadrature_likelihood"
                .into(),
        assumptions: assumptions().into_iter().map(str::to_owned).collect(),
        failure_policy: "retain_every_scenario_replicate_with_explicit_status_and_reason".into(),
        finite_result_policy: "reject_non_finite_completed_rows_failed_rows_remain_diagnostic"
            .into(),
        claim_status: "experimental_multitype_sbc_diagnostic".into(),
    })
}

impl Output {
    pub(crate) fn validate_for(&self, prepared: &Prepared) -> Result<(), BayesCliError> {
        if self.format != "marklab.replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc"
            || self.version != 1
            || self.input_sha256 != prepared.input_sha256
            || self.request_sha256 != prepared.request_sha256
            || self.source_request_sha256 != sha256_hex(prepared.fit.request_bytes())
            || self.calibration != prepared.calibration
            || self.resources != prepared.resources
            || self.scenario_dispositions.len() != prepared.resources.scenario_replicates as usize
            || self.pattern_representation
                != "complete_quadrature_resolved_counting_measure_on_exact_window"
            || self.cross_type_dependence_estimand
                != "not_estimated_current_model_uses_conditionally_independent_type_fields"
            || self.statistical_unit != "patient"
            || self.claim_status != "experimental_multitype_sbc_diagnostic"
        {
            return Err(BayesCliError::Backend(
                "durable multitype SBC result differs".into(),
            ));
        }
        Ok(())
    }
}

fn validate_worker(result: &WorkerResult, prepared: &Prepared) -> Result<(), BayesCliError> {
    let per_scenario = prepared.calibration.replicates_per_scenario as usize;
    let dispositions_valid = result.scenario_dispositions.len()
        == prepared.resources.scenario_replicates as usize
        && result
            .scenario_dispositions
            .iter()
            .enumerate()
            .all(|(index, row)| {
                row.scenario == SCENARIOS[index / per_scenario]
                    && row.replicate == (index % per_scenario) as u32
                    && ((row.status == "complete"
                        && row.failure_reason.is_none()
                        && row.ranks.is_some()
                        && row.coverage_90.is_some()
                        && row.posterior_predictive.is_some())
                        || (row.status == "failed"
                            && row
                                .failure_reason
                                .as_ref()
                                .is_some_and(|value| !value.is_empty())))
            });
    if result.format
        != "marklab.numpyro_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc_result"
        || result.version != 1
        || !backend_matches(&result.backend, &prepared.backend)
        || result.jax_version != JAX_VERSION
        || result.input_sha256 != prepared.input_sha256
        || result.request_sha256 != prepared.request_sha256
        || result.source_request_sha256 != sha256_hex(prepared.fit.request_bytes())
        || result.pattern_representation
            != "complete_quadrature_resolved_counting_measure_on_exact_window"
        || result.cross_type_dependence_estimand
            != "not_estimated_current_model_uses_conditionally_independent_type_fields"
        || !dispositions_valid
    {
        return Err(BayesCliError::Backend(
            "multitype SBC backend result differs".into(),
        ));
    }
    Ok(())
}

fn assumptions() -> [&'static str; 5] {
    [
        "simulation_and_refit_use_the_same_declared_quadrature_likelihood",
        "positive_null_weak_boundary_and_misspecified_scenarios_are_fixed_before_fitting",
        "type_fields_are_conditionally_independent_given_the_shared_kernel_scales",
        "cross_type_enrichment_is_a_posterior_predictive_summary_not_a_dependence_parameter",
        "patients_not_patterns_nodes_types_or_events_are_population_replicates",
    ]
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
