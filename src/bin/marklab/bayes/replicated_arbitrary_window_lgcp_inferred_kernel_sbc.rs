pub(crate) use marklab_bayes::protocol::replicated_arbitrary_window_lgcp_inferred_kernel_sbc::{
    validate_result, Calibration, Output, Resources, WorkerRequest, WorkerResult,
};

use crate::input_file::read_file;
use std::path::PathBuf;

use marklab_bayes::sha256_hex;
use marklab_bayes::BackendContract;
use marklab_bayes::NutsSamplingSpec;

use super::publish_json;
use super::replicated_arbitrary_window_lgcp_inferred_kernel;
use super::replicated_arbitrary_window_lgcp_inferred_kernel::PreparedReplicatedArbitraryWindowLgcpInferredKernel;
use super::run_worker;
use super::CliError;
use marklab_bayes::backend_versions::JAX_VERSION;
use marklab_bayes::backend_versions::NUMPYRO_VERSION;
use marklab_bayes::backend_versions::PYTHON_VERSION;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;

#[derive(Debug, clap::Args)]
pub(crate) struct Args {
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
    /// Prior scale for field amplitude.
    #[arg(long)]
    field_amplitude_prior_scale: f64,
    /// Prior scale, in micrometers, for field length scale.
    #[arg(long)]
    field_length_scale_prior_scale_um: f64,
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
    /// Upper bound on kernel cube work.
    #[arg(long)]
    maximum_kernel_cube_work: u64,
    /// Upper bound on tree depth.
    #[arg(long)]
    maximum_tree_depth: u32,
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

pub(super) fn run(args: Args) -> Result<(), CliError> {
    if !(20..=100).contains(&args.replicates)
        || !args.minimum_rank_uniformity_p_value.is_finite()
        || !(0.0..=1.0).contains(&args.minimum_rank_uniformity_p_value)
        || !args.minimum_coverage_90.is_finite()
        || !args.maximum_coverage_90.is_finite()
        || !(0.0..=args.maximum_coverage_90).contains(&args.minimum_coverage_90)
        || args.maximum_coverage_90 > 1.0
    {
        return Err(CliError::Input(
            "inferred-kernel replicated LGCP SBC controls are invalid".into(),
        ));
    }
    let sampling = NutsSamplingSpec {
        chains: args.chains,
        tune_per_chain: args.tune,
        draws_per_chain: args.draws,
        target_accept: args.target_accept,
        seed: args.seed,
    };
    let prepared = replicated_arbitrary_window_lgcp_inferred_kernel::prepare(
        crate::bayes::ReplicatedArbitraryWindowLgcpInferredKernelArgs {
            input: args.input,
            reference_group: args.reference_group,
            comparison_group: args.comparison_group,
            intercept_prior_mean: args.intercept_prior_mean,
            intercept_prior_sd: args.intercept_prior_sd,
            group_effect_prior_sd: args.group_effect_prior_sd,
            covariate_effect_prior_sd: args.covariate_effect_prior_sd,
            patient_sd_prior_scale: args.patient_sd_prior_scale,
            pattern_sd_prior_scale: args.pattern_sd_prior_scale,
            field_amplitude_prior_scale: args.field_amplitude_prior_scale,
            field_length_scale_prior_scale_um: args.field_length_scale_prior_scale_um,
            jitter: args.jitter,
            chains: sampling.chains,
            tune: sampling.tune_per_chain,
            draws: sampling.draws_per_chain,
            target_accept: sampling.target_accept,
            seed: sampling.seed,
            maximum_patients: args.maximum_patients,
            maximum_patterns: args.maximum_patterns,
            maximum_nodes_per_pattern: args.maximum_nodes_per_pattern,
            maximum_total_nodes: args.maximum_total_nodes,
            maximum_total_events: args.maximum_total_events,
            maximum_draw_node_work: args.maximum_draw_node_work,
            maximum_kernel_cube_work: args.maximum_kernel_cube_work,
            maximum_tree_depth: args.maximum_tree_depth,
            timeout_seconds: args.timeout_seconds,
            out: std::path::PathBuf::new(),
        },
    )?;
    let maximum_simulated_nodes = u64::from(args.replicates)
        .checked_mul(args.maximum_total_nodes as u64)
        .ok_or_else(|| CliError::Input("inferred-kernel SBC node work overflows".into()))?;
    let maximum_total_iterations = u64::from(args.replicates)
        .checked_mul(u64::from(args.chains))
        .and_then(|value| {
            u64::from(args.tune)
                .checked_add(u64::from(args.draws))
                .and_then(|iterations| value.checked_mul(iterations))
        })
        .ok_or_else(|| CliError::Input("inferred-kernel SBC iterations overflow".into()))?;
    let calibration = Calibration {
        replicates: args.replicates,
        rank_bins: 10,
        interval_probability: 0.9,
        latent_node_index: 0,
        minimum_rank_uniformity_p_value: args.minimum_rank_uniformity_p_value,
        minimum_coverage: args.minimum_coverage_90,
        maximum_coverage: args.maximum_coverage_90,
    };
    let resources = Resources {
        maximum_replicates: args.replicates,
        maximum_simulated_nodes,
        maximum_total_iterations,
        maximum_output_bytes: 2 * 1_048_576,
        timeout_seconds: args.timeout_seconds,
    };
    execute(
        prepared,
        sampling,
        calibration,
        resources,
        args.timeout_seconds,
        &args.out,
    )
}

fn execute(
    prepared: PreparedReplicatedArbitraryWindowLgcpInferredKernel,
    sampling: NutsSamplingSpec,
    calibration: Calibration,
    resources: Resources,
    timeout_seconds: u64,
    out: &std::path::Path,
) -> Result<(), CliError> {
    let repository = &marklab::backend::python_backend_assets_root()?;
    let directory = repository.join("workers/python");
    let backend = BackendContract {
        name: "numpyro",
        version: NUMPYRO_VERSION,
        python_version: PYTHON_VERSION,
        environment_lock_sha256: sha256_hex(&read_file(&directory.join("uv.lock"))?),
        worker_sha256: sha256_hex(&read_file(&directory.join(
            "marklab_numpyro_replicated_arbitrary_window_lgcp_inferred_kernel_sbc_worker.py",
        ))?),
    };
    let request = WorkerRequest {
        format: "marklab.numpyro_replicated_arbitrary_window_lgcp_inferred_kernel_sbc_request",
        version: 1,
        backend: backend.clone(),
        jax_version: JAX_VERSION,
        source_request_sha256: prepared.request_sha256(),
        source_request: serde_json::from_slice(prepared.request_bytes())?,
        calibration: calibration.clone(),
        resources: resources.clone(),
    };
    let bytes = serde_json::to_vec(&request)?;
    let request_sha = sha256_hex(&bytes);
    let worker_bytes = run_worker(
        repository,
        "marklab_numpyro_replicated_arbitrary_window_lgcp_inferred_kernel_sbc_worker.py",
        &bytes,
        timeout_seconds,
    )?;
    let result: WorkerResult = serde_json::from_slice(&worker_bytes)?;
    validate_result(
        &result,
        prepared.source_request(),
        prepared.request_sha256(),
        &backend,
        &request_sha,
        &calibration,
    )
    .map_err(super::protocol_error)?;
    publish_json(
        out,
        &Output {
            format: "marklab.replicated_arbitrary_window_lgcp_inferred_kernel_sbc",
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
            replicates: result.replicates,
            failures: result.failures,
            diagnostics: result.diagnostics,
            physical_latent_pattern_id: result.physical_latent_pattern_id,
            physical_latent_node_id: result.physical_latent_node_id,
            statistical_unit: StatisticalUnit::Patient,
            assumptions: [
                "prior_generative_counts_use_the_exact_replicated_hierarchy",
                "kernel_amplitude_and_physical_length_are_drawn_from_declared_priors",
                "every_failed_replicate_is_retained_with_its_reason",
                "patients_not_patterns_nodes_or_cells_are_population_replicates",
            ],
            finite_result_policy:
                "accept_only_complete_finite_replicates_and_declared_calibration_bounds",
            claim_status: ClaimStatus::ExperimentalSimulationCalibration,
        },
    )
}
