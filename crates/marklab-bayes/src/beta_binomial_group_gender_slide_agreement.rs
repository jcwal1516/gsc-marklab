use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::agreement_metric::AgreementVerdict;
use crate::agreement_metric::{
    compare_elements, compare_parameter, ParameterAgreement, PatientAgreement,
};
use crate::backend_versions::PYTHON_VERSION;
use crate::validation::is_sha256;
use crate::AgreementStatus;
use crate::ClaimStatus;
use crate::{
    hierarchical_agreement::{JAX_VERSION, NUMPYRO_VERSION},
    json_sha256, BackendContract, BayesError, BetaBinomialGroupGenderSlideHierarchyInputIdentity,
    BetaBinomialGroupGenderSlideHierarchyModelIr, BetaBinomialGroupGenderSlideHierarchyPosterior,
    BetaBinomialGroupGenderSlideHierarchyWorkerRequest,
    BetaBinomialGroupGenderSlideHierarchyWorkerResult,
    BetaBinomialGroupGenderSlidePatientPosterior, BetaBinomialGroupGenderSlidePosterior,
    BetaBinomialGroupGenderSlidePosteriorPredictive, FitState, McseScalarSummary,
    PosteriorDiagnostics, SamplingSummary, WorkerBackend,
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
        let source_request_sha256 = json_sha256(&source_request)?;
        Ok(Self {
            format: REQUEST_FORMAT,
            version: 1,
            backend: BackendContract {
                name: "numpyro",
                version: NUMPYRO_VERSION,
                python_version: PYTHON_VERSION,
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
    diagnostics: PosteriorDiagnostics,
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
    pub parameters: BTreeMap<String, ParameterAgreement>,
    pub patient_probabilities: PatientAgreement,
    pub patient_random_effects: PatientAgreement,
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
    pub diagnostics: PosteriorDiagnostics,
    pub posterior_predictive: BetaBinomialGroupGenderSlidePosteriorPredictive,
}

#[derive(Debug, Serialize)]
pub struct BetaBinomialGroupGenderSlideAgreementResult {
    pub format: &'static str,
    pub version: u32,
    pub model: BetaBinomialGroupGenderSlideHierarchyModelIr,
    pub input: BetaBinomialGroupGenderSlideHierarchyInputIdentity,
    pub fit_state: FitState,
    pub agreement_status: AgreementStatus,
    pub pymc: BetaBinomialGroupGenderSlideBackendSummary,
    pub numpyro: BetaBinomialGroupGenderSlideBackendSummary,
    pub comparison: BetaBinomialGroupGenderSlideAgreementComparison,
    pub seed: u64,
    pub claim_status: ClaimStatus,
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
                compare_parameter(
                    left,
                    right,
                    policy.maximum_standardized_difference,
                    tolerance,
                ),
            );
        }
        let patient_probabilities = compare_patients(
            &pymc,
            &numpyro,
            |patient| patient.posterior_probability,
            policy.minimum_patient_probability_tolerance,
            policy,
        )?;
        let patient_random_effects = compare_patients(
            &pymc,
            &numpyro,
            |patient| patient.random_log_odds_effect,
            policy.minimum_patient_effect_tolerance,
            policy,
        )?;
        let agrees = parameters.values().all(|value| value.passes)
            && patient_probabilities.agreement.passes
            && patient_random_effects.agreement.passes;
        let verdict = AgreementVerdict::new(pymc.fit_state, numpyro.fit_state, agrees);
        let seed = source_request.sampling.seed;
        Ok(Self {
            format: "marklab.bayesian_beta_binomial_group_gender_slide_hierarchy_cross_backend_agreement",
            version: 1,
            model: source_request.model,
            input,
            fit_state: verdict.fit_state,
            agreement_status: verdict.agreement_status,
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
            claim_status: verdict.claim_status,
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

fn compare_patients(
    pymc: &BetaBinomialGroupGenderSlideHierarchyWorkerResult,
    numpyro: &BetaBinomialGroupGenderSlideHierarchyWorkerResult,
    select: fn(&BetaBinomialGroupGenderSlidePatientPosterior) -> McseScalarSummary,
    minimum_tolerance: f64,
    policy: BetaBinomialGroupGenderSlideAgreementPolicy,
) -> Result<PatientAgreement, BayesError> {
    let patients = || pymc.patients.iter().zip(&numpyro.patients);
    if patients().any(|(left, right)| {
        left.patient_id != right.patient_id
            || left.group != right.group
            || left.gender != right.gender
            || left.slide_count != right.slide_count
            || left.successes != right.successes
            || left.trials != right.trials
    }) {
        return Err(BayesError::WorkerContract(
            "slide hierarchy backend patient identities differ".into(),
        ));
    }
    Ok(PatientAgreement {
        patient_count: pymc.patients.len(),
        agreement: compare_elements(
            patients().map(|(left, right)| (select(left), select(right))),
            policy.maximum_standardized_difference,
            minimum_tolerance,
        ),
    })
}
