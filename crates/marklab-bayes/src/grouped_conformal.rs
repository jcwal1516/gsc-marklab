use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{BackendContract, BayesError, WorkerBackend};

mod native;
pub use native::{fit_grouped_conformal, GroupedConformalFit, NativeConformalBackend};

const SCIPY_VERSION: &str = "1.18.1";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupedConformalPatient {
    pub patient_id: String,
    pub split: String,
    pub site: String,
    pub subgroup: String,
    pub label: u8,
    pub features: Vec<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupedConformalSpec {
    pub patients: Vec<GroupedConformalPatient>,
    pub feature_names: Vec<String>,
    pub alpha: f64,
    pub l2_penalty: f64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct GroupedConformalWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub patients: Vec<GroupedConformalPatient>,
    pub feature_names: Vec<String>,
    pub alpha: f64,
    pub l2_penalty: f64,
    pub resources: GroupedConformalResources,
}

#[derive(Clone, Debug, Serialize)]
pub struct GroupedConformalResources {
    pub maximum_patients: u32,
    pub maximum_features: u32,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

impl GroupedConformalSpec {
    pub(crate) fn validated(self) -> Result<Self, BayesError> {
        let mut spec = self;
        if !((30..=100_000).contains(&spec.patients.len())
            && (2..=128).contains(&spec.feature_names.len())
            && 0.0 < spec.alpha
            && spec.alpha < 0.5
            && spec.l2_penalty.is_finite()
            && spec.l2_penalty > 0.0
            && (1..=3_600).contains(&spec.timeout_seconds))
        {
            return Err(BayesError::InvalidSpec(
                "grouped conformal dimensions or controls are invalid".into(),
            ));
        }
        let mut names = HashSet::new();
        if spec
            .feature_names
            .iter()
            .any(|name| !name.starts_with("feature_") || !names.insert(name.as_str()))
        {
            return Err(BayesError::InvalidSpec(
                "grouped conformal feature names are invalid".into(),
            ));
        }
        spec.patients
            .sort_by(|left, right| left.patient_id.cmp(&right.patient_id));
        let mut ids = HashSet::new();
        for patient in &spec.patients {
            if patient.patient_id.is_empty()
                || patient.site.is_empty()
                || patient.subgroup.is_empty()
                || !ids.insert(patient.patient_id.as_str())
                || !matches!(patient.split.as_str(), "train" | "calibration" | "test")
                || patient.label > 1
                || patient.features.len() != spec.feature_names.len()
                || patient.features.iter().any(|value| !value.is_finite())
            {
                return Err(BayesError::InvalidSpec(
                    "grouped conformal patient row is invalid".into(),
                ));
            }
        }
        let counts = ["train", "calibration", "test"].map(|split| {
            spec.patients
                .iter()
                .filter(|patient| patient.split == split)
                .count()
        });
        if counts[0] < 12
            || counts[1] < 10
            || counts[2] < 8
            || spec.alpha < 1.0 / (counts[1] as f64 + 1.0)
        {
            return Err(BayesError::InvalidSpec(
                "grouped conformal split counts cannot support the requested alpha".into(),
            ));
        }
        let training = spec
            .patients
            .iter()
            .filter(|patient| patient.split == "train");
        let positives = training
            .clone()
            .filter(|patient| patient.label == 1)
            .count();
        if positives == 0 || positives == counts[0] {
            return Err(BayesError::InvalidSpec(
                "grouped conformal training requires both labels".into(),
            ));
        }
        Ok(spec)
    }
}

impl GroupedConformalWorkerRequest {
    pub fn new(
        spec: GroupedConformalSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
    ) -> Result<Self, BayesError> {
        let spec = spec.validated()?;
        Ok(Self {
            format: "marklab.scipy_grouped_conformal_request",
            version: 1,
            backend: BackendContract {
                name: "scipy",
                version: SCIPY_VERSION,
                python_version: "3.12",
                environment_lock_sha256,
                worker_sha256,
            },
            patients: spec.patients,
            feature_names: spec.feature_names,
            alpha: spec.alpha,
            l2_penalty: spec.l2_penalty,
            resources: GroupedConformalResources {
                maximum_patients: 100_000,
                maximum_features: 128,
                maximum_output_bytes: 16 * 1024 * 1024,
                timeout_seconds: spec.timeout_seconds,
            },
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupedConformalModel {
    pub training_mean: Vec<f64>,
    pub training_population_sd: Vec<f64>,
    pub intercept: f64,
    pub coefficients: Vec<f64>,
    pub l2_penalty: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupedConformalPrediction {
    pub patient_id: String,
    pub site: String,
    pub subgroup: String,
    pub label: u8,
    pub probability_one: f64,
    pub prediction_set: Vec<u8>,
    pub covered: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageRow {
    pub group: String,
    pub count: u32,
    pub covered: u32,
    pub coverage: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupedCoverage {
    pub overall: CoverageRow,
    pub by_site: Vec<CoverageRow>,
    pub by_subgroup: Vec<CoverageRow>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupedConformalWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub model: GroupedConformalModel,
    pub calibration_count: u32,
    pub corrected_rank: u32,
    pub nonconformity_threshold: f64,
    pub predictions: Vec<GroupedConformalPrediction>,
    pub coverage: GroupedCoverage,
}

impl GroupedConformalWorkerResult {
    pub fn validate(
        &self,
        request: &GroupedConformalWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        let dimension = request.feature_names.len();
        if self.format != "marklab.scipy_grouped_conformal_worker_result"
            || self.version != 1
            || self.backend.name != "scipy"
            || self.backend.version != SCIPY_VERSION
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
            || self.model.training_mean.len() != dimension
            || self.model.training_population_sd.len() != dimension
            || self.model.coefficients.len() != dimension
            || !self.model.intercept.is_finite()
            || self
                .model
                .coefficients
                .iter()
                .any(|value| !value.is_finite())
            || self
                .model
                .training_population_sd
                .iter()
                .any(|value| !value.is_finite() || *value <= 0.0)
            || self.model.l2_penalty.to_bits() != request.l2_penalty.to_bits()
            || !self.nonconformity_threshold.is_finite()
            || !(0.0..=1.0).contains(&self.nonconformity_threshold)
        {
            return Err(BayesError::WorkerContract(
                "grouped conformal worker identity or dimensions differ".into(),
            ));
        }
        let calibration = request
            .patients
            .iter()
            .filter(|patient| patient.split == "calibration")
            .collect::<Vec<_>>();
        let expected_rank = ((calibration.len() as f64 + 1.0) * (1.0 - request.alpha)).ceil();
        let mut scores = calibration
            .iter()
            .map(|patient| {
                let probability = probability(&self.model, &patient.features);
                if patient.label == 1 {
                    1.0 - probability
                } else {
                    probability
                }
            })
            .collect::<Vec<_>>();
        scores.sort_by(f64::total_cmp);
        if self.calibration_count != calibration.len() as u32
            || self.corrected_rank != expected_rank as u32
            || !approximately_equal(
                self.nonconformity_threshold,
                scores[expected_rank as usize - 1],
            )
        {
            return Err(BayesError::WorkerContract(
                "grouped conformal corrected quantile differs".into(),
            ));
        }
        let test = request
            .patients
            .iter()
            .filter(|patient| patient.split == "test")
            .collect::<Vec<_>>();
        if self.predictions.len() != test.len() {
            return Err(BayesError::WorkerContract(
                "grouped conformal prediction count differs".into(),
            ));
        }
        for (prediction, patient) in self.predictions.iter().zip(&test) {
            let probability_one = probability(&self.model, &patient.features);
            let expected_set = [
                (probability_one <= self.nonconformity_threshold).then_some(0),
                (1.0 - probability_one <= self.nonconformity_threshold).then_some(1),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
            if prediction.patient_id != patient.patient_id
                || prediction.site != patient.site
                || prediction.subgroup != patient.subgroup
                || prediction.label != patient.label
                || !approximately_equal(prediction.probability_one, probability_one)
                || prediction.prediction_set != expected_set
                || prediction.covered != expected_set.contains(&patient.label)
            {
                return Err(BayesError::WorkerContract(
                    "grouped conformal prediction differs".into(),
                ));
            }
        }
        if self.coverage.overall.count != test.len() as u32
            || self.coverage.overall.covered
                != self.predictions.iter().filter(|row| row.covered).count() as u32
            || self.coverage.overall.group != "all"
            || !approximately_equal(
                self.coverage.overall.coverage,
                f64::from(self.coverage.overall.covered) / f64::from(self.coverage.overall.count),
            )
            || !coverage_matches(&self.predictions, &self.coverage.by_site, |row| &row.site)
            || !coverage_matches(&self.predictions, &self.coverage.by_subgroup, |row| {
                &row.subgroup
            })
        {
            return Err(BayesError::WorkerContract(
                "grouped conformal overall coverage differs".into(),
            ));
        }
        Ok(())
    }
}

fn coverage_matches<'a>(
    predictions: &'a [GroupedConformalPrediction],
    rows: &[CoverageRow],
    group: impl Fn(&'a GroupedConformalPrediction) -> &'a str,
) -> bool {
    let mut expected = BTreeMap::<&str, (u32, u32)>::new();
    for prediction in predictions {
        let counts = expected.entry(group(prediction)).or_default();
        counts.0 += 1;
        counts.1 += u32::from(prediction.covered);
    }
    rows.len() == expected.len()
        && rows.iter().zip(expected).all(|(row, (name, counts))| {
            row.group == name
                && row.count == counts.0
                && row.covered == counts.1
                && approximately_equal(row.coverage, f64::from(counts.1) / f64::from(counts.0))
        })
}

fn probability(model: &GroupedConformalModel, features: &[f64]) -> f64 {
    let linear = model.intercept
        + model
            .coefficients
            .iter()
            .zip(features.iter().zip(&model.training_mean))
            .zip(&model.training_population_sd)
            // Match the fitted design: standardize before multiplication. Reordering this can
            // overflow a raw-feature product even when both standardized products are finite.
            .map(|((coefficient, (value, mean)), sd)| coefficient * ((value - mean) / sd))
            .sum::<f64>();
    if linear >= 0.0 {
        1.0 / (1.0 + (-linear).exp())
    } else {
        let exponential = linear.exp();
        exponential / (1.0 + exponential)
    }
}

fn approximately_equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-10 * (1.0 + left.abs().max(right.abs()))
}
