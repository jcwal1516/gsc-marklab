use crate::validation::all_finite as finite;

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{BackendContract, BayesError, WorkerBackend};

const SCIPY_VERSION: &str = "1.18.1";

#[derive(Clone, Debug, Serialize)]
pub struct ComplementarityPatientRow {
    pub patient_id: String,
    pub outer_fold: u32,
    pub inner_fold: u32,
    pub target: f64,
    pub technical: Vec<f64>,
    pub clinical: Vec<f64>,
    pub compartment: Vec<f64>,
    pub acquisition: Vec<f64>,
    pub cell: Vec<f64>,
    pub patch: Vec<f64>,
    pub neighbor: Vec<f64>,
    pub measured: Vec<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ComplementarityFeatureNames {
    pub technical: Vec<String>,
    pub clinical: Vec<String>,
    pub compartment: Vec<String>,
    pub acquisition: Vec<String>,
    pub cell: Vec<String>,
    pub patch: Vec<String>,
    pub neighbor: Vec<String>,
    pub measured: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct CellPatchComplementaritySpec {
    pub patients: Vec<ComplementarityPatientRow>,
    pub feature_names: ComplementarityFeatureNames,
    pub outer_folds: u32,
    pub inner_folds: u32,
    pub ridge_alphas: Vec<f64>,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ComplementarityResourceLimits {
    pub maximum_patients: u32,
    pub maximum_features: u32,
    pub maximum_work_units: u64,
    pub maximum_output_bytes: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct CellPatchComplementarityWorkerRequest {
    pub format: &'static str,
    pub version: u32,
    pub backend: BackendContract,
    pub patients: Vec<ComplementarityPatientRow>,
    pub feature_names: ComplementarityFeatureNames,
    pub outer_folds: u32,
    pub inner_folds: u32,
    pub ridge_alphas: Vec<f64>,
    pub resources: ComplementarityResourceLimits,
}

impl CellPatchComplementarityWorkerRequest {
    pub fn new(
        mut spec: CellPatchComplementaritySpec,
        environment_lock_sha256: String,
        worker_sha256: String,
    ) -> Result<Self, BayesError> {
        if !(24..=10_000).contains(&spec.patients.len())
            || !(2..=10).contains(&spec.outer_folds)
            || !(2..=10).contains(&spec.inner_folds)
            || !(2..=32).contains(&spec.ridge_alphas.len())
            || !(1..=3_600).contains(&spec.timeout_seconds)
        {
            return Err(BayesError::InvalidSpec(
                "complementarity dimensions, folds, alpha grid, or timeout are invalid".into(),
            ));
        }
        validate_feature_names(&spec.feature_names)?;
        spec.ridge_alphas.sort_by(f64::total_cmp);
        if spec
            .ridge_alphas
            .iter()
            .any(|alpha| !alpha.is_finite() || *alpha < 0.0)
            || spec
                .ridge_alphas
                .windows(2)
                .any(|window| window[0] == window[1])
        {
            return Err(BayesError::InvalidSpec(
                "ridge alphas must be unique finite nonnegative values".into(),
            ));
        }
        spec.patients
            .sort_by(|left, right| left.patient_id.cmp(&right.patient_id));
        validate_patients(&spec)?;
        let feature_count = all_feature_names(&spec.feature_names).len();
        let work = spec.patients.len() as u64
            * u64::from(spec.outer_folds)
            * u64::from(spec.inner_folds)
            * spec.ridge_alphas.len() as u64
            * feature_count as u64
            * 6;
        const MAXIMUM_WORK_UNITS: u64 = 250_000_000;
        if feature_count > 1_024 || work > MAXIMUM_WORK_UNITS {
            return Err(BayesError::InvalidSpec(
                "complementarity feature or nested-fit work exceeds its resource bound".into(),
            ));
        }
        Ok(Self {
            format: "marklab.scipy_cell_patch_complementarity_request",
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
            outer_folds: spec.outer_folds,
            inner_folds: spec.inner_folds,
            ridge_alphas: spec.ridge_alphas,
            resources: ComplementarityResourceLimits {
                maximum_patients: 10_000,
                maximum_features: 1_024,
                maximum_work_units: MAXIMUM_WORK_UNITS,
                maximum_output_bytes: 16 * 1024 * 1024,
                timeout_seconds: spec.timeout_seconds,
            },
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComplementarityPrediction {
    pub patient_id: String,
    pub outer_fold: u32,
    pub observed: f64,
    pub predicted: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComplementarityFoldSelection {
    pub outer_fold: u32,
    pub ridge_alpha: f64,
    pub inner_rmse: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComplementarityModelResult {
    pub model_id: String,
    pub feature_names: Vec<String>,
    pub fold_selections: Vec<ComplementarityFoldSelection>,
    pub rmse: f64,
    pub mae: f64,
    pub calibration_intercept: f64,
    pub calibration_slope: f64,
    pub predictions: Vec<ComplementarityPrediction>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellPatchComplementarityWorkerResult {
    format: String,
    version: u32,
    pub backend: WorkerBackend,
    pub request_sha256: String,
    pub models: Vec<ComplementarityModelResult>,
}

impl CellPatchComplementarityWorkerResult {
    pub fn validate(
        &self,
        request: &CellPatchComplementarityWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), BayesError> {
        if self.format != "marklab.scipy_cell_patch_complementarity_worker_result"
            || self.version != 1
            || self.backend.name != "scipy"
            || self.backend.version != SCIPY_VERSION
            || self.backend.python_version != "3.12"
            || self.backend.environment_lock_sha256 != request.backend.environment_lock_sha256
            || self.backend.worker_sha256 != request.backend.worker_sha256
            || self.request_sha256 != request_sha256
            || self.models.len() != 6
        {
            return Err(BayesError::WorkerContract(
                "complementarity worker identity or model count mismatch".into(),
            ));
        }
        let expected_models = expected_model_features(&request.feature_names);
        for (model, (model_id, features)) in self.models.iter().zip(expected_models) {
            if model.model_id != model_id
                || model.feature_names != features
                || model.fold_selections.len() != request.outer_folds as usize
                || model.predictions.len() != request.patients.len()
                || !finite(&[
                    model.rmse,
                    model.mae,
                    model.calibration_intercept,
                    model.calibration_slope,
                ])
                || model.rmse < 0.0
                || model.mae < 0.0
            {
                return Err(BayesError::WorkerContract(
                    "complementarity model identity or metrics mismatch".into(),
                ));
            }
            for (selection, fold) in model.fold_selections.iter().zip(0..request.outer_folds) {
                if selection.outer_fold != fold
                    || !request.ridge_alphas.contains(&selection.ridge_alpha)
                    || !selection.inner_rmse.is_finite()
                    || selection.inner_rmse < 0.0
                {
                    return Err(BayesError::WorkerContract(
                        "complementarity fold selection is invalid".into(),
                    ));
                }
            }
            let mut predictions = model.predictions.iter().collect::<Vec<_>>();
            predictions.sort_by(|left, right| left.patient_id.cmp(&right.patient_id));
            let mut squared = 0.0;
            let mut absolute = 0.0;
            for (prediction, patient) in predictions.iter().zip(&request.patients) {
                if prediction.patient_id != patient.patient_id
                    || prediction.outer_fold != patient.outer_fold
                    || prediction.observed.to_bits() != patient.target.to_bits()
                    || !prediction.predicted.is_finite()
                {
                    return Err(BayesError::WorkerContract(
                        "complementarity prediction identity mismatch".into(),
                    ));
                }
                let residual = prediction.predicted - prediction.observed;
                squared += residual * residual;
                absolute += residual.abs();
            }
            let rmse = (squared / predictions.len() as f64).sqrt();
            let mae = absolute / predictions.len() as f64;
            if !approximately_equal(rmse, model.rmse) || !approximately_equal(mae, model.mae) {
                return Err(BayesError::WorkerContract(
                    "complementarity metrics disagree with patient predictions".into(),
                ));
            }
        }
        Ok(())
    }
}

fn validate_feature_names(names: &ComplementarityFeatureNames) -> Result<(), BayesError> {
    for (prefix, group) in [
        ("technical_", &names.technical),
        ("clinical_", &names.clinical),
        ("compartment_", &names.compartment),
        ("acquisition_", &names.acquisition),
        ("cell_", &names.cell),
        ("patch_", &names.patch),
        ("neighbor_", &names.neighbor),
        ("measured_", &names.measured),
    ] {
        let unique = group.iter().collect::<HashSet<_>>();
        if group.is_empty()
            || unique.len() != group.len()
            || group
                .iter()
                .any(|name| !name.starts_with(prefix) || name.trim() != name)
        {
            return Err(BayesError::InvalidSpec(
                "complementarity feature groups require unique exact prefixed names".into(),
            ));
        }
    }
    Ok(())
}

fn validate_patients(spec: &CellPatchComplementaritySpec) -> Result<(), BayesError> {
    let expected_lengths = [
        spec.feature_names.technical.len(),
        spec.feature_names.clinical.len(),
        spec.feature_names.compartment.len(),
        spec.feature_names.acquisition.len(),
        spec.feature_names.cell.len(),
        spec.feature_names.patch.len(),
        spec.feature_names.neighbor.len(),
        spec.feature_names.measured.len(),
    ];
    let mut ids = HashSet::new();
    let mut outer_counts = vec![0_u32; spec.outer_folds as usize];
    for patient in &spec.patients {
        let groups = [
            &patient.technical,
            &patient.clinical,
            &patient.compartment,
            &patient.acquisition,
            &patient.cell,
            &patient.patch,
            &patient.neighbor,
            &patient.measured,
        ];
        if patient.patient_id.is_empty()
            || patient.patient_id.trim() != patient.patient_id
            || !ids.insert(patient.patient_id.as_str())
            || patient.outer_fold >= spec.outer_folds
            || patient.inner_fold >= spec.inner_folds
            || !patient.target.is_finite()
            || groups.iter().zip(expected_lengths).any(|(group, length)| {
                group.len() != length || group.iter().any(|value| !value.is_finite())
            })
        {
            return Err(BayesError::InvalidSpec(
                "complementarity patient rows or fold identities are invalid".into(),
            ));
        }
        outer_counts[patient.outer_fold as usize] += 1;
    }
    if outer_counts.contains(&0) {
        return Err(BayesError::InvalidSpec(
            "every complementarity outer fold must be nonempty".into(),
        ));
    }
    for outer in 0..spec.outer_folds {
        let present = spec
            .patients
            .iter()
            .filter(|patient| patient.outer_fold != outer)
            .map(|patient| patient.inner_fold)
            .collect::<HashSet<_>>();
        if present.len() != spec.inner_folds as usize {
            return Err(BayesError::InvalidSpec(
                "every outer-training set must contain every inner fold".into(),
            ));
        }
    }
    let columns = spec
        .patients
        .iter()
        .map(flatten_patient_features)
        .collect::<Vec<_>>();
    for column in 0..columns[0].len() {
        let first = columns[0][column];
        if columns.iter().all(|row| row[column] == first) {
            return Err(BayesError::InvalidSpec(
                "every complementarity feature must vary".into(),
            ));
        }
    }
    Ok(())
}

fn flatten_patient_features(patient: &ComplementarityPatientRow) -> Vec<f64> {
    [
        patient.technical.as_slice(),
        patient.clinical.as_slice(),
        patient.compartment.as_slice(),
        patient.acquisition.as_slice(),
        patient.cell.as_slice(),
        patient.patch.as_slice(),
        patient.neighbor.as_slice(),
        patient.measured.as_slice(),
    ]
    .concat()
}

fn all_feature_names(names: &ComplementarityFeatureNames) -> Vec<String> {
    [
        names.technical.as_slice(),
        names.clinical.as_slice(),
        names.compartment.as_slice(),
        names.acquisition.as_slice(),
        names.cell.as_slice(),
        names.patch.as_slice(),
        names.neighbor.as_slice(),
        names.measured.as_slice(),
    ]
    .concat()
}

pub fn expected_model_features(
    names: &ComplementarityFeatureNames,
) -> Vec<(&'static str, Vec<String>)> {
    let base = [
        names.technical.as_slice(),
        names.clinical.as_slice(),
        names.compartment.as_slice(),
        names.acquisition.as_slice(),
    ]
    .concat();
    let append = |groups: &[&[String]]| {
        let mut result = base.clone();
        for group in groups {
            result.extend_from_slice(group);
        }
        result
    };
    vec![
        ("m0", base.clone()),
        ("m1", append(&[&names.cell])),
        ("m2", append(&[&names.patch])),
        ("m3", append(&[&names.cell, &names.patch])),
        ("m4", append(&[&names.cell, &names.patch, &names.neighbor])),
        (
            "m5",
            append(&[&names.cell, &names.patch, &names.neighbor, &names.measured]),
        ),
    ]
}

fn approximately_equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-9 * left.abs().max(right.abs()).max(1.0)
}
