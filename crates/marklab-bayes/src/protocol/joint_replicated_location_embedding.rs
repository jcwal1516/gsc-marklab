//! Joint patient-replicated model of point locations and projected cell embeddings.
//!
//! Locations of both patterns are fitted; only the first pattern's embeddings are, and the
//! second pattern's embeddings are scored. Node counts follow `Poisson(w exp(a + b g + c
//! covariate + (s + p) . location_factor_loading))`, where `s` are unit-amplitude Matern-3/2
//! fields per pattern (`field_length_scale_um ~ HalfNormal`, plus `jitter`), centred within each
//! pattern, and `p = patient_factor_scale * z` with per-factor half-normal scales. An embedding
//! uses the factors of its nearest node in the same pattern: `e ~ Normal` or `StudentT(df)` with
//! location `m + L (s + p)` and per-dimension scale `noise ~ HalfNormal(embedding_noise_scale)`
//! (for Student-t the scale, not the SD); `m ~ Normal(0, 2)`. `L` is lower-triangular in its
//! first `factor_count` rows with a log-normal positive diagonal, fixing rotation and sign. The
//! baseline is non-spatial. The score is each held-out vector's log mean predictive density
//! over draws, summed per patient and averaged over patients; `fit_state` uses the worst case
//! over both fits (R-hat at most 1.01, ESS at least 100, E-BFMI at least 0.3).

use crate::backend_versions::JAX_VERSION;
use crate::BackendContract;
use crate::BayesError;
use crate::FitState;
use crate::WorkerBackend;
use marklab_core::backend::ClaimStatus;
use marklab_core::vocabulary::NullHypothesis;
use marklab_core::vocabulary::StatisticalUnit;
use serde::Deserialize;
use serde::Serialize;

pub struct ValidationContext<'a> {
    pub backend: &'a BackendContract,
    pub request_sha256: &'a str,
    pub location_sha256: &'a str,
    pub embedding_sha256: &'a str,
    pub embedding_projection_identity: &'a str,
    pub embedding_residual: &'a EmbeddingResidual,
    pub sampling: &'a Sampling,
    pub resources: &'a Resources,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingResidualFamily {
    #[default]
    Gaussian,
    StudentT,
}

impl EmbeddingResidualFamily {
    pub fn likelihood_name(self, degrees_of_freedom: Option<f64>) -> String {
        match self {
            Self::Gaussian => "diagonal_gaussian_on_fold_frozen_projected_embeddings".into(),
            Self::StudentT => format!(
                "independent_student_t_on_fold_frozen_projected_embeddings_df_{}",
                degrees_of_freedom.expect("validated Student-t degrees of freedom")
            ),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Priors {
    pub location_sd: f64,
    pub group_sd: f64,
    pub patient_factor_scale: f64,
    pub location_factor_loading_sd: f64,
    pub embedding_loading_sd: f64,
    pub embedding_noise_scale: f64,
    pub field_length_scale_um: f64,
    pub jitter: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Sampling {
    pub chains: u32,
    pub tune: u32,
    pub draws: u32,
    pub target_accept: f64,
    pub seed: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    pub maximum_patients: usize,
    pub maximum_patterns: usize,
    pub maximum_location_rows: usize,
    pub maximum_embedding_points: usize,
    pub maximum_embedding_dimension: usize,
    pub maximum_factor_count: usize,
    pub location_node_count: usize,
    pub embedding_point_count: usize,
    pub embedding_dimension: usize,
    pub factor_count: usize,
    pub nearest_node_visits: u64,
    pub maximum_nearest_node_visits: u64,
    pub kernel_cube_work: u64,
    pub maximum_kernel_cube_work: u64,
    pub draw_observation_work: u64,
    pub maximum_draw_observation_work: u64,
    pub estimated_working_bytes: u64,
    pub maximum_working_bytes: u64,
    pub maximum_tree_depth: u32,
    pub timeout_seconds: u64,
    pub maximum_output_bytes: usize,
}

#[derive(Serialize)]
pub struct WorkerRequest<'a> {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub jax_version: &'static str,
    pub location_csv: &'a str,
    pub embedding_csv: &'a str,
    pub location_sha256: &'a str,
    pub embedding_sha256: &'a str,
    pub reference_group: &'a str,
    pub comparison_group: &'a str,
    pub embedding_projection_identity: &'a str,
    pub embedding_residual: EmbeddingResidual,
    pub priors: Priors,
    pub sampling: Sampling,
    pub resources: Resources,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EmbeddingResidual {
    pub family: EmbeddingResidualFamily,
    pub student_t_degrees_of_freedom: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    pub mean: f64,
    pub sd: f64,
    pub interval_lower: f64,
    pub interval_upper: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PredictiveScore {
    pub mean_log_predictive_density: f64,
    pub patient_score_spread: Summary,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HeldoutEmbeddingComparison {
    pub score_definition: String,
    pub score_version: u32,
    pub score_unit: String,
    pub patient_aggregation: String,
    pub patient_count: usize,
    pub heldout_embedding_point_count: usize,
    pub heldout_embedding_point_counts_by_patient: Vec<usize>,
    pub joint_spatial: PredictiveScore,
    pub nonspatial: PredictiveScore,
    pub joint_minus_nonspatial_patient_score_spread: Summary,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PpcMetric {
    pub observed: f64,
    pub replicated_mean: f64,
    pub two_sided_tail_probability: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PosteriorPredictive {
    pub total_location_count: PpcMetric,
    pub heldout_embedding_rmse: PpcMetric,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct JointPosterior {
    pub field_length_scale_um: Summary,
    pub patient_factor_scales: Vec<Summary>,
    pub location_factor_loadings: Vec<Summary>,
    pub embedding_loading_norms: Vec<Summary>,
    pub embedding_noise_scales: Vec<Summary>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Diagnostics {
    pub maximum_r_hat: f64,
    pub minimum_bulk_ess: f64,
    pub minimum_tail_ess: f64,
    pub minimum_ebfmi: f64,
    pub divergences: u64,
    pub max_tree_depth_hits: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerResult {
    pub format: String,
    pub version: u32,
    pub backend: WorkerBackend,
    pub jax_version: String,
    pub request_sha256: String,
    pub location_sha256: String,
    pub embedding_sha256: String,
    pub patient_count: usize,
    pub pattern_count: usize,
    pub training_pattern_count: usize,
    pub heldout_pattern_count: usize,
    pub embedding_dimension: usize,
    pub factor_count: usize,
    pub training_pattern_ids: Vec<String>,
    pub heldout_pattern_ids: Vec<String>,
    pub joint_posterior: JointPosterior,
    pub heldout_embedding_comparison: HeldoutEmbeddingComparison,
    pub posterior_predictive: PosteriorPredictive,
    pub diagnostics: Diagnostics,
    pub fit_state: FitState,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
    pub format: String,
    pub version: u32,
    pub backend: WorkerBackend,
    pub jax_version: String,
    pub request_sha256: String,
    pub location_sha256: String,
    pub embedding_sha256: String,
    pub embedding_projection_identity: String,
    pub patient_count: usize,
    pub pattern_count: usize,
    pub training_pattern_count: usize,
    pub heldout_pattern_count: usize,
    pub embedding_dimension: usize,
    pub factor_count: usize,
    pub training_pattern_ids: Vec<String>,
    pub heldout_pattern_ids: Vec<String>,
    pub joint_posterior: JointPosterior,
    pub heldout_embedding_comparison: HeldoutEmbeddingComparison,
    pub posterior_predictive: PosteriorPredictive,
    pub diagnostics: Diagnostics,
    pub fit_state: FitState,
    pub sampling: Sampling,
    pub resources: Resources,
    pub statistical_unit: StatisticalUnit,
    pub location_likelihood: String,
    pub embedding_likelihood: String,
    pub factor_identifiability: String,
    pub embedding_mapping: String,
    pub holdout_policy: String,
    pub null_model: NullHypothesis,
    pub patient_score_spread_interpretation: String,
    pub posterior_predictive_interpretation: String,
    pub assumptions: Vec<String>,
    pub failure_policy: String,
    pub finite_result_policy: String,
    pub claim_status: ClaimStatus,
}

impl Output {
    pub fn validate_for(&self, expected: &ValidationContext<'_>) -> Result<(), BayesError> {
        let expected_claim_status = if self.fit_state == FitState::Complete {
            ClaimStatus::JointSpatialEmbeddingAssociationNotCommunicationOrCausality
        } else {
            ClaimStatus::DiagnosticOnlyNonconvergedJointLocationEmbedding
        };
        if self.format != "marklab.joint_replicated_location_embedding_fit"
            || self.version != 2
            || self.request_sha256 != expected.request_sha256
            || self.location_sha256 != expected.location_sha256
            || self.embedding_sha256 != expected.embedding_sha256
            || self.embedding_projection_identity != expected.embedding_projection_identity
            || self.embedding_likelihood
                != expected
                    .embedding_residual
                    .family
                    .likelihood_name(expected.embedding_residual.student_t_degrees_of_freedom)
            || self.sampling != *expected.sampling
            || self.resources != *expected.resources
            || self.statistical_unit != StatisticalUnit::Patient
            || self.heldout_embedding_comparison.score_version != 2
            || self.heldout_embedding_comparison.score_definition
                != "pointwise_joint_embedding_vector_log_predictive_density_summed_within_patient"
            || self.heldout_embedding_comparison.score_unit
                != "heldout_embedding_point_complete_vector"
            || self.heldout_embedding_comparison.patient_aggregation
                != "sum_point_scores_within_patient_then_equal_patient_mean"
            || self.patient_score_spread_interpretation
                != "descriptive_between_patient_score_spread_not_uncertainty_for_the_mean_advantage"
            || self.posterior_predictive_interpretation
                != "paired_draw_parameter_dependent_discrepancy_not_a_classically_calibrated_p_value"
            || self.claim_status != expected_claim_status
            || (self.fit_state == FitState::Complete) != diagnostics_pass(&self.diagnostics)
            || !valid_heldout_comparison(
                &self.heldout_embedding_comparison,
                self.patient_count,
                expected.resources.embedding_point_count,
            )
            || !valid_joint_posterior(
                &self.joint_posterior,
                self.factor_count,
                self.embedding_dimension,
            )
            || !valid_posterior_predictive(&self.posterior_predictive)
        {
            return Err(BayesError::WorkerContract(
                "durable joint location-embedding result differs".into(),
            ));
        }
        Ok(())
    }
}

pub fn validate_worker(
    result: &WorkerResult,
    expected: &ValidationContext<'_>,
) -> Result<(), BayesError> {
    if result.format != "marklab.numpyro_joint_replicated_location_embedding_result"
        || result.version != 2
        || !result.backend.matches(expected.backend)
        || result.jax_version != JAX_VERSION
        || result.request_sha256 != expected.request_sha256
        || result.location_sha256 != expected.location_sha256
        || result.embedding_sha256 != expected.embedding_sha256
        || result.patient_count < 2
        || result.pattern_count != result.patient_count * 2
        || result.training_pattern_count != result.patient_count
        || result.heldout_pattern_count != result.patient_count
        || result.embedding_dimension != expected.resources.embedding_dimension
        || result.factor_count != expected.resources.factor_count
        || !valid_heldout_comparison(
            &result.heldout_embedding_comparison,
            result.patient_count,
            expected.resources.embedding_point_count,
        )
        || !valid_joint_posterior(
            &result.joint_posterior,
            result.factor_count,
            result.embedding_dimension,
        )
        || (result.fit_state == FitState::Complete) != diagnostics_pass(&result.diagnostics)
        || !valid_posterior_predictive(&result.posterior_predictive)
    {
        return Err(BayesError::WorkerContract(
            "joint location-embedding backend result differs".into(),
        ));
    }
    Ok(())
}

fn diagnostics_pass(diagnostics: &Diagnostics) -> bool {
    diagnostics.maximum_r_hat.is_finite()
        && diagnostics.maximum_r_hat <= 1.01
        && diagnostics.minimum_bulk_ess.is_finite()
        && diagnostics.minimum_bulk_ess >= 100.0
        && diagnostics.minimum_tail_ess.is_finite()
        && diagnostics.minimum_tail_ess >= 100.0
        && diagnostics.minimum_ebfmi.is_finite()
        && diagnostics.minimum_ebfmi >= 0.3
        && diagnostics.divergences == 0
        && diagnostics.max_tree_depth_hits == 0
}

fn valid_heldout_comparison(
    comparison: &HeldoutEmbeddingComparison,
    patient_count: usize,
    maximum_embedding_points: usize,
) -> bool {
    comparison.score_version == 2
        && comparison.score_definition
            == "pointwise_joint_embedding_vector_log_predictive_density_summed_within_patient"
        && comparison.score_unit == "heldout_embedding_point_complete_vector"
        && comparison.patient_aggregation
            == "sum_point_scores_within_patient_then_equal_patient_mean"
        && comparison.patient_count == patient_count
        && comparison.heldout_embedding_point_counts_by_patient.len() == patient_count
        && !comparison
            .heldout_embedding_point_counts_by_patient
            .contains(&0)
        && comparison
            .heldout_embedding_point_counts_by_patient
            .iter()
            .sum::<usize>()
            == comparison.heldout_embedding_point_count
        && (1..=maximum_embedding_points).contains(&comparison.heldout_embedding_point_count)
        && comparison.joint_spatial.mean_log_predictive_density
            == comparison.joint_spatial.patient_score_spread.mean
        && comparison.nonspatial.mean_log_predictive_density
            == comparison.nonspatial.patient_score_spread.mean
        && valid_summary(&comparison.joint_spatial.patient_score_spread)
        && valid_summary(&comparison.nonspatial.patient_score_spread)
        && valid_summary(&comparison.joint_minus_nonspatial_patient_score_spread)
}

fn valid_joint_posterior(
    posterior: &JointPosterior,
    factor_count: usize,
    embedding_dimension: usize,
) -> bool {
    valid_summary(&posterior.field_length_scale_um)
        && posterior.patient_factor_scales.len() == factor_count
        && posterior.location_factor_loadings.len() == factor_count
        && posterior.embedding_loading_norms.len() == factor_count
        && posterior.embedding_noise_scales.len() == embedding_dimension
        && posterior
            .patient_factor_scales
            .iter()
            .chain(&posterior.location_factor_loadings)
            .chain(&posterior.embedding_loading_norms)
            .chain(&posterior.embedding_noise_scales)
            .all(valid_summary)
}

fn valid_posterior_predictive(posterior_predictive: &PosteriorPredictive) -> bool {
    [
        &posterior_predictive.total_location_count,
        &posterior_predictive.heldout_embedding_rmse,
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
    })
}

fn valid_summary(row: &Summary) -> bool {
    [row.mean, row.sd, row.interval_lower, row.interval_upper]
        .into_iter()
        .all(f64::is_finite)
        && row.sd >= 0.0
        && row.interval_lower <= row.interval_upper
}

impl Priors {
    pub fn values(&self) -> [f64; 8] {
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
