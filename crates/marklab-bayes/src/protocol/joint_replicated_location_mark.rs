//! Joint patient-replicated model of point locations and conditional categorical marks.
//!
//! Each patient has two patterns: the lexicographically first trains all three fits and the
//! second is held out. Location counts on quadrature nodes follow `Poisson(weight_um2 * exp(a +
//! b g + c covariate + ecology_p))` with `g = 1` for `comparison_group`. Marks are categorical
//! given the type proportions of neighbours within `neighbor_radius_um`; `reference_type` has
//! logit 0 and the others `mark_intercept + mark_group g + mark_loading ecology_p + features
//! interaction^T`. `ecology = sd z` with `sd ~ HalfNormal(patient_ecology_scale)`; its location
//! loading is fixed at 1, fixing sign and scale. Other priors are Normal with SDs `location_sd`,
//! `group_sd`, `mark_intercept_sd`, `mark_group_sd`, `mark_loading_sd` and `interaction_sd`.
//! Baselines are location-only and mark-only fits. Held-out scores are posterior means of the
//! held-out log likelihood. `fit_state` needs worst-case R-hat at most 1.01, ESS at least 100,
//! E-BFMI at least 0.3 and no divergences or depth hits across all three fits.

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
    pub mark_sha256: &'a str,
    pub sampling: &'a Sampling,
    pub resources: &'a Resources,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Priors {
    pub location_sd: f64,
    pub group_sd: f64,
    pub patient_ecology_scale: f64,
    pub mark_intercept_sd: f64,
    pub mark_group_sd: f64,
    pub mark_loading_sd: f64,
    pub interaction_sd: f64,
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
    pub maximum_types: usize,
    pub maximum_location_rows: usize,
    pub maximum_mark_points: usize,
    pub maximum_neighbor_visits: u64,
    pub neighbor_visits: u64,
    pub maximum_draw_observation_work: u64,
    pub draw_observation_work: u64,
    pub maximum_working_bytes: u64,
    pub estimated_working_bytes: u64,
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
    pub mark_csv: &'a str,
    pub location_sha256: &'a str,
    pub mark_sha256: &'a str,
    pub reference_group: &'a str,
    pub comparison_group: &'a str,
    pub reference_type: &'a str,
    pub neighbor_radius_um: f64,
    pub priors: Priors,
    pub sampling: Sampling,
    pub resources: Resources,
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
pub struct JointPosterior {
    pub patient_ecology_sd: Summary,
    pub mark_loadings: Vec<Summary>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PredictiveScore {
    pub mean_log_predictive_density: f64,
    pub uncertainty: Summary,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HeldoutComparison {
    pub joint: PredictiveScore,
    pub location_only: PredictiveScore,
    pub conditional_mark_only: PredictiveScore,
    pub joint_minus_separate_baselines: Summary,
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
    pub total_count: PpcMetric,
    pub mark_proportions: PpcMetric,
    pub cross_type_enrichment: PpcMetric,
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
    pub mark_sha256: String,
    pub patient_count: usize,
    pub pattern_count: usize,
    pub training_pattern_count: usize,
    pub heldout_pattern_count: usize,
    pub type_ids: Vec<String>,
    pub training_pattern_ids: Vec<String>,
    pub heldout_pattern_ids: Vec<String>,
    pub joint_posterior: JointPosterior,
    pub heldout_comparison: HeldoutComparison,
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
    pub mark_sha256: String,
    pub patient_count: usize,
    pub pattern_count: usize,
    pub training_pattern_count: usize,
    pub heldout_pattern_count: usize,
    pub type_ids: Vec<String>,
    pub training_pattern_ids: Vec<String>,
    pub heldout_pattern_ids: Vec<String>,
    pub joint_posterior: JointPosterior,
    pub heldout_comparison: HeldoutComparison,
    pub posterior_predictive: PosteriorPredictive,
    pub diagnostics: Diagnostics,
    pub fit_state: FitState,
    pub sampling: Sampling,
    pub resources: Resources,
    pub statistical_unit: StatisticalUnit,
    pub location_likelihood: String,
    pub mark_likelihood: String,
    pub holdout_policy: String,
    pub null_model: NullHypothesis,
    pub assumptions: Vec<String>,
    pub failure_policy: String,
    pub finite_result_policy: String,
    pub claim_status: ClaimStatus,
}

impl Output {
    pub fn validate_for(&self, expected: &ValidationContext<'_>) -> Result<(), BayesError> {
        if self.format != "marklab.joint_replicated_location_mark_fit"
            || self.version != 1
            || self.request_sha256 != expected.request_sha256
            || self.location_sha256 != expected.location_sha256
            || self.mark_sha256 != expected.mark_sha256
            || self.sampling != *expected.sampling
            || self.resources != *expected.resources
            || self.statistical_unit != StatisticalUnit::Patient
            || self.claim_status
                != if self.fit_state == FitState::Complete {
                    ClaimStatus::JointSpatialAssociationNotCommunicationOrCausality
                } else {
                    ClaimStatus::DiagnosticOnlyNonconvergedJointLocationMark
                }
        {
            return Err(BayesError::WorkerContract(
                "durable joint location-mark result differs".into(),
            ));
        }
        Ok(())
    }
}

pub fn validate_worker(
    result: &WorkerResult,
    expected: &ValidationContext<'_>,
) -> Result<(), BayesError> {
    let summaries = [
        &result.joint_posterior.patient_ecology_sd,
        &result.heldout_comparison.joint.uncertainty,
        &result.heldout_comparison.location_only.uncertainty,
        &result.heldout_comparison.conditional_mark_only.uncertainty,
        &result.heldout_comparison.joint_minus_separate_baselines,
    ];
    let finite = summaries.iter().all(|row| {
        [row.mean, row.sd, row.interval_lower, row.interval_upper]
            .into_iter()
            .all(f64::is_finite)
            && row.sd >= 0.0
            && row.interval_lower <= row.interval_upper
    }) && [
        &result.posterior_predictive.total_count,
        &result.posterior_predictive.mark_proportions,
        &result.posterior_predictive.cross_type_enrichment,
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
    if result.format != "marklab.numpyro_joint_replicated_location_mark_result"
        || result.version != 1
        || !result.backend.matches(expected.backend)
        || result.jax_version != JAX_VERSION
        || result.request_sha256 != expected.request_sha256
        || result.location_sha256 != expected.location_sha256
        || result.mark_sha256 != expected.mark_sha256
        || result.patient_count < 2
        || result.pattern_count != result.patient_count * 2
        || result.training_pattern_count != result.patient_count
        || result.heldout_pattern_count != result.patient_count
        || result.type_ids.len() < 2
        || (result.fit_state == FitState::Complete)
            != (result.diagnostics.maximum_r_hat <= 1.01
                && result.diagnostics.minimum_bulk_ess >= 100.0
                && result.diagnostics.minimum_tail_ess >= 100.0
                && result.diagnostics.minimum_ebfmi >= 0.3
                && result.diagnostics.divergences == 0
                && result.diagnostics.max_tree_depth_hits == 0)
        || !finite
    {
        return Err(BayesError::WorkerContract(
            "joint location-mark backend result differs".into(),
        ));
    }
    Ok(())
}

impl Priors {
    pub fn into_values(self) -> [f64; 7] {
        [
            self.location_sd,
            self.group_sd,
            self.patient_ecology_scale,
            self.mark_intercept_sd,
            self.mark_group_sd,
            self.mark_loading_sd,
            self.interaction_sd,
        ]
    }
}
