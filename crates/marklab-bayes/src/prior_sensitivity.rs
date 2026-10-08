use crate::validation::all_finite as finite;

use std::{
    f64::consts::PI,
    time::{Duration, Instant},
};

use statrs::distribution::{ContinuousCDF, Normal};

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

#[derive(Clone, Debug, Serialize)]
pub struct NormalMeanPriorSensitivitySpec {
    pub observations: Vec<f64>,
    pub priors: Vec<NormalPriorAlternative>,
    pub base_prior: String,
    pub known_sigma: f64,
    pub decision_threshold: f64,
    pub decision_probability_threshold: f64,
    pub material_mean_shift: f64,
}

impl NormalMeanPriorSensitivitySpec {
    fn validate_and_sort(&mut self, timeout_seconds: u64) -> Result<(), BayesError> {
        if !(2..=100_000).contains(&self.observations.len())
            || self.observations.iter().any(|value| !value.is_finite())
            || !(2..=32).contains(&self.priors.len())
            || !self.known_sigma.is_finite()
            || self.known_sigma <= 0.0
            || !self.decision_threshold.is_finite()
            || !self.decision_probability_threshold.is_finite()
            || self.decision_probability_threshold <= 0.5
            || self.decision_probability_threshold >= 1.0
            || !self.material_mean_shift.is_finite()
            || self.material_mean_shift <= 0.0
            || !(1..=3_600).contains(&timeout_seconds)
        {
            return Err(BayesError::InvalidSpec(
                "normal-mean prior-sensitivity inputs or controls are invalid".into(),
            ));
        }
        self.priors
            .sort_by(|left, right| left.prior_name.cmp(&right.prior_name));
        for (index, prior) in self.priors.iter().enumerate() {
            if prior.prior_name.is_empty()
                || prior.prior_name.len() > 128
                || prior.prior_name.trim() != prior.prior_name
                || !prior.prior_mean.is_finite()
                || !prior.prior_sd.is_finite()
                || prior.prior_sd <= 0.0
                || (index > 0 && self.priors[index - 1].prior_name == prior.prior_name)
            {
                return Err(BayesError::InvalidSpec(
                    "prior alternatives require unique exact names and finite Normal parameters"
                        .into(),
                ));
            }
        }
        if !self
            .priors
            .iter()
            .any(|prior| prior.prior_name == self.base_prior)
        {
            return Err(BayesError::InvalidSpec(
                "base prior must name exactly one supplied prior".into(),
            ));
        }
        let maximum_work_units = 3_200_000;
        let work = self.observations.len() as u64 * self.priors.len() as u64;
        if work > maximum_work_units {
            return Err(BayesError::InvalidSpec(
                "normal-mean prior-sensitivity work exceeds 3200000 units".into(),
            ));
        }
        Ok(())
    }

    fn model_ir(&self) -> PriorSensitivityModelIr {
        PriorSensitivityModelIr {
            format: "marklab.bayesian_model_ir",
            version: 1,
            family: "normal_mean_known_sigma",
            likelihood: "normal_known_sigma",
            known_sigma: self.known_sigma,
            posterior_method: "exact_conjugate",
            predictive_method: "exact_leave_one_out_normal",
            decision_quantity: "posterior_probability_mu_above_threshold",
            decision_threshold: self.decision_threshold,
            decision_probability_threshold: self.decision_probability_threshold,
            material_mean_shift: self.material_mean_shift,
            backend_capability: "prior_sensitivity",
            maturity: "experimental_sensitivity",
        }
    }
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
        spec.validate_and_sort(timeout_seconds)?;
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
            model: spec.model_ir(),
            observations: spec.observations,
            priors: spec.priors,
            base_prior: spec.base_prior,
            resources: PriorSensitivityResourceLimits {
                maximum_observations: 100_000,
                maximum_priors: 32,
                maximum_work_units: 3_200_000,
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
            .map(|prior| exact_summary(&request.observations, &request.model, prior))
            .collect::<Result<Vec<_>, _>>()?;
        for (expected, actual) in expected.iter_mut().zip(&self.priors) {
            // The cutoff can separate valid Rust/SciPy rounding. Check the reported
            // decision against the worker probability, independently validated below.
            expected.decision =
                actual.probability_above_threshold >= request.model.decision_probability_threshold;
        }
        let (changed, shifted) =
            compare_priors(&mut expected, base_index, request.model.material_mean_shift);
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
                || !approximately_equal(
                    actual.probability_above_threshold,
                    expected.probability_above_threshold,
                )
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
    observations: &[f64],
    model: &PriorSensitivityModelIr,
    prior: &NormalPriorAlternative,
) -> Result<PriorSensitivitySummary, BayesError> {
    let prior_variance = prior.prior_sd.powi(2);
    let observation_variance = model.known_sigma.powi(2);
    let total = observations.iter().sum::<f64>();
    if !finite(&[prior_variance, observation_variance, total])
        || prior_variance <= 0.0
        || observation_variance <= 0.0
    {
        return Err(BayesError::Numerical(
            "prior-sensitivity variances or observation sum are not representable".into(),
        ));
    }
    let posterior_variance =
        1.0 / (1.0 / prior_variance + observations.len() as f64 / observation_variance);
    let posterior_mean =
        posterior_variance * (prior.prior_mean / prior_variance + total / observation_variance);
    let posterior_sd = posterior_variance.sqrt();
    if !finite(&[posterior_mean, posterior_sd]) || posterior_sd <= 0.0 {
        return Err(BayesError::Numerical(
            "prior-sensitivity posterior is not representable".into(),
        ));
    }
    let posterior = Normal::new(posterior_mean, posterior_sd)
        .map_err(|error| BayesError::Numerical(error.to_string()))?;
    // Use the survival function directly so small upper-tail probabilities do not cancel.
    let probability = posterior.sf(model.decision_threshold);
    let leave_variance =
        1.0 / (1.0 / prior_variance + (observations.len() - 1) as f64 / observation_variance);
    let predictive_sd = (observation_variance + leave_variance).sqrt();
    if !predictive_sd.is_finite() || predictive_sd <= 0.0 {
        return Err(BayesError::Numerical(
            "prior-sensitivity predictive scale is not representable".into(),
        ));
    }
    let mut loo_elpd = 0.0;
    for observation in observations {
        let mean = leave_variance
            * (prior.prior_mean / prior_variance + (total - observation) / observation_variance);
        let standardized = (observation - mean) / predictive_sd;
        loo_elpd += -0.5 * (2.0 * PI).ln() - predictive_sd.ln() - 0.5 * standardized.powi(2);
    }
    if !finite(&[probability, loo_elpd]) || !(0.0..=1.0).contains(&probability) {
        return Err(BayesError::Numerical(
            "prior-sensitivity probability or predictive score is not finite".into(),
        ));
    }
    Ok(PriorSensitivitySummary {
        prior_name: prior.prior_name.clone(),
        prior_mean: prior.prior_mean,
        prior_sd: prior.prior_sd,
        posterior_mean,
        posterior_sd,
        probability_above_threshold: probability,
        decision: probability >= model.decision_probability_threshold,
        loo_elpd,
        mean_difference_from_base: 0.0,
        sd_difference_from_base: 0.0,
        probability_difference_from_base: 0.0,
        loo_elpd_difference_from_base: 0.0,
        conclusion_changed: false,
        material_mean_shift: false,
    })
}

fn compare_priors(
    summaries: &mut [PriorSensitivitySummary],
    base_index: usize,
    material_mean_shift: f64,
) -> (Vec<String>, Vec<String>) {
    let base = summaries[base_index].clone();
    let mut changed = Vec::new();
    let mut shifted = Vec::new();
    for summary in summaries {
        summary.mean_difference_from_base = summary.posterior_mean - base.posterior_mean;
        summary.sd_difference_from_base = summary.posterior_sd - base.posterior_sd;
        summary.probability_difference_from_base =
            summary.probability_above_threshold - base.probability_above_threshold;
        summary.loo_elpd_difference_from_base = summary.loo_elpd - base.loo_elpd;
        summary.conclusion_changed = summary.decision != base.decision;
        summary.material_mean_shift =
            summary.mean_difference_from_base.abs() >= material_mean_shift;
        if summary.conclusion_changed {
            changed.push(summary.prior_name.clone());
        }
        if summary.material_mean_shift {
            shifted.push(summary.prior_name.clone());
        }
    }
    (changed, shifted)
}

/// Exact native analysis of a caller-supplied Normal prior grid for a known observation SD.
///
/// All priors are retained in name order. Inputs are limited to 2–100,000 observations and 2–32
/// priors; the timeout is checked between prior calculations and before returning a result.
/// No sampling, external runtime, or outcome-based prior selection is performed.
pub fn evaluate_normal_mean_prior_sensitivity(
    mut spec: NormalMeanPriorSensitivitySpec,
    timeout_seconds: u64,
) -> Result<NormalMeanPriorSensitivityResult, BayesError> {
    let started = Instant::now();
    spec.validate_and_sort(timeout_seconds)?;
    let model = spec.model_ir();
    let mut priors = Vec::with_capacity(spec.priors.len());
    let check_time = || {
        if started.elapsed() >= Duration::from_secs(timeout_seconds) {
            Err(BayesError::InvalidSpec(format!(
                "native prior sensitivity exceeded the {timeout_seconds}-second limit",
            )))
        } else {
            Ok(())
        }
    };
    for prior in &spec.priors {
        check_time()?;
        priors.push(exact_summary(&spec.observations, &model, prior)?);
    }
    let base_index = spec
        .priors
        .iter()
        .position(|prior| prior.prior_name == spec.base_prior)
        .expect("validated base prior");
    let (changed, shifted) = compare_priors(&mut priors, base_index, spec.material_mean_shift);
    if priors.iter().any(|prior| {
        !finite(&[
            prior.mean_difference_from_base,
            prior.sd_difference_from_base,
            prior.probability_difference_from_base,
            prior.loo_elpd_difference_from_base,
        ])
    }) {
        return Err(BayesError::Numerical(
            "prior-sensitivity differences are not finite".into(),
        ));
    }
    let request_sha256 = crate::sha256_hex(&serde_json::to_vec(&(
        "marklab.normal_mean_prior_sensitivity_native",
        2,
        &spec,
        timeout_seconds,
    ))?);
    let observed_mean = spec.observations.iter().sum::<f64>() / spec.observations.len() as f64;
    check_time()?;
    Ok(NormalMeanPriorSensitivityResult {
        format: "marklab.bayesian_normal_mean_prior_sensitivity",
        version: 2,
        backend: "native_rust",
        model,
        base_prior: spec.base_prior,
        observation_count: spec.observations.len(),
        observed_mean,
        priors,
        sensitivity_state: if changed.is_empty() {
            "decision_stable"
        } else {
            "decision_sensitive"
        },
        conclusion_changed_priors: changed,
        material_mean_shift_priors: shifted,
        claim_status: "experimental_sensitivity_analysis",
        request_sha256,
    })
}

/// Native scientific result; file adapters add their own source paths and input identities.
#[derive(Debug, Serialize)]
pub struct NormalMeanPriorSensitivityResult {
    pub format: &'static str,
    pub version: u32,
    pub backend: &'static str,
    pub model: PriorSensitivityModelIr,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> NormalMeanPriorSensitivitySpec {
        NormalMeanPriorSensitivitySpec {
            observations: vec![1.0, 2.0, 3.0, 4.0],
            priors: vec![
                NormalPriorAlternative {
                    prior_name: "skeptical".into(),
                    prior_mean: -2.0,
                    prior_sd: 0.5,
                },
                NormalPriorAlternative {
                    prior_name: "base".into(),
                    prior_mean: 0.0,
                    prior_sd: 1.0,
                },
            ],
            base_prior: "base".into(),
            known_sigma: 1.0,
            decision_threshold: 1.0,
            decision_probability_threshold: 0.95,
            material_mean_shift: 0.5,
        }
    }

    #[test]
    fn native_analysis_matches_conjugate_oracle_and_canonicalizes_priors() {
        let mut specification = spec();
        let result = evaluate_normal_mean_prior_sensitivity(specification.clone(), 60).unwrap();
        let base = &result.priors[0];
        assert_eq!(base.prior_name, "base");
        assert_eq!(base.posterior_mean, 2.0);
        assert!((base.posterior_sd - 0.2_f64.sqrt()).abs() < 1e-14);
        assert!((base.probability_above_threshold - 0.9873263406612659).abs() < 1e-12);
        // Four exact predictive log densities, each with variance 1 + 1/4.
        let squared_residuals = 2.0 * 1.25_f64.powi(2) + 2.5_f64.powi(2);
        let expected_loo = -2.0 * (2.0 * PI * 1.25).ln() - squared_residuals / (2.0 * 1.25);
        assert!((base.loo_elpd - expected_loo).abs() < 1e-12);
        assert_eq!(result.conclusion_changed_priors, ["skeptical"]);
        assert_eq!(result.material_mean_shift_priors, ["skeptical"]);
        assert_eq!(result.priors[1].posterior_mean, 0.25);
        assert_eq!(result.priors[1].mean_difference_from_base, -1.75);
        assert_eq!(result.sensitivity_state, "decision_sensitive");

        specification.priors.reverse();
        let reordered = evaluate_normal_mean_prior_sensitivity(specification, 60).unwrap();
        assert_eq!(
            serde_json::to_vec(&result).unwrap(),
            serde_json::to_vec(&reordered).unwrap()
        );
    }

    #[test]
    fn native_analysis_preserves_small_upper_tails_and_inclusive_decision_threshold() {
        let mut specification = spec();
        specification.decision_threshold = 2.0 + 10.0 * 0.2_f64.sqrt();
        let result = evaluate_normal_mean_prior_sensitivity(specification.clone(), 60).unwrap();
        let probability = result.priors[0].probability_above_threshold;
        let expected = 7.61985302416047e-24;
        assert!(
            (probability / expected - 1.0).abs() < 1e-10,
            "{probability}"
        );
        assert_eq!(result.sensitivity_state, "decision_stable");

        specification.decision_threshold = 1.0;
        let result = evaluate_normal_mean_prior_sensitivity(specification.clone(), 60).unwrap();
        specification.decision_probability_threshold = result.priors[0].probability_above_threshold;
        specification.material_mean_shift = 1.75;
        let on_boundary = evaluate_normal_mean_prior_sensitivity(specification, 60).unwrap();
        assert!(on_boundary.priors[0].decision);
        assert!(on_boundary.priors[1].material_mean_shift);
    }

    type SpecMutation = fn(&mut NormalMeanPriorSensitivitySpec);

    #[test]
    fn native_analysis_rejects_invalid_admission_and_unrepresentable_results() {
        let cases: &[(&str, SpecMutation)] = &[
            ("too few observations", |s| s.observations.truncate(1)),
            ("too many observations", |s| {
                s.observations.resize(100_001, 0.0)
            }),
            ("nonfinite observation", |s| s.observations[0] = f64::NAN),
            ("too few priors", |s| s.priors.truncate(1)),
            ("too many priors", |s| {
                s.priors.resize(33, s.priors[0].clone())
            }),
            ("duplicate prior", |s| {
                s.priors[0].prior_name = "base".into()
            }),
            ("inexact prior name", |s| {
                s.priors[0].prior_name = " base".into()
            }),
            ("unknown base", |s| s.base_prior = "missing".into()),
            ("zero sigma", |s| s.known_sigma = 0.0),
            ("negative prior SD", |s| s.priors[0].prior_sd = -1.0),
            ("nonfinite prior", |s| {
                s.priors[0].prior_mean = f64::INFINITY
            }),
            ("nonfinite threshold", |s| {
                s.decision_threshold = f64::INFINITY
            }),
            ("probability lower bound", |s| {
                s.decision_probability_threshold = 0.5
            }),
            ("probability upper bound", |s| {
                s.decision_probability_threshold = 1.0
            }),
            ("zero material shift", |s| s.material_mean_shift = 0.0),
        ];
        for (label, invalidate) in cases {
            let mut specification = spec();
            invalidate(&mut specification);
            assert!(
                matches!(
                    evaluate_normal_mean_prior_sensitivity(specification, 60),
                    Err(BayesError::InvalidSpec(_))
                ),
                "{label}"
            );
        }
        for timeout in [0, 3_601] {
            assert!(matches!(
                evaluate_normal_mean_prior_sensitivity(spec(), timeout),
                Err(BayesError::InvalidSpec(_))
            ));
        }
        for (sigma, observations) in [
            (f64::MAX, vec![1.0, 2.0]),
            (f64::MIN_POSITIVE, vec![1.0, 2.0]),
            (1.0, vec![f64::MAX, f64::MAX]),
            (1.0, vec![1e200, -1e200]),
        ] {
            let mut specification = spec();
            specification.known_sigma = sigma;
            specification.observations = observations;
            assert!(matches!(
                evaluate_normal_mean_prior_sensitivity(specification, 60),
                Err(BayesError::Numerical(_))
            ));
        }
    }
}
