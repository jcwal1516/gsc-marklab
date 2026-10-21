pub(crate) use marklab_bayes::protocol::replicated_arbitrary_window_lgcp_sbc::{
    validate_result, Calibration, Resources, SbcResult, WorkerRequest, WorkerResult,
};

use crate::input_file::read_file;
use std::path::PathBuf;

use marklab_bayes::sha256_hex;
use marklab_bayes::BackendContract;
use marklab_bayes::NutsSamplingSpec;

use super::publish_json;
use super::replicated_arbitrary_window_lgcp_fit;
use super::run_worker;
use super::CliError;
use marklab_bayes::backend_versions::JAX_VERSION;
use marklab_bayes::backend_versions::NUMPYRO_VERSION;
use marklab_bayes::backend_versions::PYTHON_VERSION;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;

#[derive(Debug, clap::Args)]
pub(crate) struct Arguments {
    /// Per-node quadrature CSV (pattern_id, patient_id, group, node_id, x_um, y_um, count, ...).
    #[arg(long)]
    input: PathBuf,
    /// Group level used as the regression reference.
    #[arg(long)]
    reference_group: String,
    /// Group level whose effect is compared with the reference group.
    #[arg(long)]
    comparison_group: String,
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
    /// Number of simulated datasets used for posterior calibration.
    #[arg(long)]
    replicates: u32,
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
    /// Upper bound on nodes per pattern.
    #[arg(long)]
    maximum_nodes_per_pattern: usize,
    /// Upper bound on total nodes.
    #[arg(long)]
    maximum_total_nodes: usize,
    /// Upper bound on total events.
    #[arg(long)]
    maximum_total_events: u64,
    /// Upper bound on draw node work.
    #[arg(long)]
    maximum_draw_node_work: u64,
    /// Smallest admitted p-value for simulation-based rank uniformity.
    #[arg(long)]
    minimum_rank_uniformity_p_value: f64,
    /// Smallest admitted empirical coverage of nominal 90% intervals.
    #[arg(long)]
    minimum_coverage_90: f64,
    /// Largest admitted empirical coverage of nominal 90% intervals.
    #[arg(long)]
    maximum_coverage_90: f64,
    /// Maximum worker execution time in seconds.
    #[arg(long)]
    timeout_seconds: u64,
    /// Destination for the analysis output.
    #[arg(long)]
    out: PathBuf,
}

pub(super) fn run(arguments: Arguments) -> Result<(), CliError> {
    if !(20..=100).contains(&arguments.replicates)
        || !arguments.minimum_rank_uniformity_p_value.is_finite()
        || !(0.0..=1.0).contains(&arguments.minimum_rank_uniformity_p_value)
        || !arguments.minimum_coverage_90.is_finite()
        || !arguments.maximum_coverage_90.is_finite()
        || !(0.0..=arguments.maximum_coverage_90).contains(&arguments.minimum_coverage_90)
        || arguments.maximum_coverage_90 > 1.0
    {
        return Err(CliError::Input(
            "replicated LGCP SBC controls are invalid".into(),
        ));
    }
    let sampling = NutsSamplingSpec {
        chains: arguments.chains,
        tune_per_chain: arguments.tune,
        draws_per_chain: arguments.draws,
        target_accept: arguments.target_accept,
        seed: arguments.seed,
    };
    let prepared = replicated_arbitrary_window_lgcp_fit::prepare(
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
            field_amplitude: arguments.field_amplitude,
            field_length_scale_um: arguments.field_length_scale_um,
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
    let maximum_simulated_nodes = u64::from(arguments.replicates)
        .checked_mul(arguments.maximum_total_nodes as u64)
        .ok_or_else(|| CliError::Input("replicated LGCP SBC node work overflows".into()))?;
    let maximum_total_iterations = u64::from(arguments.replicates)
        .checked_mul(u64::from(arguments.chains))
        .and_then(|value| value.checked_mul(u64::from(arguments.tune + arguments.draws)))
        .ok_or_else(|| CliError::Input("replicated LGCP SBC iterations overflow".into()))?;
    let calibration = Calibration {
        replicates: arguments.replicates,
        rank_bins: 10,
        interval_probability: 0.9,
        latent_node_index: 0,
        minimum_rank_uniformity_p_value: arguments.minimum_rank_uniformity_p_value,
        minimum_coverage: arguments.minimum_coverage_90,
        maximum_coverage: arguments.maximum_coverage_90,
    };
    let resources = Resources {
        maximum_replicates: arguments.replicates,
        maximum_simulated_nodes,
        maximum_total_iterations,
        maximum_output_bytes: 2 * 1_048_576,
        timeout_seconds: arguments.timeout_seconds,
    };
    let repository = &marklab::backend::python_backend_assets_root()?;
    let directory = repository.join("workers/python");
    let lock = read_file(&directory.join("uv.lock"))?;
    let worker = read_file(
        &directory.join("marklab_numpyro_replicated_arbitrary_window_lgcp_sbc_worker.py"),
    )?;
    let backend = BackendContract {
        name: "numpyro",
        version: NUMPYRO_VERSION,
        python_version: PYTHON_VERSION,
        environment_lock_sha256: sha256_hex(&lock),
        worker_sha256: sha256_hex(&worker),
    };
    let request = WorkerRequest {
        format: "marklab.numpyro_replicated_arbitrary_window_lgcp_sbc_request",
        version: 1,
        backend: backend.clone(),
        jax_version: JAX_VERSION,
        source_request_sha256: &prepared.request_sha256,
        source_request: &prepared.request,
        calibration: calibration.clone(),
        resources: resources.clone(),
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let output = run_worker(
        repository,
        "marklab_numpyro_replicated_arbitrary_window_lgcp_sbc_worker.py",
        &request_bytes,
        arguments.timeout_seconds,
    )?;
    let worker_result: WorkerResult = serde_json::from_slice(&output)?;
    validate_result(
        &worker_result,
        &backend,
        &request_sha256,
        &prepared.request,
        &prepared.request_sha256,
        &calibration,
    )
    .map_err(super::protocol_error)?;
    publish_json(
        &arguments.out,
        &SbcResult {
            format: "marklab.replicated_arbitrary_window_lgcp_sbc".into(),
            version: 1,
            backend: worker_result.backend,
            jax_version: worker_result.jax_version,
            input_sha256: worker_result.input_sha256,
            request_sha256: worker_result.request_sha256,
            source_request_sha256: worker_result.source_request_sha256,
            fit_state: worker_result.fit_state,
            sampling,
            calibration,
            resources,
            replicates: worker_result.replicates,
            failures: worker_result.failures,
            diagnostics: worker_result.diagnostics,
            physical_latent_pattern_id: worker_result.physical_latent_pattern_id,
            physical_latent_node_id: worker_result.physical_latent_node_id,
            statistical_unit: StatisticalUnit::Patient,
            assumptions: vec![
                "prior_generative_counts_use_the_exact_replicated_hierarchy".into(),
                "every_failed_replicate_is_retained_with_its_reason".into(),
                "patients_not_patterns_nodes_or_cells_are_population_replicates".into(),
            ],
            finite_result_policy:
                "accept_only_complete_finite_replicates_and_declared_calibration_bounds".into(),
            claim_status: ClaimStatus::ExperimentalSimulationCalibration,
        },
    )
}
