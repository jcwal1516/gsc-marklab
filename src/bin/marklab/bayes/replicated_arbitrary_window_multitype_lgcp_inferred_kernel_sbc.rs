use marklab_bayes::protocol::replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc::SCENARIOS;
pub(crate) use marklab_bayes::protocol::replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc::{Calibration, Resources, WorkerRequest, WorkerResult, Output, validate_worker, assumptions};

use crate::input_file::read_file;
use std::path::PathBuf;

use marklab_bayes::sha256_hex;
use marklab_bayes::BackendContract;

use super::publish_json;
use super::replicated_arbitrary_window_multitype_lgcp_inferred_kernel;
use super::replicated_arbitrary_window_multitype_lgcp_inferred_kernel::Args as FitArgs;
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
    /// Diagonal stabilization added to covariance matrices.
    #[arg(long)]
    pub(crate) jitter: f64,
    /// Number of simulated replicates for each declared calibration scenario.
    #[arg(long)]
    pub(crate) replicates_per_scenario: u32,
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
    /// Upper bound on simulated node type work.
    #[arg(long)]
    pub(crate) maximum_simulated_node_type_work: u64,
    /// Upper bound on total iterations.
    #[arg(long)]
    pub(crate) maximum_total_iterations: u64,
    /// Upper bound on posterior predictive pair work.
    #[arg(long)]
    pub(crate) maximum_ppc_pair_work: u64,
    /// Upper bound on working bytes.
    #[arg(long)]
    pub(crate) maximum_working_bytes: u64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    pub(crate) timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(crate) out: PathBuf,
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

pub(super) fn run(args: Box<Args>) -> Result<(), CliError> {
    let out = args.out.clone();
    let prepared = prepare(*args)?;
    publish_json(&out, &execute(&prepared)?)
}

pub(crate) fn prepare(args: Args) -> Result<Prepared, CliError> {
    if !(1..=20).contains(&args.replicates_per_scenario)
        || args.maximum_simulated_node_type_work == 0
        || args.maximum_total_iterations == 0
        || args.maximum_ppc_pair_work == 0
        || args.maximum_working_bytes == 0
        || args.maximum_working_bytes > 8 * 1024 * 1024 * 1024
    {
        return Err(CliError::Input("multitype SBC controls are invalid".into()));
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
        .ok_or_else(|| CliError::Input("prepared SBC rows differ".into()))?
        .len() as u64;
    let nodes = fixed["nodes"]
        .as_array()
        .ok_or_else(|| CliError::Input("prepared SBC nodes differ".into()))?
        .len() as u64;
    let input_sha256 = fixed["input_sha256"]
        .as_str()
        .ok_or_else(|| CliError::Input("prepared SBC input digest differs".into()))?
        .to_owned();
    let scenario_replicates = u64::from(args.replicates_per_scenario) * SCENARIOS.len() as u64;
    let simulated_node_type_work = scenario_replicates
        .checked_mul(rows)
        .ok_or_else(|| CliError::Input("SBC simulation work overflows".into()))?;
    let total_iterations = scenario_replicates
        .checked_mul(u64::from(args.chains))
        .and_then(|value| value.checked_mul(u64::from(args.tune) + u64::from(args.draws)))
        .ok_or_else(|| CliError::Input("SBC iteration work overflows".into()))?;
    let completed_draws = u64::from(args.chains) * u64::from(args.draws);
    let ppc_pair_work = scenario_replicates
        .checked_mul(nodes)
        .and_then(|value| value.checked_mul(nodes))
        .and_then(|value| value.checked_mul(completed_draws))
        .ok_or_else(|| CliError::Input("SBC PPC work overflows".into()))?;
    let estimated_working_bytes = scenario_replicates
        .checked_mul(rows)
        .and_then(|value| value.checked_mul(256))
        .and_then(|value| value.checked_add(384 * 1024 * 1024))
        .ok_or_else(|| CliError::Input("SBC memory estimate overflows".into()))?;
    if simulated_node_type_work > args.maximum_simulated_node_type_work
        || total_iterations > args.maximum_total_iterations
        || ppc_pair_work > args.maximum_ppc_pair_work
        || estimated_working_bytes > args.maximum_working_bytes
    {
        return Err(CliError::Input(
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
    let repository = &marklab::backend::python_backend_assets_root()?;
    let directory = repository.join("workers/python");
    let backend = BackendContract {
        name: "numpyro",
        version: NUMPYRO_VERSION,
        python_version: PYTHON_VERSION,
        environment_lock_sha256: sha256_hex(&read_file(&directory.join("uv.lock"))?),
        worker_sha256: sha256_hex(&read_file(&directory.join(
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

pub(crate) fn execute(prepared: &Prepared) -> Result<Output, CliError> {
    let bytes = run_worker(
        &marklab::backend::python_backend_assets_root()?,
        "marklab_numpyro_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: WorkerResult = serde_json::from_slice(&bytes)?;
    validate_worker(&result, &prepared.validation_context()).map_err(super::protocol_error)?;
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
        statistical_unit: StatisticalUnit::Patient,
        null_model:
            NullHypothesis::PriorGenerativeOrDeclaredStressScenarioUnderTheSameQuadratureLikelihood,
        assumptions: assumptions().into_iter().map(str::to_owned).collect(),
        failure_policy: "retain_every_scenario_replicate_with_explicit_status_and_reason".into(),
        finite_result_policy: "reject_non_finite_completed_rows_failed_rows_remain_diagnostic"
            .into(),
        claim_status: ClaimStatus::ExperimentalMultitypeSbcDiagnostic,
    })
}

impl Prepared {
    fn validation_context(&self) -> marklab_bayes::protocol::replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc::ValidationContext<'_>{
        marklab_bayes::protocol::replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc::ValidationContext {
            backend: &self.backend,
            source_request_bytes: self.fit.request_bytes(),
            request_sha256: &self.request_sha256,
            input_sha256: &self.input_sha256,
            calibration: &self.calibration,
            resources: &self.resources,
        }
    }

    pub(crate) fn validate_result(&self, result: &Output) -> Result<(), CliError> {
        result
            .validate_for(&self.validation_context())
            .map_err(super::protocol_error)
    }
}
