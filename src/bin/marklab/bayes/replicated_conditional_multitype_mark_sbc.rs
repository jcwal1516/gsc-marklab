pub(crate) use marklab_bayes::protocol::replicated_conditional_multitype_mark_sbc::{
    validate_result, Calibration, Output, Resources, ValidationContext, WorkerRequest, WorkerResult,
};

use std::path::PathBuf;

use marklab_bayes::sha256_hex;
use marklab_bayes::BackendContract;
use marklab_bayes::NutsSamplingSpec;

use super::publish_json;
use super::replicated_conditional_multitype_mark;
use super::replicated_conditional_multitype_mark::Args as FitArgs;
use super::run_worker;
use super::CliError;
use marklab_bayes::backend_versions::JAX_VERSION;
use marklab_bayes::backend_versions::NUMPYRO_VERSION;
use marklab_bayes::backend_versions::PYTHON_VERSION;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::StatisticalUnit;

const MAXIMUM_ADAPTER_BYTES: u64 = 8 * 1_048_576;

#[derive(Debug, clap::Args)]
pub(crate) struct Args {
    /// CSV of points with pattern_id, patient_id, group, point_id, x_um, y_um and type_id.
    #[arg(long)]
    input: PathBuf,
    /// Group level used as the regression reference.
    #[arg(long)]
    reference_group: String,
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
    /// Prior scale for group effect.
    #[arg(long)]
    group_effect_prior_sd: f64,
    /// Prior scale for patient standard deviation.
    #[arg(long)]
    patient_sd_prior_scale: f64,
    /// Prior scale for pattern standard deviation.
    #[arg(long)]
    pattern_sd_prior_scale: f64,
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
    /// Upper bound on states per pattern.
    #[arg(long)]
    maximum_states_per_pattern: u64,
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
    validate_controls(&args)?;
    let sampling = NutsSamplingSpec {
        chains: args.chains,
        tune_per_chain: args.tune,
        draws_per_chain: args.draws,
        target_accept: args.target_accept,
        seed: args.seed,
    };
    let prepared = replicated_conditional_multitype_mark::prepare(FitArgs {
        input: args.input,
        reference_group: args.reference_group,
        reference_type: args.reference_type,
        radius_um: args.radius_um,
        intercept_prior_sd: args.intercept_prior_sd,
        interaction_prior_sd: args.interaction_prior_sd,
        group_effect_prior_sd: args.group_effect_prior_sd,
        patient_sd_prior_scale: args.patient_sd_prior_scale,
        pattern_sd_prior_scale: args.pattern_sd_prior_scale,
        chains: args.chains,
        tune: args.tune,
        draws: args.draws,
        target_accept: args.target_accept,
        seed: args.seed,
        maximum_patients: args.maximum_patients,
        maximum_patterns: args.maximum_patterns,
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
    let input_sha = source["input_sha256"]
        .as_str()
        .filter(|value| value.len() == 64)
        .ok_or_else(|| CliError::Input("prepared input digest differs".into()))?
        .to_owned();
    let types = string_array(&source, "type_ids")?;
    let patterns = source["patterns"]
        .as_array()
        .ok_or_else(|| CliError::Input("prepared patterns differ".into()))?;
    let pattern_count = patterns.len();
    let point_counts = patterns
        .iter()
        .map(|row| row["point_count"].as_u64().map(|value| value as usize))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| CliError::Input("prepared pattern point counts differ".into()))?;
    let points_per_pattern = point_counts[0];
    if !(6..=8).contains(&points_per_pattern)
        || point_counts
            .iter()
            .any(|count| *count != points_per_pattern)
        || types.len() > 4
    {
        return Err(CliError::Input(
            "exact replicated conditional-mark SBC requires six to eight sites per pattern, equal pattern sizes, and at most four types"
                .into(),
        ));
    }
    let states_per_pattern = (types.len() as u64)
        .checked_pow(points_per_pattern as u32)
        .ok_or_else(|| CliError::Input("SBC state count overflows".into()))?;
    let per_replicate_enumeration = patterns.iter().try_fold(0_u64, |total, row| {
        let points = row["point_count"].as_u64()?;
        let edges = row["edge_count"].as_u64()?;
        total.checked_add(states_per_pattern.checked_mul(points.checked_add(edges)?)?)
    });
    let enumeration_work = per_replicate_enumeration
        .and_then(|work| work.checked_mul(u64::from(args.replicates)))
        .ok_or_else(|| CliError::Input("SBC enumeration work overflows".into()))?;
    let enumeration_bytes = states_per_pattern
        .checked_mul(points_per_pattern as u64)
        .and_then(|value| value.checked_mul(2))
        .and_then(|value| value.checked_add(states_per_pattern.checked_mul(16)?))
        .ok_or_else(|| CliError::Input("SBC enumeration bytes overflow".into()))?;
    let total_iterations = u64::from(args.replicates)
        .checked_mul(u64::from(args.chains))
        .and_then(|value| {
            u64::from(args.tune)
                .checked_add(u64::from(args.draws))
                .and_then(|iterations| value.checked_mul(iterations))
        })
        .ok_or_else(|| CliError::Input("SBC iteration work overflows".into()))?;
    if states_per_pattern > args.maximum_states_per_pattern
        || enumeration_work > args.maximum_enumeration_work
        || enumeration_bytes > args.maximum_enumeration_bytes
        || total_iterations > args.maximum_total_iterations
    {
        return Err(CliError::Input(
            "replicated conditional-mark SBC resource ceiling exceeded".into(),
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
        maximum_states_per_pattern: args.maximum_states_per_pattern,
        states_per_pattern,
        points_per_pattern,
        pattern_count,
        maximum_enumeration_work: args.maximum_enumeration_work,
        enumeration_work,
        maximum_enumeration_bytes: args.maximum_enumeration_bytes,
        enumeration_bytes,
        maximum_total_iterations: args.maximum_total_iterations,
        total_iterations,
        maximum_output_bytes: 4 * 1_048_576,
        timeout_seconds: args.timeout_seconds,
    };
    let repository = &marklab::backend::python_backend_assets_root()?;
    let workers = repository.join("workers/python");
    let backend = BackendContract {
        name: "numpyro",
        version: NUMPYRO_VERSION,
        python_version: PYTHON_VERSION,
        environment_lock_sha256: sha256_hex(&read_bounded(
            &workers.join("uv.lock"),
            MAXIMUM_ADAPTER_BYTES,
        )?),
        worker_sha256: sha256_hex(&read_bounded(
            &workers.join("marklab_numpyro_replicated_conditional_multitype_mark_sbc_worker.py"),
            MAXIMUM_ADAPTER_BYTES,
        )?),
    };
    let request = WorkerRequest {
        format: "marklab.numpyro_replicated_conditional_multitype_mark_sbc_request",
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
    let worker_output = run_worker(
        repository,
        "marklab_numpyro_replicated_conditional_multitype_mark_sbc_worker.py",
        &bytes,
        args.timeout_seconds,
    )?;
    let result: WorkerResult = serde_json::from_slice(&worker_output)?;
    validate_result(
        &result,
        ValidationContext {
            backend: &backend,
            request_sha256: &request_sha,
            source_request_sha256: prepared.request_sha256(),
            input_sha256: &input_sha,
            types: &types,
            pattern_count,
            points_per_pattern,
            sampling: &sampling,
            calibration: &calibration,
        },
    )
    .map_err(super::protocol_error)?;
    publish_json(
        &args.out,
        &Output {
            format: "marklab.replicated_conditional_multitype_mark_sbc",
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
            statistical_unit: StatisticalUnit::Patient,
            generative_model: "exact_normalized_finite_state_patient_pattern_hierarchical_gibbs",
            fitted_model: "patient_pattern_hierarchical_single_site_composite_pseudoposterior",
            assumptions: [
                "patient_and_pattern_hierarchy_truths_are_drawn_from_the_declared_fit_priors",
                "every_pattern_state_is_enumerated_exactly_conditional_on_its_hierarchy_truth",
                "patients_are_the_population_replicates_and_patterns_remain_nested",
                "every_failed_replicate_is_retained_with_its_exact_reason",
                "calibration_failure_is_not_repaired_by_threshold_or_seed_search",
            ],
            finite_result_policy: "accept_only_complete_dispositions_finite_diagnostics_and_declared_rank_coverage_bounds",
            claim_status: ClaimStatus::ExperimentalSimulationCalibration,
        },
    )
}

fn validate_controls(args: &Args) -> Result<(), CliError> {
    if !(20..=100).contains(&args.replicates)
        || !args.minimum_rank_uniformity_p_value.is_finite()
        || !(0.0..=1.0).contains(&args.minimum_rank_uniformity_p_value)
        || !args.minimum_coverage_90.is_finite()
        || !args.maximum_coverage_90.is_finite()
        || !(0.0..=args.maximum_coverage_90).contains(&args.minimum_coverage_90)
        || args.maximum_coverage_90 > 1.0
        || args.maximum_states_per_pattern == 0
        || args.maximum_enumeration_work == 0
        || args.maximum_enumeration_bytes == 0
        || args.maximum_total_iterations == 0
    {
        return Err(CliError::Input(
            "replicated conditional-mark SBC controls are invalid".into(),
        ));
    }
    Ok(())
}

fn string_array(value: &serde_json::Value, name: &str) -> Result<Vec<String>, CliError> {
    value[name]
        .as_array()
        .and_then(|rows| {
            rows.iter()
                .map(|row| row.as_str().map(ToOwned::to_owned))
                .collect::<Option<Vec<_>>>()
        })
        .filter(|rows| !rows.is_empty())
        .ok_or_else(|| CliError::Input(format!("prepared {name} differ")))
}

fn read_bounded(path: &std::path::Path, maximum: u64) -> Result<Vec<u8>, CliError> {
    crate::input_file::read_regular_file(
        path,
        maximum,
        "replicated conditional-mark SBC adapter is absent or oversized",
    )
}
