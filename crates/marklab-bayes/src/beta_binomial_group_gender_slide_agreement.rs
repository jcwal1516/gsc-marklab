use crate::validation::is_lower_hex_sha256 as is_sha256;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    hierarchical_agreement::{JAX_VERSION, NUMPYRO_VERSION},
    sha256_hex, BackendContract, BayesError, BetaBinomialGroupGenderSlideHierarchyInputIdentity,
    BetaBinomialGroupGenderSlideHierarchyModelIr, BetaBinomialGroupGenderSlideHierarchyPosterior,
    BetaBinomialGroupGenderSlideHierarchyWorkerRequest,
    BetaBinomialGroupGenderSlideHierarchyWorkerResult,
    BetaBinomialGroupGenderSlidePatientPosterior, BetaBinomialGroupGenderSlidePosterior,
    BetaBinomialGroupGenderSlidePosteriorPredictive, BetaBinomialParameterAgreement,
    BetaBinomialPatientAgreement, FitState, NormalMeanDiagnostics, SamplingSummary,
    SarScalarSummary, WorkerBackend,
};

const REQUEST_FORMAT: &str =
    "marklab.numpyro_beta_binomial_group_gender_slide_hierarchy_worker_request";
const RESULT_FORMAT: &str =
    "marklab.numpyro_beta_binomial_group_gender_slide_hierarchy_worker_result";

#[derive(Clone, Debug, Serialize)]
pub struct NumpyroBetaBinomialGroupGenderSlideHierarchyWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub jax_version: &'static str,
    pub source_request_sha256: String,
    pub source_request: BetaBinomialGroupGenderSlideHierarchyWorkerRequest,
}

impl NumpyroBetaBinomialGroupGenderSlideHierarchyWorkerRequest {
    pub fn new(
        source_request: BetaBinomialGroupGenderSlideHierarchyWorkerRequest,
        environment_lock_sha256: String,
        worker_sha256: String,
    ) -> Result<Self, BayesError> {
        if !is_sha256(&environment_lock_sha256) || !is_sha256(&worker_sha256) {
            return Err(BayesError::InvalidSpec(
                "NumPyro slide hierarchy identities must be SHA-256 digests".into(),
            ));
        }
        let source_request_sha256 = sha256_hex(&serde_json::to_vec(&source_request)?);
        Ok(Self {
            format: REQUEST_FORMAT,
            version: 1,
            backend: BackendContract {
                name: "numpyro",
                version: NUMPYRO_VERSION,
                python_version: "3.12",
                environment_lock_sha256,
                worker_sha256,
            },
            jax_version: JAX_VERSION,
            source_request_sha256,
            source_request,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NumpyroBetaBinomialGroupGenderSlideHierarchyWorkerResult {
    format: String,
    version: u32,
    backend: WorkerBackend,
    jax_version: String,
    request_sha256: String,
    source_request_sha256: String,
    fit_state: FitState,
    sampling: SamplingSummary,
    posterior: BetaBinomialGroupGenderSlideHierarchyPosterior,
    patients: Vec<BetaBinomialGroupGenderSlidePatientPosterior>,
    slides: Vec<BetaBinomialGroupGenderSlidePosterior>,
    diagnostics: NormalMeanDiagnostics,
    posterior_predictive: BetaBinomialGroupGenderSlidePosteriorPredictive,
}

impl NumpyroBetaBinomialGroupGenderSlideHierarchyWorkerResult {
    pub fn into_validated_payload(
        self,
        request: &NumpyroBetaBinomialGroupGenderSlideHierarchyWorkerRequest,
        request_sha256: &str,
    ) -> Result<BetaBinomialGroupGenderSlideHierarchyWorkerResult, BayesError> {
        if self.format != RESULT_FORMAT
            || self.version != 1
            || self.jax_version != JAX_VERSION
            || self.source_request_sha256 != request.source_request_sha256
        {
            return Err(BayesError::WorkerContract(
                "NumPyro slide hierarchy result identity mismatch".into(),
            ));
        }
        let payload = BetaBinomialGroupGenderSlideHierarchyWorkerResult {
            format: self.format,
            version: self.version,
            backend: self.backend,
            request_sha256: self.request_sha256,
            fit_state: self.fit_state,
            sampling: self.sampling,
            posterior: self.posterior,
            patients: self.patients,
            slides: self.slides,
            diagnostics: self.diagnostics,
            posterior_predictive: self.posterior_predictive,
        };
        payload.validate_for_backend(
            &request.source_request,
            request_sha256,
            RESULT_FORMAT,
            &request.backend,
        )?;
        Ok(payload)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BetaBinomialGroupGenderSlideAgreementPolicy {
    pub maximum_standardized_difference: f64,
    pub minimum_probability_tolerance: f64,
    pub minimum_log_odds_tolerance: f64,
    pub minimum_patient_sd_tolerance: f64,
    pub minimum_concentration_tolerance: f64,
    pub minimum_patient_probability_tolerance: f64,
    pub minimum_patient_effect_tolerance: f64,
}

impl BetaBinomialGroupGenderSlideAgreementPolicy {
    fn validate(self) -> Result<Self, BayesError> {
        if !self.maximum_standardized_difference.is_finite()
            || !(1.0..=10.0).contains(&self.maximum_standardized_difference)
            || !self.minimum_probability_tolerance.is_finite()
            || !(0.0..=0.25).contains(&self.minimum_probability_tolerance)
            || !self.minimum_log_odds_tolerance.is_finite()
            || !(0.0..=1.0).contains(&self.minimum_log_odds_tolerance)
            || !self.minimum_patient_sd_tolerance.is_finite()
            || !(0.0..=1.0).contains(&self.minimum_patient_sd_tolerance)
            || !self.minimum_concentration_tolerance.is_finite()
            || !(0.0..=10.0).contains(&self.minimum_concentration_tolerance)
            || !self.minimum_patient_probability_tolerance.is_finite()
            || !(0.0..=0.25).contains(&self.minimum_patient_probability_tolerance)
            || !self.minimum_patient_effect_tolerance.is_finite()
            || !(0.0..=1.0).contains(&self.minimum_patient_effect_tolerance)
        {
            return Err(BayesError::InvalidSpec(
                "slide hierarchy agreement controls are invalid".into(),
            ));
        }
        Ok(self)
    }
}

#[derive(Debug, Serialize)]
pub struct BetaBinomialGroupGenderSlideAgreementComparison {
    pub parameters: BTreeMap<String, BetaBinomialParameterAgreement>,
    pub patient_probabilities: BetaBinomialPatientAgreement,
    pub patient_random_effects: BetaBinomialPatientAgreement,
    pub maximum_standardized_difference: f64,
    pub minimum_probability_tolerance: f64,
    pub minimum_log_odds_tolerance: f64,
    pub minimum_patient_sd_tolerance: f64,
    pub minimum_concentration_tolerance: f64,
    pub minimum_patient_probability_tolerance: f64,
    pub minimum_patient_effect_tolerance: f64,
}

#[derive(Debug, Serialize)]
pub struct BetaBinomialGroupGenderSlideBackendSummary {
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: BetaBinomialGroupGenderSlideHierarchyPosterior,
    pub patients: Vec<BetaBinomialGroupGenderSlidePatientPosterior>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: BetaBinomialGroupGenderSlidePosteriorPredictive,
}

#[derive(Debug, Serialize)]
pub struct BetaBinomialGroupGenderSlideAgreementResult {
    pub format: &'static str,
    pub version: u32,
    pub model: BetaBinomialGroupGenderSlideHierarchyModelIr,
    pub input: BetaBinomialGroupGenderSlideHierarchyInputIdentity,
    pub fit_state: FitState,
    pub agreement_status: &'static str,
    pub pymc: BetaBinomialGroupGenderSlideBackendSummary,
    pub numpyro: BetaBinomialGroupGenderSlideBackendSummary,
    pub comparison: BetaBinomialGroupGenderSlideAgreementComparison,
    pub seed: u64,
    pub claim_status: &'static str,
}

impl BetaBinomialGroupGenderSlideAgreementResult {
    pub fn new(
        source_request: BetaBinomialGroupGenderSlideHierarchyWorkerRequest,
        input: BetaBinomialGroupGenderSlideHierarchyInputIdentity,
        pymc: BetaBinomialGroupGenderSlideHierarchyWorkerResult,
        numpyro: BetaBinomialGroupGenderSlideHierarchyWorkerResult,
        policy: BetaBinomialGroupGenderSlideAgreementPolicy,
    ) -> Result<Self, BayesError> {
        let policy = policy.validate()?;
        if pymc.patients.len() != numpyro.patients.len() {
            return Err(BayesError::WorkerContract(
                "slide hierarchy backend patient dimensions differ".into(),
            ));
        }
        let p = &pymc.posterior;
        let n = &numpyro.posterior;
        let mut parameters = BTreeMap::new();
        for (name, left, right, tolerance) in [
            (
                "intercept_log_odds",
                &p.intercept_log_odds,
                &n.intercept_log_odds,
                policy.minimum_log_odds_tolerance,
            ),
            (
                "group_log_odds_effect",
                &p.group_log_odds_effect,
                &n.group_log_odds_effect,
                policy.minimum_log_odds_tolerance,
            ),
            (
                "gender_log_odds_effect",
                &p.gender_log_odds_effect,
                &n.gender_log_odds_effect,
                policy.minimum_log_odds_tolerance,
            ),
            (
                "patient_log_odds_sd",
                &p.patient_log_odds_sd,
                &n.patient_log_odds_sd,
                policy.minimum_patient_sd_tolerance,
            ),
            (
                "slide_concentration",
                &p.slide_concentration,
                &n.slide_concentration,
                policy.minimum_concentration_tolerance,
            ),
            (
                "reference_group_reference_gender_probability",
                &p.reference_group_reference_gender_probability,
                &n.reference_group_reference_gender_probability,
                policy.minimum_probability_tolerance,
            ),
            (
                "comparison_group_reference_gender_probability",
                &p.comparison_group_reference_gender_probability,
                &n.comparison_group_reference_gender_probability,
                policy.minimum_probability_tolerance,
            ),
            (
                "reference_group_comparison_gender_probability",
                &p.reference_group_comparison_gender_probability,
                &n.reference_group_comparison_gender_probability,
                policy.minimum_probability_tolerance,
            ),
            (
                "comparison_group_comparison_gender_probability",
                &p.comparison_group_comparison_gender_probability,
                &n.comparison_group_comparison_gender_probability,
                policy.minimum_probability_tolerance,
            ),
            (
                "marginal_reference_group_probability",
                &p.marginal_reference_group_probability,
                &n.marginal_reference_group_probability,
                policy.minimum_probability_tolerance,
            ),
            (
                "marginal_comparison_group_probability",
                &p.marginal_comparison_group_probability,
                &n.marginal_comparison_group_probability,
                policy.minimum_probability_tolerance,
            ),
            (
                "marginal_probability_difference_comparison_minus_reference",
                &p.marginal_probability_difference_comparison_minus_reference,
                &n.marginal_probability_difference_comparison_minus_reference,
                policy.minimum_probability_tolerance,
            ),
            (
                "group_odds_ratio",
                &p.group_odds_ratio,
                &n.group_odds_ratio,
                policy.minimum_log_odds_tolerance,
            ),
            (
                "gender_odds_ratio",
                &p.gender_odds_ratio,
                &n.gender_odds_ratio,
                policy.minimum_log_odds_tolerance,
            ),
        ] {
            parameters.insert(
                name.into(),
                compare_parameter(left, &pymc, right, &numpyro, tolerance, policy),
            );
        }
        let patient_probabilities = compare_patients(
            &pymc,
            &numpyro,
            |patient| &patient.posterior_probability,
            policy.minimum_patient_probability_tolerance,
            policy,
        )?;
        let patient_random_effects = compare_patients(
            &pymc,
            &numpyro,
            |patient| &patient.random_log_odds_effect,
            policy.minimum_patient_effect_tolerance,
            policy,
        )?;
        let fits_complete =
            pymc.fit_state == FitState::Complete && numpyro.fit_state == FitState::Complete;
        let agrees = parameters.values().all(|value| value.passes)
            && patient_probabilities.passes
            && patient_random_effects.passes;
        let available = fits_complete && agrees;
        let seed = source_request.sampling.seed;
        Ok(Self {
            format: "marklab.bayesian_beta_binomial_group_gender_slide_hierarchy_cross_backend_agreement",
            version: 1,
            model: source_request.model,
            input,
            fit_state: if fits_complete { FitState::Complete } else { FitState::Nonconverged },
            agreement_status: if available {
                "agree_within_monte_carlo_error"
            } else if fits_complete {
                "diagnostic_only_backend_disagreement"
            } else {
                "diagnostic_only_nonconverged"
            },
            pymc: summarize(pymc),
            numpyro: summarize(numpyro),
            comparison: BetaBinomialGroupGenderSlideAgreementComparison {
                parameters,
                patient_probabilities,
                patient_random_effects,
                maximum_standardized_difference: policy.maximum_standardized_difference,
                minimum_probability_tolerance: policy.minimum_probability_tolerance,
                minimum_log_odds_tolerance: policy.minimum_log_odds_tolerance,
                minimum_patient_sd_tolerance: policy.minimum_patient_sd_tolerance,
                minimum_concentration_tolerance: policy.minimum_concentration_tolerance,
                minimum_patient_probability_tolerance: policy.minimum_patient_probability_tolerance,
                minimum_patient_effect_tolerance: policy.minimum_patient_effect_tolerance,
            },
            seed,
            claim_status: if available {
                "experimental_cross_backend_validation"
            } else {
                "diagnostic_only_cross_backend_validation"
            },
        })
    }
}

fn summarize(
    result: BetaBinomialGroupGenderSlideHierarchyWorkerResult,
) -> BetaBinomialGroupGenderSlideBackendSummary {
    BetaBinomialGroupGenderSlideBackendSummary {
        backend: result.backend,
        request_sha256: result.request_sha256,
        fit_state: result.fit_state,
        sampling: result.sampling,
        posterior: result.posterior,
        patients: result.patients,
        diagnostics: result.diagnostics,
        posterior_predictive: result.posterior_predictive,
    }
}

fn compare_parameter(
    left: &SarScalarSummary,
    left_result: &BetaBinomialGroupGenderSlideHierarchyWorkerResult,
    right: &SarScalarSummary,
    right_result: &BetaBinomialGroupGenderSlideHierarchyWorkerResult,
    minimum_tolerance: f64,
    policy: BetaBinomialGroupGenderSlideAgreementPolicy,
) -> BetaBinomialParameterAgreement {
    let absolute_difference = (left.mean - right.mean).abs();
    let combined_mcse = (left.sd / left_result.diagnostics.ess_bulk.sqrt())
        .hypot(right.sd / right_result.diagnostics.ess_bulk.sqrt());
    let standardized_difference = standardized_mcse_difference(absolute_difference, combined_mcse);
    let tolerance = minimum_tolerance.max(policy.maximum_standardized_difference * combined_mcse);
    let intervals_overlap =
        left.interval_lower <= right.interval_upper && right.interval_lower <= left.interval_upper;
    BetaBinomialParameterAgreement {
        pymc_mean: left.mean,
        numpyro_mean: right.mean,
        absolute_difference,
        combined_mcse,
        standardized_difference,
        tolerance,
        intervals_overlap,
        passes: intervals_overlap && absolute_difference <= tolerance,
    }
}

fn compare_patients(
    left: &BetaBinomialGroupGenderSlideHierarchyWorkerResult,
    right: &BetaBinomialGroupGenderSlideHierarchyWorkerResult,
    select: fn(&BetaBinomialGroupGenderSlidePatientPosterior) -> &SarScalarSummary,
    minimum_tolerance: f64,
    policy: BetaBinomialGroupGenderSlideAgreementPolicy,
) -> Result<BetaBinomialPatientAgreement, BayesError> {
    let mut squared = 0.0;
    let mut maximum_absolute = 0.0_f64;
    let mut maximum_standardized = 0.0_f64;
    let mut overlaps = true;
    let mut passes = true;
    for (pymc, numpyro) in left.patients.iter().zip(&right.patients) {
        if pymc.patient_id != numpyro.patient_id
            || pymc.group != numpyro.group
            || pymc.gender != numpyro.gender
            || pymc.slide_count != numpyro.slide_count
            || pymc.successes != numpyro.successes
            || pymc.trials != numpyro.trials
        {
            return Err(BayesError::WorkerContract(
                "slide hierarchy backend patient identities differ".into(),
            ));
        }
        let left_summary = select(pymc);
        let right_summary = select(numpyro);
        let difference = (left_summary.mean - right_summary.mean).abs();
        let mcse = (left_summary.sd / left.diagnostics.ess_bulk.sqrt())
            .hypot(right_summary.sd / right.diagnostics.ess_bulk.sqrt());
        let standardized = standardized_mcse_difference(difference, mcse);
        let tolerance = minimum_tolerance.max(policy.maximum_standardized_difference * mcse);
        let overlap = left_summary.interval_lower <= right_summary.interval_upper
            && right_summary.interval_lower <= left_summary.interval_upper;
        squared += difference * difference;
        maximum_absolute = maximum_absolute.max(difference);
        maximum_standardized = maximum_standardized.max(standardized);
        overlaps &= overlap;
        passes &= overlap && difference <= tolerance;
    }
    Ok(BetaBinomialPatientAgreement {
        patient_count: left.patients.len(),
        root_mean_square_difference: (squared / left.patients.len() as f64).sqrt(),
        maximum_absolute_difference: maximum_absolute,
        maximum_standardized_difference: maximum_standardized,
        all_intervals_overlap: overlaps,
        passes,
    })
}
use crate::agreement_metric::standardized_mcse_difference;
