use serde::{Deserialize, Serialize};

use crate::{
    model::{
        BackendContract, DiagnosticPolicy, WorkerResourceLimits, MODEL_FORMAT, MODEL_VERSION,
        PYMC_VERSION, WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION,
    },
    BayesError, FitState, NormalMeanDiagnostics, NutsSamplingSpec, SamplingSummary, WorkerBackend,
};

#[derive(Clone, Debug, Serialize)]
pub struct HierarchicalPatientData {
    pub patient_id: String,
    pub observations: Vec<f64>,
}

#[derive(Clone, Debug)]
pub struct GaussianHierarchySpec {
    pub global_prior_mean: f64,
    pub global_prior_sd: f64,
    pub between_patient_sd_prior: f64,
    pub known_sigma: f64,
    pub patients: Vec<HierarchicalPatientData>,
}

impl GaussianHierarchySpec {
    fn validate_and_sort(mut self) -> Result<Self, BayesError> {
        if !self.global_prior_mean.is_finite() {
            return Err(BayesError::InvalidSpec(
                "global prior mean must be finite".into(),
            ));
        }
        for (value, name) in [
            (self.global_prior_sd, "global prior standard deviation"),
            (
                self.between_patient_sd_prior,
                "between-patient standard deviation prior scale",
            ),
            (self.known_sigma, "known observation standard deviation"),
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(BayesError::InvalidSpec(format!(
                    "{name} must be finite and positive"
                )));
            }
        }
        if self.patients.len() < 3 {
            return Err(BayesError::InvalidSpec(
                "hierarchical model requires at least three patients".into(),
            ));
        }
        self.patients
            .sort_by(|left, right| left.patient_id.cmp(&right.patient_id));
        let mut observations = 0_usize;
        for (index, patient) in self.patients.iter().enumerate() {
            if patient.patient_id.is_empty() || patient.patient_id.trim() != patient.patient_id {
                return Err(BayesError::InvalidSpec(
                    "patient IDs must be non-empty without surrounding whitespace".into(),
                ));
            }
            if index > 0 && self.patients[index - 1].patient_id == patient.patient_id {
                return Err(BayesError::InvalidSpec(format!(
                    "duplicate patient group {}",
                    patient.patient_id
                )));
            }
            if patient.observations.len() < 2 {
                return Err(BayesError::InvalidSpec(format!(
                    "patient {} requires at least two observations",
                    patient.patient_id
                )));
            }
            if patient.observations.iter().any(|value| !value.is_finite()) {
                return Err(BayesError::InvalidSpec(format!(
                    "patient {} has a non-finite observation",
                    patient.patient_id
                )));
            }
            observations = observations
                .checked_add(patient.observations.len())
                .ok_or_else(|| BayesError::InvalidSpec("observation count overflow".into()))?;
        }
        if observations > 100_000 {
            return Err(BayesError::InvalidSpec(
                "observation count exceeds 100000".into(),
            ));
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct GaussianHierarchyModelIr {
    pub format: &'static str,
    pub version: u32,
    pub family: &'static str,
    pub global_mean_prior: NormalPrior,
    pub between_patient_sd_prior: HalfNormalPrior,
    pub patient_effect_parameterization: &'static str,
    pub likelihood: KnownSigmaLikelihood,
    pub observation_unit: &'static str,
    pub biological_unit: &'static str,
    pub hierarchy: [&'static str; 1],
    pub spatial_component: &'static str,
    pub generated_quantities: [&'static str; 3],
    pub posterior_predictive_statistics: [&'static str; 2],
    pub backend_capability: &'static str,
    pub maturity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct NormalPrior {
    pub family: &'static str,
    pub mean: f64,
    pub sd: f64,
    pub rationale: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct HalfNormalPrior {
    pub family: &'static str,
    pub sd: f64,
    pub support: &'static str,
    pub rationale: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct KnownSigmaLikelihood {
    pub family: &'static str,
    pub known_sigma: f64,
    pub link: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct GaussianHierarchyWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: GaussianHierarchyModelIr,
    pub patients: Vec<HierarchicalPatientData>,
    pub sampling: NutsSamplingSpec,
    pub resources: WorkerResourceLimits,
    pub diagnostic_policy: DiagnosticPolicy,
}

impl GaussianHierarchyWorkerRequest {
    pub fn new(
        spec: GaussianHierarchySpec,
        sampling: NutsSamplingSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        let spec = spec.validate_and_sort()?;
        sampling.validate()?;
        if !(1..=3_600).contains(&timeout_seconds) {
            return Err(BayesError::InvalidSpec(
                "worker timeout must be between 1 and 3600 seconds".into(),
            ));
        }
        let total_iterations = u64::from(sampling.chains)
            * u64::from(sampling.tune_per_chain + sampling.draws_per_chain);
        let maximum_total_iterations = 800_000;
        if total_iterations > maximum_total_iterations {
            return Err(BayesError::InvalidSpec(format!(
                "requested {total_iterations} NUTS iterations exceed the {maximum_total_iterations} limit"
            )));
        }
        Ok(Self {
            format: WORKER_REQUEST_FORMAT,
            version: WORKER_REQUEST_VERSION,
            backend: BackendContract {
                name: "pymc",
                version: PYMC_VERSION,
                python_version: "3.12",
                environment_lock_sha256,
                worker_sha256,
            },
            model: GaussianHierarchyModelIr {
                format: MODEL_FORMAT,
                version: MODEL_VERSION,
                family: "gaussian_patient_varying_intercept",
                global_mean_prior: NormalPrior {
                    family: "normal",
                    mean: spec.global_prior_mean,
                    sd: spec.global_prior_sd,
                    rationale: "user_supplied",
                },
                between_patient_sd_prior: HalfNormalPrior {
                    family: "half_normal",
                    sd: spec.between_patient_sd_prior,
                    support: "positive",
                    rationale: "user_supplied",
                },
                patient_effect_parameterization: "noncentered",
                likelihood: KnownSigmaLikelihood {
                    family: "normal_known_sigma",
                    known_sigma: spec.known_sigma,
                    link: "identity",
                },
                observation_unit: "patient_nested_scalar_observation",
                biological_unit: "patient",
                hierarchy: ["patient"],
                spatial_component: "none",
                generated_quantities: [
                    "patient_mean",
                    "variance_partition",
                    "posterior_predictive_observation",
                ],
                posterior_predictive_statistics: ["global_mean", "patient_mean_sd"],
                backend_capability: "nuts",
                maturity: "experimental",
            },
            patients: spec.patients,
            sampling,
            resources: WorkerResourceLimits {
                maximum_observations: 100_000,
                maximum_total_iterations,
                maximum_output_bytes: 1_048_576,
                timeout_seconds,
            },
            diagnostic_policy: DiagnosticPolicy::default(),
        })
    }

    pub fn observation_count(&self) -> usize {
        self.patients
            .iter()
            .map(|patient| patient.observations.len())
            .sum()
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScalarPosteriorSummary {
    pub mean: f64,
    pub sd: f64,
    pub interval_lower: f64,
    pub interval_upper: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GaussianHierarchyPosterior {
    pub global_mean: ScalarPosteriorSummary,
    pub between_patient_sd: ScalarPosteriorSummary,
    pub variance_partition_mean: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PartialPoolingSummary {
    pub patient_id: String,
    pub observation_count: usize,
    pub raw_mean: f64,
    pub posterior_mean: f64,
    pub posterior_sd: f64,
    pub interval_lower: f64,
    pub interval_upper: f64,
    pub shrinkage: f64,
    pub warning: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HierarchicalPosteriorPredictive {
    pub observed_global_mean: f64,
    pub replicated_global_mean_mean: f64,
    pub replicated_global_mean_sd: f64,
    pub probability_replicated_global_mean_at_least_observed: f64,
    pub observed_patient_mean_sd: f64,
    pub replicated_patient_mean_sd_mean: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HierarchicalWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: GaussianHierarchyPosterior,
    pub partial_pooling: Vec<PartialPoolingSummary>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: HierarchicalPosteriorPredictive,
}

impl HierarchicalWorkerResult {
    pub fn validate(
        &self,
        request: &GaussianHierarchyWorkerRequest,
        expected_request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.fit_state == FitState::ApproximateOnly {
            return Err(BayesError::WorkerContract(
                "hierarchical NUTS cannot return approximate-only state".into(),
            ));
        }
        if self.format != "marklab.pymc_hierarchical_worker_result" || self.version != 1 {
            return Err(BayesError::WorkerContract(
                "unsupported hierarchical worker result".into(),
            ));
        }
        if self.backend.name != "pymc"
            || self.backend.version != PYMC_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != expected_request_sha256
        {
            return Err(BayesError::WorkerContract(
                "hierarchical backend or request identity mismatch".into(),
            ));
        }
        let expected_draws =
            u64::from(request.sampling.chains) * u64::from(request.sampling.draws_per_chain);
        if self.sampling.chains != request.sampling.chains
            || self.sampling.tune_per_chain != request.sampling.tune_per_chain
            || self.sampling.draws_per_chain != request.sampling.draws_per_chain
            || self.sampling.completed_draws != expected_draws
        {
            return Err(BayesError::WorkerContract(
                "hierarchical sampling counts mismatch".into(),
            ));
        }
        if self.partial_pooling.len() != request.patients.len() {
            return Err(BayesError::WorkerContract(
                "partial-pooling patient count mismatch".into(),
            ));
        }
        validate_scalar(&self.posterior.global_mean)?;
        validate_scalar(&self.posterior.between_patient_sd)?;
        if self.posterior.between_patient_sd.mean <= 0.0
            || self.posterior.between_patient_sd.interval_lower < 0.0
            || !(0.0..=1.0).contains(&self.posterior.variance_partition_mean)
        {
            return Err(BayesError::WorkerContract(
                "invalid hierarchical variance summary".into(),
            ));
        }
        for (summary, patient) in self.partial_pooling.iter().zip(&request.patients) {
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
                    "invalid partial-pooling summary".into(),
                ));
            }
        }
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
        ]) || self.posterior_predictive.replicated_global_mean_sd < 0.0
            || self.posterior_predictive.observed_patient_mean_sd < 0.0
            || self.posterior_predictive.replicated_patient_mean_sd_mean < 0.0
            || self.diagnostics.r_hat <= 0.0
            || self.diagnostics.ess_bulk <= 0.0
            || self.diagnostics.ess_tail <= 0.0
            || self.diagnostics.mcse_mean < 0.0
            || self.diagnostics.mcse_sd < 0.0
            || self.diagnostics.minimum_ebfmi < 0.0
            || !(0.0..=1.0).contains(
                &self
                    .posterior_predictive
                    .probability_replicated_global_mean_at_least_observed,
            )
        {
            return Err(BayesError::WorkerContract(
                "invalid hierarchical diagnostics or predictive result".into(),
            ));
        }
        let observations = request
            .patients
            .iter()
            .flat_map(|patient| patient.observations.iter().copied())
            .collect::<Vec<_>>();
        let observed_global_mean = observations.iter().sum::<f64>() / observations.len() as f64;
        let patient_means = request
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
        if (self.posterior_predictive.observed_global_mean - observed_global_mean).abs()
            > 1e-12 * observed_global_mean.abs().max(1.0)
            || (self.posterior_predictive.observed_patient_mean_sd - observed_patient_mean_sd).abs()
                > 1e-12 * observed_patient_mean_sd.abs().max(1.0)
        {
            return Err(BayesError::WorkerContract(
                "hierarchical posterior predictive changed observed summaries".into(),
            ));
        }
        let diagnostics_pass = self.diagnostics.prior_predictive_finite
            && self.diagnostics.posterior_finite
            && self.diagnostics.constraints_valid
            && self.diagnostics.identifiability_checks_passed
            && self.diagnostics.r_hat <= request.diagnostic_policy.maximum_r_hat
            && self.diagnostics.ess_bulk >= request.diagnostic_policy.minimum_bulk_ess
            && self.diagnostics.ess_tail >= request.diagnostic_policy.minimum_tail_ess
            && self.diagnostics.minimum_ebfmi >= request.diagnostic_policy.minimum_ebfmi
            && self.diagnostics.divergences <= request.diagnostic_policy.maximum_divergences
            && self.diagnostics.max_tree_depth_hits
                <= request.diagnostic_policy.maximum_tree_depth_hits;
        if (self.fit_state == FitState::Complete) != diagnostics_pass {
            return Err(BayesError::WorkerContract(
                "hierarchical fit state disagrees with diagnostics".into(),
            ));
        }
        Ok(())
    }

    pub fn into_fit(
        self,
        request: GaussianHierarchyWorkerRequest,
        input: GaussianHierarchyInputIdentity,
    ) -> GaussianHierarchyFit {
        GaussianHierarchyFit {
            format: "marklab.bayesian_hierarchical_fit",
            version: 1,
            backend: self.backend,
            model: request.model,
            input,
            fit_state: self.fit_state,
            claim_status: match self.fit_state {
                FitState::Complete => "experimental",
                FitState::Nonconverged => "diagnostic_only_nonconverged",
                FitState::ApproximateOnly => "experimental_approximate_only",
            },
            sampling: self.sampling,
            posterior: self.posterior,
            partial_pooling: self.partial_pooling,
            diagnostics: self.diagnostics,
            posterior_predictive: self.posterior_predictive,
            seed: request.sampling.seed,
            request_sha256: self.request_sha256,
        }
    }
}

fn validate_scalar(summary: &ScalarPosteriorSummary) -> Result<(), BayesError> {
    if !finite(&[
        summary.mean,
        summary.sd,
        summary.interval_lower,
        summary.interval_upper,
    ]) || summary.sd <= 0.0
        || summary.interval_lower > summary.interval_upper
    {
        return Err(BayesError::WorkerContract(
            "invalid scalar posterior summary".into(),
        ));
    }
    Ok(())
}

fn finite(values: &[f64]) -> bool {
    values.iter().all(|value| value.is_finite())
}

#[derive(Clone, Debug, Serialize)]
pub struct GaussianHierarchyInputIdentity {
    pub path: String,
    pub patients: usize,
    pub observations: usize,
    pub patient_data_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct GaussianHierarchyFit {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: GaussianHierarchyModelIr,
    pub input: GaussianHierarchyInputIdentity,
    pub fit_state: FitState,
    pub claim_status: &'static str,
    pub sampling: SamplingSummary,
    pub posterior: GaussianHierarchyPosterior,
    pub partial_pooling: Vec<PartialPoolingSummary>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: HierarchicalPosteriorPredictive,
    pub seed: u64,
    pub request_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sampling() -> NutsSamplingSpec {
        NutsSamplingSpec {
            chains: 2,
            tune_per_chain: 500,
            draws_per_chain: 1_000,
            target_accept: 0.9,
            seed: 5,
        }
    }

    #[test]
    fn hierarchy_request_canonicalizes_patient_order() {
        let request = GaussianHierarchyWorkerRequest::new(
            GaussianHierarchySpec {
                global_prior_mean: 0.0,
                global_prior_sd: 5.0,
                between_patient_sd_prior: 2.0,
                known_sigma: 1.0,
                patients: vec![
                    patient("p-3", &[3.0, 4.0]),
                    patient("p-1", &[1.0, 2.0]),
                    patient("p-2", &[2.0, 3.0]),
                ],
            },
            sampling(),
            "lock".into(),
            "worker".into(),
            180,
        )
        .expect("request");
        assert_eq!(request.patients[0].patient_id, "p-1");
        assert_eq!(request.patients[2].patient_id, "p-3");
        assert_eq!(request.observation_count(), 6);
        assert_eq!(request.model.patient_effect_parameterization, "noncentered");
    }

    #[test]
    fn hierarchy_request_rejects_patient_without_replication() {
        let result = GaussianHierarchyWorkerRequest::new(
            GaussianHierarchySpec {
                global_prior_mean: 0.0,
                global_prior_sd: 5.0,
                between_patient_sd_prior: 2.0,
                known_sigma: 1.0,
                patients: vec![
                    patient("p-1", &[1.0]),
                    patient("p-2", &[2.0, 3.0]),
                    patient("p-3", &[3.0, 4.0]),
                ],
            },
            sampling(),
            "lock".into(),
            "worker".into(),
            180,
        );
        assert!(matches!(result, Err(BayesError::InvalidSpec(_))));
    }

    fn patient(patient_id: &str, observations: &[f64]) -> HierarchicalPatientData {
        HierarchicalPatientData {
            patient_id: patient_id.into(),
            observations: observations.to_vec(),
        }
    }
}
