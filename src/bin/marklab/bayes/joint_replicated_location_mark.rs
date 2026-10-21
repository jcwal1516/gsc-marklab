pub(crate) use marklab_bayes::protocol::joint_replicated_location_mark::{
    validate_worker, Output, Priors, Resources, Sampling, WorkerRequest, WorkerResult,
};

use std::collections::BTreeMap;
use std::path::PathBuf;

use marklab_bayes::sha256_hex;
use marklab_bayes::BackendContract;
use marklab_bayes::FitState;
use serde::Deserialize;

use super::publish_json;
use super::run_worker;
use super::CliError;
use marklab_bayes::backend_versions::JAX_VERSION;
use marklab_bayes::backend_versions::NUMPYRO_VERSION;
use marklab_bayes::backend_versions::PYTHON_VERSION;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::NullHypothesis;
use marklab_core::vocabulary::StatisticalUnit;

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, clap::Args)]
pub(crate) struct Args {
    /// Observed locations and their replicate bindings.
    #[arg(long)]
    pub(crate) location_input: PathBuf,
    /// Observed marks and their location or replicate bindings.
    #[arg(long)]
    pub(crate) mark_input: PathBuf,
    /// Group level used as the regression reference.
    #[arg(long)]
    pub(crate) reference_group: String,
    /// Group level whose effect is compared with the reference group.
    #[arg(long)]
    pub(crate) comparison_group: String,
    /// Point-type label used as the model reference.
    #[arg(long)]
    pub(crate) reference_type: String,
    /// Neighborhood radius in micrometers.
    #[arg(long)]
    pub(crate) neighbor_radius_um: f64,
    /// Prior scale for location.
    #[arg(long)]
    pub(crate) location_prior_sd: f64,
    /// Prior scale for group.
    #[arg(long)]
    pub(crate) group_prior_sd: f64,
    /// Prior scale for patient ecology.
    #[arg(long)]
    pub(crate) patient_ecology_prior_scale: f64,
    /// Prior scale for mark intercept.
    #[arg(long)]
    pub(crate) mark_intercept_prior_sd: f64,
    /// Prior scale for mark group.
    #[arg(long)]
    pub(crate) mark_group_prior_sd: f64,
    /// Prior scale for mark loading.
    #[arg(long)]
    pub(crate) mark_loading_prior_sd: f64,
    /// Prior scale for interaction.
    #[arg(long)]
    pub(crate) interaction_prior_sd: f64,
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
    /// Upper bound on location rows.
    #[arg(long)]
    pub(crate) maximum_location_rows: usize,
    /// Upper bound on mark points.
    #[arg(long)]
    pub(crate) maximum_mark_points: usize,
    /// Upper bound on neighbor visits.
    #[arg(long)]
    pub(crate) maximum_neighbor_visits: u64,
    /// Upper bound on draw observation work.
    #[arg(long)]
    pub(crate) maximum_draw_observation_work: u64,
    /// Upper bound on working bytes.
    #[arg(long)]
    pub(crate) maximum_working_bytes: u64,
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
    backend: BackendContract,
    request_bytes: Vec<u8>,
    request_sha256: String,
    location_sha256: String,
    mark_sha256: String,
    sampling: Sampling,
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
    let location_bytes = read(&args.location_input)?;
    let mark_bytes = read(&args.mark_input)?;
    let location_csv = String::from_utf8(location_bytes)
        .map_err(|_| CliError::Input("location CSV is not UTF-8".into()))?;
    let mark_csv = String::from_utf8(mark_bytes)
        .map_err(|_| CliError::Input("mark CSV is not UTF-8".into()))?;
    let location_rows = location_csv.lines().count().saturating_sub(1);
    let mark_rows = mark_csv.lines().count().saturating_sub(1);
    let mut pattern_counts = BTreeMap::<String, u64>::new();
    let mut reader = csv::Reader::from_reader(mark_csv.as_bytes());
    #[derive(Deserialize)]
    struct MarkRow {
        pattern_id: String,
    }
    for row in reader.deserialize::<MarkRow>() {
        let row = row?;
        *pattern_counts.entry(row.pattern_id).or_default() += 1;
    }
    let neighbor_visits = pattern_counts
        .values()
        .try_fold(0_u64, |total, count| {
            count
                .checked_mul(count.saturating_sub(1))
                .and_then(|work| total.checked_add(work))
        })
        .ok_or_else(|| CliError::Input("neighbor work overflows".into()))?;
    let draw_observation_work = u64::from(args.chains)
        .checked_mul(u64::from(args.draws))
        .and_then(|value| value.checked_mul((location_rows + mark_rows) as u64))
        .and_then(|value| value.checked_mul(3))
        .ok_or_else(|| CliError::Input("joint draw work overflows".into()))?;
    let estimated_working_bytes = (location_rows as u64)
        .checked_mul(256)
        .and_then(|value| value.checked_add((mark_rows as u64).checked_mul(512)?))
        .and_then(|value| value.checked_add(neighbor_visits.checked_mul(16)?))
        .and_then(|value| value.checked_add(384 * 1024 * 1024))
        .ok_or_else(|| CliError::Input("joint memory estimate overflows".into()))?;
    let priors = Priors {
        location_sd: args.location_prior_sd,
        group_sd: args.group_prior_sd,
        patient_ecology_scale: args.patient_ecology_prior_scale,
        mark_intercept_sd: args.mark_intercept_prior_sd,
        mark_group_sd: args.mark_group_prior_sd,
        mark_loading_sd: args.mark_loading_prior_sd,
        interaction_sd: args.interaction_prior_sd,
    };
    let sampling = Sampling {
        chains: args.chains,
        tune: args.tune,
        draws: args.draws,
        target_accept: args.target_accept,
        seed: args.seed,
    };
    if priors
        .clone()
        .into_values()
        .into_iter()
        .any(|value| !value.is_finite() || value <= 0.0)
        || !args.neighbor_radius_um.is_finite()
        || args.neighbor_radius_um <= 0.0
        || location_rows > args.maximum_location_rows
        || mark_rows > args.maximum_mark_points
        || neighbor_visits > args.maximum_neighbor_visits
        || draw_observation_work > args.maximum_draw_observation_work
        || estimated_working_bytes > args.maximum_working_bytes
        || !(10..=14).contains(&args.maximum_tree_depth)
    {
        return Err(CliError::Input(
            "joint location-mark controls or resources differ".into(),
        ));
    }
    let resources = Resources {
        maximum_patients: args.maximum_patients,
        maximum_patterns: args.maximum_patterns,
        maximum_types: args.maximum_types,
        maximum_location_rows: args.maximum_location_rows,
        maximum_mark_points: args.maximum_mark_points,
        maximum_neighbor_visits: args.maximum_neighbor_visits,
        neighbor_visits,
        maximum_draw_observation_work: args.maximum_draw_observation_work,
        draw_observation_work,
        maximum_working_bytes: args.maximum_working_bytes,
        estimated_working_bytes,
        maximum_tree_depth: args.maximum_tree_depth,
        timeout_seconds: args.timeout_seconds,
        maximum_output_bytes: 4 * 1024 * 1024,
    };
    let repository = &marklab::backend::python_backend_assets_root()?;
    let workers = repository.join("workers/python");
    let backend = BackendContract {
        name: "numpyro",
        version: NUMPYRO_VERSION,
        python_version: PYTHON_VERSION,
        environment_lock_sha256: sha256_hex(&read(&workers.join("uv.lock"))?),
        worker_sha256: sha256_hex(&read(
            &workers.join("marklab_numpyro_joint_replicated_location_mark_worker.py"),
        )?),
    };
    let location_sha256 = sha256_hex(location_csv.as_bytes());
    let mark_sha256 = sha256_hex(mark_csv.as_bytes());
    let request = WorkerRequest {
        format: "marklab.numpyro_joint_replicated_location_mark_request",
        version: 1,
        backend: backend.clone(),
        jax_version: JAX_VERSION,
        location_csv: &location_csv,
        mark_csv: &mark_csv,
        location_sha256: &location_sha256,
        mark_sha256: &mark_sha256,
        reference_group: &args.reference_group,
        comparison_group: &args.comparison_group,
        reference_type: &args.reference_type,
        neighbor_radius_um: args.neighbor_radius_um,
        priors,
        sampling: sampling.clone(),
        resources: resources.clone(),
    };
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    Ok(Prepared {
        backend,
        request_bytes,
        request_sha256,
        location_sha256,
        mark_sha256,
        sampling,
        resources,
        timeout_seconds: args.timeout_seconds,
    })
}

pub(crate) fn execute(prepared: &Prepared) -> Result<Output, CliError> {
    let bytes = run_worker(
        &marklab::backend::python_backend_assets_root()?,
        "marklab_numpyro_joint_replicated_location_mark_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: WorkerResult = serde_json::from_slice(&bytes)?;
    validate_worker(&result, &prepared.validation_context()).map_err(super::protocol_error)?;
    Ok(Output {
        format: "marklab.joint_replicated_location_mark_fit".into(),
        version: 1,
        backend: result.backend,
        jax_version: result.jax_version,
        request_sha256: result.request_sha256,
        location_sha256: result.location_sha256,
        mark_sha256: result.mark_sha256,
        patient_count: result.patient_count,
        pattern_count: result.pattern_count,
        training_pattern_count: result.training_pattern_count,
        heldout_pattern_count: result.heldout_pattern_count,
        type_ids: result.type_ids,
        training_pattern_ids: result.training_pattern_ids,
        heldout_pattern_ids: result.heldout_pattern_ids,
        joint_posterior: result.joint_posterior,
        heldout_comparison: result.heldout_comparison,
        posterior_predictive: result.posterior_predictive,
        diagnostics: result.diagnostics,
        fit_state: result.fit_state,
        sampling: prepared.sampling.clone(),
        resources: prepared.resources.clone(),
        statistical_unit: StatisticalUnit::Patient,
        location_likelihood: "exact_window_quadrature_poisson_total_location_intensity".into(),
        mark_likelihood: "conditional_categorical_mark_on_fixed_physical_radius_graph".into(),
        holdout_policy:
            "lexicographically_first_pattern_train_second_pattern_evaluation_per_patient".into(),
        null_model: NullHypothesis::ZeroSharedPatientEcologyLoadingAndZeroGroupEffects,
        assumptions: vec![
            "each_patient_has_exactly_two_provenance_matched_patterns".into(),
            "location_and_mark_tables_share_patient_pattern_group_and_type_identity".into(),
            "shared_patient_ecology_is_a_statistical_coupling_not_a_biological_mechanism".into(),
            "evaluation_patterns_are_not_used_by_any_of_the_three_fits".into(),
            "patients_not_patterns_nodes_cells_or_edges_are_population_replicates".into(),
        ],
        failure_policy: "backend_failure_or_nonfinite_result_stops_publication".into(),
        finite_result_policy: "reject_non_finite_posterior_predictive_or_heldout_summaries".into(),
        claim_status: if result.fit_state == FitState::Complete {
            ClaimStatus::JointSpatialAssociationNotCommunicationOrCausality
        } else {
            ClaimStatus::DiagnosticOnlyNonconvergedJointLocationMark
        },
    })
}

fn read(path: &std::path::Path) -> Result<Vec<u8>, CliError> {
    crate::input_file::read_regular_file(
        path,
        MAXIMUM_INPUT_BYTES,
        &format!(
            "input must be a regular file within 16 MiB: {}",
            path.display()
        ),
    )
}

impl Prepared {
    fn validation_context(
        &self,
    ) -> marklab_bayes::protocol::joint_replicated_location_mark::ValidationContext<'_> {
        marklab_bayes::protocol::joint_replicated_location_mark::ValidationContext {
            backend: &self.backend,
            request_sha256: &self.request_sha256,
            location_sha256: &self.location_sha256,
            mark_sha256: &self.mark_sha256,
            sampling: &self.sampling,
            resources: &self.resources,
        }
    }

    pub(crate) fn validate_result(&self, result: &Output) -> Result<(), CliError> {
        result
            .validate_for(&self.validation_context())
            .map_err(super::protocol_error)
    }
}
