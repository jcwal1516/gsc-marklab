use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    model::{
        MODEL_FORMAT, MODEL_VERSION, PYMC_VERSION, WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION,
    },
    validation::{
        diagnostics_satisfy_policy, is_lower_hex_sha256, validate_scalar_summary, SummarySupport,
    },
    BackendContract, BayesError, DiagnosticPolicy, FitState, NormalMeanDiagnostics,
    NutsSamplingSpec, SamplingSummary, SarScalarSummary, WorkerBackend,
};

const MAXIMUM_OBSERVATIONS: usize = 4_096;
const MAXIMUM_TOTAL_ITERATIONS: u64 = 400_000;

/// One scalar measurement in an identified nested/crossed Gaussian hierarchy.
#[derive(Clone, Debug, Serialize)]
pub struct GaussianCrossedNestedObservation {
    pub patient_id: String,
    pub slide_id: String,
    pub roi_id: String,
    pub batch_id: String,
    pub cohort_id: String,
    pub exposure: f64,
    pub outcome: f64,
}

/// Priors and observations for the fixed crossed/nested hierarchy.
#[derive(Clone, Debug)]
pub struct GaussianCrossedNestedHierarchySpec {
    pub intercept_prior_sd: f64,
    pub slope_prior_sd: f64,
    pub component_prior_sd: f64,
    pub observations: Vec<GaussianCrossedNestedObservation>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GaussianCrossedNestedCounts {
    pub patients: usize,
    pub slides: usize,
    pub rois: usize,
    pub batches: usize,
    pub cohorts: usize,
    pub observations: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct GaussianCrossedNestedHierarchyModelIr {
    pub format: &'static str,
    pub version: u32,
    pub family: &'static str,
    pub likelihood: &'static str,
    pub intercept_prior: &'static str,
    pub intercept_prior_sd: f64,
    pub exposure_slope_prior: &'static str,
    pub exposure_slope_prior_sd: f64,
    pub variance_component_prior: &'static str,
    pub variance_component_prior_sd: f64,
    pub nesting: &'static str,
    pub crossed_effect: &'static str,
    pub random_slopes: [&'static str; 2],
    pub random_effect_parameterization: &'static str,
    pub exposure_standardization: &'static str,
    pub observation_unit: &'static str,
    pub statistical_unit: &'static str,
    pub null: &'static str,
    pub backend_capability: &'static str,
    pub maturity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct GaussianCrossedNestedHierarchyResources {
    pub maximum_patients: u32,
    pub maximum_slides: u32,
    pub maximum_rois: u32,
    pub maximum_batches: u32,
    pub maximum_cohorts: u32,
    pub maximum_observations: u32,
    pub maximum_total_iterations: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct GaussianCrossedNestedHierarchyWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: GaussianCrossedNestedHierarchyModelIr,
    pub observations: Vec<GaussianCrossedNestedObservation>,
    pub sampling: NutsSamplingSpec,
    pub resources: GaussianCrossedNestedHierarchyResources,
    pub diagnostic_policy: DiagnosticPolicy,
}

impl GaussianCrossedNestedHierarchyWorkerRequest {
    pub fn new(
        mut spec: GaussianCrossedNestedHierarchySpec,
        sampling: NutsSamplingSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        if [
            spec.intercept_prior_sd,
            spec.slope_prior_sd,
            spec.component_prior_sd,
        ]
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
            || !is_lower_hex_sha256(&environment_lock_sha256)
            || !is_lower_hex_sha256(&worker_sha256)
            || !(1..=3_600).contains(&timeout_seconds)
        {
            return Err(BayesError::InvalidSpec(
                "crossed/nested hierarchy priors, identities, or timeout are invalid".into(),
            ));
        }
        sampling.validate()?;
        let total_iterations = u64::from(sampling.chains)
            .checked_mul(u64::from(
                sampling.tune_per_chain + sampling.draws_per_chain,
            ))
            .ok_or_else(|| BayesError::InvalidSpec("sampling work overflow".into()))?;
        if total_iterations > MAXIMUM_TOTAL_ITERATIONS {
            return Err(BayesError::InvalidSpec(
                "crossed/nested hierarchy exceeds 400000 NUTS iterations".into(),
            ));
        }
        spec.observations.sort_by(|left, right| {
            left.patient_id
                .cmp(&right.patient_id)
                .then_with(|| left.slide_id.cmp(&right.slide_id))
                .then_with(|| left.roi_id.cmp(&right.roi_id))
                .then_with(|| left.batch_id.cmp(&right.batch_id))
                .then_with(|| left.exposure.total_cmp(&right.exposure))
                .then_with(|| left.outcome.total_cmp(&right.outcome))
        });
        validate_design(&spec.observations)?;
        let diagnostic_policy = DiagnosticPolicy {
            maximum_tree_depth: 12,
            ..DiagnosticPolicy::default()
        };
        Ok(Self {
            format: WORKER_REQUEST_FORMAT,
            version: WORKER_REQUEST_VERSION,
            backend: BackendContract {
                name: "pymc",
                version: PYMC_VERSION,
                python_version: "3.12",
                environment_lock_sha256,
                worker_sha256,
            },
            model: GaussianCrossedNestedHierarchyModelIr {
                format: MODEL_FORMAT,
                version: MODEL_VERSION,
                family: "gaussian_crossed_nested_random_slope_hierarchy",
                likelihood: "normal_unknown_residual_scale",
                intercept_prior: "normal_zero",
                intercept_prior_sd: spec.intercept_prior_sd,
                exposure_slope_prior: "normal_zero",
                exposure_slope_prior_sd: spec.slope_prior_sd,
                variance_component_prior: "half_normal_shared_scale",
                variance_component_prior_sd: spec.component_prior_sd,
                nesting: "roi_within_slide_within_patient",
                crossed_effect: "batch_crossed_with_patient",
                random_slopes: ["patient_exposure", "cohort_exposure"],
                random_effect_parameterization:
                    "hybrid_centered_replicated_levels_noncentered_cohort_sum_zero",
                exposure_standardization: "global_center_and_population_sd",
                observation_unit: "replicated_scalar_measurement",
                statistical_unit: "patient",
                null: "zero_fixed_exposure_slope_and_zero_variance_components",
                backend_capability: "nuts",
                maturity: "experimental",
            },
            observations: spec.observations,
            sampling,
            resources: GaussianCrossedNestedHierarchyResources {
                maximum_patients: 512,
                maximum_slides: 1_024,
                maximum_rois: 2_048,
                maximum_batches: 64,
                maximum_cohorts: 32,
                maximum_observations: MAXIMUM_OBSERVATIONS as u32,
                maximum_total_iterations: MAXIMUM_TOTAL_ITERATIONS,
                maximum_output_bytes: 2 * 1_048_576,
                timeout_seconds,
            },
            diagnostic_policy,
        })
    }

    pub fn counts(&self) -> GaussianCrossedNestedCounts {
        design_counts(&self.observations)
    }
}

fn valid_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && value.trim() == value
}

fn design_counts(observations: &[GaussianCrossedNestedObservation]) -> GaussianCrossedNestedCounts {
    GaussianCrossedNestedCounts {
        patients: observations
            .iter()
            .map(|row| row.patient_id.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        slides: observations
            .iter()
            .map(|row| row.slide_id.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        rois: observations
            .iter()
            .map(|row| row.roi_id.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        batches: observations
            .iter()
            .map(|row| row.batch_id.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        cohorts: observations
            .iter()
            .map(|row| row.cohort_id.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        observations: observations.len(),
    }
}

fn validate_design(
    observations: &[GaussianCrossedNestedObservation],
) -> Result<GaussianCrossedNestedCounts, BayesError> {
    if !(48..=MAXIMUM_OBSERVATIONS).contains(&observations.len()) {
        return Err(BayesError::InvalidSpec(
            "crossed/nested hierarchy requires 48 to 4096 observations".into(),
        ));
    }
    let mut slide_patient = BTreeMap::<&str, &str>::new();
    let mut roi_slide = BTreeMap::<&str, &str>::new();
    let mut patient_cohort = BTreeMap::<&str, &str>::new();
    let mut patient_slides = BTreeMap::<&str, BTreeSet<&str>>::new();
    let mut slide_rois = BTreeMap::<&str, BTreeSet<&str>>::new();
    let mut roi_observations = BTreeMap::<&str, usize>::new();
    let mut patient_batches = BTreeMap::<&str, BTreeSet<&str>>::new();
    let mut batch_patients = BTreeMap::<&str, BTreeSet<&str>>::new();
    let mut patient_exposure = BTreeMap::<&str, BTreeSet<u64>>::new();
    let mut cohort_exposure = BTreeMap::<&str, BTreeSet<u64>>::new();
    let mut cohort_patients = BTreeMap::<&str, BTreeSet<&str>>::new();
    for row in observations {
        if !valid_id(&row.patient_id)
            || !valid_id(&row.slide_id)
            || !valid_id(&row.roi_id)
            || !valid_id(&row.batch_id)
            || !valid_id(&row.cohort_id)
            || !row.exposure.is_finite()
            || !row.outcome.is_finite()
        {
            return Err(BayesError::InvalidSpec(
                "crossed/nested hierarchy row is invalid".into(),
            ));
        }
        if slide_patient
            .insert(&row.slide_id, &row.patient_id)
            .is_some_and(|patient| patient != row.patient_id)
            || roi_slide
                .insert(&row.roi_id, &row.slide_id)
                .is_some_and(|slide| slide != row.slide_id)
            || patient_cohort
                .insert(&row.patient_id, &row.cohort_id)
                .is_some_and(|cohort| cohort != row.cohort_id)
        {
            return Err(BayesError::InvalidSpec(
                "slide, ROI, or patient nesting identity is inconsistent".into(),
            ));
        }
        patient_slides
            .entry(&row.patient_id)
            .or_default()
            .insert(&row.slide_id);
        slide_rois
            .entry(&row.slide_id)
            .or_default()
            .insert(&row.roi_id);
        *roi_observations.entry(&row.roi_id).or_default() += 1;
        patient_batches
            .entry(&row.patient_id)
            .or_default()
            .insert(&row.batch_id);
        batch_patients
            .entry(&row.batch_id)
            .or_default()
            .insert(&row.patient_id);
        patient_exposure
            .entry(&row.patient_id)
            .or_default()
            .insert(row.exposure.to_bits());
        cohort_exposure
            .entry(&row.cohort_id)
            .or_default()
            .insert(row.exposure.to_bits());
        cohort_patients
            .entry(&row.cohort_id)
            .or_default()
            .insert(&row.patient_id);
    }
    let counts = design_counts(observations);
    if !(12..=512).contains(&counts.patients)
        || !(24..=1_024).contains(&counts.slides)
        || !(48..=2_048).contains(&counts.rois)
        || !(2..=64).contains(&counts.batches)
        || !(3..=32).contains(&counts.cohorts)
        || patient_slides.values().any(|slides| slides.len() < 2)
        || slide_rois.values().any(|rois| rois.len() < 2)
        || roi_observations.values().any(|count| *count < 2)
        || patient_batches.values().any(|batches| batches.len() < 2)
        || batch_patients.values().any(|patients| patients.len() < 2)
        || patient_exposure.values().any(|values| values.len() < 2)
        || cohort_exposure.values().any(|values| values.len() < 2)
        || cohort_patients.values().any(|patients| patients.len() < 4)
    {
        return Err(BayesError::InvalidSpec(
            "crossed/nested hierarchy is not identified at every declared level".into(),
        ));
    }
    let mean = observations.iter().map(|row| row.exposure).sum::<f64>() / observations.len() as f64;
    let variance = observations
        .iter()
        .map(|row| (row.exposure - mean).powi(2))
        .sum::<f64>()
        / observations.len() as f64;
    if !variance.is_finite() || variance <= f64::EPSILON {
        return Err(BayesError::InvalidSpec(
            "crossed/nested hierarchy exposure has no usable variance".into(),
        ));
    }
    Ok(counts)
}

#[derive(Clone, Debug, Serialize)]
pub struct GaussianCrossedNestedInputIdentity {
    pub path: String,
    pub patients: usize,
    pub slides: usize,
    pub rois: usize,
    pub batches: usize,
    pub cohorts: usize,
    pub observations: usize,
    pub observation_data_sha256: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GaussianCrossedNestedPosterior {
    pub intercept: SarScalarSummary,
    pub exposure_slope: SarScalarSummary,
    pub cohort_intercept_sd: SarScalarSummary,
    pub cohort_exposure_slope_sd: SarScalarSummary,
    pub patient_intercept_sd: SarScalarSummary,
    pub patient_exposure_slope_sd: SarScalarSummary,
    pub slide_intercept_sd: SarScalarSummary,
    pub roi_intercept_sd: SarScalarSummary,
    pub batch_intercept_sd: SarScalarSummary,
    pub residual_sd: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GaussianVariancePartition {
    pub cohort_intercept: f64,
    pub cohort_exposure_slope: f64,
    pub patient_intercept: f64,
    pub patient_exposure_slope: f64,
    pub slide_intercept: f64,
    pub roi_intercept: f64,
    pub batch_intercept: f64,
    pub residual: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GaussianCrossedNestedPpc {
    pub observed_outcome_mean: f64,
    pub replicated_outcome_mean_mean: f64,
    pub probability_replicated_mean_at_least_observed: f64,
    pub observed_outcome_sd: f64,
    pub replicated_outcome_sd_mean: f64,
    pub probability_replicated_sd_at_least_observed: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GaussianCrossedNestedWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: GaussianCrossedNestedPosterior,
    pub variance_partition: GaussianVariancePartition,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: GaussianCrossedNestedPpc,
}

impl GaussianCrossedNestedWorkerResult {
    pub fn validate(
        &self,
        request: &GaussianCrossedNestedHierarchyWorkerRequest,
        expected_request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.format != "marklab.pymc_gaussian_crossed_nested_hierarchy_worker_result"
            || self.version != 1
            || self.fit_state == FitState::ApproximateOnly
        {
            return Err(BayesError::WorkerContract(
                "unsupported crossed/nested hierarchy worker result".into(),
            ));
        }
        if self.backend.name != "pymc"
            || self.backend.version != PYMC_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != expected_request_sha256
        {
            return Err(BayesError::WorkerContract(
                "crossed/nested hierarchy backend or request identity mismatch".into(),
            ));
        }
        let expected_draws =
            u64::from(request.sampling.chains) * u64::from(request.sampling.draws_per_chain);
        if self.sampling.chains != request.sampling.chains
            || self.sampling.tune_per_chain != request.sampling.tune_per_chain
            || self.sampling.draws_per_chain != request.sampling.draws_per_chain
            || self.sampling.completed_draws != expected_draws
        {
            return Err(BayesError::WorkerContract(
                "crossed/nested hierarchy sampling counts mismatch".into(),
            ));
        }
        for summary in [&self.posterior.intercept, &self.posterior.exposure_slope] {
            validate_scalar_summary(
                summary,
                SummarySupport::Real,
                "crossed/nested fixed-effect summary is invalid",
            )?;
        }
        for summary in [
            &self.posterior.cohort_intercept_sd,
            &self.posterior.cohort_exposure_slope_sd,
            &self.posterior.patient_intercept_sd,
            &self.posterior.patient_exposure_slope_sd,
            &self.posterior.slide_intercept_sd,
            &self.posterior.roi_intercept_sd,
            &self.posterior.batch_intercept_sd,
            &self.posterior.residual_sd,
        ] {
            validate_scalar_summary(
                summary,
                SummarySupport::Positive,
                "crossed/nested variance-component summary is invalid",
            )?;
        }
        let partition = [
            self.variance_partition.cohort_intercept,
            self.variance_partition.cohort_exposure_slope,
            self.variance_partition.patient_intercept,
            self.variance_partition.patient_exposure_slope,
            self.variance_partition.slide_intercept,
            self.variance_partition.roi_intercept,
            self.variance_partition.batch_intercept,
            self.variance_partition.residual,
        ];
        if partition
            .iter()
            .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
            || (partition.iter().sum::<f64>() - 1.0).abs() > 1e-9
        {
            return Err(BayesError::WorkerContract(
                "crossed/nested variance partition is invalid".into(),
            ));
        }
        let ppc = &self.posterior_predictive;
        if [
            ppc.observed_outcome_mean,
            ppc.replicated_outcome_mean_mean,
            ppc.probability_replicated_mean_at_least_observed,
            ppc.observed_outcome_sd,
            ppc.replicated_outcome_sd_mean,
            ppc.probability_replicated_sd_at_least_observed,
        ]
        .iter()
        .any(|value| !value.is_finite())
            || ppc.observed_outcome_sd < 0.0
            || ppc.replicated_outcome_sd_mean < 0.0
            || !(0.0..=1.0).contains(&ppc.probability_replicated_mean_at_least_observed)
            || !(0.0..=1.0).contains(&ppc.probability_replicated_sd_at_least_observed)
        {
            return Err(BayesError::WorkerContract(
                "crossed/nested posterior predictive summary is invalid".into(),
            ));
        }
        let observed_mean = request
            .observations
            .iter()
            .map(|row| row.outcome)
            .sum::<f64>()
            / request.observations.len() as f64;
        let observed_sd = (request
            .observations
            .iter()
            .map(|row| (row.outcome - observed_mean).powi(2))
            .sum::<f64>()
            / (request.observations.len() - 1) as f64)
            .sqrt();
        if (ppc.observed_outcome_mean - observed_mean).abs() > 1e-12 * observed_mean.abs().max(1.0)
            || (ppc.observed_outcome_sd - observed_sd).abs() > 1e-12 * observed_sd.abs().max(1.0)
        {
            return Err(BayesError::WorkerContract(
                "crossed/nested worker changed observed PPC summaries".into(),
            ));
        }
        if (self.fit_state == FitState::Complete)
            != diagnostics_satisfy_policy(&self.diagnostics, &request.diagnostic_policy)
        {
            return Err(BayesError::WorkerContract(
                "crossed/nested fit state disagrees with diagnostics".into(),
            ));
        }
        Ok(())
    }

    pub fn into_fit(
        self,
        request: GaussianCrossedNestedHierarchyWorkerRequest,
        input: GaussianCrossedNestedInputIdentity,
    ) -> GaussianCrossedNestedHierarchyFit {
        GaussianCrossedNestedHierarchyFit {
            format: "marklab.bayesian_gaussian_crossed_nested_hierarchy",
            version: 1,
            backend: self.backend,
            model: request.model,
            input,
            fit_state: self.fit_state,
            statistical_unit: "patient",
            null: "zero_fixed_exposure_slope_and_zero_variance_components",
            assumptions: [
                "declared_nesting_and_crossing_are_correct",
                "exposure_varies_within_patient_and_cohort",
                "conditional_gaussian_residuals",
                "random_effects_are_independent_under_the_declared_parameterization",
            ],
            claim_status: if self.fit_state == FitState::Complete {
                "experimental_identified_hierarchy"
            } else {
                "diagnostic_only_nonconverged"
            },
            sampling: self.sampling,
            posterior: self.posterior,
            variance_partition: self.variance_partition,
            diagnostics: self.diagnostics,
            posterior_predictive: self.posterior_predictive,
            seed: request.sampling.seed,
            request_sha256: self.request_sha256,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct GaussianCrossedNestedHierarchyFit {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: GaussianCrossedNestedHierarchyModelIr,
    pub input: GaussianCrossedNestedInputIdentity,
    pub fit_state: FitState,
    pub statistical_unit: &'static str,
    pub null: &'static str,
    pub assumptions: [&'static str; 4],
    pub claim_status: &'static str,
    pub sampling: SamplingSummary,
    pub posterior: GaussianCrossedNestedPosterior,
    pub variance_partition: GaussianVariancePartition,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: GaussianCrossedNestedPpc,
    pub seed: u64,
    pub request_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<GaussianCrossedNestedObservation> {
        let mut rows = Vec::new();
        for cohort in 0..3 {
            for patient_within in 0..4 {
                let patient = cohort * 4 + patient_within;
                for slide in 0..2 {
                    for roi in 0..2 {
                        for replicate in 0..2 {
                            rows.push(GaussianCrossedNestedObservation {
                                patient_id: format!("p{patient}"),
                                slide_id: format!("p{patient}-s{slide}"),
                                roi_id: format!("p{patient}-s{slide}-r{roi}"),
                                batch_id: format!("b{}", (patient + slide + roi + replicate) % 4),
                                cohort_id: format!("c{cohort}"),
                                exposure: if roi == 0 { -1.0 } else { 1.0 },
                                outcome: patient as f64 + roi as f64,
                            });
                        }
                    }
                }
            }
        }
        rows
    }

    #[test]
    fn design_requires_real_crossing_and_every_nested_replication_level() {
        let request = GaussianCrossedNestedHierarchyWorkerRequest::new(
            GaussianCrossedNestedHierarchySpec {
                intercept_prior_sd: 3.0,
                slope_prior_sd: 2.0,
                component_prior_sd: 1.0,
                observations: fixture(),
            },
            NutsSamplingSpec {
                chains: 2,
                tune_per_chain: 100,
                draws_per_chain: 100,
                target_accept: 0.9,
                seed: 1,
            },
            "0".repeat(64),
            "1".repeat(64),
            30,
        )
        .expect("identified design");
        assert_eq!(
            request.counts(),
            GaussianCrossedNestedCounts {
                patients: 12,
                slides: 24,
                rois: 48,
                batches: 4,
                cohorts: 3,
                observations: 96,
            }
        );

        let mut confounded = fixture();
        for row in &mut confounded {
            row.batch_id = row.patient_id.clone();
        }
        let error = GaussianCrossedNestedHierarchyWorkerRequest::new(
            GaussianCrossedNestedHierarchySpec {
                intercept_prior_sd: 3.0,
                slope_prior_sd: 2.0,
                component_prior_sd: 1.0,
                observations: confounded,
            },
            NutsSamplingSpec {
                chains: 2,
                tune_per_chain: 100,
                draws_per_chain: 100,
                target_accept: 0.9,
                seed: 1,
            },
            "0".repeat(64),
            "1".repeat(64),
            30,
        )
        .expect_err("confounded batch");
        assert!(error.to_string().contains("not identified"));
    }
}
