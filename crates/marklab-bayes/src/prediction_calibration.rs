use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{BackendContract, BayesError, WorkerBackend};

const SCIPY_VERSION: &str = "1.18.1";

#[derive(Clone, Debug, Serialize)]
pub struct PredictionCalibrationRow {
    pub patient_id: String,
    pub split: String,
    pub score: f64,
    pub label: u8,
}

#[derive(Clone, Debug)]
pub struct PredictionCalibrationSpec {
    pub rows: Vec<PredictionCalibrationRow>,
    pub bins: u32,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PredictionCalibrationResources {
    pub maximum_patients: u32,
    pub maximum_bins: u32,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PredictionCalibrationWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub method: &'static str,
    pub rows: Vec<PredictionCalibrationRow>,
    pub bins: u32,
    pub resources: PredictionCalibrationResources,
}

impl PredictionCalibrationWorkerRequest {
    pub fn new(
        mut spec: PredictionCalibrationSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
    ) -> Result<Self, BayesError> {
        if !(16..=100_000).contains(&spec.rows.len())
            || !(2..=20).contains(&spec.bins)
            || !(1..=3_600).contains(&spec.timeout_seconds)
        {
            return Err(BayesError::InvalidSpec(
                "calibration row, bin, or timeout limits are invalid".into(),
            ));
        }
        spec.rows
            .sort_by(|left, right| left.patient_id.cmp(&right.patient_id));
        let mut ids = HashSet::with_capacity(spec.rows.len());
        for row in &spec.rows {
            if row.patient_id.trim().is_empty()
                || !ids.insert(row.patient_id.as_str())
                || !matches!(row.split.as_str(), "training_oof" | "test")
                || !row.score.is_finite()
                || row.label > 1
            {
                return Err(BayesError::InvalidSpec(
                    "calibration patient identity, split, score, or label is invalid".into(),
                ));
            }
        }
        for split in ["training_oof", "test"] {
            let rows = spec.rows.iter().filter(|row| row.split == split);
            let count = rows.clone().count();
            let positives = rows.filter(|row| row.label == 1).count();
            if count < 8 || positives == 0 || positives == count {
                return Err(BayesError::InvalidSpec(format!(
                    "calibration split {split} requires at least eight patients and both labels"
                )));
            }
        }
        Ok(Self {
            format: "marklab.scipy_prediction_calibration_request",
            version: 1,
            backend: BackendContract {
                name: "scipy",
                version: SCIPY_VERSION,
                python_version: "3.12",
                environment_lock_sha256,
                worker_sha256,
            },
            method: "platt_logistic",
            rows: spec.rows,
            bins: spec.bins,
            resources: PredictionCalibrationResources {
                maximum_patients: 100_000,
                maximum_bins: 20,
                maximum_output_bytes: 16 * 1024 * 1024,
                timeout_seconds: spec.timeout_seconds,
            },
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlattCalibrator {
    pub intercept: f64,
    pub slope: f64,
    pub target_smoothing: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CalibratedPrediction {
    pub patient_id: String,
    pub score: f64,
    pub label: u8,
    pub probability: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReliabilityBin {
    pub lower: f64,
    pub upper: f64,
    pub count: u32,
    pub mean_probability: f64,
    pub observed_rate: f64,
    pub wilson_95_lower: f64,
    pub wilson_95_upper: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PredictionCalibrationMetrics {
    pub brier_score: f64,
    pub expected_calibration_error: f64,
    pub calibration_in_the_large: f64,
    pub calibration_slope: f64,
    pub reliability_bins: Vec<ReliabilityBin>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredictionCalibrationWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub calibrator: PlattCalibrator,
    pub predictions: Vec<CalibratedPrediction>,
    pub metrics: PredictionCalibrationMetrics,
}

impl PredictionCalibrationWorkerResult {
    pub fn validate(
        &self,
        request: &PredictionCalibrationWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.format != "marklab.scipy_prediction_calibration_worker_result"
            || self.version != 1
            || self.backend.name != "scipy"
            || self.backend.version != SCIPY_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
            || !finite(&[
                self.calibrator.intercept,
                self.calibrator.slope,
                self.metrics.brier_score,
                self.metrics.expected_calibration_error,
                self.metrics.calibration_in_the_large,
                self.metrics.calibration_slope,
            ])
            || self.calibrator.target_smoothing != "platt_class_count"
        {
            return Err(BayesError::WorkerContract(
                "prediction calibration worker identity or scalar result differs".into(),
            ));
        }
        let test = request
            .rows
            .iter()
            .filter(|row| row.split == "test")
            .collect::<Vec<_>>();
        if self.predictions.len() != test.len() {
            return Err(BayesError::WorkerContract(
                "prediction calibration test count differs".into(),
            ));
        }
        let mut squared_error = 0.0;
        for (prediction, expected) in self.predictions.iter().zip(&test) {
            let probability =
                sigmoid(self.calibrator.intercept + self.calibrator.slope * expected.score);
            if prediction.patient_id != expected.patient_id
                || prediction.score.to_bits() != expected.score.to_bits()
                || prediction.label != expected.label
                || !(0.0..=1.0).contains(&prediction.probability)
                || !approximately_equal(prediction.probability, probability)
            {
                return Err(BayesError::WorkerContract(
                    "calibrated prediction identity or probability differs".into(),
                ));
            }
            let residual = probability - f64::from(expected.label);
            squared_error += residual * residual;
        }
        let brier = squared_error / test.len() as f64;
        if !approximately_equal(brier, self.metrics.brier_score) {
            return Err(BayesError::WorkerContract(
                "calibration Brier score differs".into(),
            ));
        }
        validate_bins(&self.predictions, request.bins, &self.metrics)?;
        Ok(())
    }
}

fn validate_bins(
    predictions: &[CalibratedPrediction],
    bins: u32,
    metrics: &PredictionCalibrationMetrics,
) -> Result<(), BayesError> {
    let mut expected_ece = 0.0;
    let mut occupied = 0usize;
    for bin_index in 0..bins {
        let members = predictions
            .iter()
            .filter(|row| {
                ((row.probability * f64::from(bins)).floor() as u32).min(bins - 1) == bin_index
            })
            .collect::<Vec<_>>();
        if members.is_empty() {
            continue;
        }
        let row = metrics.reliability_bins.get(occupied).ok_or_else(|| {
            BayesError::WorkerContract("calibration reliability bin is missing".into())
        })?;
        occupied += 1;
        let mean = members.iter().map(|item| item.probability).sum::<f64>() / members.len() as f64;
        let rate = members
            .iter()
            .map(|item| f64::from(item.label))
            .sum::<f64>()
            / members.len() as f64;
        expected_ece += members.len() as f64 / predictions.len() as f64 * (mean - rate).abs();
        if row.count != members.len() as u32
            || !approximately_equal(row.lower, f64::from(bin_index) / f64::from(bins))
            || !approximately_equal(row.upper, f64::from(bin_index + 1) / f64::from(bins))
            || !approximately_equal(row.mean_probability, mean)
            || !approximately_equal(row.observed_rate, rate)
            || !finite(&[row.wilson_95_lower, row.wilson_95_upper])
            || row.wilson_95_lower < 0.0
            || row.wilson_95_upper > 1.0
        {
            return Err(BayesError::WorkerContract(
                "calibration reliability bin differs".into(),
            ));
        }
    }
    if occupied != metrics.reliability_bins.len()
        || !approximately_equal(expected_ece, metrics.expected_calibration_error)
    {
        return Err(BayesError::WorkerContract(
            "calibration reliability family differs".into(),
        ));
    }
    Ok(())
}

fn sigmoid(value: f64) -> f64 {
    if value >= 0.0 {
        1.0 / (1.0 + (-value).exp())
    } else {
        let exponential = value.exp();
        exponential / (1.0 + exponential)
    }
}

fn finite(values: &[f64]) -> bool {
    values.iter().all(|value| value.is_finite())
}

fn approximately_equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-10 * (1.0 + left.abs().max(right.abs()))
}
