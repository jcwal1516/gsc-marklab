use std::collections::HashSet;

use marklab_core::backend::ClaimStatus;
use marklab_numerics::{
    dense::{cholesky, cholesky_solve},
    summation::neumaier_sum,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const HARD_MAXIMUM_PATIENTS: usize = 512;
const HARD_MAXIMUM_MODALITIES: usize = 8;
const HARD_MAXIMUM_FEATURES: usize = 32_768;
const HARD_MAXIMUM_WEIGHT_CANDIDATES: usize = 4_096;
const HARD_MAXIMUM_WORK_UNITS: u64 = 2_000_000_000;
const HARD_MAXIMUM_MEMORY_MIB: u64 = 1_024;
const MAXIMUM_ABSOLUTE_INPUT: f64 = 1e100;
const MAXIMUM_IDENTIFIER_BYTES: usize = 256;
const INACTIVE_KERNEL_TOLERANCE: f64 = 1e-12;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum KernelFusionKernel {
    Linear,
    Rbf { bandwidth: f64 },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KernelFusionModality {
    pub name: String,
    pub feature_names: Vec<String>,
    pub kernel: KernelFusionKernel,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KernelFusionPatient {
    pub patient_id: String,
    pub outer_fold: u32,
    pub inner_fold: u32,
    pub target: f64,
    pub features: Vec<Vec<f64>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KernelFusionLimits {
    pub maximum_patients: usize,
    pub maximum_features: usize,
    pub maximum_weight_candidates: usize,
    pub maximum_work_units: u64,
    pub maximum_memory_mib: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KernelFusionSpec {
    pub format: String,
    pub version: u32,
    pub target_name: String,
    pub target_unit: String,
    pub modalities: Vec<KernelFusionModality>,
    pub patients: Vec<KernelFusionPatient>,
    pub outer_folds: u32,
    pub inner_folds: u32,
    pub weight_denominator: u32,
    pub ridge_penalties: Vec<f64>,
    pub limits: KernelFusionLimits,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct KernelFusionPrediction {
    pub patient_id: String,
    pub outer_fold: u32,
    pub observed: f64,
    pub predicted: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct KernelFusionFoldResult {
    pub outer_fold: u32,
    pub training_patients: u32,
    pub test_patients: u32,
    pub weights: Vec<f64>,
    pub ridge_penalty: f64,
    pub inner_rmse: f64,
    pub inactive_modalities: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct KernelFusionModelResult {
    pub id: String,
    pub rmse: f64,
    pub mae: f64,
    pub folds: Vec<KernelFusionFoldResult>,
    pub predictions: Vec<KernelFusionPrediction>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct KernelFusionResourceSummary {
    pub simplex_lattice_candidates: u32,
    pub evaluated_weight_candidates: u32,
    pub estimated_work_units: u64,
    pub estimated_peak_bytes: u64,
    pub limits: KernelFusionLimits,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct KernelFusionResult {
    pub format: &'static str,
    pub version: u32,
    pub target_name: String,
    pub target_unit: String,
    pub patient_count: u32,
    pub modality_count: u32,
    pub outer_folds: u32,
    pub inner_folds: u32,
    pub weight_denominator: u32,
    pub modalities: Vec<KernelFusionModality>,
    pub models: Vec<KernelFusionModelResult>,
    pub resources: KernelFusionResourceSummary,
    pub claim_status: ClaimStatus,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum KernelFusionError {
    #[error("invalid multimodal kernel fusion: {0}")]
    Invalid(String),
    #[error("multimodal kernel fusion resource limit exceeded: {0}")]
    Resource(String),
    #[error("multimodal kernel fusion numerical failure: {0}")]
    Numerical(String),
}

#[derive(Clone)]
struct ModelPlan {
    id: String,
    weights: Vec<Vec<f64>>,
}

struct ModelAccumulator {
    plan: ModelPlan,
    folds: Vec<KernelFusionFoldResult>,
    predictions: Vec<KernelFusionPrediction>,
}

struct PreparedKernels {
    train_count: usize,
    test_count: usize,
    train: Vec<Vec<f64>>,
    cross: Vec<Vec<f64>>,
    inactive: Vec<bool>,
}

struct StandardizedFeatures {
    train: Vec<Vec<f64>>,
    test: Vec<Vec<f64>>,
}

struct Selection {
    weights: Vec<f64>,
    ridge_penalty: f64,
    rmse: f64,
}

struct ResourceEstimate {
    work_units: u64,
    peak_bytes: u64,
}

pub fn learn_multimodal_kernels(
    mut spec: KernelFusionSpec,
) -> Result<KernelFusionResult, KernelFusionError> {
    validate_basic(&mut spec)?;
    let lattice_count = simplex_lattice_count(spec.modalities.len(), spec.weight_denominator)?;
    let learned_weights = weight_candidates(spec.modalities.len(), spec.weight_denominator)?;
    if learned_weights.len() > spec.limits.maximum_weight_candidates
        || learned_weights.len() > HARD_MAXIMUM_WEIGHT_CANDIDATES
    {
        return Err(KernelFusionError::Resource(format!(
            "{} weight candidates exceed the declared or hard limit",
            learned_weights.len()
        )));
    }
    let estimate = estimate_resources(&spec, learned_weights.len())?;
    admit_resources(&spec.limits, &estimate)?;
    let evaluated_weight_candidates = learned_weights.len() as u32;

    let modality_count = spec.modalities.len();
    let equal_weights = vec![1.0 / modality_count as f64; modality_count];
    let mut plans = vec![
        ModelPlan {
            id: "learned".into(),
            weights: learned_weights,
        },
        ModelPlan {
            id: "equal_weights".into(),
            weights: vec![equal_weights],
        },
    ];
    for (index, modality) in spec.modalities.iter().enumerate() {
        let mut weights = vec![0.0; modality_count];
        weights[index] = 1.0;
        plans.push(ModelPlan {
            id: format!("only_{}", modality.name),
            weights: vec![weights],
        });
    }
    let mut models = plans
        .into_iter()
        .map(|plan| ModelAccumulator {
            plan,
            folds: Vec::with_capacity(spec.outer_folds as usize),
            predictions: Vec::with_capacity(spec.patients.len()),
        })
        .collect::<Vec<_>>();

    for outer_fold in 0..spec.outer_folds {
        let outer_train =
            indices_matching(&spec.patients, |patient| patient.outer_fold != outer_fold);
        let outer_test =
            indices_matching(&spec.patients, |patient| patient.outer_fold == outer_fold);
        let mut inner = Vec::with_capacity(spec.inner_folds as usize);
        for inner_fold in 0..spec.inner_folds {
            let inner_train = outer_train
                .iter()
                .copied()
                .filter(|index| spec.patients[*index].inner_fold != inner_fold)
                .collect::<Vec<_>>();
            let inner_validation = outer_train
                .iter()
                .copied()
                .filter(|index| spec.patients[*index].inner_fold == inner_fold)
                .collect::<Vec<_>>();
            let kernels = prepare_kernels(&spec, &inner_train, &inner_validation)?;
            inner.push((inner_train, inner_validation, kernels));
        }
        let outer_kernels = prepare_kernels(&spec, &outer_train, &outer_test)?;
        let inactive_modalities = outer_kernels
            .inactive
            .iter()
            .zip(&spec.modalities)
            .filter(|(inactive, _)| **inactive)
            .map(|(_, modality)| modality.name.clone())
            .collect::<Vec<_>>();

        for model in &mut models {
            let selection = select_hyperparameters(&spec, &inner, &model.plan.weights)?;
            let predictions = fit_predict(
                &spec,
                &outer_train,
                &outer_test,
                &outer_kernels,
                &selection.weights,
                selection.ridge_penalty,
            )?;
            model.folds.push(KernelFusionFoldResult {
                outer_fold,
                training_patients: outer_train.len() as u32,
                test_patients: outer_test.len() as u32,
                weights: selection.weights,
                ridge_penalty: selection.ridge_penalty,
                inner_rmse: selection.rmse,
                inactive_modalities: inactive_modalities.clone(),
            });
            model
                .predictions
                .extend(
                    outer_test
                        .iter()
                        .zip(predictions)
                        .map(|(index, predicted)| KernelFusionPrediction {
                            patient_id: spec.patients[*index].patient_id.clone(),
                            outer_fold,
                            observed: spec.patients[*index].target,
                            predicted,
                        }),
                );
        }
    }

    let models = models
        .into_iter()
        .map(finish_model)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(KernelFusionResult {
        format: "marklab.multimodal_kernel_fusion",
        version: 1,
        target_name: spec.target_name,
        target_unit: spec.target_unit,
        patient_count: spec.patients.len() as u32,
        modality_count: spec.modalities.len() as u32,
        outer_folds: spec.outer_folds,
        inner_folds: spec.inner_folds,
        weight_denominator: spec.weight_denominator,
        modalities: spec.modalities,
        models,
        resources: KernelFusionResourceSummary {
            simplex_lattice_candidates: lattice_count as u32,
            evaluated_weight_candidates,
            estimated_work_units: estimate.work_units,
            estimated_peak_bytes: estimate.peak_bytes,
            limits: spec.limits,
        },
        claim_status: ClaimStatus::ExperimentalPredictiveComparison,
    })
}

fn validate_basic(spec: &mut KernelFusionSpec) -> Result<(), KernelFusionError> {
    if spec.format != "marklab.multimodal_kernel_fusion_request"
        || spec.version != 1
        || !valid_text(&spec.target_name)
        || !valid_text(&spec.target_unit)
        || !(2..=HARD_MAXIMUM_MODALITIES).contains(&spec.modalities.len())
        || !(6..=HARD_MAXIMUM_PATIENTS).contains(&spec.patients.len())
        || !(2..=10).contains(&spec.outer_folds)
        || !(2..=10).contains(&spec.inner_folds)
        || !(1..=20).contains(&spec.weight_denominator)
        || spec.ridge_penalties.is_empty()
        || spec.ridge_penalties.len() > 16
        || spec
            .ridge_penalties
            .iter()
            .any(|value| !value.is_finite() || *value < 1e-8 || *value > 1e12)
    {
        return Err(KernelFusionError::Invalid(
            "request identity, dimensions, folds, weights, or ridge penalties are invalid".into(),
        ));
    }
    spec.ridge_penalties.sort_by(f64::total_cmp);
    spec.ridge_penalties
        .dedup_by(|left, right| left.to_bits() == right.to_bits());
    validate_limits(&spec.limits, spec.patients.len())?;

    let mut modality_names = HashSet::new();
    let mut total_features = 0_usize;
    for modality in &spec.modalities {
        let mut feature_names = HashSet::new();
        if !valid_name(&modality.name)
            || !modality_names.insert(modality.name.as_str())
            || modality.name.len() > MAXIMUM_IDENTIFIER_BYTES
            || modality.feature_names.is_empty()
            || modality
                .feature_names
                .iter()
                .any(|name| !valid_text(name) || !feature_names.insert(name.as_str()))
            || matches!(
                modality.kernel,
                KernelFusionKernel::Rbf { bandwidth }
                    if !bandwidth.is_finite() || bandwidth <= 0.0 || bandwidth > 1e6
            )
        {
            return Err(KernelFusionError::Invalid(
                "modality names, feature names, or kernels are invalid".into(),
            ));
        }
        total_features = total_features
            .checked_add(modality.feature_names.len())
            .ok_or_else(|| KernelFusionError::Resource("feature count overflowed".into()))?;
    }
    if total_features > spec.limits.maximum_features || total_features > HARD_MAXIMUM_FEATURES {
        return Err(KernelFusionError::Resource(format!(
            "{total_features} features exceed the declared or hard limit"
        )));
    }

    spec.patients
        .sort_by(|left, right| left.patient_id.cmp(&right.patient_id));
    let mut patient_ids = HashSet::new();
    for patient in &spec.patients {
        if !valid_text(&patient.patient_id)
            || !patient_ids.insert(patient.patient_id.as_str())
            || patient.outer_fold >= spec.outer_folds
            || patient.inner_fold >= spec.inner_folds
            || !finite_bounded(patient.target)
            || patient.features.len() != spec.modalities.len()
            || patient
                .features
                .iter()
                .zip(&spec.modalities)
                .any(|(features, modality)| {
                    features.len() != modality.feature_names.len()
                        || features.iter().any(|value| !finite_bounded(*value))
                })
        {
            return Err(KernelFusionError::Invalid(
                "patient identity, folds, target, or feature dimensions are invalid".into(),
            ));
        }
    }
    for outer_fold in 0..spec.outer_folds {
        let outer_train = spec
            .patients
            .iter()
            .filter(|patient| patient.outer_fold != outer_fold)
            .collect::<Vec<_>>();
        let outer_test_count = spec
            .patients
            .iter()
            .filter(|patient| patient.outer_fold == outer_fold)
            .count();
        if outer_test_count == 0 || outer_train.len() < 4 {
            return Err(KernelFusionError::Invalid(
                "every outer fold requires held-out patients and at least four training patients"
                    .into(),
            ));
        }
        for inner_fold in 0..spec.inner_folds {
            let validation_count = outer_train
                .iter()
                .filter(|patient| patient.inner_fold == inner_fold)
                .count();
            if validation_count == 0 || outer_train.len() - validation_count < 2 {
                return Err(KernelFusionError::Invalid(
                    "every nested inner fold requires validation patients and at least two training patients"
                        .into(),
                ));
            }
        }
    }
    Ok(())
}

fn validate_limits(limits: &KernelFusionLimits, patients: usize) -> Result<(), KernelFusionError> {
    if limits.maximum_patients < patients
        || limits.maximum_patients > HARD_MAXIMUM_PATIENTS
        || limits.maximum_features == 0
        || limits.maximum_features > HARD_MAXIMUM_FEATURES
        || limits.maximum_weight_candidates == 0
        || limits.maximum_weight_candidates > HARD_MAXIMUM_WEIGHT_CANDIDATES
        || limits.maximum_work_units == 0
        || limits.maximum_work_units > HARD_MAXIMUM_WORK_UNITS
        || limits.maximum_memory_mib == 0
        || limits.maximum_memory_mib > HARD_MAXIMUM_MEMORY_MIB
    {
        return Err(KernelFusionError::Invalid(
            "declared resource limits are invalid".into(),
        ));
    }
    Ok(())
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn valid_text(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAXIMUM_IDENTIFIER_BYTES
}

fn finite_bounded(value: f64) -> bool {
    value.is_finite() && value.abs() <= MAXIMUM_ABSOLUTE_INPUT
}

fn simplex_lattice_count(modalities: usize, denominator: u32) -> Result<usize, KernelFusionError> {
    let n = denominator as u128 + modalities as u128 - 1;
    let k = (modalities - 1).min(denominator as usize) as u128;
    let mut value = 1_u128;
    for step in 1..=k {
        value = value
            .checked_mul(n - k + step)
            .and_then(|product| product.checked_div(step))
            .ok_or_else(|| KernelFusionError::Resource("weight count overflowed".into()))?;
    }
    usize::try_from(value)
        .map_err(|_| KernelFusionError::Resource("weight count exceeds usize".into()))
}

fn weight_candidates(
    modalities: usize,
    denominator: u32,
) -> Result<Vec<Vec<f64>>, KernelFusionError> {
    let count = simplex_lattice_count(modalities, denominator)?;
    if count > HARD_MAXIMUM_WEIGHT_CANDIDATES {
        return Err(KernelFusionError::Resource(format!(
            "{count} simplex lattice weights exceed the hard limit"
        )));
    }
    let mut integer_weights = Vec::with_capacity(count);
    enumerate_weights(
        modalities,
        denominator,
        &mut Vec::with_capacity(modalities),
        &mut integer_weights,
    );
    let scale = f64::from(denominator);
    let mut weights = integer_weights
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|value| f64::from(value) / scale)
                .collect()
        })
        .collect::<Vec<Vec<f64>>>();
    if !denominator.is_multiple_of(modalities as u32) {
        weights.push(vec![1.0 / modalities as f64; modalities]);
    }
    weights.sort_by(|left, right| lexicographic_f64(left, right));
    Ok(weights)
}

fn enumerate_weights(
    modalities: usize,
    remaining: u32,
    current: &mut Vec<u32>,
    output: &mut Vec<Vec<u32>>,
) {
    if current.len() + 1 == modalities {
        current.push(remaining);
        output.push(current.clone());
        current.pop();
        return;
    }
    for weight in 0..=remaining {
        current.push(weight);
        enumerate_weights(modalities, remaining - weight, current, output);
        current.pop();
    }
}

fn lexicographic_f64(left: &[f64], right: &[f64]) -> std::cmp::Ordering {
    left.iter()
        .zip(right)
        .map(|(left, right)| left.total_cmp(right))
        .find(|ordering| !ordering.is_eq())
        .unwrap_or_else(|| left.len().cmp(&right.len()))
}

fn estimate_resources(
    spec: &KernelFusionSpec,
    learned_candidates: usize,
) -> Result<ResourceEstimate, KernelFusionError> {
    let total_features = spec
        .modalities
        .iter()
        .map(|modality| modality.feature_names.len() as u128)
        .sum::<u128>();
    let modalities = spec.modalities.len() as u128;
    let fixed_models = modalities + 1;
    let all_models = modalities + 2;
    let penalties = spec.ridge_penalties.len() as u128;
    let mut work = 0_u128;
    let mut maximum_kernel_values = 0_u128;
    let mut maximum_train = 0_u128;
    let mut maximum_cross_values = 0_u128;
    let mut maximum_split_rows = 0_u128;
    for outer_fold in 0..spec.outer_folds {
        let outer_train = spec
            .patients
            .iter()
            .filter(|patient| patient.outer_fold != outer_fold)
            .count() as u128;
        let outer_test = spec.patients.len() as u128 - outer_train;
        maximum_train = maximum_train.max(outer_train);
        maximum_cross_values = maximum_cross_values.max(outer_test * outer_train);
        maximum_split_rows = maximum_split_rows.max(outer_train + outer_test);
        work = checked_add(
            work,
            kernel_preparation_work(outer_train, outer_test, total_features)?,
        )?;
        let mut retained_kernel_values =
            modalities * (outer_train * outer_train + outer_test * outer_train);
        work = checked_add(
            work,
            all_models * fit_work(outer_train, outer_test, modalities)?,
        )?;
        for inner_fold in 0..spec.inner_folds {
            let inner_validation = spec
                .patients
                .iter()
                .filter(|patient| {
                    patient.outer_fold != outer_fold && patient.inner_fold == inner_fold
                })
                .count() as u128;
            let inner_train = outer_train - inner_validation;
            maximum_cross_values = maximum_cross_values.max(inner_validation * inner_train);
            work = checked_add(
                work,
                kernel_preparation_work(inner_train, inner_validation, total_features)?,
            )?;
            retained_kernel_values +=
                modalities * (inner_train * inner_train + inner_validation * inner_train);
            let fits = (learned_candidates as u128 + fixed_models) * penalties;
            work = checked_add(
                work,
                fits * fit_work(inner_train, inner_validation, modalities)?,
            )?;
        }
        maximum_kernel_values = maximum_kernel_values.max(retained_kernel_values);
    }
    let input_values = spec.patients.len() as u128 * total_features;
    let feature_workspace = maximum_split_rows * total_features * 2;
    let matrix_workspace = maximum_train * maximum_train * 5 + maximum_cross_values;
    let candidate_values = learned_candidates as u128 * modalities;
    let numeric_bytes = (input_values
        + feature_workspace
        + maximum_kernel_values
        + matrix_workspace
        + candidate_values)
        .checked_mul(8)
        .ok_or_else(|| KernelFusionError::Resource("numeric memory estimate overflowed".into()))?;
    let peak_bytes = checked_add(
        numeric_bytes,
        metadata_and_result_bytes(spec, learned_candidates)?,
    )?;
    Ok(ResourceEstimate {
        work_units: u64::try_from(work)
            .map_err(|_| KernelFusionError::Resource("work estimate exceeds u64".into()))?,
        peak_bytes: u64::try_from(peak_bytes)
            .map_err(|_| KernelFusionError::Resource("memory estimate exceeds u64".into()))?,
    })
}

fn metadata_and_result_bytes(
    spec: &KernelFusionSpec,
    learned_candidates: usize,
) -> Result<u128, KernelFusionError> {
    let patients = spec.patients.len() as u128;
    let modalities = spec.modalities.len() as u128;
    let models = modalities + 2;
    let folds = models * u128::from(spec.outer_folds);
    let predictions = models * patients;
    let feature_names = spec
        .modalities
        .iter()
        .map(|modality| modality.feature_names.len() as u128)
        .sum::<u128>();
    let input_string_bytes = (spec.target_name.len() + spec.target_unit.len()) as u128
        + spec
            .modalities
            .iter()
            .map(|modality| {
                modality.name.len()
                    + modality
                        .feature_names
                        .iter()
                        .map(String::len)
                        .sum::<usize>()
            })
            .sum::<usize>() as u128
        + spec
            .patients
            .iter()
            .map(|patient| patient.patient_id.len())
            .sum::<usize>() as u128;
    let input_structure_bytes = std::mem::size_of::<KernelFusionSpec>() as u128
        + patients * std::mem::size_of::<KernelFusionPatient>() as u128
        + patients * modalities * std::mem::size_of::<Vec<f64>>() as u128
        + modalities * std::mem::size_of::<KernelFusionModality>() as u128
        + feature_names * std::mem::size_of::<String>() as u128
        + spec.ridge_penalties.len() as u128 * std::mem::size_of::<f64>() as u128;

    // Reserve room for candidate generation and model-plan storage.
    let learned_grid_bytes = 2
        * (learned_candidates as u128 * std::mem::size_of::<Vec<f64>>() as u128
            + learned_candidates as u128 * modalities * std::mem::size_of::<f64>() as u128);
    let fixed_weight_rows = modalities + 1;
    let fixed_grid_bytes = fixed_weight_rows * std::mem::size_of::<Vec<f64>>() as u128
        + fixed_weight_rows * modalities * std::mem::size_of::<f64>() as u128;
    let plan_bytes = models * std::mem::size_of::<ModelAccumulator>() as u128;
    let retained_split_count = u128::from(spec.inner_folds) + 1;
    let kernel_container_bytes = retained_split_count
        * (std::mem::size_of::<PreparedKernels>() as u128
            + 2 * modalities * std::mem::size_of::<Vec<f64>>() as u128
            + modalities * std::mem::size_of::<bool>() as u128)
        + 2 * patients * std::mem::size_of::<Vec<f64>>() as u128
        + patients * (u128::from(spec.inner_folds) + 2) * std::mem::size_of::<usize>() as u128;

    let patient_identifier_bytes = spec
        .patients
        .iter()
        .map(|patient| patient.patient_id.len() as u128)
        .sum::<u128>();
    let modality_identifier_bytes = spec
        .modalities
        .iter()
        .map(|modality| modality.name.len() as u128)
        .sum::<u128>();
    let prediction_bytes = predictions * std::mem::size_of::<KernelFusionPrediction>() as u128
        + models * patient_identifier_bytes;
    let fold_bytes = folds * std::mem::size_of::<KernelFusionFoldResult>() as u128
        + folds * modalities * std::mem::size_of::<f64>() as u128
        + folds * modalities * std::mem::size_of::<String>() as u128
        + folds * modality_identifier_bytes;
    let model_identifier_bytes = "learned".len() as u128
        + "equal_weights".len() as u128
        + spec
            .modalities
            .iter()
            .map(|modality| "only_".len() as u128 + modality.name.len() as u128)
            .sum::<u128>();
    let result_bytes = models * std::mem::size_of::<KernelFusionModelResult>() as u128
        + prediction_bytes
        + fold_bytes
        + model_identifier_bytes;
    [
        input_string_bytes,
        input_structure_bytes,
        learned_grid_bytes,
        fixed_grid_bytes,
        plan_bytes,
        kernel_container_bytes,
        result_bytes,
    ]
    .into_iter()
    .try_fold(0_u128, checked_add)
}

fn kernel_preparation_work(
    train: u128,
    test: u128,
    features: u128,
) -> Result<u128, KernelFusionError> {
    (train * train + test * train)
        .checked_mul(features.max(1))
        .ok_or_else(|| KernelFusionError::Resource("kernel work overflowed".into()))
}

fn fit_work(train: u128, test: u128, modalities: u128) -> Result<u128, KernelFusionError> {
    train
        .checked_mul(train)
        .and_then(|square| square.checked_mul(train + modalities))
        .and_then(|work| work.checked_add(test * train * modalities))
        .ok_or_else(|| KernelFusionError::Resource("fit work overflowed".into()))
}

fn checked_add(left: u128, right: u128) -> Result<u128, KernelFusionError> {
    left.checked_add(right)
        .ok_or_else(|| KernelFusionError::Resource("work estimate overflowed".into()))
}

fn admit_resources(
    limits: &KernelFusionLimits,
    estimate: &ResourceEstimate,
) -> Result<(), KernelFusionError> {
    if estimate.work_units > limits.maximum_work_units {
        return Err(KernelFusionError::Resource(format!(
            "estimated work {} exceeds maximum_work_units {}",
            estimate.work_units, limits.maximum_work_units
        )));
    }
    let maximum_bytes = limits
        .maximum_memory_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| KernelFusionError::Resource("memory limit overflowed".into()))?;
    if estimate.peak_bytes > maximum_bytes {
        return Err(KernelFusionError::Resource(format!(
            "estimated peak memory {} bytes exceeds the declared {maximum_bytes} bytes",
            estimate.peak_bytes
        )));
    }
    Ok(())
}

fn indices_matching(
    patients: &[KernelFusionPatient],
    predicate: impl Fn(&KernelFusionPatient) -> bool,
) -> Vec<usize> {
    patients
        .iter()
        .enumerate()
        .filter(|(_, patient)| predicate(patient))
        .map(|(index, _)| index)
        .collect()
}

fn prepare_kernels(
    spec: &KernelFusionSpec,
    train_indices: &[usize],
    test_indices: &[usize],
) -> Result<PreparedKernels, KernelFusionError> {
    let mut train_kernels = Vec::with_capacity(spec.modalities.len());
    let mut cross_kernels = Vec::with_capacity(spec.modalities.len());
    let mut inactive = Vec::with_capacity(spec.modalities.len());
    for (modality_index, modality) in spec.modalities.iter().enumerate() {
        let standardized =
            standardized_features(&spec.patients, train_indices, test_indices, modality_index)?;
        let (train, cross, is_inactive) =
            centered_normalized_kernel(&standardized.train, &standardized.test, &modality.kernel)?;
        train_kernels.push(train);
        cross_kernels.push(cross);
        inactive.push(is_inactive);
    }
    Ok(PreparedKernels {
        train_count: train_indices.len(),
        test_count: test_indices.len(),
        train: train_kernels,
        cross: cross_kernels,
        inactive,
    })
}

fn standardized_features(
    patients: &[KernelFusionPatient],
    train_indices: &[usize],
    test_indices: &[usize],
    modality: usize,
) -> Result<StandardizedFeatures, KernelFusionError> {
    let dimensions = patients[train_indices[0]].features[modality].len();
    let mut means = vec![0.0; dimensions];
    let mut standard_deviations = vec![0.0; dimensions];
    for dimension in 0..dimensions {
        means[dimension] = neumaier_sum(
            train_indices
                .iter()
                .map(|index| patients[*index].features[modality][dimension]),
        ) / train_indices.len() as f64;
        let residual_scale = train_indices
            .iter()
            .map(|index| (patients[*index].features[modality][dimension] - means[dimension]).abs())
            .fold(0.0_f64, f64::max);
        if !residual_scale.is_finite() {
            return Err(KernelFusionError::Numerical(
                "feature residual scale is not finite".into(),
            ));
        }
        if residual_scale == 0.0 {
            standard_deviations[dimension] = 0.0;
            continue;
        }
        let normalized_variance = neumaier_sum(train_indices.iter().map(|index| {
            let residual = patients[*index].features[modality][dimension] - means[dimension];
            (residual / residual_scale).powi(2)
        })) / train_indices.len() as f64;
        let standard_deviation = residual_scale * normalized_variance.sqrt();
        if !normalized_variance.is_finite()
            || normalized_variance <= 0.0
            || !standard_deviation.is_finite()
            || standard_deviation == 0.0
        {
            return Err(KernelFusionError::Numerical(
                "nonconstant feature standard deviation is not finite and positive".into(),
            ));
        }
        standard_deviations[dimension] = standard_deviation;
    }
    let transform = |indices: &[usize]| {
        indices
            .iter()
            .map(|index| {
                patients[*index].features[modality]
                    .iter()
                    .enumerate()
                    .map(|(dimension, value)| {
                        if standard_deviations[dimension] == 0.0 {
                            0.0
                        } else {
                            (value - means[dimension]) / standard_deviations[dimension]
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    let train = transform(train_indices);
    let test = transform(test_indices);
    if train
        .iter()
        .chain(&test)
        .flatten()
        .any(|value| !value.is_finite())
    {
        return Err(KernelFusionError::Numerical(
            "standardized feature is not finite".into(),
        ));
    }
    Ok(StandardizedFeatures { train, test })
}

fn centered_normalized_kernel(
    train_features: &[Vec<f64>],
    test_features: &[Vec<f64>],
    kernel: &KernelFusionKernel,
) -> Result<(Vec<f64>, Vec<f64>, bool), KernelFusionError> {
    let train_count = train_features.len();
    let test_count = test_features.len();
    let mut train = vec![0.0; train_count * train_count];
    for row in 0..train_count {
        for column in 0..train_count {
            train[row * train_count + column] =
                kernel_value(&train_features[row], &train_features[column], kernel);
        }
    }
    let mut cross = vec![0.0; test_count * train_count];
    for row in 0..test_count {
        for column in 0..train_count {
            cross[row * train_count + column] =
                kernel_value(&test_features[row], &train_features[column], kernel);
        }
    }
    if train.iter().chain(&cross).any(|value| !value.is_finite()) {
        return Err(KernelFusionError::Numerical(
            "raw kernel contains a non-finite value".into(),
        ));
    }
    let column_means = (0..train_count)
        .map(|column| {
            neumaier_sum((0..train_count).map(|row| train[row * train_count + column]))
                / train_count as f64
        })
        .collect::<Vec<_>>();
    let grand_mean = neumaier_sum(column_means.iter().copied()) / train_count as f64;
    for row in 0..train_count {
        let row_mean =
            neumaier_sum((0..train_count).map(|column| train[row * train_count + column]))
                / train_count as f64;
        for column in 0..train_count {
            train[row * train_count + column] =
                train[row * train_count + column] - row_mean - column_means[column] + grand_mean;
        }
    }
    for row in 0..test_count {
        let row_mean =
            neumaier_sum((0..train_count).map(|column| cross[row * train_count + column]))
                / train_count as f64;
        for column in 0..train_count {
            cross[row * train_count + column] =
                cross[row * train_count + column] - row_mean - column_means[column] + grand_mean;
        }
    }
    let mean_diagonal = neumaier_sum((0..train_count).map(|row| train[row * train_count + row]))
        / train_count as f64;
    if !mean_diagonal.is_finite() {
        return Err(KernelFusionError::Numerical(
            "centered kernel diagonal is not finite".into(),
        ));
    }
    if mean_diagonal <= INACTIVE_KERNEL_TOLERANCE {
        train.fill(0.0);
        cross.fill(0.0);
        return Ok((train, cross, true));
    }
    for value in train.iter_mut().chain(&mut cross) {
        *value /= mean_diagonal;
        if !value.is_finite() {
            return Err(KernelFusionError::Numerical(
                "normalized kernel contains a non-finite value".into(),
            ));
        }
    }
    Ok((train, cross, false))
}

fn kernel_value(left: &[f64], right: &[f64], kernel: &KernelFusionKernel) -> f64 {
    match *kernel {
        KernelFusionKernel::Linear => {
            neumaier_sum(left.iter().zip(right).map(|(left, right)| left * right))
                / left.len() as f64
        }
        KernelFusionKernel::Rbf { bandwidth } => {
            let mean_squared_distance = neumaier_sum(
                left.iter()
                    .zip(right)
                    .map(|(left, right)| (left - right).powi(2)),
            ) / left.len() as f64;
            (-mean_squared_distance / (2.0 * bandwidth.powi(2))).exp()
        }
    }
}

fn select_hyperparameters(
    spec: &KernelFusionSpec,
    inner: &[(Vec<usize>, Vec<usize>, PreparedKernels)],
    candidates: &[Vec<f64>],
) -> Result<Selection, KernelFusionError> {
    let mut best: Option<Selection> = None;
    for weights in candidates {
        for ridge_penalty in &spec.ridge_penalties {
            let mut squared_error = 0.0;
            let mut count = 0_usize;
            for (train, validation, kernels) in inner {
                let predictions =
                    fit_predict(spec, train, validation, kernels, weights, *ridge_penalty)?;
                squared_error += neumaier_sum(predictions.iter().zip(validation).map(
                    |(prediction, index)| (prediction - spec.patients[*index].target).powi(2),
                ));
                count += validation.len();
            }
            let rmse = (squared_error / count as f64).sqrt();
            if !rmse.is_finite() {
                return Err(KernelFusionError::Numerical(
                    "inner-fold RMSE is not finite".into(),
                ));
            }
            let candidate = Selection {
                weights: weights.clone(),
                ridge_penalty: *ridge_penalty,
                rmse,
            };
            if best
                .as_ref()
                .is_none_or(|current| selection_is_better(&candidate, current))
            {
                best = Some(candidate);
            }
        }
    }
    best.ok_or_else(|| KernelFusionError::Invalid("no hyperparameter candidate exists".into()))
}

fn selection_is_better(candidate: &Selection, current: &Selection) -> bool {
    candidate
        .rmse
        .total_cmp(&current.rmse)
        .then_with(|| candidate.ridge_penalty.total_cmp(&current.ridge_penalty))
        .then_with(|| lexicographic_f64(&candidate.weights, &current.weights))
        .is_lt()
}

fn fit_predict(
    spec: &KernelFusionSpec,
    train_indices: &[usize],
    test_indices: &[usize],
    kernels: &PreparedKernels,
    weights: &[f64],
    ridge_penalty: f64,
) -> Result<Vec<f64>, KernelFusionError> {
    if kernels.train_count != train_indices.len()
        || kernels.test_count != test_indices.len()
        || weights.len() != spec.modalities.len()
    {
        return Err(KernelFusionError::Numerical(
            "kernel split dimensions differ from the requested fit".into(),
        ));
    }
    let target_mean = neumaier_sum(
        train_indices
            .iter()
            .map(|index| spec.patients[*index].target),
    ) / train_indices.len() as f64;
    let centered_target = train_indices
        .iter()
        .map(|index| spec.patients[*index].target - target_mean)
        .collect::<Vec<_>>();
    let mut train_kernel = mix_kernels(&kernels.train, weights, kernels.train_count.pow(2))?;
    for diagonal in 0..kernels.train_count {
        train_kernel[diagonal * kernels.train_count + diagonal] += ridge_penalty;
    }
    let lower = cholesky(&train_kernel, kernels.train_count).ok_or_else(|| {
        KernelFusionError::Numerical("regularized kernel matrix is not positive definite".into())
    })?;
    let coefficients = cholesky_solve(&lower, kernels.train_count, &centered_target);
    let cross_kernel = mix_kernels(
        &kernels.cross,
        weights,
        kernels.test_count * kernels.train_count,
    )?;
    let predictions = (0..kernels.test_count)
        .map(|row| {
            target_mean
                + neumaier_sum((0..kernels.train_count).map(|column| {
                    cross_kernel[row * kernels.train_count + column] * coefficients[column]
                }))
        })
        .collect::<Vec<_>>();
    if coefficients
        .iter()
        .chain(&predictions)
        .any(|value| !value.is_finite())
    {
        return Err(KernelFusionError::Numerical(
            "kernel ridge fit produced a non-finite value".into(),
        ));
    }
    Ok(predictions)
}

fn mix_kernels(
    kernels: &[Vec<f64>],
    weights: &[f64],
    expected_values: usize,
) -> Result<Vec<f64>, KernelFusionError> {
    if kernels.len() != weights.len()
        || kernels.iter().any(|kernel| kernel.len() != expected_values)
        || weights
            .iter()
            .any(|weight| !weight.is_finite() || *weight < 0.0)
        || (neumaier_sum(weights.iter().copied()) - 1.0).abs() > 1e-12
    {
        return Err(KernelFusionError::Numerical(
            "kernel weights or dimensions are invalid".into(),
        ));
    }
    let mut mixed = vec![0.0; expected_values];
    for (kernel, weight) in kernels.iter().zip(weights) {
        for (mixed, value) in mixed.iter_mut().zip(kernel) {
            *mixed += weight * value;
        }
    }
    if mixed.iter().any(|value| !value.is_finite()) {
        return Err(KernelFusionError::Numerical(
            "mixed kernel contains a non-finite value".into(),
        ));
    }
    Ok(mixed)
}

fn finish_model(model: ModelAccumulator) -> Result<KernelFusionModelResult, KernelFusionError> {
    let squared_error = neumaier_sum(
        model
            .predictions
            .iter()
            .map(|prediction| (prediction.predicted - prediction.observed).powi(2)),
    );
    let absolute_error = neumaier_sum(
        model
            .predictions
            .iter()
            .map(|prediction| (prediction.predicted - prediction.observed).abs()),
    );
    let rmse = (squared_error / model.predictions.len() as f64).sqrt();
    let mae = absolute_error / model.predictions.len() as f64;
    if !rmse.is_finite() || !mae.is_finite() {
        return Err(KernelFusionError::Numerical(
            "outer-fold prediction metric is not finite".into(),
        ));
    }
    Ok(KernelFusionModelResult {
        id: model.plan.id,
        rmse,
        mae,
        folds: model.folds,
        predictions: model.predictions,
    })
}
