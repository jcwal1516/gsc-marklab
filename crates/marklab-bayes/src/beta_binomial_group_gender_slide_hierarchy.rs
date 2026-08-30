use crate::validation::is_lower_hex_sha256 as is_sha256;

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    model::{PYMC_VERSION, WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION},
    sha256_hex, BackendContract, BayesError, DiagnosticPolicy, FitState, NormalMeanDiagnostics,
    NutsSamplingSpec, SamplingSummary, SarScalarSummary, WorkerBackend,
};

#[derive(Clone, Debug, Serialize)]
pub struct BetaBinomialGroupGenderSlideData {
    pub slide_id: String,
    pub patient_id: String,
    pub group: String,
    pub gender: String,
    pub successes: u64,
    pub trials: u64,
}

#[derive(Clone, Debug)]
pub struct BetaBinomialGroupGenderSlideHierarchySpec {
    pub reference_group: String,
    pub comparison_group: String,
    pub reference_gender: String,
    pub comparison_gender: String,
    pub intercept_prior_mean: f64,
    pub intercept_prior_sd: f64,
    pub group_effect_prior_sd: f64,
    pub gender_effect_prior_sd: f64,
    pub patient_log_odds_sd_prior_sd: f64,
    pub slide_concentration_prior_sd: f64,
    pub slides: Vec<BetaBinomialGroupGenderSlideData>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BetaBinomialGroupGenderSlideHierarchyModelIr {
    pub format: &'static str,
    pub version: u32,
    pub family: &'static str,
    pub reference_group: String,
    pub comparison_group: String,
    pub reference_gender: String,
    pub comparison_gender: String,
    pub intercept_prior: &'static str,
    pub intercept_prior_mean: f64,
    pub intercept_prior_sd: f64,
    pub group_effect_prior: &'static str,
    pub group_effect_prior_sd: f64,
    pub gender_effect_prior: &'static str,
    pub gender_effect_prior_sd: f64,
    pub patient_effect: &'static str,
    pub patient_log_odds_sd_prior: &'static str,
    pub patient_log_odds_sd_prior_sd: f64,
    pub slide_concentration_prior: &'static str,
    pub slide_concentration_prior_sd: f64,
    pub likelihood: &'static str,
    pub interaction: &'static str,
    pub standardization: &'static str,
    pub nesting: &'static str,
    pub biological_unit: &'static str,
    pub backend_capability: &'static str,
    pub maturity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct BetaBinomialGroupGenderSlideHierarchyResourceLimits {
    pub maximum_patients: u32,
    pub maximum_slides: u32,
    pub maximum_total_trials: u64,
    pub maximum_total_iterations: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct BetaBinomialGroupGenderSlideHierarchyWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: BetaBinomialGroupGenderSlideHierarchyModelIr,
    pub slides: Vec<BetaBinomialGroupGenderSlideData>,
    pub sampling: NutsSamplingSpec,
    pub resources: BetaBinomialGroupGenderSlideHierarchyResourceLimits,
    pub diagnostic_policy: DiagnosticPolicy,
}

impl BetaBinomialGroupGenderSlideHierarchyWorkerRequest {
    pub fn new(
        spec: BetaBinomialGroupGenderSlideHierarchySpec,
        sampling: NutsSamplingSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        if spec.reference_group.is_empty()
            || spec.comparison_group.is_empty()
            || spec.reference_group == spec.comparison_group
            || spec.reference_gender.is_empty()
            || spec.comparison_gender.is_empty()
            || spec.reference_gender == spec.comparison_gender
            || !spec.intercept_prior_mean.is_finite()
            || [
                spec.intercept_prior_sd,
                spec.group_effect_prior_sd,
                spec.gender_effect_prior_sd,
                spec.patient_log_odds_sd_prior_sd,
                spec.slide_concentration_prior_sd,
            ]
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
            || !is_sha256(&environment_lock_sha256)
            || !is_sha256(&worker_sha256)
            || !(1..=3_600).contains(&timeout_seconds)
        {
            return Err(BayesError::InvalidSpec(
                "beta-binomial group/gender slide hierarchy controls are invalid".into(),
            ));
        }
        if !(32..=2_048).contains(&spec.slides.len()) {
            return Err(BayesError::InvalidSpec(
                "beta-binomial group/gender slide hierarchy requires 32 to 2048 slides".into(),
            ));
        }
        let mut slides = spec.slides;
        slides.sort_by(|left, right| {
            left.patient_id
                .cmp(&right.patient_id)
                .then_with(|| left.slide_id.cmp(&right.slide_id))
        });
        let mut slide_ids = BTreeSet::new();
        let mut patients = BTreeMap::<String, (String, String, usize)>::new();
        let mut total_trials = 0_u64;
        for slide in &slides {
            if slide.slide_id.is_empty()
                || slide.slide_id.len() > 128
                || slide.slide_id.trim() != slide.slide_id
                || !slide_ids.insert(slide.slide_id.as_str())
                || slide.patient_id.is_empty()
                || slide.patient_id.len() > 128
                || slide.patient_id.trim() != slide.patient_id
                || ![
                    spec.reference_group.as_str(),
                    spec.comparison_group.as_str(),
                ]
                .contains(&slide.group.as_str())
                || ![
                    spec.reference_gender.as_str(),
                    spec.comparison_gender.as_str(),
                ]
                .contains(&slide.gender.as_str())
                || slide.trials == 0
                || slide.successes > slide.trials
            {
                return Err(BayesError::InvalidSpec(
                    "beta-binomial group/gender slide row is invalid".into(),
                ));
            }
            let patient = patients
                .entry(slide.patient_id.clone())
                .or_insert_with(|| (slide.group.clone(), slide.gender.clone(), 0));
            if patient.0 != slide.group || patient.1 != slide.gender {
                return Err(BayesError::InvalidSpec(
                    "slide rows disagree on patient group or gender".into(),
                ));
            }
            patient.2 += 1;
            total_trials = total_trials.checked_add(slide.trials).ok_or_else(|| {
                BayesError::InvalidSpec("beta-binomial slide trials overflow".into())
            })?;
        }
        if !(16..=512).contains(&patients.len())
            || patients.values().filter(|patient| patient.2 >= 2).count() < 8
        {
            return Err(BayesError::InvalidSpec(
                "slide hierarchy requires 16 to 512 patients and eight repeated patients".into(),
            ));
        }
        for group in [&spec.reference_group, &spec.comparison_group] {
            for gender in [&spec.reference_gender, &spec.comparison_gender] {
                if patients
                    .values()
                    .filter(|patient| &patient.0 == group && &patient.1 == gender)
                    .count()
                    < 4
                {
                    return Err(BayesError::InvalidSpec(
                        "slide hierarchy requires four patients per group/gender cell".into(),
                    ));
                }
            }
        }
        let maximum_total_trials = 10_000_000;
        let maximum_total_iterations = 400_000;
        if total_trials > maximum_total_trials
            || u64::from(sampling.chains)
                * u64::from(sampling.tune_per_chain + sampling.draws_per_chain)
                > maximum_total_iterations
        {
            return Err(BayesError::InvalidSpec(
                "slide hierarchy exceeds trial or iteration limits".into(),
            ));
        }
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
            model: BetaBinomialGroupGenderSlideHierarchyModelIr {
                format: "marklab.bayesian_model_ir",
                version: 1,
                family: "beta_binomial_group_gender_slide_within_patient_hierarchy",
                reference_group: spec.reference_group,
                comparison_group: spec.comparison_group,
                reference_gender: spec.reference_gender,
                comparison_gender: spec.comparison_gender,
                intercept_prior: "normal_log_odds",
                intercept_prior_mean: spec.intercept_prior_mean,
                intercept_prior_sd: spec.intercept_prior_sd,
                group_effect_prior: "normal_log_odds_difference",
                group_effect_prior_sd: spec.group_effect_prior_sd,
                gender_effect_prior: "normal_log_odds_difference",
                gender_effect_prior_sd: spec.gender_effect_prior_sd,
                patient_effect: "noncentered_normal_random_intercept",
                patient_log_odds_sd_prior: "half_normal",
                patient_log_odds_sd_prior_sd: spec.patient_log_odds_sd_prior_sd,
                slide_concentration_prior: "half_normal",
                slide_concentration_prior_sd: spec.slide_concentration_prior_sd,
                likelihood: "beta_binomial_slide_counts",
                interaction: "none",
                standardization: "observed_gender_distribution_typical_patient",
                nesting: "slide_within_patient",
                biological_unit: "patient",
                backend_capability: "nuts",
                maturity: "experimental",
            },
            slides,
            sampling,
            resources: BetaBinomialGroupGenderSlideHierarchyResourceLimits {
                maximum_patients: 512,
                maximum_slides: 2_048,
                maximum_total_trials,
                maximum_total_iterations,
                maximum_output_bytes: 2 * 1_048_576,
                timeout_seconds,
            },
            diagnostic_policy: DiagnosticPolicy::default(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BetaBinomialGroupGenderSlideHierarchyPosterior {
    pub intercept_log_odds: SarScalarSummary,
    pub group_log_odds_effect: SarScalarSummary,
    pub gender_log_odds_effect: SarScalarSummary,
    pub patient_log_odds_sd: SarScalarSummary,
    pub slide_concentration: SarScalarSummary,
    pub reference_group_reference_gender_probability: SarScalarSummary,
    pub comparison_group_reference_gender_probability: SarScalarSummary,
    pub reference_group_comparison_gender_probability: SarScalarSummary,
    pub comparison_group_comparison_gender_probability: SarScalarSummary,
    pub marginal_reference_group_probability: SarScalarSummary,
    pub marginal_comparison_group_probability: SarScalarSummary,
    pub marginal_probability_difference_comparison_minus_reference: SarScalarSummary,
    pub group_odds_ratio: SarScalarSummary,
    pub gender_odds_ratio: SarScalarSummary,
    pub slide_overdispersion_mean: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BetaBinomialGroupGenderSlidePatientPosterior {
    pub patient_id: String,
    pub group: String,
    pub gender: String,
    pub slide_count: u32,
    pub successes: u64,
    pub trials: u64,
    pub observed_proportion: f64,
    pub random_log_odds_effect: SarScalarSummary,
    pub posterior_probability: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BetaBinomialGroupGenderSlidePosterior {
    pub slide_id: String,
    pub patient_id: String,
    pub group: String,
    pub gender: String,
    pub successes: u64,
    pub trials: u64,
    pub observed_proportion: f64,
    pub posterior_patient_probability: SarScalarSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BetaBinomialGroupGenderSlidePosteriorPredictive {
    pub observed_total_successes: u64,
    pub replicated_total_successes_mean: f64,
    pub probability_replicated_total_successes_at_least_observed: f64,
    pub observed_patient_group_mean_proportion_difference: f64,
    pub replicated_patient_group_mean_proportion_difference_mean: f64,
    pub probability_replicated_patient_group_difference_at_least_observed: f64,
    pub observed_patient_gender_mean_proportion_difference: f64,
    pub replicated_patient_gender_mean_proportion_difference_mean: f64,
    pub probability_replicated_patient_gender_difference_at_least_observed: f64,
    pub observed_mean_absolute_slide_patient_deviation: f64,
    pub replicated_mean_absolute_slide_patient_deviation_mean: f64,
    pub probability_replicated_slide_deviation_at_least_observed: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BetaBinomialGroupGenderSlideHierarchyWorkerResult {
    pub(crate) format: String,
    pub(crate) version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: BetaBinomialGroupGenderSlideHierarchyPosterior,
    pub patients: Vec<BetaBinomialGroupGenderSlidePatientPosterior>,
    pub slides: Vec<BetaBinomialGroupGenderSlidePosterior>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: BetaBinomialGroupGenderSlidePosteriorPredictive,
}

impl BetaBinomialGroupGenderSlideHierarchyWorkerResult {
    pub fn validate(
        &self,
        request: &BetaBinomialGroupGenderSlideHierarchyWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        self.validate_for_backend(
            request,
            request_sha256,
            "marklab.pymc_beta_binomial_group_gender_slide_hierarchy_worker_result",
            &request.backend,
        )
    }

    pub(crate) fn validate_for_backend(
        &self,
        request: &BetaBinomialGroupGenderSlideHierarchyWorkerRequest,
        request_sha256: &str,
        result_format: &str,
        backend: &BackendContract,
    ) -> Result<(), BayesError> {
        let patient_count = request
            .slides
            .iter()
            .map(|slide| slide.patient_id.as_str())
            .collect::<BTreeSet<_>>()
            .len();
        if self.format != result_format
            || self.version != 1
            || self.backend.name != backend.name
            || self.backend.version != backend.version
            || self.backend.python_version != backend.python_version
            || self.backend.environment_lock_sha256 != backend.environment_lock_sha256
            || self.backend.worker_sha256 != backend.worker_sha256
            || self.request_sha256 != request_sha256
            || self.patients.len() != patient_count
            || self.slides.len() != request.slides.len()
        {
            return Err(BayesError::WorkerContract(
                "slide hierarchy result identity or dimensions mismatch".into(),
            ));
        }
        let expected_draws =
            u64::from(request.sampling.chains) * u64::from(request.sampling.draws_per_chain);
        if self.sampling.completed_draws != expected_draws
            || self.sampling.chains != request.sampling.chains
            || self.sampling.tune_per_chain != request.sampling.tune_per_chain
            || self.sampling.draws_per_chain != request.sampling.draws_per_chain
        {
            return Err(BayesError::WorkerContract(
                "slide hierarchy sampling summary mismatch".into(),
            ));
        }
        for summary in [
            &self.posterior.intercept_log_odds,
            &self.posterior.group_log_odds_effect,
            &self.posterior.gender_log_odds_effect,
        ] {
            validate_summary(summary, SummarySupport::Real)?;
        }
        for summary in [
            &self.posterior.patient_log_odds_sd,
            &self.posterior.slide_concentration,
            &self.posterior.group_odds_ratio,
            &self.posterior.gender_odds_ratio,
        ] {
            validate_summary(summary, SummarySupport::Positive)?;
        }
        for summary in [
            &self.posterior.reference_group_reference_gender_probability,
            &self.posterior.comparison_group_reference_gender_probability,
            &self.posterior.reference_group_comparison_gender_probability,
            &self
                .posterior
                .comparison_group_comparison_gender_probability,
            &self.posterior.marginal_reference_group_probability,
            &self.posterior.marginal_comparison_group_probability,
        ] {
            validate_summary(summary, SummarySupport::Unit)?;
        }
        validate_summary(
            &self
                .posterior
                .marginal_probability_difference_comparison_minus_reference,
            SummarySupport::Difference,
        )?;
        if !(0.0..1.0).contains(&self.posterior.slide_overdispersion_mean) {
            return Err(BayesError::WorkerContract(
                "slide hierarchy overdispersion is invalid".into(),
            ));
        }
        let mut aggregates = BTreeMap::<&str, (&str, &str, u32, u64, u64)>::new();
        for slide in &request.slides {
            let row = aggregates.entry(slide.patient_id.as_str()).or_insert((
                slide.group.as_str(),
                slide.gender.as_str(),
                0,
                0,
                0,
            ));
            row.2 += 1;
            row.3 += slide.successes;
            row.4 += slide.trials;
        }
        for (actual, (patient_id, expected)) in self.patients.iter().zip(aggregates) {
            validate_summary(&actual.random_log_odds_effect, SummarySupport::Real)?;
            validate_summary(&actual.posterior_probability, SummarySupport::Unit)?;
            if actual.patient_id != patient_id
                || actual.group != expected.0
                || actual.gender != expected.1
                || actual.slide_count != expected.2
                || actual.successes != expected.3
                || actual.trials != expected.4
                || (actual.observed_proportion - expected.3 as f64 / expected.4 as f64).abs()
                    > 1e-12
            {
                return Err(BayesError::WorkerContract(
                    "slide hierarchy patient summary is invalid".into(),
                ));
            }
        }
        for (actual, expected) in self.slides.iter().zip(&request.slides) {
            validate_summary(&actual.posterior_patient_probability, SummarySupport::Unit)?;
            if actual.slide_id != expected.slide_id
                || actual.patient_id != expected.patient_id
                || actual.group != expected.group
                || actual.gender != expected.gender
                || actual.successes != expected.successes
                || actual.trials != expected.trials
                || (actual.observed_proportion - expected.successes as f64 / expected.trials as f64)
                    .abs()
                    > 1e-12
            {
                return Err(BayesError::WorkerContract(
                    "slide hierarchy slide summary is invalid".into(),
                ));
            }
        }
        let predictive = &self.posterior_predictive;
        if predictive.observed_total_successes
            != request
                .slides
                .iter()
                .map(|slide| slide.successes)
                .sum::<u64>()
            || ![
                predictive.replicated_total_successes_mean,
                predictive.observed_patient_group_mean_proportion_difference,
                predictive.replicated_patient_group_mean_proportion_difference_mean,
                predictive.observed_patient_gender_mean_proportion_difference,
                predictive.replicated_patient_gender_mean_proportion_difference_mean,
                predictive.observed_mean_absolute_slide_patient_deviation,
                predictive.replicated_mean_absolute_slide_patient_deviation_mean,
            ]
            .iter()
            .all(|value| value.is_finite())
            || ![
                predictive.probability_replicated_total_successes_at_least_observed,
                predictive.probability_replicated_patient_group_difference_at_least_observed,
                predictive.probability_replicated_patient_gender_difference_at_least_observed,
                predictive.probability_replicated_slide_deviation_at_least_observed,
            ]
            .iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        {
            return Err(BayesError::WorkerContract(
                "slide hierarchy posterior predictive summary is invalid".into(),
            ));
        }
        let complete = diagnostic_pass(&self.diagnostics, &request.diagnostic_policy);
        if (self.fit_state == FitState::Complete) != complete {
            return Err(BayesError::WorkerContract(
                "slide hierarchy fit state disagrees with diagnostics".into(),
            ));
        }
        Ok(())
    }

    pub fn into_result(
        self,
        request: BetaBinomialGroupGenderSlideHierarchyWorkerRequest,
        input: BetaBinomialGroupGenderSlideHierarchyInputIdentity,
    ) -> BetaBinomialGroupGenderSlideHierarchyResult {
        BetaBinomialGroupGenderSlideHierarchyResult {
            format: "marklab.bayesian_beta_binomial_group_gender_slide_hierarchy",
            version: 1,
            backend: self.backend,
            model: request.model,
            input,
            sampling: self.sampling,
            fit_state: self.fit_state,
            posterior: self.posterior,
            patients: self.patients,
            slides: self.slides,
            diagnostics: self.diagnostics,
            posterior_predictive: self.posterior_predictive,
            seed: request.sampling.seed,
            claim_status: if self.fit_state == FitState::Complete {
                "experimental_repeated_slide_patient_hierarchy"
            } else {
                "diagnostic_only_nonconverged"
            },
            request_sha256: self.request_sha256,
        }
    }
}

#[derive(Clone, Copy)]
enum SummarySupport {
    Real,
    Unit,
    Difference,
    Positive,
}

fn validate_summary(summary: &SarScalarSummary, support: SummarySupport) -> Result<(), BayesError> {
    if ![
        summary.mean,
        summary.sd,
        summary.interval_lower,
        summary.interval_upper,
    ]
    .iter()
    .all(|value| value.is_finite())
        || summary.sd <= 0.0
        || summary.interval_lower > summary.interval_upper
        || matches!(support, SummarySupport::Unit)
            && (!(0.0..=1.0).contains(&summary.mean)
                || !(0.0..=1.0).contains(&summary.interval_lower)
                || !(0.0..=1.0).contains(&summary.interval_upper))
        || matches!(support, SummarySupport::Difference)
            && (!(-1.0..=1.0).contains(&summary.mean)
                || !(-1.0..=1.0).contains(&summary.interval_lower)
                || !(-1.0..=1.0).contains(&summary.interval_upper))
        || matches!(support, SummarySupport::Positive)
            && (summary.mean <= 0.0 || summary.interval_lower < 0.0)
    {
        return Err(BayesError::WorkerContract(
            "slide hierarchy posterior summary is invalid".into(),
        ));
    }
    Ok(())
}

fn diagnostic_pass(diagnostics: &NormalMeanDiagnostics, policy: &DiagnosticPolicy) -> bool {
    diagnostics.prior_predictive_finite
        && diagnostics.posterior_finite
        && diagnostics.constraints_valid
        && diagnostics.identifiability_checks_passed
        && diagnostics.r_hat <= policy.maximum_r_hat
        && diagnostics.ess_bulk >= policy.minimum_bulk_ess
        && diagnostics.ess_tail >= policy.minimum_tail_ess
        && diagnostics.minimum_ebfmi >= policy.minimum_ebfmi
        && diagnostics.divergences <= policy.maximum_divergences
        && diagnostics.max_tree_depth_hits <= policy.maximum_tree_depth_hits
}

#[derive(Clone, Debug, Serialize)]
pub struct BetaBinomialGroupGenderSlideHierarchyInputIdentity {
    pub path: String,
    pub slide_data_sha256: String,
    pub patients: usize,
    pub slides: usize,
    pub repeated_patients: usize,
    pub design_cell_patient_counts: BTreeMap<String, usize>,
}

pub fn beta_binomial_group_gender_slide_data_sha256(
    slides: &[BetaBinomialGroupGenderSlideData],
) -> Result<String, BayesError> {
    Ok(sha256_hex(&serde_json::to_vec(slides)?))
}

#[derive(Debug, Serialize)]
pub struct BetaBinomialGroupGenderSlideHierarchyResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: BetaBinomialGroupGenderSlideHierarchyModelIr,
    pub input: BetaBinomialGroupGenderSlideHierarchyInputIdentity,
    pub sampling: SamplingSummary,
    pub fit_state: FitState,
    pub posterior: BetaBinomialGroupGenderSlideHierarchyPosterior,
    pub patients: Vec<BetaBinomialGroupGenderSlidePatientPosterior>,
    pub slides: Vec<BetaBinomialGroupGenderSlidePosterior>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: BetaBinomialGroupGenderSlidePosteriorPredictive,
    pub seed: u64,
    pub claim_status: &'static str,
    pub request_sha256: String,
}
