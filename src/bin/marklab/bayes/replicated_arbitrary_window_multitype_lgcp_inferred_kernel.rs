pub(crate) use marklab_bayes::protocol::replicated_arbitrary_window_multitype_lgcp_inferred_kernel::{KernelPriors, KernelResources, WorkerRequest, WorkerResult, Output, validate_result, array, strings, string};

use std::path::PathBuf;

use marklab_bayes::sha256_hex;
use marklab_bayes::BackendContract;
use marklab_bayes::FitState;

use super::publish_json;
use super::replicated_arbitrary_window_multitype_lgcp;
use super::replicated_arbitrary_window_multitype_lgcp::Args as SourceArgs;
use super::run_worker;
use super::CliError;
use marklab_bayes::backend_versions::PYMC_VERSION;
use marklab_bayes::backend_versions::PYTHON_VERSION;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::NullHypothesis;
use marklab_core::vocabulary::StatisticalUnit;

const MAXIMUM_ADAPTER_BYTES: u64 = 8 * 1_048_576;

#[derive(Clone, Debug, clap::Args)]
pub(crate) struct Args {
    /// Per-node, per-type quadrature CSV (pattern_id, patient_id, group, type_id, count, ...).
    #[arg(long)]
    pub(crate) input: PathBuf,
    /// Group level used as the regression reference.
    #[arg(long)]
    pub(crate) reference_group: String,
    /// Group level whose effect is compared with the reference group.
    #[arg(long)]
    pub(crate) comparison_group: String,
    /// Point-type label used as the model reference.
    #[arg(long)]
    pub(crate) reference_type: String,
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
    /// Upper bound on types.
    #[arg(long)]
    pub(crate) maximum_types: usize,
    /// Upper bound on nodes per pattern.
    #[arg(long)]
    pub(crate) maximum_nodes_per_pattern: usize,
    /// Upper bound on total nodes.
    #[arg(long)]
    pub(crate) maximum_total_nodes: usize,
    /// Upper bound on total node type rows.
    #[arg(long)]
    pub(crate) maximum_total_node_type_rows: usize,
    /// Upper bound on total events.
    #[arg(long)]
    pub(crate) maximum_total_events: u64,
    /// Upper bound on draw node type work.
    #[arg(long)]
    pub(crate) maximum_draw_node_type_work: u64,
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

pub(crate) struct Prepared {
    source: replicated_arbitrary_window_multitype_lgcp::Prepared,
    source_value: serde_json::Value,
    backend: BackendContract,
    kernel_priors: KernelPriors,
    resources: KernelResources,
    request_bytes: Vec<u8>,
    request_sha256: String,
    timeout_seconds: u64,
}

impl Prepared {
    pub(crate) fn backend(&self) -> &BackendContract {
        &self.backend
    }

    pub(crate) fn request_bytes(&self) -> &[u8] {
        &self.request_bytes
    }
}

pub(super) fn run(args: Box<Args>) -> Result<(), CliError> {
    let out = args.out.clone();
    let prepared = prepare(*args)?;
    publish_json(&out, &execute(&prepared)?)
}

pub(crate) fn prepare(args: Args) -> Result<Prepared, CliError> {
    if ![
        args.field_amplitude_prior_scale,
        args.field_length_scale_prior_scale_um,
        args.jitter,
    ]
    .into_iter()
    .all(|value| value.is_finite() && value > 0.0)
        || args.maximum_kernel_cube_work == 0
        || args.maximum_kernel_cube_work > 1_000_000
        || !(10..=14).contains(&args.maximum_tree_depth)
    {
        return Err(CliError::Input(
            "replicated multitype inferred-kernel controls are invalid".into(),
        ));
    }
    let source = replicated_arbitrary_window_multitype_lgcp::prepare(SourceArgs {
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
        field_amplitude: args.field_amplitude_prior_scale,
        field_length_scale_um: args.field_length_scale_prior_scale_um,
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
        timeout_seconds: args.timeout_seconds,
        out: PathBuf::new(),
    })?;
    let source_value: serde_json::Value = serde_json::from_slice(source.request_bytes())?;
    let kernel_cube_work = array(&source_value, "patterns")
        .map_err(super::protocol_error)?
        .iter()
        .try_fold(0_u64, |total, row| {
            row["node_count"]
                .as_u64()
                .and_then(|nodes| nodes.checked_pow(3))
                .and_then(|work| total.checked_add(work))
        })
        .ok_or_else(|| CliError::Input("multitype kernel work overflows".into()))?;
    if kernel_cube_work > args.maximum_kernel_cube_work {
        return Err(CliError::Input(format!(
            "multitype kernel cube work exceeds maximum: {kernel_cube_work} > {}",
            args.maximum_kernel_cube_work
        )));
    }
    let kernel_priors = KernelPriors {
        field_amplitude_scale: args.field_amplitude_prior_scale,
        field_length_scale_scale_um: args.field_length_scale_prior_scale_um,
        jitter: args.jitter,
    };
    let resources = KernelResources {
        maximum_kernel_cube_work: args.maximum_kernel_cube_work,
        kernel_cube_work,
        maximum_tree_depth: args.maximum_tree_depth,
        timeout_seconds: args.timeout_seconds,
    };
    let repository = &marklab::backend::python_backend_assets_root()?;
    let directory = repository.join("workers/python");
    let backend = BackendContract {
        name: "pymc",
        version: PYMC_VERSION,
        python_version: PYTHON_VERSION,
        environment_lock_sha256: sha256_hex(&read_bounded(&directory.join("uv.lock"))?),
        worker_sha256: sha256_hex(&read_bounded(&directory.join(
            "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_worker.py",
        ))?),
    };
    let request = WorkerRequest {
        format: "marklab.pymc_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_request",
        version: 1,
        backend: backend.clone(),
        source_backend: source.backend().clone(),
        source_request_sha256: source.request_sha256(),
        source_request: source_value.clone(),
        kernel_priors: kernel_priors.clone(),
        resources: resources.clone(),
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    Ok(Prepared {
        source,
        source_value,
        backend,
        kernel_priors,
        resources,
        request_bytes,
        request_sha256,
        timeout_seconds: args.timeout_seconds,
    })
}

pub(crate) fn execute(prepared: &Prepared) -> Result<Output, CliError> {
    let bytes = run_worker(
        &marklab::backend::python_backend_assets_root()?,
        "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: WorkerResult = serde_json::from_slice(&bytes)?;
    validate_result(&result, &prepared.validation_context()).map_err(super::protocol_error)?;
    let source = &prepared.source_value;
    let patients = array(source, "patients").map_err(super::protocol_error)?;
    let patterns = array(source, "patterns").map_err(super::protocol_error)?;
    let types = strings(source, "type_ids").map_err(super::protocol_error)?;
    let nodes = array(source, "nodes").map_err(super::protocol_error)?;
    let counts = array(source, "node_type_counts").map_err(super::protocol_error)?;
    Ok(Output {
        format: result.format,
        version: result.version,
        backend: result.backend,
        source_backend: result.source_backend,
        input_sha256: result.input_sha256,
        request_sha256: result.request_sha256,
        source_request_sha256: result.source_request_sha256,
        fit_state: result.fit_state,
        sampling: result.sampling,
        kernel_posterior: result.kernel_posterior,
        type_posteriors: result.type_posteriors,
        group_effect_differences: result.group_effect_differences,
        patient_type_effects: result.patient_type_effects,
        pattern_type_effects: result.pattern_type_effects,
        node_type_posteriors: result.node_type_posteriors,
        pattern_type_posterior_predictive: result.pattern_type_posterior_predictive,
        diagnostics: result.diagnostics,
        patient_count: patients.len(),
        pattern_count: patterns.len(),
        type_count: types.len(),
        total_node_count: nodes.len(),
        total_node_type_row_count: counts.len(),
        total_event_count: counts.iter().filter_map(|row| row["count"].as_u64()).sum(),
        groups: [
            string(source, "reference_group").map_err(super::protocol_error)?,
            string(source, "comparison_group").map_err(super::protocol_error)?,
        ],
        reference_type: string(source, "reference_type").map_err(super::protocol_error)?,
        cohort_count: patterns
            .iter()
            .filter_map(|row| row["cohort"].as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        kernel_priors: prepared.kernel_priors.clone(),
        resources: prepared.resources.clone(),
        statistical_unit: StatisticalUnit::Patient,
        pattern_unit: "slide_pattern_nested_in_patient".into(),
        likelihood: "independent_type_specific_poisson_intensities_over_one_shared_exact_window"
            .into(),
        null_model: NullHypothesis::EveryTypeComparisonGroupLogIntensityEffectEqualsZero,
        assumptions: [
            "one_shared_isotropic_matern_kernel_across_types_and_slide_patterns",
            "type_specific_latent_fields_are_conditionally_independent_given_shared_kernel_scales",
            "kernel_scales_are_inferred_inside_the_model_not_selected_post_hoc",
            "each_exact_window_and_typed_event_partition_is_provenance_complete",
            "patients_not_slides_nodes_types_or_cells_are_population_replicates",
            "field_and_slide_effects_are_zero_sum_within_declared_levels",
            "no_pairwise_gibbs_interaction_or_cross_type_latent_covariance_is_claimed",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        finite_result_policy: "nonconverged_is_diagnostic_only_reject_non_finite".into(),
        claim_status: if result.fit_state == FitState::Complete {
            ClaimStatus::ExperimentalInferredSharedMultitypeKernel
        } else {
            ClaimStatus::DiagnosticOnlyNonconverged
        },
    })
}

fn read_bounded(path: &std::path::Path) -> Result<Vec<u8>, CliError> {
    crate::input_file::read_regular_file(
        path,
        MAXIMUM_ADAPTER_BYTES,
        "replicated multitype inferred-kernel adapter is absent or oversized",
    )
}

impl Prepared {
    fn validation_context(&self) -> marklab_bayes::protocol::replicated_arbitrary_window_multitype_lgcp_inferred_kernel::ValidationContext<'_>{
        marklab_bayes::protocol::replicated_arbitrary_window_multitype_lgcp_inferred_kernel::ValidationContext {
            source_value: &self.source_value,
            backend: &self.backend,
            source_backend: self.source.backend(),
            request_sha256: &self.request_sha256,
            source_request_sha256: self.source.request_sha256(),
            resources: &self.resources,
        }
    }

    pub(crate) fn validate_result(&self, result: &Output) -> Result<(), CliError> {
        result
            .validate_for(&self.validation_context())
            .map_err(super::protocol_error)
    }
}
