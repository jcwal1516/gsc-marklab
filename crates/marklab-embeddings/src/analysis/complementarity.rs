use marklab_core::backend::is_worker_timeout;
use marklab_numerics::all_finite as finite;
use marklab_numerics::relative_eq;

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use marklab_core::backend::versions::PYTHON_VERSION;
use marklab_core::backend::versions::SCIPY_VERSION;
use marklab_core::backend::{BackendContract, WorkerBackend, WorkerError};

const RELATIVE_TOLERANCE: f64 = 1e-9;

/// One patient response, fold assignment, and ordered predictor blocks.
#[derive(Clone, Debug, Serialize)]
pub struct ComplementarityPatientRow {
    /// Stable patient identity.
    pub patient_id: String,
    /// Prespecified held-out outer-fold assignment.
    pub outer_fold: u32,
    /// Prespecified inner validation-fold assignment.
    pub inner_fold: u32,
    /// Observed patient-level response.
    pub target: f64,
    /// Technical predictor block in declared feature order.
    pub technical: Vec<f64>,
    /// Clinical predictor block in declared feature order.
    pub clinical: Vec<f64>,
    /// Compartment predictor block in declared feature order.
    pub compartment: Vec<f64>,
    /// Acquisition predictor block in declared feature order.
    pub acquisition: Vec<f64>,
    /// Cell predictor block in declared feature order.
    pub cell: Vec<f64>,
    /// Patch predictor block in declared feature order.
    pub patch: Vec<f64>,
    /// Neighbor predictor block in declared feature order.
    pub neighbor: Vec<f64>,
    /// Measured predictor block in declared feature order.
    pub measured: Vec<f64>,
}

/// Ordered feature names for each complementarity predictor block.
#[derive(Clone, Debug, Serialize)]
pub struct ComplementarityFeatureNames {
    /// Technical predictor block in declared feature order.
    pub technical: Vec<String>,
    /// Clinical predictor block in declared feature order.
    pub clinical: Vec<String>,
    /// Compartment predictor block in declared feature order.
    pub compartment: Vec<String>,
    /// Acquisition predictor block in declared feature order.
    pub acquisition: Vec<String>,
    /// Cell predictor block in declared feature order.
    pub cell: Vec<String>,
    /// Patch predictor block in declared feature order.
    pub patch: Vec<String>,
    /// Neighbor predictor block in declared feature order.
    pub neighbor: Vec<String>,
    /// Measured predictor block in declared feature order.
    pub measured: Vec<String>,
}

/// Patient-level nested cross-validation design for comparing cell and patch feature blocks.
#[derive(Clone, Debug)]
pub struct CellPatchComplementaritySpec {
    /// One response and predictor row per patient.
    pub patients: Vec<ComplementarityPatientRow>,
    /// Feature names in the same order as the input vectors.
    pub feature_names: ComplementarityFeatureNames,
    /// Number of held-out outer folds.
    pub outer_folds: u32,
    /// Number of inner validation folds.
    pub inner_folds: u32,
    /// Candidate ridge regularization strengths for inner validation.
    pub ridge_alphas: Vec<f64>,
    /// Maximum wall-clock execution time of the worker, in seconds.
    pub timeout_seconds: u64,
}

/// Declared bounds on complementarity data, computation, and worker execution.
#[derive(Clone, Debug, Serialize)]
pub struct ComplementarityResourceLimits {
    /// Maximum number of patient rows.
    pub maximum_patients: u32,
    /// Maximum number of predictor features.
    pub maximum_features: u32,
    /// Maximum permitted declared computation units.
    pub maximum_work_units: u64,
    /// Maximum permitted worker response size, in bytes.
    pub maximum_output_bytes: u64,
    /// Maximum wall-clock execution time of the worker, in seconds.
    pub timeout_seconds: u64,
}

/// Validated request for the pinned cell/patch complementarity worker.
#[derive(Clone, Debug, Serialize)]
pub struct CellPatchComplementarityWorkerRequest {
    /// Versioned document-format identifier.
    pub format: &'static str,
    /// Version of the recorded document format.
    pub version: u32,
    /// Pinned worker implementation and asset identity.
    pub backend: BackendContract,
    /// One response and predictor row per patient.
    pub patients: Vec<ComplementarityPatientRow>,
    /// Feature names in the same order as the input vectors.
    pub feature_names: ComplementarityFeatureNames,
    /// Number of held-out outer folds.
    pub outer_folds: u32,
    /// Number of inner validation folds.
    pub inner_folds: u32,
    /// Candidate ridge regularization strengths for inner validation.
    pub ridge_alphas: Vec<f64>,
    /// Declared data, computation, and execution limits.
    pub resources: ComplementarityResourceLimits,
}

impl CellPatchComplementarityWorkerRequest {
    /// Validate the specification and bind the request to the supplied worker assets.
    pub fn new(
        mut spec: CellPatchComplementaritySpec,
        environment_lock_sha256: String,
        worker_sha256: String,
    ) -> Result<Self, WorkerError> {
        if !(24..=10_000).contains(&spec.patients.len())
            || !(2..=10).contains(&spec.outer_folds)
            || !(2..=10).contains(&spec.inner_folds)
            || !(2..=32).contains(&spec.ridge_alphas.len())
            || !is_worker_timeout(spec.timeout_seconds)
        {
            return Err(WorkerError::InvalidSpec(
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
            return Err(WorkerError::InvalidSpec(
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
            return Err(WorkerError::InvalidSpec(
                "complementarity feature or nested-fit work exceeds its resource bound".into(),
            ));
        }
        Ok(Self {
            format: "marklab.scipy_cell_patch_complementarity_request",
            version: 1,
            backend: BackendContract {
                name: "scipy",
                version: SCIPY_VERSION,
                python_version: PYTHON_VERSION,
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

/// One patient prediction obtained without fitting on its outer fold.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComplementarityPrediction {
    /// Stable patient identity.
    pub patient_id: String,
    /// Prespecified held-out outer-fold assignment.
    pub outer_fold: u32,
    /// Observed statistic or response value.
    pub observed: f64,
    /// Held-out prediction for the observed response.
    pub predicted: f64,
}

/// Regularization selected within one held-out outer fold.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComplementarityFoldSelection {
    /// Prespecified held-out outer-fold assignment.
    pub outer_fold: u32,
    /// Selected ridge regularization strength.
    pub ridge_alpha: f64,
    /// Inner-validation root mean squared prediction error.
    pub inner_rmse: f64,
}

/// Held-out predictions and prediction-error summaries for one feature-block model.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComplementarityModelResult {
    /// Identity of the fitted feature-block model.
    pub model_id: String,
    /// Feature names in the same order as the input vectors.
    pub feature_names: Vec<String>,
    /// Regularization selected separately for each outer fold.
    pub fold_selections: Vec<ComplementarityFoldSelection>,
    /// Root mean squared held-out prediction error.
    pub rmse: f64,
    /// Mean absolute held-out prediction error.
    pub mae: f64,
    /// Intercept from regressing observed held-out responses on predictions.
    pub calibration_intercept: f64,
    /// Slope from regressing observed held-out responses on predictions.
    pub calibration_slope: f64,
    /// Patient predictions assembled from held-out outer folds.
    pub predictions: Vec<ComplementarityPrediction>,
}

/// Worker predictions and diagnostics for the declared complementarity model family.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellPatchComplementarityWorkerResult {
    format: String,
    version: u32,
    /// Pinned worker implementation and asset identity.
    pub backend: WorkerBackend,
    /// SHA-256 of the exact request bytes accepted by the worker.
    pub request_sha256: String,
    /// Held-out results for the declared feature-block model family.
    pub models: Vec<ComplementarityModelResult>,
}

impl CellPatchComplementarityWorkerResult {
    /// Check the worker identity, dimensions, and numerical summaries against the exact request.
    pub fn validate(
        &self,
        request: &CellPatchComplementarityWorkerRequest,
        request_sha256: &str,
    ) -> Result<(), WorkerError> {
        if self.format != "marklab.scipy_cell_patch_complementarity_worker_result"
            || self.version != 1
            || !self.backend.matches(&request.backend)
            || self.request_sha256 != request_sha256
            || self.models.len() != 6
        {
            return Err(WorkerError::WorkerContract(
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
                return Err(WorkerError::WorkerContract(
                    "complementarity model identity or metrics mismatch".into(),
                ));
            }
            for (selection, fold) in model.fold_selections.iter().zip(0..request.outer_folds) {
                if selection.outer_fold != fold
                    || !request.ridge_alphas.contains(&selection.ridge_alpha)
                    || !selection.inner_rmse.is_finite()
                    || selection.inner_rmse < 0.0
                {
                    return Err(WorkerError::WorkerContract(
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
                    return Err(WorkerError::WorkerContract(
                        "complementarity prediction identity mismatch".into(),
                    ));
                }
                let residual = prediction.predicted - prediction.observed;
                squared += residual * residual;
                absolute += residual.abs();
            }
            let rmse = (squared / predictions.len() as f64).sqrt();
            let mae = absolute / predictions.len() as f64;
            if !relative_eq(rmse, model.rmse, RELATIVE_TOLERANCE)
                || !relative_eq(mae, model.mae, RELATIVE_TOLERANCE)
            {
                return Err(WorkerError::WorkerContract(
                    "complementarity metrics disagree with patient predictions".into(),
                ));
            }
        }
        Ok(())
    }
}

fn validate_feature_names(names: &ComplementarityFeatureNames) -> Result<(), WorkerError> {
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
            return Err(WorkerError::InvalidSpec(
                "complementarity feature groups require unique exact prefixed names".into(),
            ));
        }
    }
    Ok(())
}

fn validate_patients(spec: &CellPatchComplementaritySpec) -> Result<(), WorkerError> {
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
            return Err(WorkerError::InvalidSpec(
                "complementarity patient rows or fold identities are invalid".into(),
            ));
        }
        outer_counts[patient.outer_fold as usize] += 1;
    }
    if outer_counts.contains(&0) {
        return Err(WorkerError::InvalidSpec(
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
            return Err(WorkerError::InvalidSpec(
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
            return Err(WorkerError::InvalidSpec(
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

/// Return the ordered predictor names required by one complementarity model.
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
        ("baseline", base.clone()),
        ("baseline_plus_cell", append(&[&names.cell])),
        ("baseline_plus_patch", append(&[&names.patch])),
        (
            "baseline_plus_cell_patch",
            append(&[&names.cell, &names.patch]),
        ),
        (
            "baseline_plus_cell_patch_neighbor",
            append(&[&names.cell, &names.patch, &names.neighbor]),
        ),
        (
            "baseline_plus_cell_patch_neighbor_measured",
            append(&[&names.cell, &names.patch, &names.neighbor, &names.measured]),
        ),
    ]
}
