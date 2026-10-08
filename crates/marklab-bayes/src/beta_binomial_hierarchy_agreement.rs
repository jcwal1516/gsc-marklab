use crate::validation::is_lower_hex_sha256 as is_sha256;

use serde::{Deserialize, Serialize};

use crate::{
    hierarchical_agreement::{JAX_VERSION, NUMPYRO_VERSION},
    sha256_hex, BackendContract, BayesError, BetaBinomialHierarchyInputIdentity,
    BetaBinomialHierarchyModelIr, BetaBinomialHierarchyPosterior,
    BetaBinomialHierarchyWorkerRequest, BetaBinomialHierarchyWorkerResult,
    BetaBinomialPatientPosterior, BetaBinomialPosteriorPredictive, FitState, NormalMeanDiagnostics,
    SamplingSummary, SarScalarSummary, WorkerBackend,
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
    diagnostics: NormalMeanDiagnostics,
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
pub struct BetaBinomialParameterAgreement {
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
pub struct BetaBinomialPatientAgreement {
    pub patient_count: usize,
    pub root_mean_square_difference: f64,
    pub maximum_absolute_difference: f64,
    pub maximum_standardized_difference: f64,
    pub all_intervals_overlap: bool,
    pub passes: bool,
}

#[derive(Debug, Serialize)]
pub struct BetaBinomialAgreementComparison {
    pub population_probability: BetaBinomialParameterAgreement,
    pub concentration: BetaBinomialParameterAgreement,
    pub patient_probabilities: BetaBinomialPatientAgreement,
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
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: BetaBinomialPosteriorPredictive,
}

#[derive(Debug, Serialize)]
pub struct BetaBinomialAgreementResult {
    pub format: &'static str,
    pub version: u32,
    pub model: BetaBinomialHierarchyModelIr,
    pub input: BetaBinomialHierarchyInputIdentity,
    pub fit_state: FitState,
    pub agreement_status: &'static str,
    pub pymc: BetaBinomialBackendSummary,
    pub numpyro: BetaBinomialBackendSummary,
    pub comparison: BetaBinomialAgreementComparison,
    pub seed: u64,
    pub claim_status: &'static str,
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
            pymc.diagnostics.ess_bulk,
            &numpyro.posterior.population_probability,
            numpyro.diagnostics.ess_bulk,
            policy.maximum_standardized_difference,
            policy.minimum_probability_tolerance,
        );
        let concentration = compare_parameter(
            &pymc.posterior.concentration,
            pymc.diagnostics.ess_bulk,
            &numpyro.posterior.concentration,
            numpyro.diagnostics.ess_bulk,
            policy.maximum_standardized_difference,
            policy.minimum_concentration_tolerance,
        );
        let patient_probabilities = compare_patients(&pymc, &numpyro, policy)?;
        let fits_complete =
            pymc.fit_state == FitState::Complete && numpyro.fit_state == FitState::Complete;
        let agrees =
            population_probability.passes && concentration.passes && patient_probabilities.passes;
        let available = fits_complete && agrees;
        let seed = source_request.sampling.seed;
        Ok(Self {
            format: "marklab.bayesian_beta_binomial_cross_backend_agreement",
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
            claim_status: if available {
                "experimental_cross_backend_validation"
            } else {
                "diagnostic_only_cross_backend_validation"
            },
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

fn compare_parameter(
    pymc: &SarScalarSummary,
    pymc_ess: f64,
    numpyro: &SarScalarSummary,
    numpyro_ess: f64,
    maximum_standardized_difference: f64,
    minimum_tolerance: f64,
) -> BetaBinomialParameterAgreement {
    compare_beta_binomial_parameter(
        pymc,
        pymc_ess,
        numpyro,
        numpyro_ess,
        maximum_standardized_difference,
        minimum_tolerance,
    )
}

fn compare_patients(
    pymc: &BetaBinomialHierarchyWorkerResult,
    numpyro: &BetaBinomialHierarchyWorkerResult,
    policy: BetaBinomialAgreementPolicy,
) -> Result<BetaBinomialPatientAgreement, BayesError> {
    let mut squared = 0.0;
    let mut maximum_absolute = 0.0_f64;
    let mut maximum_standardized = 0.0_f64;
    let mut overlaps = true;
    let mut passes = true;
    for (left, right) in pymc.patients.iter().zip(&numpyro.patients) {
        if left.patient_id != right.patient_id
            || left.successes != right.successes
            || left.trials != right.trials
        {
            return Err(BayesError::WorkerContract(
                "beta-binomial backend patient identities differ".into(),
            ));
        }
        let difference = (left.posterior_probability.mean - right.posterior_probability.mean).abs();
        let mcse = (left.posterior_probability.sd / pymc.diagnostics.ess_bulk.sqrt())
            .hypot(right.posterior_probability.sd / numpyro.diagnostics.ess_bulk.sqrt());
        let standardized_difference = standardized(difference, mcse);
        let tolerance = policy
            .minimum_patient_tolerance
            .max(policy.maximum_standardized_difference * mcse);
        let overlap = left.posterior_probability.interval_lower
            <= right.posterior_probability.interval_upper
            && right.posterior_probability.interval_lower
                <= left.posterior_probability.interval_upper;
        squared += difference * difference;
        maximum_absolute = maximum_absolute.max(difference);
        maximum_standardized = maximum_standardized.max(standardized_difference);
        overlaps &= overlap;
        passes &= overlap && difference <= tolerance;
    }
    Ok(BetaBinomialPatientAgreement {
        patient_count: pymc.patients.len(),
        root_mean_square_difference: (squared / pymc.patients.len() as f64).sqrt(),
        maximum_absolute_difference: maximum_absolute,
        maximum_standardized_difference: maximum_standardized,
        all_intervals_overlap: overlaps,
        passes,
    })
}

use crate::agreement_metric::{
    compare_beta_binomial_parameter, standardized_mcse_difference as standardized,
};
