use serde::{Deserialize, Serialize};

use crate::agreement_metric::AgreementVerdict;
use crate::agreement_metric::{compare_parameter, ParameterAgreement};
use crate::backend_versions::PYTHON_VERSION;
use crate::validation::is_sha256;
use crate::AgreementStatus;
use crate::ClaimStatus;
use crate::{
    hierarchical_agreement::{JAX_VERSION, NUMPYRO_VERSION},
    json_sha256, BackendContract, BayesError, DirichletMultinomialClassPosterior,
    DirichletMultinomialGroupInputIdentity, DirichletMultinomialGroupModelIr,
    DirichletMultinomialGroupPosterior, DirichletMultinomialGroupPosteriorPredictive,
    DirichletMultinomialGroupWorkerRequest, DirichletMultinomialGroupWorkerResult, FitState,
    McseScalarSummary, PosteriorDiagnostics, SamplingSummary, WorkerBackend,
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
    diagnostics: PosteriorDiagnostics,
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
    pub reference_probability: ParameterAgreement,
    pub comparison_probability: ParameterAgreement,
    pub difference_comparison_minus_reference: ParameterAgreement,
}

#[derive(Debug, Serialize)]
pub struct DirichletMultinomialGroupAgreementComparison {
    pub classes: Vec<DirichletMultinomialClassAgreement>,
    pub concentration: ParameterAgreement,
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
    pub diagnostics: PosteriorDiagnostics,
    pub posterior_predictive: DirichletMultinomialGroupPosteriorPredictive,
}

#[derive(Debug, Serialize)]
pub struct DirichletMultinomialGroupAgreementResult {
    pub format: &'static str,
    pub version: u32,
    pub model: DirichletMultinomialGroupModelIr,
    pub input: DirichletMultinomialGroupInputIdentity,
    pub fit_state: FitState,
    pub agreement_status: AgreementStatus,
    pub pymc: DirichletMultinomialGroupBackendSummary,
    pub numpyro: DirichletMultinomialGroupBackendSummary,
    pub comparison: DirichletMultinomialGroupAgreementComparison,
    pub seed: u64,
    pub claim_status: ClaimStatus,
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
            .map(|(left, right)| compare_class(left, right, policy))
            .collect::<Result<Vec<_>, _>>()?;
        let concentration = compare_parameter(
            &pymc.posterior.concentration,
            &numpyro.posterior.concentration,
            policy.maximum_standardized_difference,
            policy.minimum_concentration_tolerance,
        );
        let agrees = classes.iter().all(|class| {
            class.reference_probability.passes
                && class.comparison_probability.passes
                && class.difference_comparison_minus_reference.passes
        }) && concentration.passes;
        let verdict = AgreementVerdict::new(pymc.fit_state, numpyro.fit_state, agrees);
        let seed = source_request.sampling.seed;
        Ok(Self {
            format: "marklab.bayesian_dirichlet_multinomial_group_cross_backend_agreement",
            version: 1,
            model: source_request.model,
            input,
            fit_state: verdict.fit_state,
            agreement_status: verdict.agreement_status,
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
            claim_status: verdict.claim_status,
        })
    }
}

fn compare_class(
    pymc: &DirichletMultinomialClassPosterior,
    numpyro: &DirichletMultinomialClassPosterior,
    policy: DirichletMultinomialGroupAgreementPolicy,
) -> Result<DirichletMultinomialClassAgreement, BayesError> {
    if pymc.class_id != numpyro.class_id {
        return Err(BayesError::WorkerContract(
            "Dirichlet-multinomial backend class identities differ".into(),
        ));
    }
    let compare = |left: &McseScalarSummary, right: &McseScalarSummary| {
        compare_parameter(
            left,
            right,
            policy.maximum_standardized_difference,
            policy.minimum_probability_tolerance,
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
