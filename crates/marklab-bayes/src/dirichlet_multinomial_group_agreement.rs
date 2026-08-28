use serde::{Deserialize, Serialize};

use crate::{
    hierarchical_agreement::{JAX_VERSION, NUMPYRO_VERSION},
    sha256_hex, BackendContract, BayesError, BetaBinomialParameterAgreement,
    DirichletMultinomialClassPosterior, DirichletMultinomialGroupInputIdentity,
    DirichletMultinomialGroupModelIr, DirichletMultinomialGroupPosterior,
    DirichletMultinomialGroupPosteriorPredictive, DirichletMultinomialGroupWorkerRequest,
    DirichletMultinomialGroupWorkerResult, FitState, NormalMeanDiagnostics, SamplingSummary,
    SarScalarSummary, WorkerBackend,
};

const REQUEST_FORMAT: &str = "marklab.numpyro_dirichlet_multinomial_group_worker_request";
const RESULT_FORMAT: &str = "marklab.numpyro_dirichlet_multinomial_group_worker_result";

#[derive(Clone, Debug, Serialize)]
pub struct NumpyroDirichletMultinomialGroupWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub jax_version: &'static str,
    pub source_request_sha256: String,
    pub source_request: DirichletMultinomialGroupWorkerRequest,
}

impl NumpyroDirichletMultinomialGroupWorkerRequest {
    pub fn new(
        source_request: DirichletMultinomialGroupWorkerRequest,
        environment_lock_sha256: String,
        worker_sha256: String,
    ) -> Result<Self, BayesError> {
        if !is_sha256(&environment_lock_sha256) || !is_sha256(&worker_sha256) {
            return Err(BayesError::InvalidSpec(
                "NumPyro Dirichlet-multinomial identities must be SHA-256 digests".into(),
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
pub struct NumpyroDirichletMultinomialGroupWorkerResult {
    format: String,
    version: u32,
    backend: WorkerBackend,
    jax_version: String,
    request_sha256: String,
    source_request_sha256: String,
    fit_state: FitState,
    sampling: SamplingSummary,
    posterior: DirichletMultinomialGroupPosterior,
    diagnostics: NormalMeanDiagnostics,
    posterior_predictive: DirichletMultinomialGroupPosteriorPredictive,
}

impl NumpyroDirichletMultinomialGroupWorkerResult {
    pub fn into_validated_payload(
        self,
        request: &NumpyroDirichletMultinomialGroupWorkerRequest,
        request_sha256: &str,
    ) -> Result<DirichletMultinomialGroupWorkerResult, BayesError> {
        if self.format != RESULT_FORMAT
            || self.version != 1
            || self.jax_version != JAX_VERSION
            || self.source_request_sha256 != request.source_request_sha256
        {
            return Err(BayesError::WorkerContract(
                "NumPyro Dirichlet-multinomial result identity mismatch".into(),
            ));
        }
        let payload = DirichletMultinomialGroupWorkerResult {
            format: self.format,
            version: self.version,
            backend: self.backend,
            request_sha256: self.request_sha256,
            fit_state: self.fit_state,
            sampling: self.sampling,
            posterior: self.posterior,
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
pub struct DirichletMultinomialGroupAgreementPolicy {
    pub maximum_standardized_difference: f64,
    pub minimum_probability_tolerance: f64,
    pub minimum_concentration_tolerance: f64,
}

impl DirichletMultinomialGroupAgreementPolicy {
    fn validate(self) -> Result<Self, BayesError> {
        if !self.maximum_standardized_difference.is_finite()
            || !(1.0..=10.0).contains(&self.maximum_standardized_difference)
            || !self.minimum_probability_tolerance.is_finite()
            || !(0.0..=0.25).contains(&self.minimum_probability_tolerance)
            || !self.minimum_concentration_tolerance.is_finite()
            || self.minimum_concentration_tolerance <= 0.0
        {
            return Err(BayesError::InvalidSpec(
                "Dirichlet-multinomial agreement controls are invalid".into(),
            ));
        }
        Ok(self)
    }
}

#[derive(Debug, Serialize)]
pub struct DirichletMultinomialClassAgreement {
    pub class_id: String,
    pub reference_probability: BetaBinomialParameterAgreement,
    pub comparison_probability: BetaBinomialParameterAgreement,
    pub difference_comparison_minus_reference: BetaBinomialParameterAgreement,
}

#[derive(Debug, Serialize)]
pub struct DirichletMultinomialGroupAgreementComparison {
    pub classes: Vec<DirichletMultinomialClassAgreement>,
    pub concentration: BetaBinomialParameterAgreement,
    pub maximum_standardized_difference: f64,
    pub minimum_probability_tolerance: f64,
    pub minimum_concentration_tolerance: f64,
}

#[derive(Debug, Serialize)]
pub struct DirichletMultinomialGroupBackendSummary {
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: DirichletMultinomialGroupPosterior,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: DirichletMultinomialGroupPosteriorPredictive,
}

#[derive(Debug, Serialize)]
pub struct DirichletMultinomialGroupAgreementResult {
    pub format: &'static str,
    pub version: u32,
    pub model: DirichletMultinomialGroupModelIr,
    pub input: DirichletMultinomialGroupInputIdentity,
    pub fit_state: FitState,
    pub agreement_status: &'static str,
    pub pymc: DirichletMultinomialGroupBackendSummary,
    pub numpyro: DirichletMultinomialGroupBackendSummary,
    pub comparison: DirichletMultinomialGroupAgreementComparison,
    pub seed: u64,
    pub claim_status: &'static str,
}

impl DirichletMultinomialGroupAgreementResult {
    pub fn new(
        source_request: DirichletMultinomialGroupWorkerRequest,
        input: DirichletMultinomialGroupInputIdentity,
        pymc: DirichletMultinomialGroupWorkerResult,
        numpyro: DirichletMultinomialGroupWorkerResult,
        policy: DirichletMultinomialGroupAgreementPolicy,
    ) -> Result<Self, BayesError> {
        let policy = policy.validate()?;
        if pymc.posterior.classes.len() != numpyro.posterior.classes.len() {
            return Err(BayesError::WorkerContract(
                "Dirichlet-multinomial backend class dimensions differ".into(),
            ));
        }
        let classes = pymc
            .posterior
            .classes
            .iter()
            .zip(&numpyro.posterior.classes)
            .map(|(left, right)| compare_class(left, &pymc, right, &numpyro, policy))
            .collect::<Result<Vec<_>, _>>()?;
        let concentration = compare_parameter(
            &pymc.posterior.concentration,
            &pymc,
            &numpyro.posterior.concentration,
            &numpyro,
            policy.minimum_concentration_tolerance,
            policy,
        );
        let fits_complete =
            pymc.fit_state == FitState::Complete && numpyro.fit_state == FitState::Complete;
        let agrees = classes.iter().all(|class| {
            class.reference_probability.passes
                && class.comparison_probability.passes
                && class.difference_comparison_minus_reference.passes
        }) && concentration.passes;
        let available = fits_complete && agrees;
        let seed = source_request.sampling.seed;
        Ok(Self {
            format: "marklab.bayesian_dirichlet_multinomial_group_cross_backend_agreement",
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
            comparison: DirichletMultinomialGroupAgreementComparison {
                classes,
                concentration,
                maximum_standardized_difference: policy.maximum_standardized_difference,
                minimum_probability_tolerance: policy.minimum_probability_tolerance,
                minimum_concentration_tolerance: policy.minimum_concentration_tolerance,
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

fn compare_class(
    pymc: &DirichletMultinomialClassPosterior,
    pymc_result: &DirichletMultinomialGroupWorkerResult,
    numpyro: &DirichletMultinomialClassPosterior,
    numpyro_result: &DirichletMultinomialGroupWorkerResult,
    policy: DirichletMultinomialGroupAgreementPolicy,
) -> Result<DirichletMultinomialClassAgreement, BayesError> {
    if pymc.class_id != numpyro.class_id {
        return Err(BayesError::WorkerContract(
            "Dirichlet-multinomial backend class identities differ".into(),
        ));
    }
    let compare = |left: &SarScalarSummary, right: &SarScalarSummary| {
        compare_parameter(
            left,
            pymc_result,
            right,
            numpyro_result,
            policy.minimum_probability_tolerance,
            policy,
        )
    };
    Ok(DirichletMultinomialClassAgreement {
        class_id: pymc.class_id.clone(),
        reference_probability: compare(&pymc.reference_probability, &numpyro.reference_probability),
        comparison_probability: compare(
            &pymc.comparison_probability,
            &numpyro.comparison_probability,
        ),
        difference_comparison_minus_reference: compare(
            &pymc.difference_comparison_minus_reference,
            &numpyro.difference_comparison_minus_reference,
        ),
    })
}

fn compare_parameter(
    pymc: &SarScalarSummary,
    pymc_result: &DirichletMultinomialGroupWorkerResult,
    numpyro: &SarScalarSummary,
    numpyro_result: &DirichletMultinomialGroupWorkerResult,
    minimum_tolerance: f64,
    policy: DirichletMultinomialGroupAgreementPolicy,
) -> BetaBinomialParameterAgreement {
    let absolute_difference = (pymc.mean - numpyro.mean).abs();
    let combined_mcse = (pymc.sd / pymc_result.diagnostics.ess_bulk.sqrt())
        .hypot(numpyro.sd / numpyro_result.diagnostics.ess_bulk.sqrt());
    let standardized_difference = if combined_mcse > 0.0 {
        absolute_difference / combined_mcse
    } else if absolute_difference == 0.0 {
        0.0
    } else {
        f64::INFINITY
    };
    let tolerance = minimum_tolerance.max(policy.maximum_standardized_difference * combined_mcse);
    let intervals_overlap = pymc.interval_lower <= numpyro.interval_upper
        && numpyro.interval_lower <= pymc.interval_upper;
    BetaBinomialParameterAgreement {
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

fn summarize(
    result: DirichletMultinomialGroupWorkerResult,
) -> DirichletMultinomialGroupBackendSummary {
    DirichletMultinomialGroupBackendSummary {
        backend: result.backend,
        request_sha256: result.request_sha256,
        fit_state: result.fit_state,
        sampling: result.sampling,
        posterior: result.posterior,
        diagnostics: result.diagnostics,
        posterior_predictive: result.posterior_predictive,
    }
}
