use marklab_workflow::ContentDigest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    compartment_interface::analysis::measurement_status_name, DeclaredScalarPatternInput,
    ScalarMarkId,
};

/// Hard resource ceilings for one ordinal composition.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrdinalClassCompositionLimits {
    /// Maximum admitted rows.
    pub maximum_points: usize,
    /// Maximum ordered levels.
    pub maximum_levels: usize,
    /// Maximum conservatively estimated retained bytes.
    pub maximum_retained_bytes: usize,
}

impl OrdinalClassCompositionLimits {
    /// Validate strictly positive resource ceilings.
    pub fn new(
        maximum_points: usize,
        maximum_levels: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, OrdinalClassCompositionError> {
        if [maximum_points, maximum_levels, maximum_retained_bytes].contains(&0) {
            return Err(OrdinalClassCompositionError::InvalidResourceLimit);
        }
        Ok(Self {
            maximum_points,
            maximum_levels,
            maximum_retained_bytes,
        })
    }
}

/// Exact ordinal composition request and its resource policy.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrdinalClassCompositionConfig {
    limits: OrdinalClassCompositionLimits,
}

impl OrdinalClassCompositionConfig {
    /// Construct a request from already validated limits.
    pub fn new(limits: OrdinalClassCompositionLimits) -> Self {
        Self { limits }
    }

    /// Return the exact resource policy bound into result identity.
    pub fn limits(self) -> OrdinalClassCompositionLimits {
        self.limits
    }
}

/// Ordered descriptive composition for one complete ordinal per-cell mark.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrdinalClassCompositionResult {
    /// Case identity inherited from the typed pattern.
    pub case_id: String,
    /// Timepoint identity inherited from the typed pattern.
    pub timepoint: String,
    /// Stable ordinal mark identity.
    pub mark_id: String,
    /// Declared measurement status.
    pub measurement_status: String,
    /// Number of complete ordinal rows.
    pub row_count: usize,
    /// Ordered level codebook.
    pub level_ids: Vec<String>,
    /// Exact row count per ordered level.
    pub counts: Vec<usize>,
    /// Exact count divided by total rows per ordered level.
    pub proportions: Vec<f64>,
    /// Inclusive cumulative proportion in declared level order.
    pub cumulative_proportions: Vec<f64>,
    /// Level containing the lower empirical median order statistic.
    pub lower_median_level: String,
    /// Level containing the upper empirical median order statistic.
    pub upper_median_level: String,
    /// Shannon entropy of level proportions in natural-log units.
    pub entropy_nats: f64,
    /// Entropy divided by `ln(level_count)`.
    pub normalized_entropy: f64,
    /// Exponential of Shannon entropy.
    pub effective_level_count: f64,
    /// Exact input, mark, and resource-policy identity.
    pub configuration_digest: String,
    /// Conservative retained-byte estimate.
    pub estimated_storage_bytes: usize,
    /// Exact resource ceilings applied to this result.
    pub limits: OrdinalClassCompositionLimits,
}

/// Compute composition, CDF, median interval, and entropy without interval-code arithmetic.
pub fn ordinal_class_composition(
    input: &DeclaredScalarPatternInput<'_>,
    mark_id: &ScalarMarkId,
    config: &OrdinalClassCompositionConfig,
) -> Result<OrdinalClassCompositionResult, OrdinalClassCompositionError> {
    let table = input
        .mark_table()
        .ok_or(OrdinalClassCompositionError::MissingOrdinalMark)?;
    let values = table
        .ordinal_values(mark_id)
        .ok_or(OrdinalClassCompositionError::MissingOrdinalMark)?;
    let levels = table
        .ordinal_levels(mark_id)
        .ok_or(OrdinalClassCompositionError::MissingOrdinalMark)?;
    let status = table
        .measurement_status(mark_id)
        .ok_or(OrdinalClassCompositionError::MissingOrdinalMark)?;
    let rows = values.len();
    if rows == 0 {
        return Err(OrdinalClassCompositionError::EmptyInput);
    }
    if rows > config.limits.maximum_points {
        return Err(OrdinalClassCompositionError::PointLimitExceeded {
            observed: rows,
            maximum: config.limits.maximum_points,
        });
    }
    if levels.len() > config.limits.maximum_levels {
        return Err(OrdinalClassCompositionError::LevelLimitExceeded {
            observed: levels.len(),
            maximum: config.limits.maximum_levels,
        });
    }
    let vector_storage = levels
        .len()
        .checked_mul(
            std::mem::size_of::<String>()
                + std::mem::size_of::<usize>()
                + 2 * std::mem::size_of::<f64>(),
        )
        .ok_or(OrdinalClassCompositionError::SizeOverflow)?;
    let text_storage = levels
        .iter()
        .try_fold(0_usize, |total, level| total.checked_add(level.len()))
        .ok_or(OrdinalClassCompositionError::SizeOverflow)?;
    let longest_level = levels.iter().map(String::len).max().unwrap_or(0);
    let fixed_text_storage = input
        .pattern()
        .meta
        .case_id
        .len()
        .checked_add(input.pattern().meta.timepoint.len())
        .and_then(|value| value.checked_add(mark_id.as_str().len()))
        .and_then(|value| value.checked_add(measurement_status_name(status).len()))
        .and_then(|value| value.checked_add(64))
        .and_then(|value| value.checked_add(longest_level.saturating_mul(2)))
        .ok_or(OrdinalClassCompositionError::SizeOverflow)?;
    let estimated_storage_bytes = vector_storage
        .checked_add(text_storage)
        .and_then(|value| value.checked_add(fixed_text_storage))
        .ok_or(OrdinalClassCompositionError::SizeOverflow)?;
    if estimated_storage_bytes > config.limits.maximum_retained_bytes {
        return Err(OrdinalClassCompositionError::RetainedByteLimitExceeded {
            required: estimated_storage_bytes,
            maximum: config.limits.maximum_retained_bytes,
        });
    }
    let mut counts = vec![0_usize; levels.len()];
    for value in values {
        let level =
            usize::try_from(*value).map_err(|_| OrdinalClassCompositionError::SizeOverflow)?;
        let count = counts
            .get_mut(level)
            .ok_or(OrdinalClassCompositionError::InvalidRetainedCode)?;
        *count = count
            .checked_add(1)
            .ok_or(OrdinalClassCompositionError::SizeOverflow)?;
    }
    let proportions = counts
        .iter()
        .map(|count| *count as f64 / rows as f64)
        .collect::<Vec<_>>();
    let mut cumulative_count = 0_usize;
    let cumulative_proportions = counts
        .iter()
        .map(|count| {
            cumulative_count = cumulative_count
                .checked_add(*count)
                .ok_or(OrdinalClassCompositionError::SizeOverflow)?;
            Ok(cumulative_count as f64 / rows as f64)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let lower = median_level(&counts, (rows - 1) / 2)?;
    let upper = median_level(&counts, rows / 2)?;
    let entropy_nats = proportions
        .iter()
        .filter(|proportion| **proportion > 0.0)
        .map(|proportion| -*proportion * proportion.ln())
        .sum::<f64>();
    let normalized_entropy = entropy_nats / (levels.len() as f64).ln();
    let configuration_digest = configuration_digest(input, mark_id, config)?;
    Ok(OrdinalClassCompositionResult {
        case_id: input.pattern().meta.case_id.clone(),
        timepoint: input.pattern().meta.timepoint.clone(),
        mark_id: mark_id.as_str().into(),
        measurement_status: measurement_status_name(status).into(),
        row_count: rows,
        level_ids: levels.to_vec(),
        counts,
        proportions,
        cumulative_proportions,
        lower_median_level: levels[lower].clone(),
        upper_median_level: levels[upper].clone(),
        entropy_nats,
        normalized_entropy,
        effective_level_count: entropy_nats.exp(),
        configuration_digest: configuration_digest.to_string(),
        estimated_storage_bytes,
        limits: config.limits,
    })
}

fn median_level(counts: &[usize], target: usize) -> Result<usize, OrdinalClassCompositionError> {
    let mut cumulative = 0_usize;
    for (level, count) in counts.iter().enumerate() {
        cumulative = cumulative
            .checked_add(*count)
            .ok_or(OrdinalClassCompositionError::SizeOverflow)?;
        if cumulative > target {
            return Ok(level);
        }
    }
    Err(OrdinalClassCompositionError::InvalidRetainedCode)
}

pub(crate) fn configuration_digest(
    input: &DeclaredScalarPatternInput<'_>,
    mark_id: &ScalarMarkId,
    config: &OrdinalClassCompositionConfig,
) -> Result<ContentDigest, OrdinalClassCompositionError> {
    let declared = input.declared_artifact_ref().map_err(|error| {
        OrdinalClassCompositionError::InvalidDeclaredInput {
            reason: error.to_string(),
        }
    })?;
    Ok(ContentDigest::from_framed([
        b"marklab-ordinal-class-composition-v1".as_slice(),
        declared.digest().as_bytes(),
        mark_id.as_str().as_bytes(),
        &(config.limits.maximum_points as u128).to_be_bytes(),
        &(config.limits.maximum_levels as u128).to_be_bytes(),
        &(config.limits.maximum_retained_bytes as u128).to_be_bytes(),
    ]))
}

/// Explicit input, resource, arithmetic, and retained-code failures.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum OrdinalClassCompositionError {
    /// At least one resource ceiling was zero.
    #[error("ordinal composition resource limits must be positive")]
    InvalidResourceLimit,
    /// The requested typed ordinal mark was absent.
    #[error("typed ordinal mark is missing")]
    MissingOrdinalMark,
    /// The admitted ordinal table had no rows.
    #[error("ordinal composition requires at least one row")]
    EmptyInput,
    /// The point ceiling was exceeded.
    #[error("ordinal composition has {observed} rows; maximum is {maximum}")]
    PointLimitExceeded { observed: usize, maximum: usize },
    /// The ordered-level ceiling was exceeded.
    #[error("ordinal composition has {observed} levels; maximum is {maximum}")]
    LevelLimitExceeded { observed: usize, maximum: usize },
    /// The retained-byte estimate exceeded its ceiling.
    #[error("ordinal composition requires {required} bytes; maximum is {maximum}")]
    RetainedByteLimitExceeded { required: usize, maximum: usize },
    /// The declared typed input failed revalidation.
    #[error("ordinal composition input is invalid: {reason}")]
    InvalidDeclaredInput { reason: String },
    /// A retained code no longer indexes its exact ordered codebook.
    #[error("ordinal composition retained an invalid level code")]
    InvalidRetainedCode,
    /// Checked size or count arithmetic overflowed.
    #[error("ordinal composition size arithmetic overflow")]
    SizeOverflow,
}
