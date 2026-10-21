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
    hierarchical::{GaussianHierarchyInputIdentity, PartialPoolingSummary},
    hierarchical_agreement::{JAX_VERSION, NUMPYRO_VERSION},
    json_sha256, BackendContract, BayesError, FitState, PosteriorDiagnostics, SamplingSummary,
    StudentTHierarchyModelIr, StudentTHierarchyPosterior, StudentTHierarchyPosteriorPredictive,
    StudentTHierarchyWorkerRequest, StudentTHierarchyWorkerResult, WorkerBackend,
};

const REQUEST_FORMAT: &str = "marklab.numpyro_student_t_hierarchy_worker_request";
const RESULT_FORMAT: &str = "marklab.numpyro_student_t_hierarchy_worker_result";

#[derive(Clone, Debug, Serialize)]
pub struct NumpyroStudentTHierarchyWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub jax_version: &'static str,
    pub source_request_sha256: String,
    pub source_request: StudentTHierarchyWorkerRequest,
}

impl NumpyroStudentTHierarchyWorkerRequest {
    pub fn new(
        source_request: StudentTHierarchyWorkerRequest,
        environment_lock_sha256: String,
        worker_sha256: String,
    ) -> Result<Self, BayesError> {
        if !is_sha256(&environment_lock_sha256) || !is_sha256(&worker_sha256) {
            return Err(BayesError::InvalidSpec(
                "NumPyro Student-t identities must be SHA-256 digests".into(),
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
pub struct NumpyroStudentTHierarchyWorkerResult {
    format: String,
    version: u32,
    backend: WorkerBackend,
    jax_version: String,
    request_sha256: String,
    source_request_sha256: String,
    fit_state: FitState,
    sampling: SamplingSummary,
    posterior: StudentTHierarchyPosterior,
    partial_pooling: Vec<PartialPoolingSummary>,
    diagnostics: PosteriorDiagnostics,
    posterior_predictive: StudentTHierarchyPosteriorPredictive,
}

impl NumpyroStudentTHierarchyWorkerResult {
    pub fn into_validated_payload(
        self,
        request: &NumpyroStudentTHierarchyWorkerRequest,
        request_sha256: &str,
    ) -> Result<StudentTHierarchyWorkerResult, BayesError> {
        if self.format != RESULT_FORMAT
            || self.version != 1
            || self.jax_version != JAX_VERSION
            || self.source_request_sha256 != request.source_request_sha256
        {
            return Err(BayesError::WorkerContract(
                "NumPyro Student-t result identity mismatch".into(),
            ));
        }
        let payload = StudentTHierarchyWorkerResult {
            format: self.format,
            version: self.version,
            backend: self.backend,
            request_sha256: self.request_sha256,
            fit_state: self.fit_state,
            sampling: self.sampling,
            posterior: self.posterior,
            partial_pooling: self.partial_pooling,
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
pub struct StudentTHierarchyAgreementPolicy {
    pub maximum_standardized_difference: f64,
    pub minimum_location_scale_tolerance: f64,
    pub minimum_degrees_of_freedom_tolerance: f64,
    pub minimum_patient_tolerance: f64,
}

impl StudentTHierarchyAgreementPolicy {
    fn validate(self) -> Result<Self, BayesError> {
        if !self.maximum_standardized_difference.is_finite()
            || !(1.0..=10.0).contains(&self.maximum_standardized_difference)
            || [
                self.minimum_location_scale_tolerance,
                self.minimum_degrees_of_freedom_tolerance,
                self.minimum_patient_tolerance,
            ]
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
        {
            return Err(BayesError::InvalidSpec(
                "Student-t agreement controls are invalid".into(),
            ));
        }
        Ok(self)
    }
}

#[derive(Debug, Serialize)]
pub struct StudentTHierarchyAgreementComparison {
    pub global_mean: ParameterAgreement,
    pub between_patient_sd: ParameterAgreement,
    pub observation_sd: ParameterAgreement,
    pub degrees_of_freedom: ParameterAgreement,
    pub patient_means: PatientAgreement,
    pub maximum_standardized_difference: f64,
    pub minimum_location_scale_tolerance: f64,
    pub minimum_degrees_of_freedom_tolerance: f64,
    pub minimum_patient_tolerance: f64,
}

#[derive(Debug, Serialize)]
pub struct StudentTHierarchyBackendSummary {
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: StudentTHierarchyPosterior,
    pub partial_pooling: Vec<PartialPoolingSummary>,
    pub diagnostics: PosteriorDiagnostics,
    pub posterior_predictive: StudentTHierarchyPosteriorPredictive,
}

#[derive(Debug, Serialize)]
pub struct StudentTHierarchyAgreementResult {
    pub format: &'static str,
    pub version: u32,
    pub model: StudentTHierarchyModelIr,
    pub input: GaussianHierarchyInputIdentity,
    pub fit_state: FitState,
    pub agreement_status: AgreementStatus,
    pub pymc: StudentTHierarchyBackendSummary,
    pub numpyro: StudentTHierarchyBackendSummary,
    pub comparison: StudentTHierarchyAgreementComparison,
    pub seed: u64,
    pub claim_status: ClaimStatus,
}

impl StudentTHierarchyAgreementResult {
    pub fn new(
        source_request: StudentTHierarchyWorkerRequest,
        input: GaussianHierarchyInputIdentity,
        pymc: StudentTHierarchyWorkerResult,
        numpyro: StudentTHierarchyWorkerResult,
        policy: StudentTHierarchyAgreementPolicy,
    ) -> Result<Self, BayesError> {
        let policy = policy.validate()?;
        if pymc.partial_pooling.len() != numpyro.partial_pooling.len() {
            return Err(BayesError::WorkerContract(
                "Student-t backend patient dimensions differ".into(),
            ));
        }
        let global_mean = compare_parameter(
            &pymc.posterior.global_mean,
            &numpyro.posterior.global_mean,
            policy.maximum_standardized_difference,
            policy.minimum_location_scale_tolerance,
        );
        let between_patient_sd = compare_parameter(
            &pymc.posterior.between_patient_sd,
            &numpyro.posterior.between_patient_sd,
            policy.maximum_standardized_difference,
            policy.minimum_location_scale_tolerance,
        );
        let observation_sd = compare_parameter(
            &pymc.posterior.observation_sd,
            &numpyro.posterior.observation_sd,
            policy.maximum_standardized_difference,
            policy.minimum_location_scale_tolerance,
        );
        let degrees_of_freedom = compare_parameter(
            &pymc.posterior.degrees_of_freedom,
            &numpyro.posterior.degrees_of_freedom,
            policy.maximum_standardized_difference,
            policy.minimum_degrees_of_freedom_tolerance,
        );
        let patient_means =
            compare_patients(&pymc.partial_pooling, &numpyro.partial_pooling, policy);
        let agrees = global_mean.passes
            && between_patient_sd.passes
            && observation_sd.passes
            && degrees_of_freedom.passes
            && patient_means.agreement.passes;
        let verdict = AgreementVerdict::new(pymc.fit_state, numpyro.fit_state, agrees);
        Ok(Self {
            format: "marklab.bayesian_student_t_cross_backend_agreement",
            version: 1,
            model: source_request.model,
            input,
            fit_state: verdict.fit_state,
            agreement_status: verdict.agreement_status,
            comparison: StudentTHierarchyAgreementComparison {
                global_mean,
                between_patient_sd,
                observation_sd,
                degrees_of_freedom,
                patient_means,
                maximum_standardized_difference: policy.maximum_standardized_difference,
                minimum_location_scale_tolerance: policy.minimum_location_scale_tolerance,
                minimum_degrees_of_freedom_tolerance: policy.minimum_degrees_of_freedom_tolerance,
                minimum_patient_tolerance: policy.minimum_patient_tolerance,
            },
            pymc: summarize(pymc),
            numpyro: summarize(numpyro),
            seed: source_request.sampling.seed,
            claim_status: verdict.claim_status,
        })
    }
}

fn summarize(result: StudentTHierarchyWorkerResult) -> StudentTHierarchyBackendSummary {
    StudentTHierarchyBackendSummary {
        backend: result.backend,
        request_sha256: result.request_sha256,
        fit_state: result.fit_state,
        sampling: result.sampling,
        posterior: result.posterior,
        partial_pooling: result.partial_pooling,
        diagnostics: result.diagnostics,
        posterior_predictive: result.posterior_predictive,
    }
}

fn compare_patients(
    pymc: &[PartialPoolingSummary],
    numpyro: &[PartialPoolingSummary],
    policy: StudentTHierarchyAgreementPolicy,
) -> PatientAgreement {
    let identities_match = pymc
        .iter()
        .zip(numpyro)
        .all(|(left, right)| left.patient_id == right.patient_id);
    let mut agreement = compare_elements(
        pymc.iter()
            .zip(numpyro)
            .map(|(left, right)| (left.posterior(), right.posterior())),
        policy.maximum_standardized_difference,
        policy.minimum_patient_tolerance,
    );
    agreement.passes &= identities_match;
    PatientAgreement {
        patient_count: pymc.len(),
        agreement,
    }
}
