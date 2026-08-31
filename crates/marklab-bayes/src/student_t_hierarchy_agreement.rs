use crate::validation::is_lower_hex_sha256 as is_sha256;

use serde::{Deserialize, Serialize};

use crate::{
    hierarchical::{GaussianHierarchyInputIdentity, PartialPoolingSummary, ScalarPosteriorSummary},
    hierarchical_agreement::{JAX_VERSION, NUMPYRO_VERSION},
    sha256_hex, BackendContract, BayesError, FitState, NormalMeanDiagnostics, SamplingSummary,
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
    diagnostics: NormalMeanDiagnostics,
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
pub struct StudentTParameterAgreement {
    pub pymc_mean: f64,
    pub numpyro_mean: f64,
    pub absolute_difference: f64,
    pub combined_mcse: f64,
    pub standardized_difference: f64,
    pub tolerance: f64,
    pub intervals_overlap: bool,
    pub passes: bool,
}

#[derive(Debug, Serialize)]
pub struct StudentTPatientMeanAgreement {
    pub patient_count: usize,
    pub root_mean_square_difference: f64,
    pub maximum_absolute_difference: f64,
    pub maximum_standardized_difference: f64,
    pub all_intervals_overlap: bool,
    pub passes: bool,
}

#[derive(Debug, Serialize)]
pub struct StudentTHierarchyAgreementComparison {
    pub global_mean: StudentTParameterAgreement,
    pub between_patient_sd: StudentTParameterAgreement,
    pub observation_sd: StudentTParameterAgreement,
    pub degrees_of_freedom: StudentTParameterAgreement,
    pub patient_means: StudentTPatientMeanAgreement,
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
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: StudentTHierarchyPosteriorPredictive,
}

#[derive(Debug, Serialize)]
pub struct StudentTHierarchyAgreementResult {
    pub format: &'static str,
    pub version: u32,
    pub model: StudentTHierarchyModelIr,
    pub input: GaussianHierarchyInputIdentity,
    pub fit_state: FitState,
    pub agreement_status: &'static str,
    pub pymc: StudentTHierarchyBackendSummary,
    pub numpyro: StudentTHierarchyBackendSummary,
    pub comparison: StudentTHierarchyAgreementComparison,
    pub seed: u64,
    pub claim_status: &'static str,
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
            pymc.diagnostics.ess_bulk,
            &numpyro.posterior.global_mean,
            numpyro.diagnostics.ess_bulk,
            policy.maximum_standardized_difference,
            policy.minimum_location_scale_tolerance,
        );
        let between_patient_sd = compare_parameter(
            &pymc.posterior.between_patient_sd,
            pymc.diagnostics.ess_bulk,
            &numpyro.posterior.between_patient_sd,
            numpyro.diagnostics.ess_bulk,
            policy.maximum_standardized_difference,
            policy.minimum_location_scale_tolerance,
        );
        let observation_sd = compare_parameter(
            &pymc.posterior.observation_sd,
            pymc.diagnostics.ess_bulk,
            &numpyro.posterior.observation_sd,
            numpyro.diagnostics.ess_bulk,
            policy.maximum_standardized_difference,
            policy.minimum_location_scale_tolerance,
        );
        let degrees_of_freedom = compare_parameter(
            &pymc.posterior.degrees_of_freedom,
            pymc.diagnostics.ess_bulk,
            &numpyro.posterior.degrees_of_freedom,
            numpyro.diagnostics.ess_bulk,
            policy.maximum_standardized_difference,
            policy.minimum_degrees_of_freedom_tolerance,
        );
        let patient_means = compare_patients(
            &pymc.partial_pooling,
            &numpyro.partial_pooling,
            pymc.diagnostics.ess_bulk,
            numpyro.diagnostics.ess_bulk,
            policy,
        );
        let fits_complete =
            pymc.fit_state == FitState::Complete && numpyro.fit_state == FitState::Complete;
        let agrees = global_mean.passes
            && between_patient_sd.passes
            && observation_sd.passes
            && degrees_of_freedom.passes
            && patient_means.passes;
        let available = fits_complete && agrees;
        Ok(Self {
            format: "marklab.bayesian_student_t_cross_backend_agreement",
            version: 1,
            model: source_request.model,
            input,
            fit_state: if fits_complete {
                FitState::Complete
            } else {
                FitState::Nonconverged
            },
            agreement_status: if available {
                "agree_within_monte_carlo_error"
            } else if fits_complete {
                "diagnostic_only_backend_disagreement"
            } else {
                "diagnostic_only_nonconverged"
            },
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
            claim_status: if available {
                "experimental_cross_backend_validation"
            } else {
                "diagnostic_only_cross_backend_validation"
            },
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

fn compare_parameter(
    pymc: &ScalarPosteriorSummary,
    pymc_ess: f64,
    numpyro: &ScalarPosteriorSummary,
    numpyro_ess: f64,
    maximum_standardized_difference: f64,
    minimum_tolerance: f64,
) -> StudentTParameterAgreement {
    let absolute_difference = (pymc.mean - numpyro.mean).abs();
    let combined_mcse = (pymc.sd / pymc_ess.sqrt()).hypot(numpyro.sd / numpyro_ess.sqrt());
    let standardized_difference = standardized(absolute_difference, combined_mcse);
    let tolerance = minimum_tolerance.max(maximum_standardized_difference * combined_mcse);
    let intervals_overlap = pymc.interval_lower <= numpyro.interval_upper
        && numpyro.interval_lower <= pymc.interval_upper;
    StudentTParameterAgreement {
        pymc_mean: pymc.mean,
        numpyro_mean: numpyro.mean,
        absolute_difference,
        combined_mcse,
        standardized_difference,
        tolerance,
        intervals_overlap,
        passes: intervals_overlap && absolute_difference <= tolerance,
    }
}

fn compare_patients(
    pymc: &[PartialPoolingSummary],
    numpyro: &[PartialPoolingSummary],
    pymc_ess: f64,
    numpyro_ess: f64,
    policy: StudentTHierarchyAgreementPolicy,
) -> StudentTPatientMeanAgreement {
    let mut squared = 0.0;
    let mut maximum_absolute = 0.0_f64;
    let mut maximum_standardized = 0.0_f64;
    let mut overlaps = true;
    let mut passes = true;
    for (left, right) in pymc.iter().zip(numpyro) {
        let difference = (left.posterior_mean - right.posterior_mean).abs();
        let mcse =
            (left.posterior_sd / pymc_ess.sqrt()).hypot(right.posterior_sd / numpyro_ess.sqrt());
        let standardized_difference = standardized(difference, mcse);
        let tolerance = policy
            .minimum_patient_tolerance
            .max(policy.maximum_standardized_difference * mcse);
        let overlap = left.interval_lower <= right.interval_upper
            && right.interval_lower <= left.interval_upper;
        squared += difference * difference;
        maximum_absolute = maximum_absolute.max(difference);
        maximum_standardized = maximum_standardized.max(standardized_difference);
        overlaps &= overlap;
        passes &= left.patient_id == right.patient_id && overlap && difference <= tolerance;
    }
    StudentTPatientMeanAgreement {
        patient_count: pymc.len(),
        root_mean_square_difference: (squared / pymc.len() as f64).sqrt(),
        maximum_absolute_difference: maximum_absolute,
        maximum_standardized_difference: maximum_standardized,
        all_intervals_overlap: overlaps,
        passes,
    }
}

use crate::agreement_metric::standardized_mcse_difference as standardized;
