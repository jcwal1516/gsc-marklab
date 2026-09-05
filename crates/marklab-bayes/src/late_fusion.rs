use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{
    probability_transform::{clipped_logit, sigmoid},
    BackendContract, BayesError, WorkerBackend,
};

mod native;
pub use native::{fit_late_fusion, LateFusionFit, NativeFusionBackend};

const SCIPY_VERSION: &str = "1.18.1";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LateFusionPatient {
    pub patient_id: String,
    pub split: String,
    pub base_prediction_source: String,
    pub label: u8,
    pub modality_probabilities: Vec<Option<f64>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LateFusionSpec {
    pub patients: Vec<LateFusionPatient>,
    pub modalities: Vec<String>,
    pub l2_penalty: f64,
    pub timeout_seconds: u64,
}

impl LateFusionSpec {
    pub(crate) fn validated(mut self) -> Result<Self, BayesError> {
        if !((30..=100_000).contains(&self.patients.len())
            && (2..=16).contains(&self.modalities.len())
            && self.l2_penalty.is_finite()
            && self.l2_penalty > 0.0
            && (1..=3_600).contains(&self.timeout_seconds))
        {
            return Err(BayesError::InvalidSpec(
                "late-fusion dimensions or controls are invalid".into(),
            ));
        }
        let mut modalities = HashSet::new();
        if self
            .modalities
            .iter()
            .any(|name| !name.starts_with("modality_") || !modalities.insert(name.as_str()))
        {
            return Err(BayesError::InvalidSpec(
                "late-fusion modality names are invalid".into(),
            ));
        }
        self.patients
            .sort_by(|left, right| left.patient_id.cmp(&right.patient_id));
        let mut ids = HashSet::new();
        for patient in &self.patients {
            if patient.patient_id.is_empty()
                || !ids.insert(patient.patient_id.as_str())
                || !matches!(
                    patient.split.as_str(),
                    "meta_train" | "calibration" | "test"
                )
                || patient.base_prediction_source != "patient_level_out_of_fold"
                || patient.label > 1
                || patient.modality_probabilities.len() != self.modalities.len()
                || patient.modality_probabilities.iter().all(Option::is_none)
                || patient
                    .modality_probabilities
                    .iter()
                    .flatten()
                    .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
            {
                return Err(BayesError::InvalidSpec(
                    "late-fusion patient row is invalid".into(),
                ));
            }
        }
        for split in ["meta_train", "calibration", "test"] {
            let rows = self.patients.iter().filter(|row| row.split == split);
            let count = rows.clone().count();
            let positives = rows.filter(|row| row.label == 1).count();
            if count < 10 || positives == 0 || positives == count {
                return Err(BayesError::InvalidSpec(format!(
                    "late-fusion split {split} requires ten patients and both labels"
                )));
            }
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct LateFusionWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub base_prediction_source: &'static str,
    pub patients: Vec<LateFusionPatient>,
    pub modalities: Vec<String>,
    pub l2_penalty: f64,
    pub resources: LateFusionResources,
}

#[derive(Clone, Debug, Serialize)]
pub struct LateFusionResources {
    pub maximum_patients: u32,
    pub maximum_modalities: u32,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

impl LateFusionWorkerRequest {
    pub fn new(
        spec: LateFusionSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
    ) -> Result<Self, BayesError> {
        let spec = spec.validated()?;
        Ok(Self {
            format: "marklab.scipy_late_fusion_request",
            version: 1,
            backend: BackendContract {
                name: "scipy",
                version: SCIPY_VERSION,
                python_version: "3.12",
                environment_lock_sha256,
                worker_sha256,
            },
            base_prediction_source: "patient_level_out_of_fold",
            patients: spec.patients,
            modalities: spec.modalities,
            l2_penalty: spec.l2_penalty,
            resources: LateFusionResources {
                maximum_patients: 100_000,
                maximum_modalities: 16,
                maximum_output_bytes: 16 * 1024 * 1024,
                timeout_seconds: spec.timeout_seconds,
            },
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LateFusionModel {
    pub intercept: f64,
    pub coefficients: Vec<f64>,
    pub feature_names: Vec<String>,
    pub l2_penalty: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LateFusionCalibrator {
    pub intercept: f64,
    pub slope: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LateFusionPrediction {
    pub patient_id: String,
    pub label: u8,
    pub availability: Vec<bool>,
    pub raw_probability: f64,
    pub probability: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LateFusionMetric {
    pub brier_score: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MissingScenario {
    pub scenario: String,
    pub count: u32,
    pub brier_score: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModalityAblation {
    pub modality: String,
    pub brier_score: f64,
    pub brier_difference_ablated_minus_full: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LateFusionWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub model: LateFusionModel,
    pub calibrator: LateFusionCalibrator,
    pub predictions: Vec<LateFusionPrediction>,
    pub metrics: LateFusionMetric,
    pub missing_scenarios: Vec<MissingScenario>,
    pub modality_ablations: Vec<ModalityAblation>,
}

impl LateFusionWorkerResult {
    pub fn validate(
        &self,
        request: &LateFusionWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        let expected_features = request
            .modalities
            .iter()
            .flat_map(|name| [format!("{name}_probability"), format!("{name}_available")])
            .collect::<Vec<_>>();
        if self.format != "marklab.scipy_late_fusion_worker_result"
            || self.version != 1
            || self.backend.name != "scipy"
            || self.backend.version != SCIPY_VERSION
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
            || self.model.feature_names != expected_features
            || self.model.coefficients.len() != expected_features.len()
            || self.model.l2_penalty.to_bits() != request.l2_penalty.to_bits()
            || self.modality_ablations.len() != request.modalities.len()
        {
            return Err(BayesError::WorkerContract(
                "late-fusion worker identity or model differs".into(),
            ));
        }
        let test = request
            .patients
            .iter()
            .filter(|patient| patient.split == "test")
            .collect::<Vec<_>>();
        if self.predictions.len() != test.len() {
            return Err(BayesError::WorkerContract(
                "late-fusion prediction count differs".into(),
            ));
        }
        let mut squared = 0.0;
        for (prediction, patient) in self.predictions.iter().zip(&test) {
            let features = fusion_features(&patient.modality_probabilities);
            let raw = sigmoid(
                self.model.intercept
                    + self
                        .model
                        .coefficients
                        .iter()
                        .zip(&features)
                        .map(|(coefficient, value)| coefficient * value)
                        .sum::<f64>(),
            );
            let calibrated =
                sigmoid(self.calibrator.intercept + self.calibrator.slope * clipped_logit(raw));
            if prediction.patient_id != patient.patient_id
                || prediction.label != patient.label
                || prediction.availability
                    != patient
                        .modality_probabilities
                        .iter()
                        .map(Option::is_some)
                        .collect::<Vec<_>>()
                || !approximately_equal(prediction.raw_probability, raw)
                || !approximately_equal(prediction.probability, calibrated)
            {
                return Err(BayesError::WorkerContract(
                    "late-fusion prediction differs".into(),
                ));
            }
            squared += (calibrated - f64::from(patient.label)).powi(2);
        }
        if !approximately_equal(self.metrics.brier_score, squared / test.len() as f64) {
            return Err(BayesError::WorkerContract(
                "late-fusion Brier score differs".into(),
            ));
        }
        Ok(())
    }
}

fn fusion_features(probabilities: &[Option<f64>]) -> Vec<f64> {
    probabilities
        .iter()
        .flat_map(|value| [value.unwrap_or(0.5), f64::from(value.is_some())])
        .collect()
}

fn approximately_equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-10 * (1.0 + left.abs().max(right.abs()))
}
