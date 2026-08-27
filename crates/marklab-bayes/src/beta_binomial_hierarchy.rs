use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    model::{PYMC_VERSION, WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION},
    sha256_hex, BackendContract, BayesError, DiagnosticPolicy, FitState, NormalMeanDiagnostics,
    NutsSamplingSpec, SamplingSummary, SarScalarSummary, WorkerBackend,
};

#[derive(Clone, Debug, Serialize)]
pub struct BetaBinomialPatientData {
    pub patient_id: String,
    pub successes: u64,
    pub trials: u64,
}

#[derive(Clone, Debug)]
pub struct BetaBinomialHierarchySpec {
    pub population_alpha: f64,
    pub population_beta: f64,
    pub concentration_prior_sd: f64,
    pub patients: Vec<BetaBinomialPatientData>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BetaBinomialHierarchyModelIr {
    pub format: &'static str,
    pub version: u32,
    pub family: &'static str,
    pub population_probability_prior: &'static str,
    pub population_alpha: f64,
    pub population_beta: f64,
    pub concentration_prior: &'static str,
    pub concentration_prior_sd: f64,
    pub patient_probability: &'static str,
    pub likelihood: &'static str,
    pub overdispersion: &'static str,
    pub biological_unit: &'static str,
    pub backend_capability: &'static str,
    pub maturity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct BetaBinomialHierarchyResourceLimits {
    pub maximum_patients: u32,
    pub maximum_total_trials: u64,
    pub maximum_total_iterations: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct BetaBinomialHierarchyWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: BetaBinomialHierarchyModelIr,
    pub patients: Vec<BetaBinomialPatientData>,
    pub sampling: NutsSamplingSpec,
    pub resources: BetaBinomialHierarchyResourceLimits,
    pub diagnostic_policy: DiagnosticPolicy,
}

impl BetaBinomialHierarchyWorkerRequest {
    pub fn new(
        spec: BetaBinomialHierarchySpec,
        sampling: NutsSamplingSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        sampling.validate()?;
        if !spec.population_alpha.is_finite()
            || spec.population_alpha <= 0.0
            || !spec.population_beta.is_finite()
            || spec.population_beta <= 0.0
            || !spec.concentration_prior_sd.is_finite()
            || spec.concentration_prior_sd <= 0.0
            || !(3..=512).contains(&spec.patients.len())
            || !is_sha256(&environment_lock_sha256)
            || !is_sha256(&worker_sha256)
            || !(1..=3_600).contains(&timeout_seconds)
        {
            return Err(BayesError::InvalidSpec(
                "beta-binomial hierarchy priors, patients, identities, or timeout are invalid"
                    .into(),
            ));
        }
        let mut patients = spec.patients;
        patients.sort_by(|left, right| left.patient_id.cmp(&right.patient_id));
        let mut ids = BTreeSet::new();
        let mut total_trials = 0_u64;
        for patient in &patients {
            if patient.patient_id.is_empty()
                || patient.patient_id.trim() != patient.patient_id
                || patient.patient_id.len() > 128
                || !ids.insert(patient.patient_id.as_str())
                || patient.trials == 0
                || patient.successes > patient.trials
            {
                return Err(BayesError::InvalidSpec(
                    "beta-binomial patient rows are invalid".into(),
                ));
            }
            total_trials = total_trials.checked_add(patient.trials).ok_or_else(|| {
                BayesError::InvalidSpec("beta-binomial trial count overflow".into())
            })?;
        }
        let maximum_total_trials = 10_000_000;
        if total_trials > maximum_total_trials {
            return Err(BayesError::InvalidSpec(
                "beta-binomial hierarchy exceeds 10000000 total trials".into(),
            ));
        }
        let maximum_total_iterations = 400_000;
        let iterations = u64::from(sampling.chains)
            * u64::from(sampling.tune_per_chain + sampling.draws_per_chain);
        if iterations > maximum_total_iterations {
            return Err(BayesError::InvalidSpec(
                "beta-binomial hierarchy exceeds 400000 NUTS iterations".into(),
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
            model: BetaBinomialHierarchyModelIr {
                format: "marklab.bayesian_model_ir",
                version: 1,
                family: "patient_beta_binomial_hierarchy",
                population_probability_prior: "beta",
                population_alpha: spec.population_alpha,
                population_beta: spec.population_beta,
                concentration_prior: "half_normal",
                concentration_prior_sd: spec.concentration_prior_sd,
                patient_probability: "beta_population_mean_concentration",
                likelihood: "binomial_successes_given_patient_trials",
                overdispersion: "one_over_concentration_plus_one",
                biological_unit: "patient",
                backend_capability: "nuts",
                maturity: "experimental",
            },
            patients,
            sampling,
            resources: BetaBinomialHierarchyResourceLimits {
                maximum_patients: 512,
                maximum_total_trials,
                maximum_total_iterations,
                maximum_output_bytes: 2 * 1_048_576,
                timeout_seconds,
            },
            diagnostic_policy: DiagnosticPolicy::default(),
        })
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
pub struct BetaBinomialHierarchyPosterior {
    pub population_probability: SarScalarSummary,
    pub concentration: SarScalarSummary,
    pub overdispersion_mean: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BetaBinomialPatientPosterior {
    pub patient_id: String,
    pub successes: u64,
    pub trials: u64,
    pub observed_proportion: f64,
    pub posterior_probability: SarScalarSummary,
    pub shrinkage_toward_population: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BetaBinomialPosteriorPredictive {
    pub observed_total_successes: u64,
    pub replicated_total_successes_mean: f64,
    pub replicated_total_successes_sd: f64,
    pub observed_patient_proportion_sd: f64,
    pub replicated_patient_proportion_sd_mean: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BetaBinomialHierarchyWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub fit_state: FitState,
    pub sampling: SamplingSummary,
    pub posterior: BetaBinomialHierarchyPosterior,
    pub patients: Vec<BetaBinomialPatientPosterior>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: BetaBinomialPosteriorPredictive,
}

impl BetaBinomialHierarchyWorkerResult {
    pub fn validate(
        &self,
        request: &BetaBinomialHierarchyWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.format != "marklab.pymc_beta_binomial_hierarchy_worker_result"
            || self.version != 1
            || self.backend.name != request.backend.name
            || self.backend.version != request.backend.version
            || self.backend.python_version != request.backend.python_version
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
            || self.patients.len() != request.patients.len()
        {
            return Err(BayesError::WorkerContract(
                "beta-binomial hierarchy result identity or dimensions mismatch".into(),
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
                "beta-binomial hierarchy sampling counts mismatch".into(),
            ));
        }
        validate_summary(&self.posterior.population_probability, true)?;
        validate_summary(&self.posterior.concentration, false)?;
        if !(0.0..1.0).contains(&self.posterior.overdispersion_mean) {
            return Err(BayesError::WorkerContract(
                "beta-binomial overdispersion is invalid".into(),
            ));
        }
        for (actual, expected) in self.patients.iter().zip(&request.patients) {
            validate_summary(&actual.posterior_probability, true)?;
            let observed = expected.successes as f64 / expected.trials as f64;
            if actual.patient_id != expected.patient_id
                || actual.successes != expected.successes
                || actual.trials != expected.trials
                || (actual.observed_proportion - observed).abs() > 1e-12
                || !actual.shrinkage_toward_population.is_finite()
            {
                return Err(BayesError::WorkerContract(
                    "beta-binomial patient summary is invalid".into(),
                ));
            }
        }
        let observed_total: u64 = request
            .patients
            .iter()
            .map(|patient| patient.successes)
            .sum();
        let observed_proportions = request
            .patients
            .iter()
            .map(|patient| patient.successes as f64 / patient.trials as f64)
            .collect::<Vec<_>>();
        let observed_mean =
            observed_proportions.iter().sum::<f64>() / observed_proportions.len() as f64;
        let observed_sd = (observed_proportions
            .iter()
            .map(|value| (value - observed_mean).powi(2))
            .sum::<f64>()
            / (observed_proportions.len() - 1) as f64)
            .sqrt();
        if self.posterior_predictive.observed_total_successes != observed_total
            || ![
                self.posterior_predictive.replicated_total_successes_mean,
                self.posterior_predictive.replicated_total_successes_sd,
                self.posterior_predictive.observed_patient_proportion_sd,
                self.posterior_predictive
                    .replicated_patient_proportion_sd_mean,
            ]
            .iter()
            .all(|value| value.is_finite() && *value >= 0.0)
            || (self.posterior_predictive.observed_patient_proportion_sd - observed_sd).abs()
                > 1e-12
        {
            return Err(BayesError::WorkerContract(
                "beta-binomial posterior-predictive summary is invalid".into(),
            ));
        }
        let diagnostics_pass = diagnostic_pass(&self.diagnostics, &request.diagnostic_policy);
        if (self.fit_state == FitState::Complete) != diagnostics_pass {
            return Err(BayesError::WorkerContract(
                "beta-binomial fit state disagrees with diagnostics".into(),
            ));
        }
        Ok(())
    }

    pub fn into_result(
        self,
        request: BetaBinomialHierarchyWorkerRequest,
        input: BetaBinomialHierarchyInputIdentity,
    ) -> BetaBinomialHierarchyResult {
        BetaBinomialHierarchyResult {
            format: "marklab.bayesian_beta_binomial_hierarchy",
            version: 1,
            backend: self.backend,
            model: request.model,
            input,
            sampling: self.sampling,
            fit_state: self.fit_state,
            posterior: self.posterior,
            patients: self.patients,
            diagnostics: self.diagnostics,
            posterior_predictive: self.posterior_predictive,
            seed: request.sampling.seed,
            claim_status: if self.fit_state == FitState::Complete {
                "experimental_non_gaussian_hierarchy"
            } else {
                "diagnostic_only_nonconverged"
            },
            request_sha256: self.request_sha256,
        }
    }
}

fn validate_summary(summary: &SarScalarSummary, unit_interval: bool) -> Result<(), BayesError> {
    if ![
        summary.mean,
        summary.sd,
        summary.interval_lower,
        summary.interval_upper,
    ]
    .iter()
    .all(|value| value.is_finite())
        || summary.sd <= 0.0
        || summary.interval_lower > summary.interval_upper
        || (unit_interval
            && (!(0.0..=1.0).contains(&summary.mean)
                || !(0.0..=1.0).contains(&summary.interval_lower)
                || !(0.0..=1.0).contains(&summary.interval_upper)))
        || (!unit_interval && (summary.mean <= 0.0 || summary.interval_lower < 0.0))
    {
        return Err(BayesError::WorkerContract(
            "beta-binomial posterior summary is invalid".into(),
        ));
    }
    Ok(())
}

fn diagnostic_pass(diagnostics: &NormalMeanDiagnostics, policy: &DiagnosticPolicy) -> bool {
    diagnostics.prior_predictive_finite
        && diagnostics.posterior_finite
        && diagnostics.constraints_valid
        && diagnostics.identifiability_checks_passed
        && diagnostics.r_hat <= policy.maximum_r_hat
        && diagnostics.ess_bulk >= policy.minimum_bulk_ess
        && diagnostics.ess_tail >= policy.minimum_tail_ess
        && diagnostics.minimum_ebfmi >= policy.minimum_ebfmi
        && diagnostics.divergences <= policy.maximum_divergences
        && diagnostics.max_tree_depth_hits <= policy.maximum_tree_depth_hits
}

#[derive(Clone, Debug, Serialize)]
pub struct BetaBinomialHierarchyInputIdentity {
    pub path: String,
    pub patient_data_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct BetaBinomialHierarchyResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: BetaBinomialHierarchyModelIr,
    pub input: BetaBinomialHierarchyInputIdentity,
    pub sampling: SamplingSummary,
    pub fit_state: FitState,
    pub posterior: BetaBinomialHierarchyPosterior,
    pub patients: Vec<BetaBinomialPatientPosterior>,
    pub diagnostics: NormalMeanDiagnostics,
    pub posterior_predictive: BetaBinomialPosteriorPredictive,
    pub seed: u64,
    pub claim_status: &'static str,
    pub request_sha256: String,
}

pub fn patient_data_sha256(patients: &[BetaBinomialPatientData]) -> Result<String, BayesError> {
    Ok(sha256_hex(&serde_json::to_vec(patients)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(
        patients: Vec<BetaBinomialPatientData>,
    ) -> Result<BetaBinomialHierarchyWorkerRequest, BayesError> {
        BetaBinomialHierarchyWorkerRequest::new(
            BetaBinomialHierarchySpec {
                population_alpha: 2.0,
                population_beta: 2.0,
                concentration_prior_sd: 20.0,
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
    }

    #[test]
    fn canonicalizes_patient_order() {
        let request = request(vec![
            BetaBinomialPatientData {
                patient_id: "p-2".into(),
                successes: 2,
                trials: 5,
            },
            BetaBinomialPatientData {
                patient_id: "p-1".into(),
                successes: 1,
                trials: 5,
            },
            BetaBinomialPatientData {
                patient_id: "p-3".into(),
                successes: 3,
                trials: 5,
            },
        ])
        .expect("request");
        assert_eq!(request.patients[0].patient_id, "p-1");
        assert_eq!(request.patients[2].patient_id, "p-3");
    }

    #[test]
    fn rejects_successes_above_trials() {
        let error = request(vec![
            BetaBinomialPatientData {
                patient_id: "p-1".into(),
                successes: 6,
                trials: 5,
            },
            BetaBinomialPatientData {
                patient_id: "p-2".into(),
                successes: 1,
                trials: 5,
            },
            BetaBinomialPatientData {
                patient_id: "p-3".into(),
                successes: 1,
                trials: 5,
            },
        ])
        .expect_err("invalid successes");
        assert!(error.to_string().contains("patient rows"));
    }
}
