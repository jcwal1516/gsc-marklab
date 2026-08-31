use crate::validation::all_finite as finite;
use crate::validation::is_lower_hex_sha256 as is_sha256;

use serde::{Deserialize, Serialize};

use crate::{
    hierarchical::{
        GaussianHierarchyInputIdentity, GaussianHierarchyModelIr, GaussianHierarchyPosterior,
        GaussianHierarchyWorkerRequest, HierarchicalPosteriorPredictive, HierarchicalWorkerResult,
        PartialPoolingSummary,
    },
    sha256_hex, BackendContract, BayesError, FitState, NormalMeanDiagnostics, SamplingSummary,
    WorkerBackend,
};

pub const NUMPYRO_VERSION: &str = "0.21.0";
pub const JAX_VERSION: &str = "0.11.1";
const REQUEST_FORMAT: &str = "marklab.numpyro_hierarchical_worker_request";
const RESULT_FORMAT: &str = "marklab.numpyro_hierarchical_worker_result";

#[derive(Clone, Debug, Serialize)]
pub struct NumpyroHierarchyWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub jax_version: &'static str,
    pub source_request_sha256: String,
    pub source_request: GaussianHierarchyWorkerRequest,
}

impl NumpyroHierarchyWorkerRequest {
    pub fn new(
        source_request: GaussianHierarchyWorkerRequest,
        environment_lock_sha256: String,
        worker_sha256: String,
    ) -> Result<Self, BayesError> {
        if !is_sha256(&environment_lock_sha256) || !is_sha256(&worker_sha256) {
            return Err(BayesError::InvalidSpec(
                "NumPyro environment and worker identities must be SHA-256 digests".into(),
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
pub struct NumpyroHierarchyWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub jax_version: String,
    pub request_sha256: String,
    pub source_request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: GaussianHierarchyPosterior,
    pub partial_pooling: Vec<PartialPoolingSummary>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: HierarchicalPosteriorPredictive,
}

impl NumpyroHierarchyWorkerResult {
    pub fn validate(
        &self,
        request: &NumpyroHierarchyWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.format != RESULT_FORMAT
            || self.version != 1
            || self.backend.name != "numpyro"
            || self.backend.version != NUMPYRO_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.jax_version != JAX_VERSION
            || self.request_sha256 != request_sha256
            || self.source_request_sha256 != request.source_request_sha256
        {
            return Err(BayesError::WorkerContract(
                "NumPyro hierarchy result identity mismatch".into(),
            ));
        }
        let source = &request.source_request;
        let expected_draws =
            u64::from(source.sampling.chains) * u64::from(source.sampling.draws_per_chain);
        if self.sampling.chains != source.sampling.chains
            || self.sampling.tune_per_chain != source.sampling.tune_per_chain
            || self.sampling.draws_per_chain != source.sampling.draws_per_chain
            || self.sampling.completed_draws != expected_draws
            || self.partial_pooling.len() != source.patients.len()
        {
            return Err(BayesError::WorkerContract(
                "NumPyro hierarchy dimensions or sampling counts mismatch".into(),
            ));
        }
        for summary in [
            &self.posterior.global_mean,
            &self.posterior.between_patient_sd,
        ] {
            if !finite(&[
                summary.mean,
                summary.sd,
                summary.interval_lower,
                summary.interval_upper,
            ]) || summary.sd <= 0.0
                || summary.interval_lower > summary.interval_upper
            {
                return Err(BayesError::WorkerContract(
                    "NumPyro hierarchy posterior summary is invalid".into(),
                ));
            }
        }
        if self.posterior.between_patient_sd.mean <= 0.0
            || self.posterior.between_patient_sd.interval_lower < 0.0
            || !(0.0..=1.0).contains(&self.posterior.variance_partition_mean)
        {
            return Err(BayesError::WorkerContract(
                "NumPyro hierarchy variance summary is invalid".into(),
            ));
        }
        for (summary, patient) in self.partial_pooling.iter().zip(&source.patients) {
            let raw_mean =
                patient.observations.iter().sum::<f64>() / patient.observations.len() as f64;
            if summary.patient_id != patient.patient_id
                || summary.observation_count != patient.observations.len()
                || summary.warning != "shrinkage_is_model_dependent_not_a_quality_score"
                || !finite(&[
                    summary.raw_mean,
                    summary.posterior_mean,
                    summary.posterior_sd,
                    summary.interval_lower,
                    summary.interval_upper,
                    summary.shrinkage,
                ])
                || summary.posterior_sd <= 0.0
                || summary.interval_lower > summary.interval_upper
                || (summary.raw_mean - raw_mean).abs() > 1e-12 * raw_mean.abs().max(1.0)
            {
                return Err(BayesError::WorkerContract(
                    "NumPyro partial-pooling summary is invalid".into(),
                ));
            }
        }
        let observations = source
            .patients
            .iter()
            .flat_map(|patient| patient.observations.iter().copied())
            .collect::<Vec<_>>();
        let observed_global_mean = observations.iter().sum::<f64>() / observations.len() as f64;
        let patient_means = source
            .patients
            .iter()
            .map(|patient| {
                patient.observations.iter().sum::<f64>() / patient.observations.len() as f64
            })
            .collect::<Vec<_>>();
        let patient_mean = patient_means.iter().sum::<f64>() / patient_means.len() as f64;
        let observed_patient_mean_sd = (patient_means
            .iter()
            .map(|value| (value - patient_mean).powi(2))
            .sum::<f64>()
            / (patient_means.len() - 1) as f64)
            .sqrt();
        if !finite(&[
            self.diagnostics.r_hat,
            self.diagnostics.ess_bulk,
            self.diagnostics.ess_tail,
            self.diagnostics.mcse_mean,
            self.diagnostics.mcse_sd,
            self.diagnostics.minimum_ebfmi,
            self.posterior_predictive.observed_global_mean,
            self.posterior_predictive.replicated_global_mean_mean,
            self.posterior_predictive.replicated_global_mean_sd,
            self.posterior_predictive
                .probability_replicated_global_mean_at_least_observed,
            self.posterior_predictive.observed_patient_mean_sd,
            self.posterior_predictive.replicated_patient_mean_sd_mean,
        ]) || self.diagnostics.r_hat <= 0.0
            || self.diagnostics.ess_bulk <= 0.0
            || self.diagnostics.ess_tail <= 0.0
            || self.diagnostics.mcse_mean < 0.0
            || self.diagnostics.mcse_sd < 0.0
            || self.diagnostics.minimum_ebfmi < 0.0
            || (self.posterior_predictive.observed_global_mean - observed_global_mean).abs()
                > 1e-12 * observed_global_mean.abs().max(1.0)
            || (self.posterior_predictive.observed_patient_mean_sd - observed_patient_mean_sd).abs()
                > 1e-12 * observed_patient_mean_sd.abs().max(1.0)
            || !(0.0..=1.0).contains(
                &self
                    .posterior_predictive
                    .probability_replicated_global_mean_at_least_observed,
            )
        {
            return Err(BayesError::WorkerContract(
                "NumPyro hierarchy diagnostics or predictive result is invalid".into(),
            ));
        }
        let policy = &source.diagnostic_policy;
        let diagnostics_pass = self.diagnostics.prior_predictive_finite
            && self.diagnostics.posterior_finite
            && self.diagnostics.constraints_valid
            && self.diagnostics.identifiability_checks_passed
            && self.diagnostics.r_hat <= policy.maximum_r_hat
            && self.diagnostics.ess_bulk >= policy.minimum_bulk_ess
            && self.diagnostics.ess_tail >= policy.minimum_tail_ess
            && self.diagnostics.minimum_ebfmi >= policy.minimum_ebfmi
            && self.diagnostics.divergences <= policy.maximum_divergences
            && self.diagnostics.max_tree_depth_hits <= policy.maximum_tree_depth_hits;
        if (self.fit_state == FitState::Complete) != diagnostics_pass {
            return Err(BayesError::WorkerContract(
                "NumPyro hierarchy fit state disagrees with diagnostics".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct HierarchicalAgreementPolicy {
    pub maximum_standardized_difference: f64,
    pub minimum_absolute_tolerance: f64,
}

impl HierarchicalAgreementPolicy {
    pub fn validate(self) -> Result<Self, BayesError> {
        if !self.maximum_standardized_difference.is_finite()
            || !(1.0..=10.0).contains(&self.maximum_standardized_difference)
            || !self.minimum_absolute_tolerance.is_finite()
            || self.minimum_absolute_tolerance <= 0.0
        {
            return Err(BayesError::InvalidSpec(
                "cross-backend agreement controls are invalid".into(),
            ));
        }
        Ok(self)
    }
}

#[derive(Debug, Serialize)]
pub struct ParameterAgreement {
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
pub struct HierarchicalAgreementComparison {
    pub global_mean: ParameterAgreement,
    pub between_patient_sd: ParameterAgreement,
    pub maximum_standardized_difference: f64,
    pub minimum_absolute_tolerance: f64,
}

#[derive(Debug, Serialize)]
pub struct HierarchicalBackendSummary {
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: GaussianHierarchyPosterior,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: HierarchicalPosteriorPredictive,
}

#[derive(Debug, Serialize)]
pub struct HierarchicalAgreementResult {
    pub format: &'static str,
    pub version: u32,
    pub model: GaussianHierarchyModelIr,
    pub input: GaussianHierarchyInputIdentity,
    pub fit_state: FitState,
    pub agreement_status: &'static str,
    pub pymc: HierarchicalBackendSummary,
    pub numpyro: HierarchicalBackendSummary,
    pub comparison: HierarchicalAgreementComparison,
    pub seed: u64,
    pub claim_status: &'static str,
}

impl HierarchicalAgreementResult {
    pub fn new(
        source_request: GaussianHierarchyWorkerRequest,
        input: GaussianHierarchyInputIdentity,
        pymc: HierarchicalWorkerResult,
        numpyro: NumpyroHierarchyWorkerResult,
        policy: HierarchicalAgreementPolicy,
    ) -> Result<Self, BayesError> {
        let policy = policy.validate()?;
        let global_mean = compare(
            &pymc.posterior.global_mean,
            pymc.diagnostics.mcse_mean,
            &numpyro.posterior.global_mean,
            numpyro.diagnostics.mcse_mean,
            policy,
        );
        let between_patient_sd = compare(
            &pymc.posterior.between_patient_sd,
            pymc.diagnostics.mcse_mean,
            &numpyro.posterior.between_patient_sd,
            numpyro.diagnostics.mcse_mean,
            policy,
        );
        let fits_complete =
            pymc.fit_state == FitState::Complete && numpyro.fit_state == FitState::Complete;
        let agrees = global_mean.passes && between_patient_sd.passes;
        let available = fits_complete && agrees;
        Ok(Self {
            format: "marklab.bayesian_hierarchical_cross_backend_agreement",
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
            } else if !fits_complete {
                "diagnostic_only_nonconverged"
            } else {
                "diagnostic_only_backend_disagreement"
            },
            pymc: HierarchicalBackendSummary {
                backend: pymc.backend,
                request_sha256: pymc.request_sha256,
                fit_state: pymc.fit_state,
                sampling: pymc.sampling,
                posterior: pymc.posterior,
                diagnostics: pymc.diagnostics,
                posterior_predictive: pymc.posterior_predictive,
            },
            numpyro: HierarchicalBackendSummary {
                backend: numpyro.backend,
                request_sha256: numpyro.request_sha256,
                fit_state: numpyro.fit_state,
                sampling: numpyro.sampling,
                posterior: numpyro.posterior,
                diagnostics: numpyro.diagnostics,
                posterior_predictive: numpyro.posterior_predictive,
            },
            comparison: HierarchicalAgreementComparison {
                global_mean,
                between_patient_sd,
                maximum_standardized_difference: policy.maximum_standardized_difference,
                minimum_absolute_tolerance: policy.minimum_absolute_tolerance,
            },
            seed: source_request.sampling.seed,
            claim_status: if available {
                "experimental_cross_backend_validation"
            } else {
                "diagnostic_only_cross_backend_validation"
            },
        })
    }
}

fn compare(
    pymc: &crate::hierarchical::ScalarPosteriorSummary,
    pymc_mcse: f64,
    numpyro: &crate::hierarchical::ScalarPosteriorSummary,
    numpyro_mcse: f64,
    policy: HierarchicalAgreementPolicy,
) -> ParameterAgreement {
    let absolute_difference = (pymc.mean - numpyro.mean).abs();
    let combined_mcse = pymc_mcse.hypot(numpyro_mcse);
    let standardized_difference = standardized_mcse_difference(absolute_difference, combined_mcse);
    let tolerance = policy
        .minimum_absolute_tolerance
        .max(policy.maximum_standardized_difference * combined_mcse);
    let intervals_overlap = pymc.interval_lower <= numpyro.interval_upper
        && numpyro.interval_lower <= pymc.interval_upper;
    ParameterAgreement {
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
use crate::agreement_metric::standardized_mcse_difference;
