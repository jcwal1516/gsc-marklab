use serde::{Deserialize, Serialize};

use crate::{
    hierarchical_agreement::{JAX_VERSION, NUMPYRO_VERSION},
    sha256_hex, BackendContract, BayesError, BetaBinomialGroupPatientPosterior,
    BetaBinomialGroupPosteriorPredictive, BetaBinomialGroupRegressionInputIdentity,
    BetaBinomialGroupRegressionModelIr, BetaBinomialGroupRegressionPosterior,
    BetaBinomialGroupRegressionWorkerRequest, BetaBinomialGroupRegressionWorkerResult,
    BetaBinomialParameterAgreement, BetaBinomialPatientAgreement, FitState, NormalMeanDiagnostics,
    SamplingSummary, SarScalarSummary, WorkerBackend,
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

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
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
    diagnostics: NormalMeanDiagnostics,
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
    pub intercept_log_odds: BetaBinomialParameterAgreement,
    pub group_log_odds_effect: BetaBinomialParameterAgreement,
    pub reference_probability: BetaBinomialParameterAgreement,
    pub comparison_probability: BetaBinomialParameterAgreement,
    pub probability_difference: BetaBinomialParameterAgreement,
    pub odds_ratio: BetaBinomialParameterAgreement,
    pub concentration: BetaBinomialParameterAgreement,
    pub patient_probabilities: BetaBinomialPatientAgreement,
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
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: BetaBinomialGroupPosteriorPredictive,
}

#[derive(Debug, Serialize)]
pub struct BetaBinomialGroupAgreementResult {
    pub format: &'static str,
    pub version: u32,
    pub model: BetaBinomialGroupRegressionModelIr,
    pub input: BetaBinomialGroupRegressionInputIdentity,
    pub fit_state: FitState,
    pub agreement_status: &'static str,
    pub pymc: BetaBinomialGroupBackendSummary,
    pub numpyro: BetaBinomialGroupBackendSummary,
    pub comparison: BetaBinomialGroupAgreementComparison,
    pub seed: u64,
    pub claim_status: &'static str,
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
        let probability = |left: &SarScalarSummary, right: &SarScalarSummary| {
            compare_parameter(
                left,
                &pymc,
                right,
                &numpyro,
                policy.minimum_probability_tolerance,
                policy,
            )
        };
        let log_odds = |left: &SarScalarSummary, right: &SarScalarSummary| {
            compare_parameter(
                left,
                &pymc,
                right,
                &numpyro,
                policy.minimum_log_odds_tolerance,
                policy,
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
            &pymc,
            &numpyro.posterior.concentration,
            &numpyro,
            policy.minimum_concentration_tolerance,
            policy,
        );
        let patient_probabilities = compare_patients(&pymc, &numpyro, policy)?;
        let fits_complete =
            pymc.fit_state == FitState::Complete && numpyro.fit_state == FitState::Complete;
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
            && patient_probabilities.passes;
        let available = fits_complete && agrees;
        let seed = source_request.sampling.seed;
        Ok(Self {
            format: "marklab.bayesian_beta_binomial_group_cross_backend_agreement",
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
            claim_status: if available {
                "experimental_cross_backend_validation"
            } else {
                "diagnostic_only_cross_backend_validation"
            },
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

fn compare_parameter(
    pymc_summary: &SarScalarSummary,
    pymc: &BetaBinomialGroupRegressionWorkerResult,
    numpyro_summary: &SarScalarSummary,
    numpyro: &BetaBinomialGroupRegressionWorkerResult,
    minimum_tolerance: f64,
    policy: BetaBinomialGroupAgreementPolicy,
) -> BetaBinomialParameterAgreement {
    let absolute_difference = (pymc_summary.mean - numpyro_summary.mean).abs();
    let combined_mcse = (pymc_summary.sd / pymc.diagnostics.ess_bulk.sqrt())
        .hypot(numpyro_summary.sd / numpyro.diagnostics.ess_bulk.sqrt());
    let standardized_difference = standardized(absolute_difference, combined_mcse);
    let tolerance = minimum_tolerance.max(policy.maximum_standardized_difference * combined_mcse);
    let intervals_overlap = pymc_summary.interval_lower <= numpyro_summary.interval_upper
        && numpyro_summary.interval_lower <= pymc_summary.interval_upper;
    BetaBinomialParameterAgreement {
        pymc_mean: pymc_summary.mean,
        numpyro_mean: numpyro_summary.mean,
        absolute_difference,
        combined_mcse,
        standardized_difference,
        tolerance,
        intervals_overlap,
        passes: intervals_overlap && absolute_difference <= tolerance,
    }
}

fn compare_patients(
    pymc: &BetaBinomialGroupRegressionWorkerResult,
    numpyro: &BetaBinomialGroupRegressionWorkerResult,
    policy: BetaBinomialGroupAgreementPolicy,
) -> Result<BetaBinomialPatientAgreement, BayesError> {
    let mut squared = 0.0;
    let mut maximum_absolute = 0.0_f64;
    let mut maximum_standardized = 0.0_f64;
    let mut overlaps = true;
    let mut passes = true;
    for (left, right) in pymc.patients.iter().zip(&numpyro.patients) {
        if left.patient_id != right.patient_id
            || left.group != right.group
            || left.successes != right.successes
            || left.trials != right.trials
        {
            return Err(BayesError::WorkerContract(
                "beta-binomial group backend patient identities differ".into(),
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

fn standardized(difference: f64, mcse: f64) -> f64 {
    if mcse > 0.0 {
        difference / mcse
    } else if difference == 0.0 {
        0.0
    } else {
        f64::INFINITY
    }
}
