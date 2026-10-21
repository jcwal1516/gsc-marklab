pub(crate) use marklab_bayes::protocol::correlated_replicated_arbitrary_window_multitype_lgcp::{
    validate_worker, CorrelationPrior, Output, Resources, WorkerRequest, WorkerResult,
};

use crate::input_file::read_file;
use std::path::PathBuf;

use marklab_bayes::sha256_hex;
use marklab_bayes::BackendContract;
use marklab_bayes::FitState;

use super::publish_json;
use super::replicated_arbitrary_window_multitype_lgcp_inferred_kernel as inferred_kernel;
use super::replicated_arbitrary_window_multitype_lgcp_inferred_kernel::Args as SourceArgs;
use super::run_worker;
use super::CliError;
use marklab_bayes::backend_versions::JAX_VERSION;
use marklab_bayes::backend_versions::NUMPYRO_VERSION;
use marklab_bayes::backend_versions::PYTHON_VERSION;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::NullHypothesis;
use marklab_core::vocabulary::StatisticalUnit;

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
    /// LKJ prior concentration for the correlation matrix.
    #[arg(long)]
    pub(crate) lkj_concentration: f64,
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
    /// Upper bound on coregionalization work.
    #[arg(long)]
    pub(crate) maximum_coregionalization_work: u64,
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
    source: inferred_kernel::Prepared,
    source_value: serde_json::Value,
    backend: BackendContract,
    correlation_prior: CorrelationPrior,
    resources: Resources,
    request_bytes: Vec<u8>,
    request_sha256: String,
    input_sha256: String,
    timeout_seconds: u64,
}

impl Prepared {
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
    if !args.lkj_concentration.is_finite()
        || args.lkj_concentration < 1.0
        || args.maximum_coregionalization_work == 0
    {
        return Err(CliError::Input(
            "correlated multitype LGCP controls are invalid".into(),
        ));
    }
    let completed_draws = u64::from(args.chains)
        .checked_mul(u64::from(args.draws))
        .ok_or_else(|| CliError::Input("correlated draw count overflows".into()))?;
    let source = inferred_kernel::prepare(SourceArgs {
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
    let source_value: serde_json::Value = serde_json::from_slice(source.request_bytes())?;
    let fixed = source_value["source_request"]
        .as_object()
        .ok_or_else(|| CliError::Input("correlated source request differs".into()))?;
    let types = fixed["type_ids"]
        .as_array()
        .ok_or_else(|| CliError::Input("correlated type identities differ".into()))?
        .len() as u64;
    let nodes = fixed["nodes"]
        .as_array()
        .ok_or_else(|| CliError::Input("correlated nodes differ".into()))?
        .len() as u64;
    let coregionalization_work = completed_draws
        .checked_mul(types)
        .and_then(|value| value.checked_mul(types))
        .and_then(|value| value.checked_mul(nodes))
        .ok_or_else(|| CliError::Input("coregionalization work overflows".into()))?;
    if coregionalization_work > args.maximum_coregionalization_work {
        return Err(CliError::Input(format!(
            "coregionalization work exceeds maximum: {coregionalization_work} > {}",
            args.maximum_coregionalization_work
        )));
    }
    let input_sha256 = fixed["input_sha256"]
        .as_str()
        .ok_or_else(|| CliError::Input("correlated input digest differs".into()))?
        .to_owned();
    let correlation_prior = CorrelationPrior {
        lkj_concentration: args.lkj_concentration,
        marginal_field_scale: args.field_amplitude_prior_scale,
    };
    let resources = Resources {
        coregionalization_work,
        maximum_coregionalization_work: args.maximum_coregionalization_work,
        maximum_output_bytes: 16 * 1_048_576,
        timeout_seconds: args.timeout_seconds,
    };
    let directory = marklab::backend::python_backend_assets_root()?.join("workers/python");
    let backend = BackendContract {
        name: "numpyro",
        version: NUMPYRO_VERSION,
        python_version: PYTHON_VERSION,
        environment_lock_sha256: sha256_hex(&read_file(&directory.join("uv.lock"))?),
        worker_sha256: sha256_hex(&read_file(&directory.join(
            "marklab_numpyro_correlated_replicated_arbitrary_window_multitype_lgcp_worker.py",
        ))?),
    };
    let source_request_sha256 = sha256_hex(source.request_bytes());
    let request = WorkerRequest {
        format: "marklab.numpyro_correlated_replicated_arbitrary_window_multitype_lgcp_request",
        version: 1,
        backend: backend.clone(),
        jax_version: JAX_VERSION,
        source_request_sha256: &source_request_sha256,
        source_request: source_value.clone(),
        correlation_prior: correlation_prior.clone(),
        resources: resources.clone(),
        maximum_tree_depth: args.maximum_tree_depth,
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    Ok(Prepared {
        source,
        source_value,
        backend,
        correlation_prior,
        resources,
        request_bytes,
        request_sha256,
        input_sha256,
        timeout_seconds: args.timeout_seconds,
    })
}

pub(crate) fn execute(prepared: &Prepared) -> Result<Output, CliError> {
    let bytes = run_worker(
        &marklab::backend::python_backend_assets_root()?,
        "marklab_numpyro_correlated_replicated_arbitrary_window_multitype_lgcp_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: WorkerResult = serde_json::from_slice(&bytes)?;
    validate_worker(&result, &prepared.validation_context()).map_err(super::protocol_error)?;
    let fixed = &prepared.source_value["source_request"];
    let patients = fixed["patients"].as_array().unwrap();
    let patterns = fixed["patterns"].as_array().unwrap();
    let types = fixed["type_ids"].as_array().unwrap();
    let nodes = fixed["nodes"].as_array().unwrap();
    let counts = fixed["node_type_counts"].as_array().unwrap();
    Ok(Output {
        format: "marklab.correlated_replicated_arbitrary_window_multitype_lgcp".into(),
        version: 1,
        backend: result.backend,
        jax_version: result.jax_version,
        input_sha256: result.input_sha256,
        request_sha256: result.request_sha256,
        source_request_sha256: result.source_request_sha256,
        fit_state: result.fit_state,
        sampling: result.sampling,
        field_length_scale_um: result.field_length_scale_um,
        type_field_scales: result.type_field_scales,
        cross_type_correlations: result.cross_type_correlations,
        posterior_mean_correlation_matrix: result.posterior_mean_correlation_matrix,
        type_posteriors: result.type_posteriors,
        group_effect_differences: result.group_effect_differences,
        pattern_type_posterior_predictive: result.pattern_type_posterior_predictive,
        diagnostics: result.diagnostics,
        coregionalization_oracle: result.coregionalization_oracle,
        correlation_prior: prepared.correlation_prior.clone(),
        resources: prepared.resources.clone(),
        patient_count: patients.len(),
        pattern_count: patterns.len(),
        type_count: types.len(),
        total_node_count: nodes.len(),
        total_event_count: counts.iter().filter_map(|row| row["count"].as_u64()).sum(),
        statistical_unit: StatisticalUnit::Patient,
        pattern_unit: "slide_pattern_nested_in_patient".into(),
        likelihood: "correlated_type_specific_log_gaussian_cox_intensities_over_one_exact_window"
            .into(),
        null_model: NullHypothesis::EveryOffDiagonalLatentFieldCorrelationEqualsZero,
        correlation_parameterization:
            "lower_triangular_lkj_cholesky_with_positive_marginal_field_scales".into(),
        assumptions: [
            "one_shared_isotropic_matern_length_scale_across_types_and_patterns",
            "cross_type_dependence_is_spatial_latent_covariance_not_pairwise_interaction",
            "lkj_cholesky_and_positive_field_scales_identify_the_coregionalization",
            "patients_not_patterns_nodes_types_or_events_are_population_replicates",
            "exact_windows_and_complete_typed_quadrature_counts_are_provenance_complete",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        finite_result_policy: "nonconverged_is_diagnostic_only_reject_non_finite".into(),
        claim_status: if result.fit_state == FitState::Complete {
            ClaimStatus::ExperimentalCorrelatedMultitypeLatentField
        } else {
            ClaimStatus::DiagnosticOnlyNonconvergedCorrelatedMultitypeLatentField
        },
    })
}

impl Prepared {
    fn validation_context(&self) -> marklab_bayes::protocol::correlated_replicated_arbitrary_window_multitype_lgcp::ValidationContext<'_>{
        marklab_bayes::protocol::correlated_replicated_arbitrary_window_multitype_lgcp::ValidationContext {
            source_value: &self.source_value,
            backend: &self.backend,
            source_request_bytes: self.source.request_bytes(),
            request_sha256: &self.request_sha256,
            input_sha256: &self.input_sha256,
            correlation_prior: &self.correlation_prior,
            resources: &self.resources,
        }
    }

    pub(crate) fn validate_result(&self, result: &Output) -> Result<(), CliError> {
        result
            .validate_for(&self.validation_context())
            .map_err(super::protocol_error)
    }
}
