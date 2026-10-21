pub(crate) use marklab_bayes::protocol::conditional_multitype_mark_sbc::{
    validate_result, Calibration, Output, Resources, ValidationContext, WorkerRequest, WorkerResult,
};

use crate::input_file::read_file;
use std::path::PathBuf;

use marklab_bayes::sha256_hex;
use marklab_bayes::BackendContract;
use marklab_bayes::NutsSamplingSpec;

use super::conditional_multitype_mark;
use super::conditional_multitype_mark::Args as FitArgs;
use super::publish_json;
use super::run_worker;
use super::CliError;
use marklab_bayes::backend_versions::JAX_VERSION;
use marklab_bayes::backend_versions::NUMPYRO_VERSION;
use marklab_bayes::backend_versions::PYTHON_VERSION;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;

#[derive(Debug, clap::Args)]
pub(crate) struct Args {
    /// CSV of points: point_id or cell_id, x_um, y_um, and type_id or histologic_compartment.
    #[arg(long)]
    input: PathBuf,
    /// Point-type label used as the model reference.
    #[arg(long)]
    reference_type: String,
    /// Spatial radius in micrometers.
    #[arg(long)]
    radius_um: f64,
    /// Prior scale for intercept.
    #[arg(long)]
    intercept_prior_sd: f64,
    /// Prior scale for interaction.
    #[arg(long)]
    interaction_prior_sd: f64,
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
    /// Upper bound on points.
    #[arg(long)]
    maximum_points: usize,
    /// Upper bound on types.
    #[arg(long)]
    maximum_types: usize,
    /// Upper bound on neighbor visits.
    #[arg(long)]
    maximum_neighbor_visits: u64,
    /// Upper bound on edges.
    #[arg(long)]
    maximum_edges: usize,
    /// Upper bound on draw parameter work.
    #[arg(long)]
    maximum_draw_parameter_work: u64,
    /// Upper bound on working bytes.
    #[arg(long)]
    maximum_working_bytes: usize,
    /// Upper bound on tree depth.
    #[arg(long)]
    maximum_tree_depth: u32,
    /// Upper bound on states.
    #[arg(long)]
    maximum_states: u64,
    /// Upper bound on enumeration work.
    #[arg(long)]
    maximum_enumeration_work: u64,
    /// Upper bound on enumeration bytes.
    #[arg(long)]
    maximum_enumeration_bytes: u64,
    /// Upper bound on total iterations.
    #[arg(long)]
    maximum_total_iterations: u64,
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
        || args.maximum_states == 0
        || args.maximum_enumeration_work == 0
        || args.maximum_enumeration_bytes == 0
    {
        return Err(CliError::Input(
            "conditional multitype SBC controls are invalid".into(),
        ));
    }
    let sampling = NutsSamplingSpec {
        chains: args.chains,
        tune_per_chain: args.tune,
        draws_per_chain: args.draws,
        target_accept: args.target_accept,
        seed: args.seed,
    };
    let prepared = conditional_multitype_mark::prepare(FitArgs {
        input: args.input,
        reference_type: args.reference_type,
        radius_um: args.radius_um,
        intercept_prior_sd: args.intercept_prior_sd,
        interaction_prior_sd: args.interaction_prior_sd,
        chains: args.chains,
        tune: args.tune,
        draws: args.draws,
        target_accept: args.target_accept,
        seed: args.seed,
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
    let point_count = source["points"]
        .as_array()
        .ok_or_else(|| CliError::Input("prepared points differ".into()))?
        .len();
    let type_ids = source["type_ids"]
        .as_array()
        .ok_or_else(|| CliError::Input("prepared types differ".into()))?
        .iter()
        .map(|value| value.as_str().unwrap_or_default().to_owned())
        .collect::<Vec<_>>();
    let edge_count = source["edges"]
        .as_array()
        .ok_or_else(|| CliError::Input("prepared edges differ".into()))?
        .len();
    if point_count > 10 || type_ids.len() > 4 || type_ids.iter().any(String::is_empty) {
        return Err(CliError::Input(
            "exact conditional multitype SBC requires at most ten sites and four types".into(),
        ));
    }
    let state_count = (type_ids.len() as u64)
        .checked_pow(point_count as u32)
        .ok_or_else(|| CliError::Input("SBC state count overflows".into()))?;
    let enumeration_work = state_count
        .checked_mul((point_count + edge_count) as u64)
        .ok_or_else(|| CliError::Input("SBC enumeration work overflows".into()))?;
    let enumeration_bytes = state_count
        .checked_mul(point_count as u64)
        .and_then(|value| value.checked_mul(2))
        .and_then(|value| value.checked_add(state_count.checked_mul(16)?))
        .ok_or_else(|| CliError::Input("SBC enumeration bytes overflow".into()))?;
    let total_iterations = u64::from(args.replicates)
        .checked_mul(u64::from(args.chains))
        .and_then(|value| {
            u64::from(args.tune)
                .checked_add(u64::from(args.draws))
                .and_then(|iterations| value.checked_mul(iterations))
        })
        .ok_or_else(|| CliError::Input("SBC total iterations overflow".into()))?;
    if state_count > args.maximum_states
        || enumeration_work > args.maximum_enumeration_work
        || enumeration_bytes > args.maximum_enumeration_bytes
        || total_iterations > args.maximum_total_iterations
    {
        return Err(CliError::Input(
            "conditional multitype SBC resource ceiling exceeded".into(),
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
        maximum_states: args.maximum_states,
        state_count,
        maximum_enumeration_work: args.maximum_enumeration_work,
        enumeration_work,
        maximum_enumeration_bytes: args.maximum_enumeration_bytes,
        enumeration_bytes,
        maximum_total_iterations: args.maximum_total_iterations,
        maximum_output_bytes: 2 * 1_048_576,
        timeout_seconds: args.timeout_seconds,
    };
    let repository = &marklab::backend::python_backend_assets_root()?;
    let directory = repository.join("workers/python");
    let backend = BackendContract {
        name: "numpyro",
        version: NUMPYRO_VERSION,
        python_version: PYTHON_VERSION,
        environment_lock_sha256: sha256_hex(&read_file(&directory.join("uv.lock"))?),
        worker_sha256: sha256_hex(&read_file(
            &directory.join("marklab_numpyro_conditional_multitype_mark_sbc_worker.py"),
        )?),
    };
    let request = WorkerRequest {
        format: "marklab.numpyro_conditional_multitype_mark_sbc_request",
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
    let output = run_worker(
        repository,
        "marklab_numpyro_conditional_multitype_mark_sbc_worker.py",
        &bytes,
        args.timeout_seconds,
    )?;
    let result: WorkerResult = serde_json::from_slice(&output)?;
    validate_result(
        &result,
        ValidationContext {
            backend: &backend,
            request_sha256: &request_sha,
            source_request_sha256: prepared.request_sha256(),
            type_ids: &type_ids,
            point_count,
            sampling: &sampling,
            calibration: &calibration,
        },
    )
    .map_err(super::protocol_error)?;
    publish_json(
        &args.out,
        &Output {
            format: "marklab.conditional_multitype_mark_sbc",
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
            statistical_unit: StatisticalUnit::OneFixedLocationPattern,
            generative_model: "exact_normalized_finite_state_symmetric_categorical_gibbs",
            fitted_model: "single_site_conditional_composite_pseudoposterior",
            assumptions: [
                "all_finite_joint_states_are_enumerated_exactly",
                "truths_are_drawn_from_the_declared_fit_priors",
                "every_failed_replicate_is_retained_with_its_reason",
                "calibration_failure_is_not_repaired_by_threshold_or_seed_search",
            ],
            finite_result_policy: "accept_only_exact_dispositions_finite_diagnostics_and_declared_rank_coverage_bounds",
            claim_status: ClaimStatus::ExperimentalSimulationCalibration,
        },
    )
}
