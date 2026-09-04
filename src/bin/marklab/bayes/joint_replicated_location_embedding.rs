use std::{collections::BTreeMap, fs, path::PathBuf};

use clap::{Parser, Subcommand};
use marklab_bayes::{sha256_hex, BackendContract, FitState, WorkerBackend};
use serde::{Deserialize, Serialize};

use super::{
    publish_json, replicated_arbitrary_window_lgcp_agreement::backend_matches, run_worker,
    BayesCliError,
};

const NUMPYRO_VERSION: &str = "0.21.0";
const JAX_VERSION: &str = "0.11.1";
const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct Cli {
    #[command(subcommand)]
    command: Top,
}

#[derive(Debug, Subcommand)]
enum Top {
    Bayes {
        #[command(subcommand)]
        command: Command,
    },
}

#[derive(Debug, Subcommand)]
enum Command {
    FitJointReplicatedLocationEmbedding(Box<Args>),
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EmbeddingResidualFamily {
    #[default]
    Gaussian,
    StudentT,
}

impl EmbeddingResidualFamily {
    fn likelihood_name(self, degrees_of_freedom: Option<f64>) -> String {
        match self {
            Self::Gaussian => "diagonal_gaussian_on_fold_frozen_projected_embeddings".into(),
            Self::StudentT => format!(
                "independent_student_t_on_fold_frozen_projected_embeddings_df_{}",
                degrees_of_freedom.expect("validated Student-t degrees of freedom")
            ),
        }
    }
}

#[derive(Clone, Debug, clap::Args)]
pub(crate) struct Args {
    #[arg(long)]
    pub(crate) location_input: PathBuf,
    #[arg(long)]
    pub(crate) embedding_input: PathBuf,
    #[arg(long)]
    pub(crate) reference_group: String,
    #[arg(long)]
    pub(crate) comparison_group: String,
    #[arg(long)]
    pub(crate) embedding_projection_identity: String,
    #[arg(long)]
    pub(crate) factors: usize,
    #[arg(long)]
    pub(crate) location_prior_sd: f64,
    #[arg(long)]
    pub(crate) group_prior_sd: f64,
    #[arg(long)]
    pub(crate) patient_factor_prior_scale: f64,
    #[arg(long)]
    pub(crate) location_factor_loading_prior_sd: f64,
    #[arg(long)]
    pub(crate) embedding_loading_prior_sd: f64,
    #[arg(long)]
    pub(crate) embedding_noise_prior_scale: f64,
    #[arg(long, value_enum, default_value_t)]
    pub(crate) embedding_residual_family: EmbeddingResidualFamily,
    #[arg(long)]
    pub(crate) student_t_degrees_of_freedom: Option<f64>,
    #[arg(long)]
    pub(crate) field_length_scale_prior_scale_um: f64,
    #[arg(long)]
    pub(crate) jitter: f64,
    #[arg(long)]
    pub(crate) chains: u32,
    #[arg(long)]
    pub(crate) tune: u32,
    #[arg(long)]
    pub(crate) draws: u32,
    #[arg(long)]
    pub(crate) target_accept: f64,
    #[arg(long)]
    pub(crate) seed: u64,
    #[arg(long)]
    pub(crate) maximum_patients: usize,
    #[arg(long)]
    pub(crate) maximum_patterns: usize,
    #[arg(long)]
    pub(crate) maximum_location_rows: usize,
    #[arg(long)]
    pub(crate) maximum_embedding_points: usize,
    #[arg(long)]
    pub(crate) maximum_embedding_dimension: usize,
    #[arg(long)]
    pub(crate) maximum_factor_count: usize,
    #[arg(long)]
    pub(crate) maximum_nearest_node_visits: u64,
    #[arg(long)]
    pub(crate) maximum_kernel_cube_work: u64,
    #[arg(long)]
    pub(crate) maximum_draw_observation_work: u64,
    #[arg(long)]
    pub(crate) maximum_working_bytes: u64,
    #[arg(long)]
    pub(crate) maximum_tree_depth: u32,
    #[arg(long)]
    pub(crate) timeout_seconds: u64,
    #[arg(long)]
    pub(crate) out: PathBuf,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Priors {
    location_sd: f64,
    group_sd: f64,
    patient_factor_scale: f64,
    location_factor_loading_sd: f64,
    embedding_loading_sd: f64,
    embedding_noise_scale: f64,
    field_length_scale_um: f64,
    jitter: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Sampling {
    chains: u32,
    tune: u32,
    draws: u32,
    target_accept: f64,
    seed: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Resources {
    maximum_patients: usize,
    maximum_patterns: usize,
    maximum_location_rows: usize,
    maximum_embedding_points: usize,
    maximum_embedding_dimension: usize,
    maximum_factor_count: usize,
    location_node_count: usize,
    embedding_point_count: usize,
    embedding_dimension: usize,
    factor_count: usize,
    nearest_node_visits: u64,
    maximum_nearest_node_visits: u64,
    kernel_cube_work: u64,
    maximum_kernel_cube_work: u64,
    draw_observation_work: u64,
    maximum_draw_observation_work: u64,
    estimated_working_bytes: u64,
    maximum_working_bytes: u64,
    maximum_tree_depth: u32,
    timeout_seconds: u64,
    maximum_output_bytes: usize,
}

#[derive(Serialize)]
struct WorkerRequest<'a> {
    format: &'static str,
    version: u32,
    backend: BackendContract,
    jax_version: &'static str,
    location_csv: &'a str,
    embedding_csv: &'a str,
    location_sha256: &'a str,
    embedding_sha256: &'a str,
    reference_group: &'a str,
    comparison_group: &'a str,
    embedding_projection_identity: &'a str,
    embedding_residual: EmbeddingResidual,
    priors: Priors,
    sampling: Sampling,
    resources: Resources,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct EmbeddingResidual {
    family: EmbeddingResidualFamily,
    student_t_degrees_of_freedom: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Summary {
    mean: f64,
    sd: f64,
    interval_lower: f64,
    interval_upper: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PredictiveScore {
    mean_log_predictive_density: f64,
    uncertainty: Summary,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct HeldoutEmbeddingComparison {
    joint_spatial: PredictiveScore,
    nonspatial: PredictiveScore,
    joint_minus_nonspatial: Summary,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PpcMetric {
    observed: f64,
    replicated_mean: f64,
    two_sided_tail_probability: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PosteriorPredictive {
    total_location_count: PpcMetric,
    heldout_embedding_rmse: PpcMetric,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct JointPosterior {
    field_length_scale_um: Summary,
    patient_factor_scales: Vec<Summary>,
    location_factor_loadings: Vec<Summary>,
    embedding_loading_norms: Vec<Summary>,
    embedding_noise_scales: Vec<Summary>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Diagnostics {
    maximum_r_hat: f64,
    minimum_bulk_ess: f64,
    minimum_tail_ess: f64,
    minimum_ebfmi: f64,
    divergences: u64,
    max_tree_depth_hits: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerResult {
    format: String,
    version: u32,
    backend: WorkerBackend,
    jax_version: String,
    request_sha256: String,
    location_sha256: String,
    embedding_sha256: String,
    patient_count: usize,
    pattern_count: usize,
    training_pattern_count: usize,
    heldout_pattern_count: usize,
    embedding_dimension: usize,
    factor_count: usize,
    training_pattern_ids: Vec<String>,
    heldout_pattern_ids: Vec<String>,
    joint_posterior: JointPosterior,
    heldout_embedding_comparison: HeldoutEmbeddingComparison,
    posterior_predictive: PosteriorPredictive,
    diagnostics: Diagnostics,
    fit_state: FitState,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Output {
    format: String,
    version: u32,
    backend: WorkerBackend,
    jax_version: String,
    request_sha256: String,
    location_sha256: String,
    embedding_sha256: String,
    embedding_projection_identity: String,
    patient_count: usize,
    pattern_count: usize,
    training_pattern_count: usize,
    heldout_pattern_count: usize,
    embedding_dimension: usize,
    factor_count: usize,
    training_pattern_ids: Vec<String>,
    heldout_pattern_ids: Vec<String>,
    joint_posterior: JointPosterior,
    heldout_embedding_comparison: HeldoutEmbeddingComparison,
    posterior_predictive: PosteriorPredictive,
    diagnostics: Diagnostics,
    fit_state: FitState,
    sampling: Sampling,
    resources: Resources,
    statistical_unit: String,
    location_likelihood: String,
    embedding_likelihood: String,
    factor_identifiability: String,
    embedding_mapping: String,
    holdout_policy: String,
    null_model: String,
    assumptions: Vec<String>,
    failure_policy: String,
    finite_result_policy: String,
    claim_status: String,
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

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    let Top::Bayes { command } = Cli::parse_from(std::env::args_os()).command;
    let Command::FitJointReplicatedLocationEmbedding(args) = command;
    let out = args.out.clone();
    let prepared = prepare(*args)?;
    publish_json(&out, &execute(&prepared)?)
}

pub(crate) fn prepare(args: Args) -> Result<Prepared, BayesCliError> {
    let location_bytes = read(&args.location_input)?;
    let embedding_bytes = read(&args.embedding_input)?;
    let location_csv = String::from_utf8(location_bytes)
        .map_err(|_| BayesCliError::Input("location CSV is not UTF-8".into()))?;
    let embedding_csv = String::from_utf8(embedding_bytes)
        .map_err(|_| BayesCliError::Input("embedding CSV is not UTF-8".into()))?;
    if args.embedding_projection_identity.len() != 64
        || !args
            .embedding_projection_identity
            .bytes()
            .all(|value| value.is_ascii_hexdigit())
    {
        return Err(BayesCliError::Input(
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
        return Err(BayesCliError::Input("embedding CSV headers differ".into()));
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
        .ok_or_else(|| BayesCliError::Input("nearest-node work overflows".into()))?;
    let kernel_cube_work = node_counts
        .values()
        .try_fold(0_u64, |total, count| {
            (*count as u64)
                .checked_pow(3)
                .and_then(|work| total.checked_add(work))
        })
        .ok_or_else(|| BayesCliError::Input("embedding kernel work overflows".into()))?;
    let draw_observation_work = u64::from(args.chains)
        .checked_mul(u64::from(args.draws))
        .and_then(|value| {
            value.checked_mul(
                location_node_count as u64
                    + (embedding_point_count as u64).checked_mul(embedding_dimension as u64)?,
            )
        })
        .and_then(|value| value.checked_mul(2))
        .ok_or_else(|| BayesCliError::Input("embedding draw work overflows".into()))?;
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
        .ok_or_else(|| BayesCliError::Input("embedding memory estimate overflows".into()))?;
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
        return Err(BayesCliError::Input(
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
    let workers = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("workers/python");
    let backend = BackendContract {
        name: "numpyro",
        version: NUMPYRO_VERSION,
        python_version: "3.12",
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

pub(crate) fn execute(prepared: &Prepared) -> Result<Output, BayesCliError> {
    let bytes = run_worker(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
        "marklab_numpyro_joint_replicated_location_embedding_worker.py",
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: WorkerResult = serde_json::from_slice(&bytes)?;
    validate_worker(&result, prepared)?;
    Ok(Output {
        format: "marklab.joint_replicated_location_embedding_fit".into(),
        version: 1,
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
        statistical_unit: "patient".into(),
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
        null_model: "zero_spatial_factor_embedding_increment_over_nonspatial_patient_model".into(),
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
            "joint_spatial_embedding_association_not_communication_or_causality"
        } else {
            "diagnostic_only_nonconverged_joint_location_embedding"
        }
        .into(),
    })
}

impl Output {
    pub(crate) fn validate_for(&self, prepared: &Prepared) -> Result<(), BayesCliError> {
        if self.format != "marklab.joint_replicated_location_embedding_fit"
            || self.version != 1
            || self.request_sha256 != prepared.request_sha256
            || self.location_sha256 != prepared.location_sha256
            || self.embedding_sha256 != prepared.embedding_sha256
            || self.embedding_projection_identity != prepared.embedding_projection_identity
            || self.embedding_likelihood
                != prepared
                    .embedding_residual
                    .family
                    .likelihood_name(prepared.embedding_residual.student_t_degrees_of_freedom)
            || self.sampling != prepared.sampling
            || self.resources != prepared.resources
            || self.statistical_unit != "patient"
        {
            return Err(BayesCliError::Backend(
                "durable joint location-embedding result differs".into(),
            ));
        }
        Ok(())
    }
}

fn validate_worker(result: &WorkerResult, prepared: &Prepared) -> Result<(), BayesCliError> {
    let summaries = [
        &result.joint_posterior.field_length_scale_um,
        &result
            .heldout_embedding_comparison
            .joint_spatial
            .uncertainty,
        &result.heldout_embedding_comparison.nonspatial.uncertainty,
        &result.heldout_embedding_comparison.joint_minus_nonspatial,
    ];
    let finite = summaries.iter().all(|row| valid_summary(row))
        && result
            .joint_posterior
            .patient_factor_scales
            .iter()
            .chain(&result.joint_posterior.location_factor_loadings)
            .chain(&result.joint_posterior.embedding_loading_norms)
            .chain(&result.joint_posterior.embedding_noise_scales)
            .all(valid_summary)
        && [
            &result.posterior_predictive.total_location_count,
            &result.posterior_predictive.heldout_embedding_rmse,
        ]
        .iter()
        .all(|row| {
            [
                row.observed,
                row.replicated_mean,
                row.two_sided_tail_probability,
            ]
            .into_iter()
            .all(f64::is_finite)
                && (0.0..=1.0).contains(&row.two_sided_tail_probability)
        });
    let diagnostics_pass = result.diagnostics.maximum_r_hat <= 1.01
        && result.diagnostics.minimum_bulk_ess >= 100.0
        && result.diagnostics.minimum_tail_ess >= 100.0
        && result.diagnostics.minimum_ebfmi >= 0.3
        && result.diagnostics.divergences == 0
        && result.diagnostics.max_tree_depth_hits == 0;
    if result.format != "marklab.numpyro_joint_replicated_location_embedding_result"
        || result.version != 1
        || !backend_matches(&result.backend, &prepared.backend)
        || result.jax_version != JAX_VERSION
        || result.request_sha256 != prepared.request_sha256
        || result.location_sha256 != prepared.location_sha256
        || result.embedding_sha256 != prepared.embedding_sha256
        || result.patient_count < 2
        || result.pattern_count != result.patient_count * 2
        || result.training_pattern_count != result.patient_count
        || result.heldout_pattern_count != result.patient_count
        || result.embedding_dimension != prepared.resources.embedding_dimension
        || result.factor_count != prepared.resources.factor_count
        || result.joint_posterior.patient_factor_scales.len() != result.factor_count
        || result.joint_posterior.location_factor_loadings.len() != result.factor_count
        || result.joint_posterior.embedding_loading_norms.len() != result.factor_count
        || result.joint_posterior.embedding_noise_scales.len() != result.embedding_dimension
        || (result.fit_state == FitState::Complete) != diagnostics_pass
        || !finite
    {
        return Err(BayesCliError::Backend(
            "joint location-embedding backend result differs".into(),
        ));
    }
    Ok(())
}

fn valid_summary(row: &Summary) -> bool {
    [row.mean, row.sd, row.interval_lower, row.interval_upper]
        .into_iter()
        .all(f64::is_finite)
        && row.sd >= 0.0
        && row.interval_lower <= row.interval_upper
}

impl Priors {
    fn values(&self) -> [f64; 8] {
        [
            self.location_sd,
            self.group_sd,
            self.patient_factor_scale,
            self.location_factor_loading_sd,
            self.embedding_loading_sd,
            self.embedding_noise_scale,
            self.field_length_scale_um,
            self.jitter,
        ]
    }
}

fn read(path: &std::path::Path) -> Result<Vec<u8>, BayesCliError> {
    let metadata = fs::metadata(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(format!(
            "input must be a regular file within 16 MiB: {}",
            path.display()
        )));
    }
    fs::read(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })
}

pub(super) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<Cli>(|| run_cli().map_err(super::into_marklab_error))
}
