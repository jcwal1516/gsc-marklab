pub(crate) use marklab_bayes::protocol::replicated_arbitrary_window_multitype_lgcp_prior_calibration::{Calibration, Resources, WorkerRequest, WorkerResult, Output, validate_result};

use std::path::PathBuf;

use marklab_bayes::sha256_hex;
use marklab_bayes::BackendContract;

use super::publish_json;
use super::replicated_arbitrary_window_multitype_lgcp;
use super::replicated_arbitrary_window_multitype_lgcp::Args as FitArgs;
use super::run_worker;
use super::CliError;
use marklab_bayes::backend_versions::NUMPY_VERSION;
use marklab_bayes::backend_versions::PYTHON_VERSION;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;

const MAXIMUM_ADAPTER_BYTES: u64 = 8 * 1_048_576;

#[derive(Debug, clap::Args)]
pub(crate) struct Args {
    /// Per-node, per-type quadrature CSV (pattern_id, patient_id, group, type_id, count, ...).
    #[arg(long)]
    input: PathBuf,
    /// Group level used as the regression reference.
    #[arg(long)]
    reference_group: String,
    /// Group level whose effect is compared with the reference group.
    #[arg(long)]
    comparison_group: String,
    /// Point-type label used as the model reference.
    #[arg(long)]
    reference_type: String,
    /// Prior mean for intercept.
    #[arg(long, allow_hyphen_values = true)]
    intercept_prior_mean: f64,
    /// Prior scale for intercept.
    #[arg(long)]
    intercept_prior_sd: f64,
    /// Prior scale for group effect.
    #[arg(long)]
    group_effect_prior_sd: f64,
    /// Prior scale for covariate effect.
    #[arg(long)]
    covariate_effect_prior_sd: f64,
    /// Prior scale for patient standard deviation.
    #[arg(long)]
    patient_sd_prior_scale: f64,
    /// Prior scale for pattern standard deviation.
    #[arg(long)]
    pattern_sd_prior_scale: f64,
    /// Amplitude of the latent spatial field covariance.
    #[arg(long)]
    field_amplitude: f64,
    /// Spatial covariance length scale in micrometers.
    #[arg(long)]
    field_length_scale_um: f64,
    /// Diagonal stabilization added to covariance matrices.
    #[arg(long)]
    jitter: f64,
    /// Number of independent posterior sampling chains.
    #[arg(long)]
    chains: u32,
    /// Warmup iterations per posterior sampling chain.
    #[arg(long)]
    tune: u32,
    /// Posterior draws retained per chain after warmup.
    #[arg(long)]
    draws: u32,
    /// Target NUTS acceptance probability.
    #[arg(long)]
    target_accept: f64,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    seed: u64,
    /// Upper bound on patients.
    #[arg(long)]
    maximum_patients: usize,
    /// Upper bound on patterns.
    #[arg(long)]
    maximum_patterns: usize,
    /// Upper bound on types.
    #[arg(long)]
    maximum_types: usize,
    /// Upper bound on nodes per pattern.
    #[arg(long)]
    maximum_nodes_per_pattern: usize,
    /// Upper bound on total nodes.
    #[arg(long)]
    maximum_total_nodes: usize,
    /// Upper bound on total node type rows.
    #[arg(long)]
    maximum_total_node_type_rows: usize,
    /// Upper bound on total events.
    #[arg(long)]
    maximum_total_events: u64,
    /// Upper bound on draw node type work.
    #[arg(long)]
    maximum_draw_node_type_work: u64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    timeout_seconds: u64,
    /// Number of simulated datasets used for posterior calibration.
    #[arg(long)]
    replicates: u32,
    /// Upper bound on simulation node type work.
    #[arg(long)]
    maximum_simulation_node_type_work: u64,
    /// Upper bound on working bytes.
    #[arg(long)]
    maximum_working_bytes: u64,
    /// Largest posterior-mean error in standard-deviation units.
    #[arg(long)]
    maximum_mean_error_sd: f64,
    /// Largest admitted relative error in posterior standard deviation.
    #[arg(long)]
    maximum_relative_sd_error: f64,
    /// Largest admitted scaled covariance RMSE in field calibration.
    #[arg(long)]
    maximum_field_covariance_rmse_scale: f64,
    /// Largest admitted mean of standardized Poisson residuals.
    #[arg(long)]
    maximum_poisson_residual_mean: f64,
    /// Largest admitted error in the Poisson residual second moment.
    #[arg(long)]
    maximum_poisson_residual_second_moment_error: f64,
    /// Destination for the analysis output.
    #[arg(long)]
    out: PathBuf,
}

pub(super) fn run(args: Args) -> Result<(), CliError> {
    validate_controls(&args)?;
    let prepared = replicated_arbitrary_window_multitype_lgcp::prepare(FitArgs {
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
        field_amplitude: args.field_amplitude,
        field_length_scale_um: args.field_length_scale_um,
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
    let source: serde_json::Value = serde_json::from_slice(prepared.request_bytes())?;
    let input_sha = source["input_sha256"]
        .as_str()
        .filter(|value| value.len() == 64)
        .ok_or_else(|| CliError::Input("prepared calibration input digest differs".into()))?
        .to_owned();
    let node_type_rows = source["node_type_counts"]
        .as_array()
        .ok_or_else(|| CliError::Input("prepared calibration rows differ".into()))?
        .len();
    let node_count = source["nodes"]
        .as_array()
        .ok_or_else(|| CliError::Input("prepared calibration nodes differ".into()))?
        .len();
    let patient_count = source["patients"]
        .as_array()
        .ok_or_else(|| CliError::Input("prepared calibration patients differ".into()))?
        .len();
    let pattern_count = source["patterns"]
        .as_array()
        .ok_or_else(|| CliError::Input("prepared calibration patterns differ".into()))?
        .len();
    let type_count = source["type_ids"]
        .as_array()
        .ok_or_else(|| CliError::Input("prepared calibration types differ".into()))?
        .len();
    let replicates = u64::from(args.replicates);
    let simulation_work = replicates
        .checked_mul(node_type_rows as u64)
        .ok_or_else(|| CliError::Input("calibration work overflows".into()))?;
    let estimated_working_bytes = replicates
        .checked_mul(type_count as u64)
        .and_then(|value| value.checked_mul(node_count as u64))
        .and_then(|value| value.checked_mul(96))
        .and_then(|field| {
            replicates
                .checked_mul(type_count as u64)
                .and_then(|value| value.checked_mul((patient_count + pattern_count) as u64))
                .and_then(|value| value.checked_mul(32))
                .and_then(|hierarchy| field.checked_add(hierarchy))
        })
        .and_then(|arrays| arrays.checked_add(384 * 1_048_576))
        .ok_or_else(|| CliError::Input("calibration memory estimate overflows".into()))?;
    if simulation_work > args.maximum_simulation_node_type_work
        || estimated_working_bytes > args.maximum_working_bytes
    {
        return Err(CliError::Input(
            "replicated multitype LGCP prior-calibration resource ceiling exceeded".into(),
        ));
    }
    let calibration = Calibration {
        replicates: args.replicates,
        maximum_mean_error_sd: args.maximum_mean_error_sd,
        maximum_relative_sd_error: args.maximum_relative_sd_error,
        maximum_field_covariance_rmse_scale: args.maximum_field_covariance_rmse_scale,
        maximum_poisson_residual_mean: args.maximum_poisson_residual_mean,
        maximum_poisson_residual_second_moment_error: args
            .maximum_poisson_residual_second_moment_error,
    };
    let resources = Resources {
        node_type_rows,
        simulation_node_type_work: simulation_work,
        maximum_simulation_node_type_work: args.maximum_simulation_node_type_work,
        estimated_working_bytes,
        maximum_working_bytes: args.maximum_working_bytes,
        timeout_seconds: args.timeout_seconds,
    };
    let repository = &marklab::backend::python_backend_assets_root()?;
    let workers = repository.join("workers/python");
    let backend = BackendContract {
        name: "numpy",
        version: NUMPY_VERSION,
        python_version: PYTHON_VERSION,
        environment_lock_sha256: sha256_hex(&read_bounded(&workers.join("uv.lock"))?),
        worker_sha256: sha256_hex(&read_bounded(&workers.join(
            "marklab_numpy_replicated_arbitrary_window_multitype_lgcp_prior_calibration_worker.py",
        ))?),
    };
    let request = WorkerRequest {
        format:
            "marklab.numpy_replicated_arbitrary_window_multitype_lgcp_prior_calibration_request",
        version: 1,
        backend: backend.clone(),
        numpy_version: NUMPY_VERSION,
        source_request_sha256: prepared.request_sha256(),
        source_request: source,
        calibration: calibration.clone(),
        resources: resources.clone(),
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha = sha256_hex(&request_bytes);
    let worker_output = run_worker(
        repository,
        "marklab_numpy_replicated_arbitrary_window_multitype_lgcp_prior_calibration_worker.py",
        &request_bytes,
        args.timeout_seconds,
    )?;
    let result: WorkerResult = serde_json::from_slice(&worker_output)?;
    validate_result(
        &result,
        &backend,
        &request_sha,
        prepared.request_sha256(),
        &input_sha,
        &calibration,
        simulation_work,
    )
    .map_err(super::protocol_error)?;
    publish_json(
        &args.out,
        &Output {
            format: "marklab.replicated_arbitrary_window_multitype_lgcp_prior_calibration",
            version: 1,
            backend: result.backend,
            numpy_version: result.numpy_version,
            input_sha256: result.input_sha256,
            request_sha256: result.request_sha256,
            source_request_sha256: result.source_request_sha256,
            fit_state: result.fit_state,
            calibration,
            resources,
            moment_checks: result.moment_checks,
            field_oracle: result.field_oracle,
            poisson_oracle: result.poisson_oracle,
            statistical_unit: StatisticalUnit::Patient,
            generative_model:
                "patient_pattern_hierarchical_fixed_matern_multitype_poisson_intensity",
            analytic_oracles: [
                "gaussian_and_half_normal_prior_moments",
                "exact_centered_cholesky_covariance",
                "conditional_poisson_standardized_residual_moments",
            ],
            assumptions: [
                "patients_are_the_population_units_and_patterns_remain_nested",
                "type_specific_intensities_are_conditionally_independent_given_latent_effects",
                "the_matern_amplitude_and_length_are_fixed_controls",
                "calibration_thresholds_and_seed_are_declared_before_simulation",
            ],
            finite_result_policy:
                "every_calibration_metric_must_be_finite_and_every_analytic_check_must_pass",
            claim_status: ClaimStatus::ExperimentalPriorGenerativeCalibration,
        },
    )
}

fn validate_controls(args: &Args) -> Result<(), CliError> {
    let thresholds = [
        args.maximum_mean_error_sd,
        args.maximum_relative_sd_error,
        args.maximum_field_covariance_rmse_scale,
        args.maximum_poisson_residual_mean,
        args.maximum_poisson_residual_second_moment_error,
    ];
    if !(1_024..=65_536).contains(&args.replicates)
        || thresholds
            .into_iter()
            .any(|value| !value.is_finite() || value <= 0.0 || value > 1.0)
        || args.maximum_simulation_node_type_work == 0
        || args.maximum_working_bytes == 0
    {
        return Err(CliError::Input(
            "replicated multitype LGCP prior-calibration controls are invalid".into(),
        ));
    }
    Ok(())
}

fn read_bounded(path: &std::path::Path) -> Result<Vec<u8>, CliError> {
    crate::input_file::read_regular_file(
        path,
        MAXIMUM_ADAPTER_BYTES,
        "replicated multitype LGCP prior-calibration adapter is absent or oversized",
    )
}
