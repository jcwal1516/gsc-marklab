use marklab_workflow::ContentDigest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::common::summation::kahan_add;

use crate::{
    compartment_interface::analysis::measurement_status_name, DeclaredScalarPatternInput,
    ScalarMarkId,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SoftClassCompositionLimits {
    pub maximum_points: usize,
    pub maximum_classes: usize,
    pub maximum_values: usize,
    pub maximum_retained_bytes: usize,
}

impl SoftClassCompositionLimits {
    pub fn new(
        maximum_points: usize,
        maximum_classes: usize,
        maximum_values: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, SoftClassCompositionError> {
        let capacity = maximum_points
            .checked_mul(maximum_classes)
            .ok_or(SoftClassCompositionError::InvalidResourceLimit)?;
        if [
            maximum_points,
            maximum_classes,
            maximum_values,
            maximum_retained_bytes,
        ]
        .contains(&0)
            || maximum_values < capacity
        {
            return Err(SoftClassCompositionError::InvalidResourceLimit);
        }
        Ok(Self {
            maximum_points,
            maximum_classes,
            maximum_values,
            maximum_retained_bytes,
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SoftClassCompositionClass {
    pub class_id: String,
    pub mean_probability: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SoftClassCompositionResult {
    pub case_id: String,
    pub timepoint: String,
    pub mark_id: String,
    pub measurement_status: String,
    pub row_count: usize,
    pub class_count: usize,
    pub classes: Vec<SoftClassCompositionClass>,
    pub mean_row_entropy_nats: f64,
    pub aggregate_composition_entropy_nats: f64,
    pub effective_class_count: f64,
    pub maximum_row_sum_absolute_error: f64,
    pub configuration_digest: String,
    pub estimated_storage_bytes: usize,
    pub limits: SoftClassCompositionLimits,
}

pub fn soft_class_composition(
    input: &DeclaredScalarPatternInput<'_>,
    mark_id: &ScalarMarkId,
    limits: &SoftClassCompositionLimits,
) -> Result<SoftClassCompositionResult, SoftClassCompositionError> {
    let table = input
        .mark_table()
        .ok_or(SoftClassCompositionError::MissingProbabilitySimplex)?;
    let values = table
        .probability_simplex_values(mark_id)
        .ok_or(SoftClassCompositionError::MissingProbabilitySimplex)?;
    let levels = table
        .probability_simplex_levels(mark_id)
        .ok_or(SoftClassCompositionError::MissingProbabilitySimplex)?;
    let status = table
        .measurement_status(mark_id)
        .ok_or(SoftClassCompositionError::MissingProbabilitySimplex)?;
    let rows = input.pattern().len();
    let classes = levels.len();
    if rows == 0 {
        return Err(SoftClassCompositionError::EmptyInput);
    }
    if rows > limits.maximum_points {
        return Err(SoftClassCompositionError::PointLimitExceeded {
            observed: rows,
            maximum: limits.maximum_points,
        });
    }
    if classes > limits.maximum_classes {
        return Err(SoftClassCompositionError::ClassLimitExceeded {
            observed: classes,
            maximum: limits.maximum_classes,
        });
    }
    let expected_values = rows
        .checked_mul(classes)
        .ok_or(SoftClassCompositionError::SizeOverflow)?;
    if values.len() != expected_values || values.len() > limits.maximum_values {
        return Err(SoftClassCompositionError::ValueLimitExceeded {
            observed: values.len(),
            maximum: limits.maximum_values,
        });
    }
    let estimated_storage_bytes = classes
        .checked_mul(
            std::mem::size_of::<SoftClassCompositionClass>() + 3 * std::mem::size_of::<f64>(),
        )
        .and_then(|value| {
            levels
                .iter()
                .try_fold(value, |total, level| total.checked_add(level.len()))
        })
        .ok_or(SoftClassCompositionError::SizeOverflow)?;
    if estimated_storage_bytes > limits.maximum_retained_bytes {
        return Err(SoftClassCompositionError::RetainedByteLimitExceeded {
            required: estimated_storage_bytes,
            maximum: limits.maximum_retained_bytes,
        });
    }
    let mut sums = vec![0.0; classes];
    let mut corrections = vec![0.0; classes];
    let mut row_entropy_sum = 0.0;
    let mut row_entropy_correction = 0.0;
    let mut maximum_row_sum_absolute_error = 0.0_f64;
    for row in values.chunks_exact(classes) {
        let mut row_sum = 0.0;
        let mut entropy = 0.0;
        for (class, value) in row.iter().copied().enumerate() {
            let value = f64::from(value);
            row_sum += value;
            kahan_add(&mut sums[class], &mut corrections[class], value);
            if value > 0.0 {
                entropy -= value * value.ln();
            }
        }
        maximum_row_sum_absolute_error = maximum_row_sum_absolute_error.max((row_sum - 1.0).abs());
        kahan_add(&mut row_entropy_sum, &mut row_entropy_correction, entropy);
    }
    let means = sums
        .into_iter()
        .zip(corrections)
        .map(|(sum, correction)| (sum + correction) / rows as f64)
        .collect::<Vec<_>>();
    let aggregate_entropy = entropy(&means);
    let classes = levels
        .iter()
        .zip(means)
        .map(|(class_id, mean_probability)| SoftClassCompositionClass {
            class_id: class_id.clone(),
            mean_probability,
        })
        .collect();
    let configuration_digest = configuration_digest(input, mark_id, limits)?;
    Ok(SoftClassCompositionResult {
        case_id: input.pattern().meta.case_id.clone(),
        timepoint: input.pattern().meta.timepoint.clone(),
        mark_id: mark_id.as_str().into(),
        measurement_status: measurement_status_name(status).into(),
        row_count: rows,
        class_count: levels.len(),
        classes,
        mean_row_entropy_nats: (row_entropy_sum + row_entropy_correction) / rows as f64,
        aggregate_composition_entropy_nats: aggregate_entropy,
        effective_class_count: aggregate_entropy.exp(),
        maximum_row_sum_absolute_error,
        configuration_digest: configuration_digest.to_string(),
        estimated_storage_bytes,
        limits: *limits,
    })
}

fn entropy(values: &[f64]) -> f64 {
    values
        .iter()
        .filter(|value| **value > 0.0)
        .map(|value| -*value * value.ln())
        .sum()
}

pub(crate) fn configuration_digest(
    input: &DeclaredScalarPatternInput<'_>,
    mark_id: &ScalarMarkId,
    limits: &SoftClassCompositionLimits,
) -> Result<ContentDigest, SoftClassCompositionError> {
    let declared = input.declared_artifact_ref().map_err(|error| {
        SoftClassCompositionError::InvalidDeclaredInput {
            reason: error.to_string(),
        }
    })?;
    Ok(ContentDigest::from_framed([
        b"marklab-soft-class-composition-v1".as_slice(),
        declared.digest().as_bytes(),
        mark_id.as_str().as_bytes(),
        &(limits.maximum_points as u128).to_be_bytes(),
        &(limits.maximum_classes as u128).to_be_bytes(),
        &(limits.maximum_values as u128).to_be_bytes(),
        &(limits.maximum_retained_bytes as u128).to_be_bytes(),
    ]))
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum SoftClassCompositionError {
    #[error("soft class-composition resource limits are invalid")]
    InvalidResourceLimit,
    #[error("typed probability-simplex column is missing")]
    MissingProbabilitySimplex,
    #[error("soft class-composition requires at least one row")]
    EmptyInput,
    #[error("soft composition has {observed} rows; maximum is {maximum}")]
    PointLimitExceeded { observed: usize, maximum: usize },
    #[error("soft composition has {observed} classes; maximum is {maximum}")]
    ClassLimitExceeded { observed: usize, maximum: usize },
    #[error("soft composition has {observed} values; maximum is {maximum}")]
    ValueLimitExceeded { observed: usize, maximum: usize },
    #[error("soft composition requires {required} bytes; maximum is {maximum}")]
    RetainedByteLimitExceeded { required: usize, maximum: usize },
    #[error("soft class-composition input is invalid: {reason}")]
    InvalidDeclaredInput { reason: String },
    #[error("soft class-composition size arithmetic overflow")]
    SizeOverflow,
}
