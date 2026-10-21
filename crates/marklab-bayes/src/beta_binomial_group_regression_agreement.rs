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
    json_sha256, BackendContract, BayesError, BetaBinomialGroupPatientPosterior,
    BetaBinomialGroupPosteriorPredictive, BetaBinomialGroupRegressionInputIdentity,
    BetaBinomialGroupRegressionModelIr, BetaBinomialGroupRegressionPosterior,
    BetaBinomialGroupRegressionWorkerRequest, BetaBinomialGroupRegressionWorkerResult, FitState,
    McseScalarSummary, PosteriorDiagnostics, SamplingSummary, WorkerBackend,
};

const REQUEST_FORMAT: &str = "marklab.numpyro_beta_binomial_group_regression_worker_request";
const RESULT_FORMAT: &str = "marklab.numpyro_beta_binomial_group_regression_worker_result";

#[derive(Clone, Debug, Serialize)]
pub struct NumpyroBetaBinomialGroupRegressionWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub jax_version: &'static str,
    pub source_request_sha256: String,
    pub source_request: BetaBinomialGroupRegressionWorkerRequest,
}

impl NumpyroBetaBinomialGroupRegressionWorkerRequest {
    pub fn new(
        source_request: BetaBinomialGroupRegressionWorkerRequest,
        environment_lock_sha256: String,
        worker_sha256: String,
    ) -> Result<Self, BayesError> {
        if !is_sha256(&environment_lock_sha256) || !is_sha256(&worker_sha256) {
            return Err(BayesError::InvalidSpec(
                "NumPyro beta-binomial group identities must be SHA-256 digests".into(),
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
pub struct NumpyroBetaBinomialGroupRegressionWorkerResult {
    format: String,
    version: u32,
    backend: WorkerBackend,
    jax_version: String,
    request_sha256: String,
    source_request_sha256: String,
    fit_state: FitState,
    sampling: SamplingSummary,
    posterior: BetaBinomialGroupRegressionPosterior,
    patients: Vec<BetaBinomialGroupPatientPosterior>,
    diagnostics: PosteriorDiagnostics,
    posterior_predictive: BetaBinomialGroupPosteriorPredictive,
}

impl NumpyroBetaBinomialGroupRegressionWorkerResult {
    pub fn into_validated_payload(
        self,
        request: &NumpyroBetaBinomialGroupRegressionWorkerRequest,
        request_sha256: &str,
    ) -> Result<BetaBinomialGroupRegressionWorkerResult, BayesError> {
        if self.format != RESULT_FORMAT
            || self.version != 1
            || self.jax_version != JAX_VERSION
            || self.source_request_sha256 != request.source_request_sha256
        {
            return Err(BayesError::WorkerContract(
                "NumPyro beta-binomial group result identity mismatch".into(),
            ));
        }
        let payload = BetaBinomialGroupRegressionWorkerResult {
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
pub struct BetaBinomialGroupAgreementPolicy {
    pub maximum_standardized_difference: f64,
    pub minimum_probability_tolerance: f64,
    pub minimum_log_odds_tolerance: f64,
    pub minimum_concentration_tolerance: f64,
    pub minimum_patient_tolerance: f64,
}

impl BetaBinomialGroupAgreementPolicy {
    fn validate(self) -> Result<Self, BayesError> {
        if !self.maximum_standardized_difference.is_finite()
            || !(1.0..=10.0).contains(&self.maximum_standardized_difference)
            || !self.minimum_probability_tolerance.is_finite()
            || !(0.0..=0.25).contains(&self.minimum_probability_tolerance)
            || !self.minimum_log_odds_tolerance.is_finite()
            || !(0.0..=1.0).contains(&self.minimum_log_odds_tolerance)
            || !self.minimum_concentration_tolerance.is_finite()
            || self.minimum_concentration_tolerance <= 0.0
            || !self.minimum_patient_tolerance.is_finite()
            || !(0.0..=0.25).contains(&self.minimum_patient_tolerance)
        {
            return Err(BayesError::InvalidSpec(
                "beta-binomial group agreement controls are invalid".into(),
            ));
        }
        Ok(self)
    }
}

#[derive(Debug, Serialize)]
pub struct BetaBinomialGroupAgreementComparison {
    pub intercept_log_odds: ParameterAgreement,
    pub group_log_odds_effect: ParameterAgreement,
    pub reference_probability: ParameterAgreement,
    pub comparison_probability: ParameterAgreement,
    pub probability_difference: ParameterAgreement,
    pub odds_ratio: ParameterAgreement,
    pub concentration: ParameterAgreement,
    pub patient_probabilities: PatientAgreement,
    pub maximum_standardized_difference: f64,
    pub minimum_probability_tolerance: f64,
    pub minimum_log_odds_tolerance: f64,
    pub minimum_concentration_tolerance: f64,
    pub minimum_patient_tolerance: f64,
}

#[derive(Debug, Serialize)]
pub struct BetaBinomialGroupBackendSummary {
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: BetaBinomialGroupRegressionPosterior,
    pub patients: Vec<BetaBinomialGroupPatientPosterior>,
    pub diagnostics: PosteriorDiagnostics,
    pub posterior_predictive: BetaBinomialGroupPosteriorPredictive,
}

#[derive(Debug, Serialize)]
pub struct BetaBinomialGroupAgreementResult {
    pub format: &'static str,
    pub version: u32,
    pub model: BetaBinomialGroupRegressionModelIr,
    pub input: BetaBinomialGroupRegressionInputIdentity,
    pub fit_state: FitState,
    pub agreement_status: AgreementStatus,
    pub pymc: BetaBinomialGroupBackendSummary,
    pub numpyro: BetaBinomialGroupBackendSummary,
    pub comparison: BetaBinomialGroupAgreementComparison,
    pub seed: u64,
    pub claim_status: ClaimStatus,
}

impl BetaBinomialGroupAgreementResult {
    pub fn new(
        source_request: BetaBinomialGroupRegressionWorkerRequest,
        input: BetaBinomialGroupRegressionInputIdentity,
        pymc: BetaBinomialGroupRegressionWorkerResult,
        numpyro: BetaBinomialGroupRegressionWorkerResult,
        policy: BetaBinomialGroupAgreementPolicy,
    ) -> Result<Self, BayesError> {
        let policy = policy.validate()?;
        if pymc.patients.len() != numpyro.patients.len() {
            return Err(BayesError::WorkerContract(
                "beta-binomial group backend patient dimensions differ".into(),
            ));
        }
        let probability = |left: &McseScalarSummary, right: &McseScalarSummary| {
            compare_parameter(
                left,
                right,
                policy.maximum_standardized_difference,
                policy.minimum_probability_tolerance,
            )
        };
        let log_odds = |left: &McseScalarSummary, right: &McseScalarSummary| {
            compare_parameter(
                left,
                right,
                policy.maximum_standardized_difference,
                policy.minimum_log_odds_tolerance,
            )
        };
        let intercept_log_odds = log_odds(
            &pymc.posterior.intercept_log_odds,
            &numpyro.posterior.intercept_log_odds,
        );
        let group_log_odds_effect = log_odds(
            &pymc.posterior.group_log_odds_effect,
            &numpyro.posterior.group_log_odds_effect,
        );
        let reference_probability = probability(
            &pymc.posterior.reference_probability,
            &numpyro.posterior.reference_probability,
        );
        let comparison_probability = probability(
            &pymc.posterior.comparison_probability,
            &numpyro.posterior.comparison_probability,
        );
        let probability_difference = probability(
            &pymc
                .posterior
                .probability_difference_comparison_minus_reference,
            &numpyro
                .posterior
                .probability_difference_comparison_minus_reference,
        );
        let odds_ratio = log_odds(&pymc.posterior.odds_ratio, &numpyro.posterior.odds_ratio);
        let concentration = compare_parameter(
            &pymc.posterior.concentration,
            &numpyro.posterior.concentration,
            policy.maximum_standardized_difference,
            policy.minimum_concentration_tolerance,
        );
        let patient_probabilities = compare_patients(&pymc, &numpyro, policy)?;
        let agrees = [
            &intercept_log_odds,
            &group_log_odds_effect,
            &reference_probability,
            &comparison_probability,
            &probability_difference,
            &odds_ratio,
            &concentration,
        ]
        .iter()
        .all(|comparison| comparison.passes)
            && patient_probabilities.agreement.passes;
        let verdict = AgreementVerdict::new(pymc.fit_state, numpyro.fit_state, agrees);
        let seed = source_request.sampling.seed;
        Ok(Self {
            format: "marklab.bayesian_beta_binomial_group_cross_backend_agreement",
            version: 1,
            model: source_request.model,
            input,
            fit_state: verdict.fit_state,
            agreement_status: verdict.agreement_status,
            pymc: summarize(pymc),
            numpyro: summarize(numpyro),
            comparison: BetaBinomialGroupAgreementComparison {
                intercept_log_odds,
                group_log_odds_effect,
                reference_probability,
                comparison_probability,
                probability_difference,
                odds_ratio,
                concentration,
                patient_probabilities,
                maximum_standardized_difference: policy.maximum_standardized_difference,
                minimum_probability_tolerance: policy.minimum_probability_tolerance,
                minimum_log_odds_tolerance: policy.minimum_log_odds_tolerance,
                minimum_concentration_tolerance: policy.minimum_concentration_tolerance,
                minimum_patient_tolerance: policy.minimum_patient_tolerance,
            },
            seed,
            claim_status: verdict.claim_status,
        })
    }
}

fn summarize(result: BetaBinomialGroupRegressionWorkerResult) -> BetaBinomialGroupBackendSummary {
    BetaBinomialGroupBackendSummary {
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
    pymc: &BetaBinomialGroupRegressionWorkerResult,
    numpyro: &BetaBinomialGroupRegressionWorkerResult,
    policy: BetaBinomialGroupAgreementPolicy,
) -> Result<PatientAgreement, BayesError> {
    let patients = || pymc.patients.iter().zip(&numpyro.patients);
    if patients().any(|(left, right)| {
        left.patient_id != right.patient_id
            || left.group != right.group
            || left.successes != right.successes
            || left.trials != right.trials
    }) {
        return Err(BayesError::WorkerContract(
            "beta-binomial group backend patient identities differ".into(),
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
