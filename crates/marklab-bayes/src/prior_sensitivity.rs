use crate::validation::all_finite as finite;

use std::f64::consts::PI;

use serde::{Deserialize, Serialize};

use crate::{
    model::{BackendContract, WORKER_REQUEST_FORMAT, WORKER_REQUEST_VERSION},
    BayesError, WorkerBackend,
};

const SENSITIVITY_BACKEND_VERSION: &str = "scipy-1.18.1";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NormalPriorAlternative {
    pub prior_name: String,
    pub prior_mean: f64,
    pub prior_sd: f64,
}

#[derive(Clone, Debug)]
pub struct NormalMeanPriorSensitivitySpec {
    pub observations: Vec<f64>,
    pub priors: Vec<NormalPriorAlternative>,
    pub base_prior: String,
    pub known_sigma: f64,
    pub decision_threshold: f64,
    pub decision_probability_threshold: f64,
    pub material_mean_shift: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PriorSensitivityModelIr {
    pub format: &'static str,
    pub version: u32,
    pub family: &'static str,
    pub likelihood: &'static str,
    pub known_sigma: f64,
    pub posterior_method: &'static str,
    pub predictive_method: &'static str,
    pub decision_quantity: &'static str,
    pub decision_threshold: f64,
    pub decision_probability_threshold: f64,
    pub material_mean_shift: f64,
    pub backend_capability: &'static str,
    pub maturity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct PriorSensitivityResourceLimits {
    pub maximum_observations: u32,
    pub maximum_priors: u32,
    pub maximum_work_units: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PriorSensitivityWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub model: PriorSensitivityModelIr,
    pub observations: Vec<f64>,
    pub priors: Vec<NormalPriorAlternative>,
    pub base_prior: String,
    pub resources: PriorSensitivityResourceLimits,
}

impl PriorSensitivityWorkerRequest {
    pub fn new(
        mut spec: NormalMeanPriorSensitivitySpec,
        environment_lock_sha256: String,
        worker_sha256: String,
        timeout_seconds: u64,
    ) -> Result<Self, BayesError> {
        if !(2..=100_000).contains(&spec.observations.len())
            || spec.observations.iter().any(|value| !value.is_finite())
            || !(2..=32).contains(&spec.priors.len())
            || !spec.known_sigma.is_finite()
            || spec.known_sigma <= 0.0
            || !spec.decision_threshold.is_finite()
            || !spec.decision_probability_threshold.is_finite()
            || spec.decision_probability_threshold <= 0.5
            || spec.decision_probability_threshold >= 1.0
            || !spec.material_mean_shift.is_finite()
            || spec.material_mean_shift <= 0.0
            || !(1..=3_600).contains(&timeout_seconds)
        {
            return Err(BayesError::InvalidSpec(
                "normal-mean prior-sensitivity inputs or controls are invalid".into(),
            ));
        }
        spec.priors
            .sort_by(|left, right| left.prior_name.cmp(&right.prior_name));
        for (index, prior) in spec.priors.iter().enumerate() {
            if prior.prior_name.is_empty()
                || prior.prior_name.len() > 128
                || prior.prior_name.trim() != prior.prior_name
                || !prior.prior_mean.is_finite()
                || !prior.prior_sd.is_finite()
                || prior.prior_sd <= 0.0
                || (index > 0 && spec.priors[index - 1].prior_name == prior.prior_name)
            {
                return Err(BayesError::InvalidSpec(
                    "prior alternatives require unique exact names and finite Normal parameters"
                        .into(),
                ));
            }
        }
        if !spec
            .priors
            .iter()
            .any(|prior| prior.prior_name == spec.base_prior)
        {
            return Err(BayesError::InvalidSpec(
                "base prior must name exactly one supplied prior".into(),
            ));
        }
        let maximum_work_units = 3_200_000;
        let work = spec.observations.len() as u64 * spec.priors.len() as u64;
        if work > maximum_work_units {
            return Err(BayesError::InvalidSpec(
                "normal-mean prior-sensitivity work exceeds 3200000 units".into(),
            ));
        }
        Ok(Self {
            format: WORKER_REQUEST_FORMAT,
            version: WORKER_REQUEST_VERSION,
            backend: BackendContract {
                name: "scipy",
                version: SENSITIVITY_BACKEND_VERSION,
                python_version: "3.12",
                environment_lock_sha256,
                worker_sha256,
            },
            model: PriorSensitivityModelIr {
                format: "marklab.bayesian_model_ir",
                version: 1,
                family: "normal_mean_known_sigma",
                likelihood: "normal_known_sigma",
                known_sigma: spec.known_sigma,
                posterior_method: "exact_conjugate",
                predictive_method: "exact_leave_one_out_normal",
                decision_quantity: "posterior_probability_mu_above_threshold",
                decision_threshold: spec.decision_threshold,
                decision_probability_threshold: spec.decision_probability_threshold,
                material_mean_shift: spec.material_mean_shift,
                backend_capability: "prior_sensitivity",
                maturity: "experimental_sensitivity",
            },
            observations: spec.observations,
            priors: spec.priors,
            base_prior: spec.base_prior,
            resources: PriorSensitivityResourceLimits {
                maximum_observations: 100_000,
                maximum_priors: 32,
                maximum_work_units,
                maximum_output_bytes: 1_048_576,
                timeout_seconds,
            },
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PriorSensitivitySummary {
    pub prior_name: String,
    pub prior_mean: f64,
    pub prior_sd: f64,
    pub posterior_mean: f64,
    pub posterior_sd: f64,
    pub probability_above_threshold: f64,
    pub decision: bool,
    pub loo_elpd: f64,
    pub mean_difference_from_base: f64,
    pub sd_difference_from_base: f64,
    pub probability_difference_from_base: f64,
    pub loo_elpd_difference_from_base: f64,
    pub conclusion_changed: bool,
    pub material_mean_shift: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PriorSensitivityWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub observation_count: usize,
    pub observed_mean: f64,
    pub priors: Vec<PriorSensitivitySummary>,
    pub conclusion_changed_priors: Vec<String>,
    pub material_mean_shift_priors: Vec<String>,
}

impl PriorSensitivityWorkerResult {
    pub fn validate(
        &self,
        request: &PriorSensitivityWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        let observed_mean =
            request.observations.iter().sum::<f64>() / request.observations.len() as f64;
        if self.format != "marklab.scipy_prior_sensitivity_worker_result"
            || self.version != 1
            || self.backend.name != "scipy"
            || self.backend.version != SENSITIVITY_BACKEND_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
            || self.observation_count != request.observations.len()
            || !approximately_equal(self.observed_mean, observed_mean)
            || self.priors.len() != request.priors.len()
        {
            return Err(BayesError::WorkerContract(
                "prior-sensitivity result identity or dimensions mismatch".into(),
            ));
        }
        let base_index = request
            .priors
            .iter()
            .position(|prior| prior.prior_name == request.base_prior)
            .expect("validated base prior");
        let mut expected = request
            .priors
            .iter()
            .map(|prior| exact_summary(request, prior))
            .collect::<Vec<_>>();
        for (expected, actual) in expected.iter_mut().zip(&self.priors) {
            expected.probability_above_threshold = actual.probability_above_threshold;
            expected.decision =
                actual.probability_above_threshold >= request.model.decision_probability_threshold;
        }
        let base = expected[base_index].clone();
        let mut changed = Vec::new();
        let mut shifted = Vec::new();
        for summary in &mut expected {
            summary.mean_difference_from_base = summary.posterior_mean - base.posterior_mean;
            summary.sd_difference_from_base = summary.posterior_sd - base.posterior_sd;
            summary.probability_difference_from_base =
                summary.probability_above_threshold - base.probability_above_threshold;
            summary.loo_elpd_difference_from_base = summary.loo_elpd - base.loo_elpd;
            summary.conclusion_changed = summary.decision != base.decision;
            summary.material_mean_shift =
                summary.mean_difference_from_base.abs() >= request.model.material_mean_shift;
            if summary.conclusion_changed {
                changed.push(summary.prior_name.clone());
            }
            if summary.material_mean_shift {
                shifted.push(summary.prior_name.clone());
            }
        }
        for (actual, expected) in self.priors.iter().zip(&expected) {
            if actual.prior_name != expected.prior_name
                || actual.decision != expected.decision
                || actual.conclusion_changed != expected.conclusion_changed
                || actual.material_mean_shift != expected.material_mean_shift
                || !finite(&[
                    actual.prior_mean,
                    actual.prior_sd,
                    actual.posterior_mean,
                    actual.posterior_sd,
                    actual.probability_above_threshold,
                    actual.loo_elpd,
                    actual.mean_difference_from_base,
                    actual.sd_difference_from_base,
                    actual.probability_difference_from_base,
                    actual.loo_elpd_difference_from_base,
                ])
                || !(0.0..=1.0).contains(&actual.probability_above_threshold)
                || !approximately_equal(actual.prior_mean, expected.prior_mean)
                || !approximately_equal(actual.prior_sd, expected.prior_sd)
                || !approximately_equal(actual.posterior_mean, expected.posterior_mean)
                || !approximately_equal(actual.posterior_sd, expected.posterior_sd)
                || !approximately_equal(actual.loo_elpd, expected.loo_elpd)
                || !approximately_equal(
                    actual.mean_difference_from_base,
                    expected.mean_difference_from_base,
                )
                || !approximately_equal(
                    actual.sd_difference_from_base,
                    expected.sd_difference_from_base,
                )
                || !approximately_equal(
                    actual.probability_difference_from_base,
                    expected.probability_difference_from_base,
                )
                || !approximately_equal(
                    actual.loo_elpd_difference_from_base,
                    expected.loo_elpd_difference_from_base,
                )
            {
                return Err(BayesError::WorkerContract(
                    "prior-sensitivity summary is invalid".into(),
                ));
            }
        }
        if self.conclusion_changed_priors != changed || self.material_mean_shift_priors != shifted {
            return Err(BayesError::WorkerContract(
                "prior-sensitivity flags disagree with summaries".into(),
            ));
        }
        Ok(())
    }

    pub fn into_result(
        self,
        request: PriorSensitivityWorkerRequest,
        input: PriorSensitivityInputIdentity,
    ) -> PriorSensitivityResult {
        PriorSensitivityResult {
            format: "marklab.bayesian_normal_mean_prior_sensitivity",
            version: 1,
            backend: self.backend,
            model: request.model,
            input,
            base_prior: request.base_prior,
            observation_count: self.observation_count,
            observed_mean: self.observed_mean,
            priors: self.priors,
            conclusion_changed_priors: self.conclusion_changed_priors.clone(),
            material_mean_shift_priors: self.material_mean_shift_priors,
            sensitivity_state: if self.conclusion_changed_priors.is_empty() {
                "decision_stable"
            } else {
                "decision_sensitive"
            },
            claim_status: "experimental_sensitivity_analysis",
            request_sha256: self.request_sha256,
        }
    }
}

fn exact_summary(
    request: &PriorSensitivityWorkerRequest,
    prior: &NormalPriorAlternative,
) -> PriorSensitivitySummary {
    let prior_variance = prior.prior_sd.powi(2);
    let observation_variance = request.model.known_sigma.powi(2);
    let posterior_variance =
        1.0 / (1.0 / prior_variance + request.observations.len() as f64 / observation_variance);
    let posterior_mean = posterior_variance
        * (prior.prior_mean / prior_variance
            + request.observations.iter().sum::<f64>() / observation_variance);
    let mut loo_elpd = 0.0;
    let total = request.observations.iter().sum::<f64>();
    for observation in &request.observations {
        let variance = 1.0
            / (1.0 / prior_variance
                + (request.observations.len() - 1) as f64 / observation_variance);
        let mean = variance
            * (prior.prior_mean / prior_variance + (total - observation) / observation_variance);
        let predictive_variance = observation_variance + variance;
        loo_elpd += -0.5 * (2.0 * PI * predictive_variance).ln()
            - 0.5 * (observation - mean).powi(2) / predictive_variance;
    }
    PriorSensitivitySummary {
        prior_name: prior.prior_name.clone(),
        prior_mean: prior.prior_mean,
        prior_sd: prior.prior_sd,
        posterior_mean,
        posterior_sd: posterior_variance.sqrt(),
        probability_above_threshold: 0.0,
        decision: false,
        loo_elpd,
        mean_difference_from_base: 0.0,
        sd_difference_from_base: 0.0,
        probability_difference_from_base: 0.0,
        loo_elpd_difference_from_base: 0.0,
        conclusion_changed: false,
        material_mean_shift: false,
    }
}

fn approximately_equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-10 * left.abs().max(right.abs()).max(1.0)
}

#[derive(Debug, Serialize)]
pub struct PriorSensitivityInputIdentity {
    pub observations_path: String,
    pub priors_path: String,
    pub observations_sha256: String,
    pub priors_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct PriorSensitivityResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: WorkerBackend,
    pub model: PriorSensitivityModelIr,
    pub input: PriorSensitivityInputIdentity,
    pub base_prior: String,
    pub observation_count: usize,
    pub observed_mean: f64,
    pub priors: Vec<PriorSensitivitySummary>,
    pub conclusion_changed_priors: Vec<String>,
    pub material_mean_shift_priors: Vec<String>,
    pub sensitivity_state: &'static str,
    pub claim_status: &'static str,
    pub request_sha256: String,
}
