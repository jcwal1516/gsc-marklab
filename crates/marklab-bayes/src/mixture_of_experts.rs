use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{
    probability_transform::{clipped_logit, sigmoid},
    BackendContract, BayesError, WorkerBackend,
};

const SCIPY_VERSION: &str = "1.18.1";

#[derive(Clone, Debug, Serialize)]
pub struct MixtureOfExpertsPatient {
    pub patient_id: String,
    pub split: String,
    pub expert_prediction_source: String,
    pub label: u8,
    pub context: Vec<f64>,
    pub expert_probabilities: Vec<Option<f64>>,
}

#[derive(Clone, Debug)]
pub struct MixtureOfExpertsSpec {
    pub patients: Vec<MixtureOfExpertsPatient>,
    pub context_names: Vec<String>,
    pub expert_names: Vec<String>,
    pub l2_penalty: f64,
    pub entropy_regularization: f64,
    pub ood_validation_quantile: f64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct MixtureOfExpertsWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub patients: Vec<MixtureOfExpertsPatient>,
    pub context_names: Vec<String>,
    pub expert_names: Vec<String>,
    pub l2_penalty: f64,
    pub entropy_regularization: f64,
    pub ood_validation_quantile: f64,
    pub resources: MixtureOfExpertsResources,
}

#[derive(Clone, Debug, Serialize)]
pub struct MixtureOfExpertsResources {
    pub maximum_patients: u32,
    pub maximum_context_features: u32,
    pub maximum_experts: u32,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

impl MixtureOfExpertsWorkerRequest {
    pub fn new(
        mut spec: MixtureOfExpertsSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
    ) -> Result<Self, BayesError> {
        if !((30..=10_000).contains(&spec.patients.len())
            && (1..=16).contains(&spec.context_names.len())
            && (2..=8).contains(&spec.expert_names.len())
            && spec.l2_penalty.is_finite()
            && spec.l2_penalty > 0.0
            && spec.entropy_regularization.is_finite()
            && spec.entropy_regularization > 0.0
            && spec.entropy_regularization <= 1.0
            && 0.0 < spec.ood_validation_quantile
            && spec.ood_validation_quantile < 1.0
            && (1..=3_600).contains(&spec.timeout_seconds))
        {
            return Err(BayesError::InvalidSpec(
                "mixture-of-experts dimensions or controls are invalid".into(),
            ));
        }
        validate_names(&spec.context_names, "context_", true)?;
        validate_names(&spec.expert_names, "expert_", false)?;
        spec.patients
            .sort_by(|left, right| left.patient_id.cmp(&right.patient_id));
        let mut ids = HashSet::new();
        for patient in &spec.patients {
            if patient.patient_id.is_empty()
                || !ids.insert(patient.patient_id.as_str())
                || !matches!(
                    patient.split.as_str(),
                    "gate_train" | "calibration" | "test"
                )
                || patient.expert_prediction_source != "patient_level_out_of_fold"
                || patient.label > 1
                || patient.context.len() != spec.context_names.len()
                || patient.context.iter().any(|value| !value.is_finite())
                || patient.expert_probabilities.len() != spec.expert_names.len()
                || patient.expert_probabilities.iter().all(Option::is_none)
                || patient
                    .expert_probabilities
                    .iter()
                    .flatten()
                    .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
            {
                return Err(BayesError::InvalidSpec(
                    "mixture-of-experts patient row is invalid".into(),
                ));
            }
        }
        let counts = ["gate_train", "calibration", "test"].map(|split| {
            let rows = spec.patients.iter().filter(|row| row.split == split);
            let count = rows.clone().count();
            let positives = rows.filter(|row| row.label == 1).count();
            (count, positives)
        });
        if counts
            .iter()
            .any(|(count, positives)| *count < 8 || *positives == 0 || *positives == *count)
        {
            return Err(BayesError::InvalidSpec(
                "every mixture-of-experts split requires eight patients and both labels".into(),
            ));
        }
        Ok(Self {
            format: "marklab.scipy_mixture_of_experts_request",
            version: 1,
            backend: BackendContract {
                name: "scipy",
                version: SCIPY_VERSION,
                python_version: "3.12",
                environment_lock_sha256,
                worker_sha256,
            },
            patients: spec.patients,
            context_names: spec.context_names,
            expert_names: spec.expert_names,
            l2_penalty: spec.l2_penalty,
            entropy_regularization: spec.entropy_regularization,
            ood_validation_quantile: spec.ood_validation_quantile,
            resources: MixtureOfExpertsResources {
                maximum_patients: 10_000,
                maximum_context_features: 16,
                maximum_experts: 8,
                maximum_output_bytes: 16 * 1024 * 1024,
                timeout_seconds: spec.timeout_seconds,
            },
        })
    }
}

fn validate_names(names: &[String], prefix: &str, reject_site: bool) -> Result<(), BayesError> {
    let mut unique = HashSet::new();
    if names.iter().any(|name| {
        !name.starts_with(prefix)
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
            || !unique.insert(name.as_str())
            || (reject_site
                && ["site", "scanner", "stain", "batch"]
                    .iter()
                    .any(|token| name.contains(token)))
    }) {
        return Err(BayesError::InvalidSpec(format!(
            "mixture-of-experts {prefix} names are invalid or admit a technical shortcut"
        )));
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MixtureGateModel {
    pub context_training_mean: Vec<f64>,
    pub context_training_population_sd: Vec<f64>,
    pub gating_feature_names: Vec<String>,
    pub coefficients_row_major: Vec<f64>,
    pub coefficient_columns: u32,
    pub l2_penalty: f64,
    pub entropy_regularization: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MixtureCalibrator {
    pub intercept: f64,
    pub slope: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MixturePrediction {
    pub patient_id: String,
    pub label: u8,
    pub availability: Vec<bool>,
    pub gating_weights: Vec<f64>,
    pub raw_probability: f64,
    pub probability: f64,
    pub ood_score: f64,
    pub exceeds_ood_threshold: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MixtureMetrics {
    pub brier_score: f64,
    pub mean_gate_entropy: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MixtureOfExpertsWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub model: MixtureGateModel,
    pub calibrator: MixtureCalibrator,
    pub ood_threshold: f64,
    pub predictions: Vec<MixturePrediction>,
    pub metrics: MixtureMetrics,
}

impl MixtureOfExpertsWorkerResult {
    pub fn validate(
        &self,
        request: &MixtureOfExpertsWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        let gating_names = request
            .context_names
            .iter()
            .cloned()
            .chain(
                request
                    .expert_names
                    .iter()
                    .map(|name| format!("{name}_available")),
            )
            .collect::<Vec<_>>();
        let columns = 1 + gating_names.len();
        if self.format != "marklab.scipy_mixture_of_experts_worker_result"
            || self.version != 1
            || self.backend.name != "scipy"
            || self.backend.version != SCIPY_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
            || self.model.gating_feature_names != gating_names
            || self.model.coefficient_columns != columns as u32
            || self.model.coefficients_row_major.len() != columns * request.expert_names.len()
            || self.model.context_training_mean.len() != request.context_names.len()
            || self.model.context_training_population_sd.len() != request.context_names.len()
            || self
                .model
                .context_training_mean
                .iter()
                .any(|value| !value.is_finite())
            || self
                .model
                .context_training_population_sd
                .iter()
                .any(|value| !value.is_finite() || *value <= 0.0)
            || self
                .model
                .coefficients_row_major
                .iter()
                .any(|value| !value.is_finite())
            || self.model.l2_penalty.to_bits() != request.l2_penalty.to_bits()
            || self.model.entropy_regularization.to_bits()
                != request.entropy_regularization.to_bits()
            || !self.ood_threshold.is_finite()
            || self.ood_threshold < 0.0
            || !self.calibrator.intercept.is_finite()
            || !self.calibrator.slope.is_finite()
        {
            return Err(BayesError::WorkerContract(
                "mixture-of-experts worker identity or model differs".into(),
            ));
        }
        let calibration = request
            .patients
            .iter()
            .filter(|row| row.split == "calibration")
            .collect::<Vec<_>>();
        let mut calibration_ood = calibration
            .iter()
            .map(|row| ood_score(&self.model, &row.context))
            .collect::<Vec<_>>();
        calibration_ood.sort_by(f64::total_cmp);
        let rank = (request.ood_validation_quantile * calibration_ood.len() as f64).ceil() as usize;
        if !approximately_equal(self.ood_threshold, calibration_ood[rank - 1]) {
            return Err(BayesError::WorkerContract(
                "mixture-of-experts OOD threshold differs".into(),
            ));
        }
        let test = request
            .patients
            .iter()
            .filter(|row| row.split == "test")
            .collect::<Vec<_>>();
        if self.predictions.len() != test.len() {
            return Err(BayesError::WorkerContract(
                "mixture-of-experts prediction count differs".into(),
            ));
        }
        let mut squared = 0.0;
        for (index, (prediction, patient)) in self.predictions.iter().zip(test).enumerate() {
            let weights =
                gate_weights(&self.model, &patient.context, &patient.expert_probabilities);
            let raw = weights
                .iter()
                .zip(&patient.expert_probabilities)
                .map(|(weight, probability)| weight * probability.unwrap_or(0.0))
                .sum::<f64>();
            let probability =
                sigmoid(self.calibrator.intercept + self.calibrator.slope * clipped_logit(raw));
            let ood = ood_score(&self.model, &patient.context);
            let expected_availability = patient
                .expert_probabilities
                .iter()
                .map(Option::is_some)
                .collect::<Vec<_>>();
            let issue = if prediction.patient_id != patient.patient_id
                || prediction.label != patient.label
            {
                Some("identity")
            } else if prediction.availability != expected_availability {
                Some("availability")
            } else if prediction.gating_weights.len() != weights.len()
                || prediction
                    .gating_weights
                    .iter()
                    .zip(&weights)
                    .any(|(left, right)| !approximately_equal(*left, *right))
            {
                Some("gating weights")
            } else if !approximately_equal(prediction.raw_probability, raw) {
                Some("raw mixture probability")
            } else if !approximately_equal(prediction.probability, probability) {
                Some("calibrated probability")
            } else if !approximately_equal(prediction.ood_score, ood) {
                Some("OOD score")
            } else if prediction.exceeds_ood_threshold
                != (prediction.ood_score > self.ood_threshold)
            {
                Some("OOD threshold decision")
            } else {
                None
            };
            if let Some(issue) = issue {
                return Err(BayesError::WorkerContract(format!(
                    "mixture-of-experts prediction {index} {issue} differs"
                )));
            }
            squared += (probability - f64::from(patient.label)).powi(2);
        }
        let gate_train = request
            .patients
            .iter()
            .filter(|row| row.split == "gate_train");
        let mean_entropy = gate_train
            .map(|patient| {
                gate_weights(&self.model, &patient.context, &patient.expert_probabilities)
                    .into_iter()
                    .filter(|weight| *weight > 0.0)
                    .map(|weight| -weight * weight.ln())
                    .sum::<f64>()
            })
            .sum::<f64>()
            / request
                .patients
                .iter()
                .filter(|row| row.split == "gate_train")
                .count() as f64;
        if !approximately_equal(
            self.metrics.brier_score,
            squared / self.predictions.len() as f64,
        ) || !approximately_equal(self.metrics.mean_gate_entropy, mean_entropy)
        {
            return Err(BayesError::WorkerContract(
                "mixture-of-experts metrics differ".into(),
            ));
        }
        Ok(())
    }
}

fn gate_weights(
    model: &MixtureGateModel,
    context: &[f64],
    probabilities: &[Option<f64>],
) -> Vec<f64> {
    let mut features = context
        .iter()
        .zip(&model.context_training_mean)
        .zip(&model.context_training_population_sd)
        .map(|((value, mean), sd)| (value - mean) / sd)
        .collect::<Vec<_>>();
    features.extend(probabilities.iter().map(|value| f64::from(value.is_some())));
    let columns = model.coefficient_columns as usize;
    let mut logits = model
        .coefficients_row_major
        .chunks_exact(columns)
        .zip(probabilities)
        .map(|(coefficients, probability)| {
            if probability.is_none() {
                f64::NEG_INFINITY
            } else {
                coefficients[0]
                    + coefficients[1..]
                        .iter()
                        .zip(&features)
                        .map(|(coefficient, value)| coefficient * value)
                        .sum::<f64>()
            }
        })
        .collect::<Vec<_>>();
    let maximum = logits.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    for value in &mut logits {
        *value = (*value - maximum).exp();
    }
    let total = logits.iter().sum::<f64>();
    for value in &mut logits {
        *value /= total;
    }
    logits
}

fn ood_score(model: &MixtureGateModel, context: &[f64]) -> f64 {
    context
        .iter()
        .zip(&model.context_training_mean)
        .zip(&model.context_training_population_sd)
        .map(|((value, mean), sd)| ((value - mean) / sd).powi(2))
        .sum::<f64>()
        .sqrt()
}

fn approximately_equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-9 * (1.0 + left.abs().max(right.abs()))
}
