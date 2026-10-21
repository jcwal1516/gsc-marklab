pub(crate) use marklab_bayes::protocol::replicated_arbitrary_window_lgcp_inferred_kernel::{
    validate_result, KernelPriors, KernelResources, ResultDocument, WorkerRequest, WorkerResult,
};

use crate::input_file::read_file;
use std::path::PathBuf;

use marklab_bayes::sha256_hex;
use marklab_bayes::BackendContract;
use marklab_bayes::FitState;
use marklab_bayes::NutsSamplingSpec;

use super::publish_json;
use super::replicated_arbitrary_window_lgcp_fit;
use super::run_worker;
use super::CliError;
use marklab_bayes::backend_versions::PYMC_VERSION;
use marklab_bayes::backend_versions::PYTHON_VERSION;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::NullHypothesis;
use marklab_core::vocabulary::StatisticalUnit;

#[derive(Debug, clap::Args)]
pub(crate) struct Arguments {
    /// Per-node quadrature CSV (pattern_id, patient_id, group, node_id, x_um, y_um, count, ...).
    #[arg(long)]
    pub(crate) input: PathBuf,
    /// Group level used as the regression reference.
    #[arg(long)]
    pub(crate) reference_group: String,
    /// Group level whose effect is compared with the reference group.
    #[arg(long)]
    pub(crate) comparison_group: String,
    /// Prior mean for intercept.
    #[arg(long, allow_hyphen_values = true)]
    pub(crate) intercept_prior_mean: f64,
    /// Prior scale for intercept.
    #[arg(long)]
    pub(crate) intercept_prior_sd: f64,
    /// Prior scale for group effect.
    #[arg(long)]
    pub(crate) group_effect_prior_sd: f64,
    /// Prior scale for covariate effect.
    #[arg(long)]
    pub(crate) covariate_effect_prior_sd: f64,
    /// Prior scale for patient standard deviation.
    #[arg(long)]
    pub(crate) patient_sd_prior_scale: f64,
    /// Prior scale for pattern standard deviation.
    #[arg(long)]
    pub(crate) pattern_sd_prior_scale: f64,
    /// Prior scale for field amplitude.
    #[arg(long)]
    pub(crate) field_amplitude_prior_scale: f64,
    /// Prior scale, in micrometers, for field length scale.
    #[arg(long)]
    pub(crate) field_length_scale_prior_scale_um: f64,
    /// Diagonal stabilization added to covariance matrices.
    #[arg(long)]
    pub(crate) jitter: f64,
    /// Number of independent posterior sampling chains.
    #[arg(long)]
    pub(crate) chains: u32,
    /// Warmup iterations per posterior sampling chain.
    #[arg(long)]
    pub(crate) tune: u32,
    /// Posterior draws retained per chain after warmup.
    #[arg(long)]
    pub(crate) draws: u32,
    /// Target NUTS acceptance probability.
    #[arg(long)]
    pub(crate) target_accept: f64,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(crate) seed: u64,
    /// Upper bound on patients.
    #[arg(long)]
    pub(crate) maximum_patients: usize,
    /// Upper bound on patterns.
    #[arg(long)]
    pub(crate) maximum_patterns: usize,
    /// Upper bound on nodes per pattern.
    #[arg(long)]
    pub(crate) maximum_nodes_per_pattern: usize,
    /// Upper bound on total nodes.
    #[arg(long)]
    pub(crate) maximum_total_nodes: usize,
    /// Upper bound on total events.
    #[arg(long)]
    pub(crate) maximum_total_events: u64,
    /// Upper bound on draw node work.
    #[arg(long)]
    pub(crate) maximum_draw_node_work: u64,
    /// Upper bound on kernel cube work.
    #[arg(long)]
    pub(crate) maximum_kernel_cube_work: u64,
    /// Upper bound on tree depth.
    #[arg(long)]
    pub(crate) maximum_tree_depth: u32,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(crate) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(crate) out: PathBuf,
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

    pub(crate) fn request_sha256(&self) -> &str {
        &self.request_sha256
    }

    pub(crate) fn source_request(&self) -> &replicated_arbitrary_window_lgcp_fit::WorkerRequest {
        &self.source.request
    }
}

pub(super) fn run(arguments: Box<Arguments>) -> Result<(), CliError> {
    let output = arguments.out.clone();
    let prepared = prepare(*arguments)?;
    publish_json(&output, &execute(&prepared)?)
}

pub(crate) fn prepare(
    arguments: Arguments,
) -> Result<PreparedReplicatedArbitraryWindowLgcpInferredKernel, CliError> {
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
        return Err(CliError::Input(
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
        crate::bayes::ReplicatedArbitraryWindowLgcpFitArgs {
            input: arguments.input,
            reference_group: arguments.reference_group,
            comparison_group: arguments.comparison_group,
            intercept_prior_mean: arguments.intercept_prior_mean,
            intercept_prior_sd: arguments.intercept_prior_sd,
            group_effect_prior_sd: arguments.group_effect_prior_sd,
            covariate_effect_prior_sd: arguments.covariate_effect_prior_sd,
            patient_sd_prior_scale: arguments.patient_sd_prior_scale,
            pattern_sd_prior_scale: arguments.pattern_sd_prior_scale,
            field_amplitude: arguments.field_amplitude_prior_scale,
            field_length_scale_um: arguments.field_length_scale_prior_scale_um,
            jitter: arguments.jitter,
            chains: sampling.chains,
            tune: sampling.tune_per_chain,
            draws: sampling.draws_per_chain,
            target_accept: sampling.target_accept,
            seed: sampling.seed,
            maximum_patients: arguments.maximum_patients,
            maximum_patterns: arguments.maximum_patterns,
            maximum_nodes_per_pattern: arguments.maximum_nodes_per_pattern,
            maximum_total_nodes: arguments.maximum_total_nodes,
            maximum_total_events: arguments.maximum_total_events,
            maximum_draw_node_work: arguments.maximum_draw_node_work,
            timeout_seconds: arguments.timeout_seconds,
            out: std::path::PathBuf::new(),
        },
    )?;
    let kernel_cube_work = source
        .request
        .kernel_cube_work()
        .ok_or_else(|| CliError::Input("kernel cube work overflows".into()))?;
    if kernel_cube_work > arguments.maximum_kernel_cube_work {
        return Err(CliError::Input(format!(
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
    let repository = &marklab::backend::python_backend_assets_root()?;
    let directory = repository.join("workers/python");
    let lock = read_file(&directory.join("uv.lock"))?;
    let worker = read_file(
        &directory.join("marklab_pymc_replicated_arbitrary_window_lgcp_inferred_kernel_worker.py"),
    )?;
    let backend = BackendContract {
        name: "pymc",
        version: PYMC_VERSION,
        python_version: PYTHON_VERSION,
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

pub(crate) fn execute(
    prepared: &PreparedReplicatedArbitraryWindowLgcpInferredKernel,
) -> Result<ResultDocument, CliError> {
    let repository = &marklab::backend::python_backend_assets_root()?;
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
    )
    .map_err(super::protocol_error)?;
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
        statistical_unit: StatisticalUnit::Patient,
        pattern_unit: "slide_pattern_nested_in_patient".into(),
        null_model: NullHypothesis::ComparisonGroupLogIntensityEffectEqualsZero,
        assumptions: vec![
            "one_shared_isotropic_matern_kernel_across_all_slide_patterns".into(),
            "kernel_scales_are_inferred_from_repeated_patterns_not_selected_post_hoc".into(),
            "each_exact_window_and_event_set_is_provenance_complete".into(),
            "patients_not_patterns_nodes_or_cells_are_population_replicates".into(),
            "field_and_pattern_effects_are_zero_sum_within_their_declared_levels".into(),
        ],
        finite_result_policy: "nonconverged_is_diagnostic_only_reject_non_finite".into(),
        claim_status: if worker_result.fit_state == FitState::Complete {
            ClaimStatus::ExperimentalInferredSharedKernel
        } else {
            ClaimStatus::DiagnosticOnlyNonconverged
        },
    })
}

impl PreparedReplicatedArbitraryWindowLgcpInferredKernel {
    pub(crate) fn validate_result(&self, result: &ResultDocument) -> Result<(), CliError> {
        result.validate_for(&marklab_bayes::protocol::replicated_arbitrary_window_lgcp_inferred_kernel::ValidationContext {
            source_request: &self.source.request,
            source_request_sha256: &self.source.request_sha256,
            backend: &self.backend,
            request_sha256: &self.request_sha256,
            kernel_priors: &self.kernel_priors,
            resources: &self.resources,
        }).map_err(super::protocol_error)
    }
}
