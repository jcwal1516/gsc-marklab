pub(crate) use marklab_bayes::protocol::joint_replicated_location_embedding::{
    validate_worker, EmbeddingResidual, EmbeddingResidualFamily, Output, Priors, Resources,
    Sampling, WorkerRequest, WorkerResult,
};

use clap::builder::TypedValueParser;
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
    /// Input embedding observations and their replicate bindings.
    #[arg(long)]
    pub(crate) embedding_input: PathBuf,
    /// Group level used as the regression reference.
    #[arg(long)]
    pub(crate) reference_group: String,
    /// Group level whose effect is compared with the reference group.
    #[arg(long)]
    pub(crate) comparison_group: String,
    /// Identifier of the embedding projection used by the model.
    #[arg(long)]
    pub(crate) embedding_projection_identity: String,
    /// Number of shared latent factors.
    #[arg(long)]
    pub(crate) factors: usize,
    /// Prior scale for location.
    #[arg(long)]
    pub(crate) location_prior_sd: f64,
    /// Prior scale for group.
    #[arg(long)]
    pub(crate) group_prior_sd: f64,
    /// Prior scale for patient factor.
    #[arg(long)]
    pub(crate) patient_factor_prior_scale: f64,
    /// Prior scale for location factor loading.
    #[arg(long)]
    pub(crate) location_factor_loading_prior_sd: f64,
    /// Prior scale for embedding loading.
    #[arg(long)]
    pub(crate) embedding_loading_prior_sd: f64,
    /// Prior scale for embedding noise.
    #[arg(long)]
    pub(crate) embedding_noise_prior_scale: f64,
    /// Residual distribution used for embedding features.
    #[arg(long, default_value = "gaussian", value_parser = clap::builder::PossibleValuesParser::new(["gaussian", "student-t"]).map(|value| match value.as_str() {
        "gaussian" => EmbeddingResidualFamily::Gaussian,
        "student-t" => EmbeddingResidualFamily::StudentT,
        _ => unreachable!("possible values validated by clap"),
    }))]
    pub(crate) embedding_residual_family: EmbeddingResidualFamily,
    /// Degrees of freedom of the Student-t residual distribution.
    #[arg(long)]
    pub(crate) student_t_degrees_of_freedom: Option<f64>,
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
    /// Upper bound on location rows.
    #[arg(long)]
    pub(crate) maximum_location_rows: usize,
    /// Upper bound on embedding points.
    #[arg(long)]
    pub(crate) maximum_embedding_points: usize,
    /// Upper bound on embedding dimension.
    #[arg(long)]
    pub(crate) maximum_embedding_dimension: usize,
    /// Upper bound on factor count.
    #[arg(long)]
    pub(crate) maximum_factor_count: usize,
    /// Upper bound on nearest node visits.
    #[arg(long)]
    pub(crate) maximum_nearest_node_visits: u64,
    /// Upper bound on kernel cube work.
    #[arg(long)]
    pub(crate) maximum_kernel_cube_work: u64,
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
    embedding_sha256: String,
    embedding_projection_identity: String,
    embedding_residual: EmbeddingResidual,
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
    let embedding_bytes = read(&args.embedding_input)?;
    let location_csv = String::from_utf8(location_bytes)
        .map_err(|_| CliError::Input("location CSV is not UTF-8".into()))?;
    let embedding_csv = String::from_utf8(embedding_bytes)
        .map_err(|_| CliError::Input("embedding CSV is not UTF-8".into()))?;
    if args.embedding_projection_identity.len() != 64
        || !args
            .embedding_projection_identity
            .bytes()
            .all(|value| value.is_ascii_hexdigit())
    {
        return Err(CliError::Input(
            "embedding projection identity must be a SHA-256 digest".into(),
        ));
    }
    let mut embedding_reader = csv::Reader::from_reader(embedding_csv.as_bytes());
    let headers = embedding_reader.headers()?.clone();
    let expected_prefix = [
        "pattern_id",
        "patient_id",
        "group",
        "point_id",
        "x_um",
        "y_um",
    ];
    if headers.len() < 8
        || headers.iter().take(6).collect::<Vec<_>>() != expected_prefix
        || headers
            .iter()
            .skip(6)
            .any(|name| !name.starts_with("embedding_"))
    {
        return Err(CliError::Input("embedding CSV headers differ".into()));
    }
    let embedding_dimension = headers.len() - 6;
    let embedding_point_count = embedding_reader.records().count();
    let mut node_counts = BTreeMap::<String, usize>::new();
    let mut unique_nodes = BTreeMap::<(String, String), ()>::new();
    let mut location_reader = csv::Reader::from_reader(location_csv.as_bytes());
    #[derive(Deserialize)]
    struct LocationIdentity {
        pattern_id: String,
        node_id: String,
    }
    for row in location_reader.deserialize::<LocationIdentity>() {
        let row = row?;
        unique_nodes.insert((row.pattern_id, row.node_id), ());
    }
    for (pattern, _) in unique_nodes.keys() {
        *node_counts.entry(pattern.clone()).or_default() += 1;
    }
    let location_node_count = unique_nodes.len();
    let location_rows = location_csv.lines().count().saturating_sub(1);
    let mut point_counts = BTreeMap::<String, u64>::new();
    let mut reader = csv::Reader::from_reader(embedding_csv.as_bytes());
    #[derive(Deserialize)]
    struct EmbeddingIdentity {
        pattern_id: String,
    }
    for row in reader.deserialize::<EmbeddingIdentity>() {
        let row = row?;
        *point_counts.entry(row.pattern_id).or_default() += 1;
    }
    let nearest_node_visits = point_counts
        .iter()
        .try_fold(0_u64, |total, (pattern, points)| {
            (*points)
                .checked_mul(*node_counts.get(pattern).unwrap_or(&0) as u64)
                .and_then(|work| total.checked_add(work))
        })
        .ok_or_else(|| CliError::Input("nearest-node work overflows".into()))?;
    let kernel_cube_work = node_counts
        .values()
        .try_fold(0_u64, |total, count| {
            (*count as u64)
                .checked_pow(3)
                .and_then(|work| total.checked_add(work))
        })
        .ok_or_else(|| CliError::Input("embedding kernel work overflows".into()))?;
    let draw_observation_work = u64::from(args.chains)
        .checked_mul(u64::from(args.draws))
        .and_then(|value| {
            value.checked_mul(
                location_node_count as u64
                    + (embedding_point_count as u64).checked_mul(embedding_dimension as u64)?,
            )
        })
        .and_then(|value| value.checked_mul(2))
        .ok_or_else(|| CliError::Input("embedding draw work overflows".into()))?;
    let estimated_working_bytes = (location_rows as u64)
        .checked_mul(256)
        .and_then(|value| {
            value.checked_add(
                (embedding_point_count as u64)
                    .checked_mul(embedding_dimension as u64)?
                    .checked_mul(32)?,
            )
        })
        .and_then(|value| value.checked_add(nearest_node_visits.checked_mul(16)?))
        .and_then(|value| value.checked_add(384 * 1024 * 1024))
        .ok_or_else(|| CliError::Input("embedding memory estimate overflows".into()))?;
    let priors = Priors {
        location_sd: args.location_prior_sd,
        group_sd: args.group_prior_sd,
        patient_factor_scale: args.patient_factor_prior_scale,
        location_factor_loading_sd: args.location_factor_loading_prior_sd,
        embedding_loading_sd: args.embedding_loading_prior_sd,
        embedding_noise_scale: args.embedding_noise_prior_scale,
        field_length_scale_um: args.field_length_scale_prior_scale_um,
        jitter: args.jitter,
    };
    let embedding_residual = EmbeddingResidual {
        family: args.embedding_residual_family,
        student_t_degrees_of_freedom: args.student_t_degrees_of_freedom,
    };
    let valid_residual = match embedding_residual.family {
        EmbeddingResidualFamily::Gaussian => {
            embedding_residual.student_t_degrees_of_freedom.is_none()
        }
        EmbeddingResidualFamily::StudentT => embedding_residual
            .student_t_degrees_of_freedom
            .is_some_and(|value| value.is_finite() && value > 2.0 && value <= 100.0),
    };
    if priors
        .values()
        .into_iter()
        .any(|value| !value.is_finite() || value <= 0.0)
        || !valid_residual
        || !(2..=args.maximum_embedding_dimension).contains(&embedding_dimension)
        || !(1..=args.maximum_factor_count).contains(&args.factors)
        || args.factors >= embedding_dimension
        || location_rows > args.maximum_location_rows
        || embedding_point_count > args.maximum_embedding_points
        || nearest_node_visits > args.maximum_nearest_node_visits
        || kernel_cube_work > args.maximum_kernel_cube_work
        || draw_observation_work > args.maximum_draw_observation_work
        || estimated_working_bytes > args.maximum_working_bytes
        || !(10..=14).contains(&args.maximum_tree_depth)
    {
        return Err(CliError::Input(
            "joint location-embedding controls or resources differ".into(),
        ));
    }
    let sampling = Sampling {
        chains: args.chains,
        tune: args.tune,
        draws: args.draws,
        target_accept: args.target_accept,
        seed: args.seed,
    };
    let resources = Resources {
        maximum_patients: args.maximum_patients,
        maximum_patterns: args.maximum_patterns,
        maximum_location_rows: args.maximum_location_rows,
        maximum_embedding_points: args.maximum_embedding_points,
        maximum_embedding_dimension: args.maximum_embedding_dimension,
        maximum_factor_count: args.maximum_factor_count,
        location_node_count,
        embedding_point_count,
        embedding_dimension,
        factor_count: args.factors,
        nearest_node_visits,
        maximum_nearest_node_visits: args.maximum_nearest_node_visits,
        kernel_cube_work,
        maximum_kernel_cube_work: args.maximum_kernel_cube_work,
        draw_observation_work,
        maximum_draw_observation_work: args.maximum_draw_observation_work,
        estimated_working_bytes,
        maximum_working_bytes: args.maximum_working_bytes,
        maximum_tree_depth: args.maximum_tree_depth,
        timeout_seconds: args.timeout_seconds,
        maximum_output_bytes: 4 * 1024 * 1024,
    };
    let workers = marklab::backend::python_backend_assets_root()?.join("workers/python");
    let backend = BackendContract {
        name: "numpyro",
        version: NUMPYRO_VERSION,
        python_version: PYTHON_VERSION,
        environment_lock_sha256: sha256_hex(&read(&workers.join("uv.lock"))?),
        worker_sha256: sha256_hex(&read(
            &workers.join("marklab_numpyro_joint_replicated_location_embedding_worker.py"),
        )?),
    };
    let location_sha256 = sha256_hex(location_csv.as_bytes());
    let embedding_sha256 = sha256_hex(embedding_csv.as_bytes());
    let request = WorkerRequest {
        format: "marklab.numpyro_joint_replicated_location_embedding_request",
        version: 2,
        backend: backend.clone(),
        jax_version: JAX_VERSION,
        location_csv: &location_csv,
        embedding_csv: &embedding_csv,
        location_sha256: &location_sha256,
        embedding_sha256: &embedding_sha256,
        reference_group: &args.reference_group,
        comparison_group: &args.comparison_group,
        embedding_projection_identity: &args.embedding_projection_identity,
        embedding_residual: embedding_residual.clone(),
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
        embedding_sha256,
        embedding_projection_identity: args.embedding_projection_identity,
        embedding_residual,
        sampling,
        resources,
        timeout_seconds: args.timeout_seconds,
    })
}

pub(crate) fn execute(prepared: &Prepared) -> Result<Output, CliError> {
    let bytes = run_worker(
        &marklab::backend::python_backend_assets_root()?,
        "marklab_numpyro_joint_replicated_location_embedding_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: WorkerResult = serde_json::from_slice(&bytes)?;
    validate_worker(&result, &prepared.validation_context()).map_err(super::protocol_error)?;
    Ok(Output {
        format: "marklab.joint_replicated_location_embedding_fit".into(),
        version: 2,
        backend: result.backend,
        jax_version: result.jax_version,
        request_sha256: result.request_sha256,
        location_sha256: result.location_sha256,
        embedding_sha256: result.embedding_sha256,
        embedding_projection_identity: prepared.embedding_projection_identity.clone(),
        patient_count: result.patient_count,
        pattern_count: result.pattern_count,
        training_pattern_count: result.training_pattern_count,
        heldout_pattern_count: result.heldout_pattern_count,
        embedding_dimension: result.embedding_dimension,
        factor_count: result.factor_count,
        training_pattern_ids: result.training_pattern_ids,
        heldout_pattern_ids: result.heldout_pattern_ids,
        joint_posterior: result.joint_posterior,
        heldout_embedding_comparison: result.heldout_embedding_comparison,
        posterior_predictive: result.posterior_predictive,
        diagnostics: result.diagnostics,
        fit_state: result.fit_state,
        sampling: prepared.sampling.clone(),
        resources: prepared.resources.clone(),
        statistical_unit: StatisticalUnit::Patient,
        location_likelihood: "exact_window_quadrature_poisson_total_location_intensity".into(),
        embedding_likelihood: prepared.embedding_residual.family.likelihood_name(
            prepared
                .embedding_residual
                .student_t_degrees_of_freedom,
        ),
        factor_identifiability:
            "unit_matern_fields_lower_triangular_first_factor_rows_positive_diagonal_loadings"
                .into(),
        embedding_mapping: "nearest_exact_quadrature_node_within_pattern".into(),
        holdout_policy:
            "first_pattern_embeddings_train_second_pattern_embeddings_evaluate_locations_observed_for_both"
                .into(),
        null_model: NullHypothesis::ZeroSpatialFactorEmbeddingIncrementOverNonspatialPatientModel,
        patient_score_spread_interpretation:
            "descriptive_between_patient_score_spread_not_uncertainty_for_the_mean_advantage"
                .into(),
        posterior_predictive_interpretation:
            "paired_draw_parameter_dependent_discrepancy_not_a_classically_calibrated_p_value"
                .into(),
        assumptions: vec![
            "each_patient_has_exactly_two_provenance_matched_patterns".into(),
            "embedding_projection_is_frozen_outside_the_evaluation_pattern".into(),
            "location_and_embedding_tables_share_patient_pattern_and_group_identity".into(),
            "nearest_quadrature_node_is_a_declared_piecewise_constant_field_approximation".into(),
            "patients_not_patterns_nodes_cells_or_embedding_dimensions_are_population_replicates"
                .into(),
        ],
        failure_policy: "backend_failure_or_nonfinite_result_stops_publication".into(),
        finite_result_policy: "reject_non_finite_posterior_predictive_or_heldout_summaries".into(),
        claim_status: if result.fit_state == FitState::Complete {
            ClaimStatus::JointSpatialEmbeddingAssociationNotCommunicationOrCausality
        } else {
            ClaimStatus::DiagnosticOnlyNonconvergedJointLocationEmbedding
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
    ) -> marklab_bayes::protocol::joint_replicated_location_embedding::ValidationContext<'_> {
        marklab_bayes::protocol::joint_replicated_location_embedding::ValidationContext {
            backend: &self.backend,
            request_sha256: &self.request_sha256,
            location_sha256: &self.location_sha256,
            embedding_sha256: &self.embedding_sha256,
            embedding_projection_identity: &self.embedding_projection_identity,
            embedding_residual: &self.embedding_residual,
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
