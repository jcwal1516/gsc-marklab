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
    json_sha256, BackendContract, BayesError, BetaBinomialHierarchyInputIdentity,
    BetaBinomialHierarchyModelIr, BetaBinomialHierarchyPosterior,
    BetaBinomialHierarchyWorkerRequest, BetaBinomialHierarchyWorkerResult,
    BetaBinomialPatientPosterior, BetaBinomialPosteriorPredictive, FitState, PosteriorDiagnostics,
    SamplingSummary, WorkerBackend,
};

const REQUEST_FORMAT: &str = "marklab.numpyro_beta_binomial_hierarchy_worker_request";
const RESULT_FORMAT: &str = "marklab.numpyro_beta_binomial_hierarchy_worker_result";

#[derive(Clone, Debug, Serialize)]
pub struct NumpyroBetaBinomialHierarchyWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub jax_version: &'static str,
    pub source_request_sha256: String,
    pub source_request: BetaBinomialHierarchyWorkerRequest,
}

impl NumpyroBetaBinomialHierarchyWorkerRequest {
    pub fn new(
        source_request: BetaBinomialHierarchyWorkerRequest,
        environment_lock_sha256: String,
        worker_sha256: String,
    ) -> Result<Self, BayesError> {
        if !is_sha256(&environment_lock_sha256) || !is_sha256(&worker_sha256) {
            return Err(BayesError::InvalidSpec(
                "NumPyro beta-binomial identities must be SHA-256 digests".into(),
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
pub struct NumpyroBetaBinomialHierarchyWorkerResult {
    format: String,
    version: u32,
    backend: WorkerBackend,
    jax_version: String,
    request_sha256: String,
    source_request_sha256: String,
    fit_state: FitState,
    sampling: SamplingSummary,
    posterior: BetaBinomialHierarchyPosterior,
    patients: Vec<BetaBinomialPatientPosterior>,
    diagnostics: PosteriorDiagnostics,
    posterior_predictive: BetaBinomialPosteriorPredictive,
}

impl NumpyroBetaBinomialHierarchyWorkerResult {
    pub fn into_validated_payload(
        self,
        request: &NumpyroBetaBinomialHierarchyWorkerRequest,
        request_sha256: &str,
    ) -> Result<BetaBinomialHierarchyWorkerResult, BayesError> {
        if self.format != RESULT_FORMAT
            || self.version != 1
            || self.jax_version != JAX_VERSION
            || self.source_request_sha256 != request.source_request_sha256
        {
            return Err(BayesError::WorkerContract(
                "NumPyro beta-binomial result identity mismatch".into(),
            ));
        }
        let payload = BetaBinomialHierarchyWorkerResult {
            format: self.format,
            version: self.version,
            backend: self.backend,
            request_sha256: self.request_sha256,
            fit_state: self.fit_state,
            sampling: self.sampling,
            posterior: self.posterior,
            patients: self.patients,
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
pub struct BetaBinomialAgreementPolicy {
    pub maximum_standardized_difference: f64,
    pub minimum_probability_tolerance: f64,
    pub minimum_concentration_tolerance: f64,
    pub minimum_patient_tolerance: f64,
}

impl BetaBinomialAgreementPolicy {
    fn validate(self) -> Result<Self, BayesError> {
        if !self.maximum_standardized_difference.is_finite()
            || !(1.0..=10.0).contains(&self.maximum_standardized_difference)
            || !self.minimum_probability_tolerance.is_finite()
            || !(0.0..=0.25).contains(&self.minimum_probability_tolerance)
            || !self.minimum_concentration_tolerance.is_finite()
            || self.minimum_concentration_tolerance <= 0.0
            || !self.minimum_patient_tolerance.is_finite()
            || !(0.0..=0.25).contains(&self.minimum_patient_tolerance)
        {
            return Err(BayesError::InvalidSpec(
                "beta-binomial agreement controls are invalid".into(),
            ));
        }
        Ok(self)
    }
}

#[derive(Debug, Serialize)]
pub struct BetaBinomialAgreementComparison {
    pub population_probability: ParameterAgreement,
    pub concentration: ParameterAgreement,
    pub patient_probabilities: PatientAgreement,
    pub maximum_standardized_difference: f64,
    pub minimum_probability_tolerance: f64,
    pub minimum_concentration_tolerance: f64,
    pub minimum_patient_tolerance: f64,
}

#[derive(Debug, Serialize)]
pub struct BetaBinomialBackendSummary {
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: BetaBinomialHierarchyPosterior,
    pub patients: Vec<BetaBinomialPatientPosterior>,
    pub diagnostics: PosteriorDiagnostics,
    pub posterior_predictive: BetaBinomialPosteriorPredictive,
}

#[derive(Debug, Serialize)]
pub struct BetaBinomialAgreementResult {
    pub format: &'static str,
    pub version: u32,
    pub model: BetaBinomialHierarchyModelIr,
    pub input: BetaBinomialHierarchyInputIdentity,
    pub fit_state: FitState,
    pub agreement_status: AgreementStatus,
    pub pymc: BetaBinomialBackendSummary,
    pub numpyro: BetaBinomialBackendSummary,
    pub comparison: BetaBinomialAgreementComparison,
    pub seed: u64,
    pub claim_status: ClaimStatus,
}

impl BetaBinomialAgreementResult {
    pub fn new(
        source_request: BetaBinomialHierarchyWorkerRequest,
        input: BetaBinomialHierarchyInputIdentity,
        pymc: BetaBinomialHierarchyWorkerResult,
        numpyro: BetaBinomialHierarchyWorkerResult,
        policy: BetaBinomialAgreementPolicy,
    ) -> Result<Self, BayesError> {
        let policy = policy.validate()?;
        if pymc.patients.len() != numpyro.patients.len() {
            return Err(BayesError::WorkerContract(
                "beta-binomial backend patient dimensions differ".into(),
            ));
        }
        let population_probability = compare_parameter(
            &pymc.posterior.population_probability,
            &numpyro.posterior.population_probability,
            policy.maximum_standardized_difference,
            policy.minimum_probability_tolerance,
        );
        let concentration = compare_parameter(
            &pymc.posterior.concentration,
            &numpyro.posterior.concentration,
            policy.maximum_standardized_difference,
            policy.minimum_concentration_tolerance,
        );
        let patient_probabilities = compare_patients(&pymc, &numpyro, policy)?;
        let agrees = population_probability.passes
            && concentration.passes
            && patient_probabilities.agreement.passes;
        let verdict = AgreementVerdict::new(pymc.fit_state, numpyro.fit_state, agrees);
        let seed = source_request.sampling.seed;
        Ok(Self {
            format: "marklab.bayesian_beta_binomial_cross_backend_agreement",
            version: 1,
            model: source_request.model,
            input,
            fit_state: verdict.fit_state,
            agreement_status: verdict.agreement_status,
            pymc: summarize(pymc),
            numpyro: summarize(numpyro),
            comparison: BetaBinomialAgreementComparison {
                population_probability,
                concentration,
                patient_probabilities,
                maximum_standardized_difference: policy.maximum_standardized_difference,
                minimum_probability_tolerance: policy.minimum_probability_tolerance,
                minimum_concentration_tolerance: policy.minimum_concentration_tolerance,
                minimum_patient_tolerance: policy.minimum_patient_tolerance,
            },
            seed,
            claim_status: verdict.claim_status,
        })
    }
}

fn summarize(result: BetaBinomialHierarchyWorkerResult) -> BetaBinomialBackendSummary {
    BetaBinomialBackendSummary {
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
    pymc: &BetaBinomialHierarchyWorkerResult,
    numpyro: &BetaBinomialHierarchyWorkerResult,
    policy: BetaBinomialAgreementPolicy,
) -> Result<PatientAgreement, BayesError> {
    let patients = || pymc.patients.iter().zip(&numpyro.patients);
    if patients().any(|(left, right)| {
        left.patient_id != right.patient_id
            || left.successes != right.successes
            || left.trials != right.trials
    }) {
        return Err(BayesError::WorkerContract(
            "beta-binomial backend patient identities differ".into(),
        ));
    }
    Ok(PatientAgreement {
        patient_count: pymc.patients.len(),
        agreement: compare_elements(
            patients()
                .map(|(left, right)| (left.posterior_probability, right.posterior_probability)),
            policy.maximum_standardized_difference,
            policy.minimum_patient_tolerance,
        ),
    })
}
