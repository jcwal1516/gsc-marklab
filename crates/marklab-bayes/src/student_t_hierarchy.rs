use serde::{Deserialize, Serialize};

use crate::{
    hierarchical::{
        GaussianHierarchyInputIdentity, HalfNormalPrior, HierarchicalPatientData, NormalPrior,
        PartialPoolingSummary, ScalarPosteriorSummary,
    },
    model::{
        BackendContract, DiagnosticPolicy, WorkerResourceLimits, MODEL_FORMAT, MODEL_VERSION,
        PYMC_VERSION, WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION,
    },
    BayesError, FitState, NormalMeanDiagnostics, NutsSamplingSpec, SamplingSummary, WorkerBackend,
};

#[derive(Clone, Debug)]
pub struct StudentTHierarchySpec {
    pub global_prior_mean: f64,
    pub global_prior_sd: f64,
    pub between_patient_sd_prior: f64,
    pub observation_sd_prior: f64,
    pub degrees_of_freedom_excess_rate: f64,
    pub patients: Vec<HierarchicalPatientData>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StudentTLikelihood {
    pub family: &'static str,
    pub observation_sd_prior: HalfNormalPrior,
    pub degrees_of_freedom: &'static str,
    pub degrees_of_freedom_excess_prior: &'static str,
    pub degrees_of_freedom_excess_rate: f64,
    pub finite_variance_floor: f64,
    pub link: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct StudentTHierarchyModelIr {
    pub format: &'static str,
    pub version: u32,
    pub family: &'static str,
    pub global_mean_prior: NormalPrior,
    pub between_patient_sd_prior: HalfNormalPrior,
    pub patient_effect_parameterization: &'static str,
    pub likelihood: StudentTLikelihood,
    pub observation_unit: &'static str,
    pub biological_unit: &'static str,
    pub hierarchy: [&'static str; 1],
    pub spatial_component: &'static str,
    pub posterior_predictive_statistics: [&'static str; 3],
    pub backend_capability: &'static str,
    pub maturity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct StudentTHierarchyWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: StudentTHierarchyModelIr,
    pub patients: Vec<HierarchicalPatientData>,
    pub sampling: NutsSamplingSpec,
    pub resources: WorkerResourceLimits,
    pub diagnostic_policy: DiagnosticPolicy,
}

impl StudentTHierarchyWorkerRequest {
    pub fn new(
        mut spec: StudentTHierarchySpec,
        sampling: NutsSamplingSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        if !spec.global_prior_mean.is_finite()
            || [
                spec.global_prior_sd,
                spec.between_patient_sd_prior,
                spec.observation_sd_prior,
                spec.degrees_of_freedom_excess_rate,
            ]
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
            || spec.patients.len() < 3
            || !is_sha256(&environment_lock_sha256)
            || !is_sha256(&worker_sha256)
            || !(1..=3_600).contains(&timeout_seconds)
        {
            return Err(BayesError::InvalidSpec(
                "Student-t hierarchy priors, patients, identities, or timeout are invalid".into(),
            ));
        }
        spec.patients
            .sort_by(|left, right| left.patient_id.cmp(&right.patient_id));
        let mut observations = 0_usize;
        for (index, patient) in spec.patients.iter().enumerate() {
            if patient.patient_id.is_empty()
                || patient.patient_id.trim() != patient.patient_id
                || (index > 0 && spec.patients[index - 1].patient_id == patient.patient_id)
                || patient.observations.len() < 2
                || patient.observations.iter().any(|value| !value.is_finite())
            {
                return Err(BayesError::InvalidSpec(
                    "Student-t hierarchy patient rows are invalid".into(),
                ));
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
        sampling.validate()?;
        let maximum_total_iterations = 800_000;
        if u64::from(sampling.chains)
            * u64::from(sampling.tune_per_chain + sampling.draws_per_chain)
            > maximum_total_iterations
        {
            return Err(BayesError::InvalidSpec(
                "Student-t hierarchy exceeds 800000 NUTS iterations".into(),
            ));
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
            model: StudentTHierarchyModelIr {
                format: MODEL_FORMAT,
                version: MODEL_VERSION,
                family: "student_t_patient_varying_intercept",
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
                likelihood: StudentTLikelihood {
                    family: "student_t_inferred_scale_and_degrees_of_freedom",
                    observation_sd_prior: HalfNormalPrior {
                        family: "half_normal",
                        sd: spec.observation_sd_prior,
                        support: "positive",
                        rationale: "user_supplied",
                    },
                    degrees_of_freedom: "two_plus_positive_excess",
                    degrees_of_freedom_excess_prior: "exponential_rate",
                    degrees_of_freedom_excess_rate: spec.degrees_of_freedom_excess_rate,
                    finite_variance_floor: 2.0,
                    link: "identity",
                },
                observation_unit: "patient_nested_scalar_observation",
                biological_unit: "patient",
                hierarchy: ["patient"],
                spatial_component: "none",
                posterior_predictive_statistics: [
                    "global_mean",
                    "patient_mean_sd",
                    "maximum_absolute_residual",
                ],
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

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StudentTHierarchyPosterior {
    pub global_mean: ScalarPosteriorSummary,
    pub between_patient_sd: ScalarPosteriorSummary,
    pub observation_sd: ScalarPosteriorSummary,
    pub degrees_of_freedom: ScalarPosteriorSummary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StudentTHierarchyPosteriorPredictive {
    pub observed_global_mean: f64,
    pub replicated_global_mean_mean: f64,
    pub replicated_global_mean_sd: f64,
    pub observed_patient_mean_sd: f64,
    pub replicated_patient_mean_sd_mean: f64,
    pub observed_maximum_absolute_residual: f64,
    pub replicated_maximum_absolute_residual_mean: f64,
    pub probability_replicated_maximum_absolute_residual_at_least_observed: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StudentTHierarchyWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: StudentTHierarchyPosterior,
    pub partial_pooling: Vec<PartialPoolingSummary>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: StudentTHierarchyPosteriorPredictive,
}

impl StudentTHierarchyWorkerResult {
    pub fn validate(
        &self,
        request: &StudentTHierarchyWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.format != "marklab.pymc_student_t_hierarchy_worker_result"
            || self.version != 1
            || self.backend.name != request.backend.name
            || self.backend.version != request.backend.version
            || self.backend.python_version != request.backend.python_version
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
            || self.partial_pooling.len() != request.patients.len()
        {
            return Err(BayesError::WorkerContract(
                "Student-t hierarchy result identity or dimensions mismatch".into(),
            ));
        }
        for summary in [
            &self.posterior.global_mean,
            &self.posterior.between_patient_sd,
            &self.posterior.observation_sd,
            &self.posterior.degrees_of_freedom,
        ] {
            validate_summary(summary)?;
        }
        if self.posterior.between_patient_sd.mean <= 0.0
            || self.posterior.observation_sd.mean <= 0.0
            || self.posterior.degrees_of_freedom.mean <= 2.0
            || self.posterior.degrees_of_freedom.interval_lower <= 2.0
        {
            return Err(BayesError::WorkerContract(
                "Student-t posterior constraints are invalid".into(),
            ));
        }
        let expected_draws =
            u64::from(request.sampling.chains) * u64::from(request.sampling.draws_per_chain);
        if self.sampling.completed_draws != expected_draws
            || self.sampling.chains != request.sampling.chains
            || self.sampling.tune_per_chain != request.sampling.tune_per_chain
            || self.sampling.draws_per_chain != request.sampling.draws_per_chain
        {
            return Err(BayesError::WorkerContract(
                "Student-t hierarchy sampling counts mismatch".into(),
            ));
        }
        for (summary, patient) in self.partial_pooling.iter().zip(&request.patients) {
            let raw_mean =
                patient.observations.iter().sum::<f64>() / patient.observations.len() as f64;
            if summary.patient_id != patient.patient_id
                || summary.observation_count != patient.observations.len()
                || (summary.raw_mean - raw_mean).abs() > 1e-12 * raw_mean.abs().max(1.0)
                || !finite(&[
                    summary.posterior_mean,
                    summary.posterior_sd,
                    summary.interval_lower,
                    summary.interval_upper,
                    summary.shrinkage,
                ])
                || summary.posterior_sd <= 0.0
                || summary.warning != "shrinkage_is_model_dependent_not_a_quality_score"
            {
                return Err(BayesError::WorkerContract(
                    "Student-t partial-pooling summary is invalid".into(),
                ));
            }
        }
        if !finite(&[
            self.diagnostics.r_hat,
            self.diagnostics.ess_bulk,
            self.diagnostics.ess_tail,
            self.diagnostics.minimum_ebfmi,
            self.posterior_predictive.observed_global_mean,
            self.posterior_predictive.replicated_global_mean_mean,
            self.posterior_predictive.replicated_global_mean_sd,
            self.posterior_predictive.observed_patient_mean_sd,
            self.posterior_predictive.replicated_patient_mean_sd_mean,
            self.posterior_predictive.observed_maximum_absolute_residual,
            self.posterior_predictive
                .replicated_maximum_absolute_residual_mean,
            self.posterior_predictive
                .probability_replicated_maximum_absolute_residual_at_least_observed,
        ]) || !(0.0..=1.0).contains(
            &self
                .posterior_predictive
                .probability_replicated_maximum_absolute_residual_at_least_observed,
        ) {
            return Err(BayesError::WorkerContract(
                "Student-t diagnostics or posterior predictive result is invalid".into(),
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
        let observed_maximum_absolute_residual = request
            .patients
            .iter()
            .zip(&patient_means)
            .flat_map(|(patient, mean)| {
                patient
                    .observations
                    .iter()
                    .map(move |value| (value - mean).abs())
            })
            .fold(0.0_f64, f64::max);
        if (self.posterior_predictive.observed_global_mean - observed_global_mean).abs()
            > 1e-12 * observed_global_mean.abs().max(1.0)
            || (self.posterior_predictive.observed_patient_mean_sd - observed_patient_mean_sd).abs()
                > 1e-12 * observed_patient_mean_sd.abs().max(1.0)
            || (self.posterior_predictive.observed_maximum_absolute_residual
                - observed_maximum_absolute_residual)
                .abs()
                > 1e-12 * observed_maximum_absolute_residual.abs().max(1.0)
        {
            return Err(BayesError::WorkerContract(
                "Student-t posterior predictive changed observed summaries".into(),
            ));
        }
        let pass = self.diagnostics.prior_predictive_finite
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
        if (self.fit_state == FitState::Complete) != pass {
            return Err(BayesError::WorkerContract(
                "Student-t fit state disagrees with diagnostics".into(),
            ));
        }
        Ok(())
    }

    pub fn into_result(
        self,
        request: StudentTHierarchyWorkerRequest,
        input: GaussianHierarchyInputIdentity,
    ) -> StudentTHierarchyResult {
        StudentTHierarchyResult {
            format: "marklab.bayesian_student_t_hierarchy",
            version: 1,
            backend: self.backend,
            model: request.model,
            input,
            sampling: self.sampling,
            fit_state: self.fit_state,
            posterior: self.posterior,
            partial_pooling: self.partial_pooling,
            diagnostics: self.diagnostics,
            posterior_predictive: self.posterior_predictive,
            seed: request.sampling.seed,
            claim_status: if self.fit_state == FitState::Complete {
                "experimental_robust_hierarchy"
            } else {
                "diagnostic_only_nonconverged"
            },
            request_sha256: self.request_sha256,
        }
    }
}

fn validate_summary(summary: &ScalarPosteriorSummary) -> Result<(), BayesError> {
    if !finite(&[
        summary.mean,
        summary.sd,
        summary.interval_lower,
        summary.interval_upper,
    ]) || summary.sd <= 0.0
        || summary.interval_lower > summary.interval_upper
    {
        return Err(BayesError::WorkerContract(
            "Student-t posterior summary is invalid".into(),
        ));
    }
    Ok(())
}

fn finite(values: &[f64]) -> bool {
    values.iter().all(|value| value.is_finite())
}

#[derive(Debug, Serialize)]
pub struct StudentTHierarchyResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: StudentTHierarchyModelIr,
    pub input: GaussianHierarchyInputIdentity,
    pub sampling: SamplingSummary,
    pub fit_state: FitState,
    pub posterior: StudentTHierarchyPosterior,
    pub partial_pooling: Vec<PartialPoolingSummary>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: StudentTHierarchyPosteriorPredictive,
    pub seed: u64,
    pub claim_status: &'static str,
    pub request_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_nonreplicated_patient() {
        let patients = vec![
            ("a", vec![0.0, 0.1]),
            ("b", vec![1.0]),
            ("c", vec![2.0, 2.1]),
        ]
        .into_iter()
        .map(|(patient_id, observations)| HierarchicalPatientData {
            patient_id: patient_id.into(),
            observations,
        })
        .collect();
        let error = StudentTHierarchyWorkerRequest::new(
            StudentTHierarchySpec {
                global_prior_mean: 0.0,
                global_prior_sd: 1.0,
                between_patient_sd_prior: 1.0,
                observation_sd_prior: 1.0,
                degrees_of_freedom_excess_rate: 0.1,
                patients,
            },
            NutsSamplingSpec {
                chains: 2,
                tune_per_chain: 100,
                draws_per_chain: 100,
                target_accept: 0.9,
                seed: 1,
            },
            "0".repeat(64),
            "1".repeat(64),
            60,
        )
        .expect_err("nonreplicated patient");
        assert!(error.to_string().contains("patient rows"));
    }
}
