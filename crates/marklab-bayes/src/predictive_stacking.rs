use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{BackendContract, BayesError, WorkerBackend};

const SCIPY_VERSION: &str = "1.18.1";

#[derive(Clone, Debug, Serialize)]
pub struct PredictiveStackingPatient {
    pub patient_id: String,
    pub held_out_unit: String,
    pub log_predictive_densities: Vec<f64>,
}

#[derive(Clone, Debug)]
pub struct PredictiveStackingSpec {
    pub patients: Vec<PredictiveStackingPatient>,
    pub model_names: Vec<String>,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PredictiveStackingWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub patients: Vec<PredictiveStackingPatient>,
    pub model_names: Vec<String>,
    pub resources: PredictiveStackingResources,
}

#[derive(Clone, Debug, Serialize)]
pub struct PredictiveStackingResources {
    pub maximum_patients: u32,
    pub maximum_models: u32,
    pub maximum_jackknife_density_visits: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

impl PredictiveStackingWorkerRequest {
    pub fn new(
        mut spec: PredictiveStackingSpec,
        environment_lock_sha256: String,
        worker_sha256: String,
    ) -> Result<Self, BayesError> {
        if !((8..=500).contains(&spec.patients.len())
            && (2..=16).contains(&spec.model_names.len())
            && (1..=3_600).contains(&spec.timeout_seconds))
        {
            return Err(BayesError::InvalidSpec(
                "predictive-stacking dimensions or timeout are invalid".into(),
            ));
        }
        let mut models = HashSet::new();
        if spec
            .model_names
            .iter()
            .any(|name| !name.starts_with("model_") || !models.insert(name.as_str()))
        {
            return Err(BayesError::InvalidSpec(
                "predictive-stacking model names are invalid".into(),
            ));
        }
        spec.patients
            .sort_by(|left, right| left.patient_id.cmp(&right.patient_id));
        let mut patients = HashSet::new();
        for patient in &spec.patients {
            if patient.patient_id.is_empty()
                || !patients.insert(patient.patient_id.as_str())
                || patient.held_out_unit != "patient"
                || patient.log_predictive_densities.len() != spec.model_names.len()
                || patient
                    .log_predictive_densities
                    .iter()
                    .any(|value| !value.is_finite())
            {
                return Err(BayesError::InvalidSpec(
                    "predictive-stacking patient row is invalid".into(),
                ));
            }
        }
        let visits =
            spec.patients.len() as u64 * spec.patients.len() as u64 * spec.model_names.len() as u64;
        const MAXIMUM_VISITS: u64 = 25_000_000;
        if visits > MAXIMUM_VISITS {
            return Err(BayesError::InvalidSpec(
                "predictive-stacking jackknife work exceeds its bound".into(),
            ));
        }
        Ok(Self {
            format: "marklab.scipy_predictive_stacking_request",
            version: 1,
            backend: BackendContract {
                name: "scipy",
                version: SCIPY_VERSION,
                python_version: "3.12",
                environment_lock_sha256,
                worker_sha256,
            },
            patients: spec.patients,
            model_names: spec.model_names,
            resources: PredictiveStackingResources {
                maximum_patients: 500,
                maximum_models: 16,
                maximum_jackknife_density_visits: MAXIMUM_VISITS,
                maximum_output_bytes: 16 * 1024 * 1024,
                timeout_seconds: spec.timeout_seconds,
            },
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StackingWeight {
    pub model: String,
    pub weight: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StackingPatientDensity {
    pub patient_id: String,
    pub mixture_log_predictive_density: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StackingWeightSensitivity {
    pub model: String,
    pub leave_one_patient_out_minimum: f64,
    pub leave_one_patient_out_maximum: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredictiveStackingWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub weights: Vec<StackingWeight>,
    pub objective_sum_log_predictive_density: f64,
    pub grouped_mixture_log_predictive_density: Vec<StackingPatientDensity>,
    pub leave_one_patient_out_sensitivity: Vec<StackingWeightSensitivity>,
}

impl PredictiveStackingWorkerResult {
    pub fn validate(
        &self,
        request: &PredictiveStackingWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.format != "marklab.scipy_predictive_stacking_worker_result"
            || self.version != 1
            || self.backend.name != "scipy"
            || self.backend.version != SCIPY_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
            || self.weights.len() != request.model_names.len()
            || self.grouped_mixture_log_predictive_density.len() != request.patients.len()
            || self.leave_one_patient_out_sensitivity.len() != request.model_names.len()
        {
            return Err(BayesError::WorkerContract(
                "predictive-stacking worker identity or dimensions differ".into(),
            ));
        }
        let weight_values = self
            .weights
            .iter()
            .zip(&request.model_names)
            .map(|(weight, model)| {
                if weight.model != *model
                    || !weight.weight.is_finite()
                    || !(0.0..=1.0).contains(&weight.weight)
                {
                    Err(BayesError::WorkerContract(
                        "predictive-stacking weight differs".into(),
                    ))
                } else {
                    Ok(weight.weight)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        if !approximately_equal(weight_values.iter().sum(), 1.0) {
            return Err(BayesError::WorkerContract(
                "predictive-stacking weights do not sum to one".into(),
            ));
        }
        let mut objective = 0.0;
        for ((output, patient), index) in self
            .grouped_mixture_log_predictive_density
            .iter()
            .zip(&request.patients)
            .zip(0usize..)
        {
            let expected = log_mixture(&weight_values, &patient.log_predictive_densities);
            if output.patient_id != patient.patient_id
                || !approximately_equal(output.mixture_log_predictive_density, expected)
            {
                return Err(BayesError::WorkerContract(format!(
                    "predictive-stacking mixture differs at patient {index}"
                )));
            }
            objective += expected;
        }
        if !approximately_equal(self.objective_sum_log_predictive_density, objective) {
            return Err(BayesError::WorkerContract(
                "predictive-stacking objective differs".into(),
            ));
        }
        for (row, model) in self
            .leave_one_patient_out_sensitivity
            .iter()
            .zip(&request.model_names)
        {
            if row.model != *model
                || !row.leave_one_patient_out_minimum.is_finite()
                || !row.leave_one_patient_out_maximum.is_finite()
                || !(0.0..=1.0).contains(&row.leave_one_patient_out_minimum)
                || !(0.0..=1.0).contains(&row.leave_one_patient_out_maximum)
                || row.leave_one_patient_out_minimum > row.leave_one_patient_out_maximum
            {
                return Err(BayesError::WorkerContract(
                    "predictive-stacking sensitivity differs".into(),
                ));
            }
        }
        Ok(())
    }
}

fn log_mixture(weights: &[f64], log_densities: &[f64]) -> f64 {
    let maximum = weights
        .iter()
        .zip(log_densities)
        .filter(|(weight, _)| **weight > 0.0)
        .map(|(weight, density)| weight.ln() + density)
        .fold(f64::NEG_INFINITY, f64::max);
    maximum
        + weights
            .iter()
            .zip(log_densities)
            .filter(|(weight, _)| **weight > 0.0)
            .map(|(weight, density)| (weight.ln() + density - maximum).exp())
            .sum::<f64>()
            .ln()
}

fn approximately_equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-9 * (1.0 + left.abs().max(right.abs()))
}
