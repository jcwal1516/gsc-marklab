use std::{fs, path::PathBuf};

use clap::{Parser, Subcommand};
use marklab_bayes::{
    sha256_hex, BackendContract, FitState, NormalMeanDiagnostics, NutsSamplingSpec,
    SamplingSummary, SarScalarSummary, WorkerBackend,
};
use serde::{Deserialize, Serialize};

use super::{
    publish_json,
    replicated_arbitrary_window_lgcp_fit::{
        self, predictive_valid, NodePosterior, PatientEffect, PatternEffect,
        PatternPosteriorPredictive, WorkerRequest as SourceRequest,
    },
    run_worker, BayesCliError,
};

const PYMC_VERSION: &str = "6.3.0";

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct FitCli {
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
    FitReplicatedArbitraryWindowLgcpInferredKernel(Box<Arguments>),
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
    timeout_seconds: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct KernelPriors {
    field_amplitude_scale: f64,
    field_length_scale_scale_um: f64,
    jitter: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct KernelResources {
    maximum_kernel_cube_work: u64,
    kernel_cube_work: u64,
    maximum_tree_depth: u32,
    timeout_seconds: u64,
}

#[derive(Serialize)]
struct WorkerRequest<'a> {
    format: &'static str,
    version: u32,
    backend: BackendContract,
    source_backend: BackendContract,
    source_request_sha256: &'a str,
    source_request: &'a SourceRequest,
    kernel_priors: KernelPriors,
    resources: KernelResources,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Posterior {
    intercept: SarScalarSummary,
    group_effect: SarScalarSummary,
    covariate_effect: SarScalarSummary,
    patient_sd: SarScalarSummary,
    pattern_sd: SarScalarSummary,
    field_amplitude: SarScalarSummary,
    field_length_scale_um: SarScalarSummary,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerResult {
    format: String,
    version: u32,
    backend: WorkerBackend,
    source_backend: WorkerBackend,
    input_sha256: String,
    request_sha256: String,
    source_request_sha256: String,
    fit_state: FitState,
    sampling: SamplingSummary,
    posterior: Posterior,
    patient_effects: Vec<PatientEffect>,
    pattern_effects: Vec<PatternEffect>,
    nodes: Vec<NodePosterior>,
    pattern_posterior_predictive: Vec<PatternPosteriorPredictive>,
    diagnostics: NormalMeanDiagnostics,
    kernel_cube_work: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResultDocument {
    format: String,
    version: u32,
    backend: WorkerBackend,
    source_backend: WorkerBackend,
    input_sha256: String,
    request_sha256: String,
    source_request_sha256: String,
    fit_state: FitState,
    sampling: SamplingSummary,
    posterior: Posterior,
    patient_effects: Vec<PatientEffect>,
    pattern_effects: Vec<PatternEffect>,
    nodes: Vec<NodePosterior>,
    pattern_posterior_predictive: Vec<PatternPosteriorPredictive>,
    diagnostics: NormalMeanDiagnostics,
    patient_count: usize,
    pattern_count: usize,
    total_node_count: usize,
    total_event_count: u64,
    cohort_count: usize,
    groups: [String; 2],
    kernel_priors: KernelPriors,
    resources: KernelResources,
    statistical_unit: String,
    pattern_unit: String,
    null_model: String,
    assumptions: Vec<String>,
    finite_result_policy: String,
    claim_status: String,
}

pub(crate) struct PreparedReplicatedArbitraryWindowLgcpInferredKernel {
    source: replicated_arbitrary_window_lgcp_fit::PreparedReplicatedArbitraryWindowLgcpFit,
    backend: BackendContract,
    kernel_priors: KernelPriors,
    resources: KernelResources,
    request_bytes: Vec<u8>,
    request_sha256: String,
    timeout_seconds: u64,
}

impl PreparedReplicatedArbitraryWindowLgcpInferredKernel {
    pub(crate) fn backend(&self) -> &BackendContract {
        &self.backend
    }

    pub(crate) fn request_bytes(&self) -> &[u8] {
        &self.request_bytes
    }
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let TopLevel::Bayes { command } = FitCli::parse_from(std::env::args_os()).command;
    let Command::FitReplicatedArbitraryWindowLgcpInferredKernel(arguments) = command;
    let output = arguments.out.clone();
    let prepared = prepare_arguments(*arguments)?;
    publish_json(&output, &execute(&prepared)?)
}

fn prepare_arguments(
    arguments: Arguments,
) -> Result<PreparedReplicatedArbitraryWindowLgcpInferredKernel, BayesCliError> {
    if ![
        arguments.field_amplitude_prior_scale,
        arguments.field_length_scale_prior_scale_um,
        arguments.jitter,
    ]
    .into_iter()
    .all(|value| value.is_finite() && value > 0.0)
        || arguments.maximum_kernel_cube_work == 0
        || arguments.maximum_kernel_cube_work > 1_000_000
        || !(10..=14).contains(&arguments.maximum_tree_depth)
    {
        return Err(BayesCliError::Input(
            "replicated inferred-kernel LGCP controls are invalid".into(),
        ));
    }
    let sampling = NutsSamplingSpec {
        chains: arguments.chains,
        tune_per_chain: arguments.tune,
        draws_per_chain: arguments.draws,
        target_accept: arguments.target_accept,
        seed: arguments.seed,
    };
    let source = replicated_arbitrary_window_lgcp_fit::prepare(
        arguments.input,
        arguments.reference_group,
        arguments.comparison_group,
        arguments.intercept_prior_mean,
        arguments.intercept_prior_sd,
        arguments.group_effect_prior_sd,
        arguments.covariate_effect_prior_sd,
        arguments.patient_sd_prior_scale,
        arguments.pattern_sd_prior_scale,
        arguments.field_amplitude_prior_scale,
        arguments.field_length_scale_prior_scale_um,
        arguments.jitter,
        sampling,
        arguments.maximum_patients,
        arguments.maximum_patterns,
        arguments.maximum_nodes_per_pattern,
        arguments.maximum_total_nodes,
        arguments.maximum_total_events,
        arguments.maximum_draw_node_work,
        arguments.timeout_seconds,
    )?;
    let kernel_cube_work = source
        .request
        .kernel_cube_work()
        .ok_or_else(|| BayesCliError::Input("kernel cube work overflows".into()))?;
    if kernel_cube_work > arguments.maximum_kernel_cube_work {
        return Err(BayesCliError::Input(format!(
            "kernel cube work exceeds maximum: {kernel_cube_work} > {}",
            arguments.maximum_kernel_cube_work
        )));
    }
    let kernel_priors = KernelPriors {
        field_amplitude_scale: arguments.field_amplitude_prior_scale,
        field_length_scale_scale_um: arguments.field_length_scale_prior_scale_um,
        jitter: arguments.jitter,
    };
    let resources = KernelResources {
        maximum_kernel_cube_work: arguments.maximum_kernel_cube_work,
        kernel_cube_work,
        maximum_tree_depth: arguments.maximum_tree_depth,
        timeout_seconds: arguments.timeout_seconds,
    };
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let directory = repository.join("workers/python");
    let lock = read(&directory.join("uv.lock"))?;
    let worker = read(
        &directory.join("marklab_pymc_replicated_arbitrary_window_lgcp_inferred_kernel_worker.py"),
    )?;
    let backend = BackendContract {
        name: "pymc",
        version: PYMC_VERSION,
        python_version: "3.12",
        environment_lock_sha256: sha256_hex(&lock),
        worker_sha256: sha256_hex(&worker),
    };
    let request = WorkerRequest {
        format: "marklab.pymc_replicated_arbitrary_window_lgcp_inferred_kernel_request",
        version: 1,
        backend: backend.clone(),
        source_backend: source.request.backend.clone(),
        source_request_sha256: &source.request_sha256,
        source_request: &source.request,
        kernel_priors: kernel_priors.clone(),
        resources: resources.clone(),
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    Ok(PreparedReplicatedArbitraryWindowLgcpInferredKernel {
        source,
        backend,
        kernel_priors,
        resources,
        request_bytes,
        request_sha256,
        timeout_seconds: arguments.timeout_seconds,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare(
    input: PathBuf,
    reference_group: String,
    comparison_group: String,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    group_effect_prior_sd: f64,
    covariate_effect_prior_sd: f64,
    patient_sd_prior_scale: f64,
    pattern_sd_prior_scale: f64,
    field_amplitude_prior_scale: f64,
    field_length_scale_prior_scale_um: f64,
    jitter: f64,
    sampling: NutsSamplingSpec,
    maximum_patients: usize,
    maximum_patterns: usize,
    maximum_nodes_per_pattern: usize,
    maximum_total_nodes: usize,
    maximum_total_events: u64,
    maximum_draw_node_work: u64,
    maximum_kernel_cube_work: u64,
    maximum_tree_depth: u32,
    timeout_seconds: u64,
) -> Result<PreparedReplicatedArbitraryWindowLgcpInferredKernel, BayesCliError> {
    prepare_arguments(Arguments {
        input,
        reference_group,
        comparison_group,
        intercept_prior_mean,
        intercept_prior_sd,
        group_effect_prior_sd,
        covariate_effect_prior_sd,
        patient_sd_prior_scale,
        pattern_sd_prior_scale,
        field_amplitude_prior_scale,
        field_length_scale_prior_scale_um,
        jitter,
        chains: sampling.chains,
        tune: sampling.tune_per_chain,
        draws: sampling.draws_per_chain,
        target_accept: sampling.target_accept,
        seed: sampling.seed,
        maximum_patients,
        maximum_patterns,
        maximum_nodes_per_pattern,
        maximum_total_nodes,
        maximum_total_events,
        maximum_draw_node_work,
        maximum_kernel_cube_work,
        maximum_tree_depth,
        timeout_seconds,
        out: PathBuf::new(),
    })
}

pub(crate) fn execute(
    prepared: &PreparedReplicatedArbitraryWindowLgcpInferredKernel,
) -> Result<ResultDocument, BayesCliError> {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let bytes = run_worker(
        repository,
        "marklab_pymc_replicated_arbitrary_window_lgcp_inferred_kernel_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let worker_result: WorkerResult = serde_json::from_slice(&bytes)?;
    validate_result(
        &worker_result,
        &prepared.backend,
        &prepared.request_sha256,
        &prepared.source.request,
        &prepared.source.request_sha256,
        &prepared.resources,
    )?;
    Ok(ResultDocument {
        format: worker_result.format,
        version: worker_result.version,
        backend: worker_result.backend,
        source_backend: worker_result.source_backend,
        input_sha256: worker_result.input_sha256,
        request_sha256: worker_result.request_sha256,
        source_request_sha256: worker_result.source_request_sha256,
        fit_state: worker_result.fit_state,
        sampling: worker_result.sampling,
        posterior: worker_result.posterior,
        patient_effects: worker_result.patient_effects,
        pattern_effects: worker_result.pattern_effects,
        nodes: worker_result.nodes,
        pattern_posterior_predictive: worker_result.pattern_posterior_predictive,
        diagnostics: worker_result.diagnostics,
        patient_count: prepared.source.request.patient_count(),
        pattern_count: prepared.source.request.pattern_count(),
        total_node_count: prepared.source.request.node_count(),
        total_event_count: prepared.source.request.event_count(),
        cohort_count: prepared.source.request.cohort_count(),
        groups: prepared.source.request.groups(),
        kernel_priors: prepared.kernel_priors.clone(),
        resources: prepared.resources.clone(),
        statistical_unit: "patient".into(),
        pattern_unit: "slide_pattern_nested_in_patient".into(),
        null_model: "comparison_group_log_intensity_effect_equals_zero".into(),
        assumptions: vec![
            "one_shared_isotropic_matern_kernel_across_all_slide_patterns".into(),
            "kernel_scales_are_inferred_from_repeated_patterns_not_selected_post_hoc".into(),
            "each_exact_window_and_event_set_is_provenance_complete".into(),
            "patients_not_patterns_nodes_or_cells_are_population_replicates".into(),
            "field_and_pattern_effects_are_zero_sum_within_their_declared_levels".into(),
        ],
        finite_result_policy: "nonconverged_is_diagnostic_only_reject_non_finite".into(),
        claim_status: if worker_result.fit_state == FitState::Complete {
            "experimental_inferred_shared_kernel"
        } else {
            "diagnostic_only_nonconverged"
        }
        .into(),
    })
}

fn validate_result(
    result: &WorkerResult,
    backend: &BackendContract,
    request_sha256: &str,
    source: &SourceRequest,
    source_request_sha256: &str,
    resources: &KernelResources,
) -> Result<(), BayesCliError> {
    let policy = source.diagnostic_policy();
    let summaries = [
        &result.posterior.intercept,
        &result.posterior.group_effect,
        &result.posterior.covariate_effect,
        &result.posterior.patient_sd,
        &result.posterior.pattern_sd,
        &result.posterior.field_amplitude,
        &result.posterior.field_length_scale_um,
    ];
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
    if result.format != "marklab.bayesian_replicated_arbitrary_window_lgcp_inferred_kernel_fit"
        || result.version != 1
        || !backend_matches(&result.backend, backend)
        || result.source_backend.name != source.backend.name
        || result.source_backend.version != source.backend.version
        || result.source_backend.python_version != source.backend.python_version
        || result.source_backend.environment_lock_sha256 != source.backend.environment_lock_sha256
        || result.source_backend.worker_sha256 != source.backend.worker_sha256
        || result.input_sha256 != source.input_sha256()
        || result.request_sha256 != request_sha256
        || result.source_request_sha256 != source_request_sha256
        || result.sampling.completed_draws != source.completed_draws()
        || result.kernel_cube_work != resources.kernel_cube_work
        || summaries.into_iter().any(|summary| !summary_valid(summary))
        || result.posterior.field_amplitude.mean <= 0.0
        || result.posterior.field_length_scale_um.mean <= 0.0
        || !source.patient_effects_valid(&result.patient_effects)
        || !source.pattern_effects_valid(&result.pattern_effects)
        || !source.nodes_valid(&result.nodes)
        || !predictive_valid(
            &result.pattern_posterior_predictive,
            source,
            result.sampling.completed_draws,
        )
        || (result.fit_state == FitState::Complete) != diagnostics_pass
    {
        return Err(BayesCliError::Backend(
            "replicated inferred-kernel LGCP result differs".into(),
        ));
    }
    Ok(())
}

impl ResultDocument {
    pub(crate) fn validate_for(
        &self,
        prepared: &PreparedReplicatedArbitraryWindowLgcpInferredKernel,
    ) -> Result<(), BayesCliError> {
        let source = &prepared.source.request;
        let policy = source.diagnostic_policy();
        let summaries = [
            &self.posterior.intercept,
            &self.posterior.group_effect,
            &self.posterior.covariate_effect,
            &self.posterior.patient_sd,
            &self.posterior.pattern_sd,
            &self.posterior.field_amplitude,
            &self.posterior.field_length_scale_um,
        ];
        let diagnostics_pass = self.diagnostics.prior_predictive_finite
            && self.diagnostics.posterior_finite
            && self.diagnostics.constraints_valid
            && self.diagnostics.identifiability_checks_passed
            && self.diagnostics.r_hat <= policy.maximum_r_hat
            && self.diagnostics.ess_bulk >= policy.minimum_bulk_ess
            && self.diagnostics.ess_tail >= policy.minimum_tail_ess
            && self.diagnostics.minimum_ebfmi >= policy.minimum_ebfmi
            && self.diagnostics.divergences <= policy.maximum_divergences
            && self.diagnostics.max_tree_depth_hits <= policy.maximum_tree_depth_hits;
        let resources_match = self.resources.maximum_kernel_cube_work
            == prepared.resources.maximum_kernel_cube_work
            && self.resources.kernel_cube_work == prepared.resources.kernel_cube_work
            && self.resources.maximum_tree_depth == prepared.resources.maximum_tree_depth
            && self.resources.timeout_seconds == prepared.resources.timeout_seconds;
        let priors_match = self.kernel_priors.field_amplitude_scale
            == prepared.kernel_priors.field_amplitude_scale
            && self.kernel_priors.field_length_scale_scale_um
                == prepared.kernel_priors.field_length_scale_scale_um
            && self.kernel_priors.jitter == prepared.kernel_priors.jitter;
        if self.format != "marklab.bayesian_replicated_arbitrary_window_lgcp_inferred_kernel_fit"
            || self.version != 1
            || !backend_matches(&self.backend, &prepared.backend)
            || self.source_backend.name != source.backend.name
            || self.source_backend.version != source.backend.version
            || self.source_backend.python_version != source.backend.python_version
            || self.source_backend.environment_lock_sha256 != source.backend.environment_lock_sha256
            || self.source_backend.worker_sha256 != source.backend.worker_sha256
            || self.input_sha256 != source.input_sha256()
            || self.request_sha256 != prepared.request_sha256
            || self.source_request_sha256 != prepared.source.request_sha256
            || self.sampling.completed_draws != source.completed_draws()
            || self.patient_count != source.patient_count()
            || self.pattern_count != source.pattern_count()
            || self.total_node_count != source.node_count()
            || self.total_event_count != source.event_count()
            || self.cohort_count != source.cohort_count()
            || self.groups != source.groups()
            || !resources_match
            || !priors_match
            || summaries.into_iter().any(|summary| !summary_valid(summary))
            || self.posterior.field_amplitude.mean <= 0.0
            || self.posterior.field_length_scale_um.mean <= 0.0
            || !source.patient_effects_valid(&self.patient_effects)
            || !source.pattern_effects_valid(&self.pattern_effects)
            || !source.nodes_valid(&self.nodes)
            || !predictive_valid(
                &self.pattern_posterior_predictive,
                source,
                self.sampling.completed_draws,
            )
            || self.statistical_unit != "patient"
            || self.pattern_unit != "slide_pattern_nested_in_patient"
            || (self.fit_state == FitState::Complete) != diagnostics_pass
        {
            return Err(BayesCliError::Backend(
                "durable inferred-kernel LGCP result identity or diagnostics differ".into(),
            ));
        }
        Ok(())
    }
}

fn summary_valid(summary: &SarScalarSummary) -> bool {
    [summary.mean, summary.sd, summary.interval_upper]
        .into_iter()
        .all(f64::is_finite)
        && summary.sd >= 0.0
        && summary.interval_lower <= summary.interval_upper
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
