use marklab_data::MeasurementStatus;
use marklab_workflow::ContentDigest;

use crate::common::{finite::canonical_zero, summation::kahan_add};

use crate::{BinaryCompartmentPartition2D, DeclaredScalarPatternInput, ScalarMarkId};

use super::types::*;

struct SummaryAccumulator {
    compartment_id: String,
    count: usize,
    minimum: f64,
    maximum: f64,
    absolute_sum: f64,
    correction: f64,
}

impl SummaryAccumulator {
    fn new(compartment_id: &str) -> Self {
        Self {
            compartment_id: compartment_id.into(),
            count: 0,
            minimum: f64::INFINITY,
            maximum: f64::NEG_INFINITY,
            absolute_sum: 0.0,
            correction: 0.0,
        }
    }

    fn push(&mut self, distance: f64) -> Result<(), CompartmentInterfaceError> {
        self.count = self
            .count
            .checked_add(1)
            .ok_or(CompartmentInterfaceError::SizeOverflow)?;
        self.minimum = self.minimum.min(distance);
        self.maximum = self.maximum.max(distance);
        kahan_add(&mut self.absolute_sum, &mut self.correction, distance.abs());
        Ok(())
    }

    fn finish(self) -> Result<CompartmentInterfaceSummary, CompartmentInterfaceError> {
        if self.count == 0 {
            return Err(CompartmentInterfaceError::EmptyCompartment {
                compartment_id: self.compartment_id,
            });
        }
        let mean = (self.absolute_sum + self.correction) / self.count as f64;
        if !mean.is_finite() {
            return Err(CompartmentInterfaceError::SizeOverflow);
        }
        Ok(CompartmentInterfaceSummary {
            compartment_id: self.compartment_id,
            cell_count: self.count,
            minimum_signed_distance_um: canonical_zero(self.minimum),
            maximum_signed_distance_um: canonical_zero(self.maximum),
            mean_absolute_distance_um: canonical_zero(mean),
        })
    }
}

/// Compute one bounded descriptive cell profile against an exact oriented compartment interface.
pub fn analyze_compartment_interface_profile(
    input: &DeclaredScalarPatternInput<'_>,
    partition: &BinaryCompartmentPartition2D,
    limits: &CompartmentInterfaceLimits,
) -> Result<CompartmentInterfaceProfile, CompartmentInterfaceError> {
    let descriptor = partition.descriptor();
    if input.coordinate_frame_id() != &descriptor.coordinate_frame_id {
        return Err(CompartmentInterfaceError::CoordinateFrameMismatch);
    }
    let configuration_digest = configuration_digest(input, partition, limits)?;
    let table = input
        .mark_table()
        .ok_or(CompartmentInterfaceError::MissingCompartmentMark)?;
    let mark_id = ScalarMarkId::new("histologic_compartment").map_err(|error| {
        CompartmentInterfaceError::InvalidDeclaredInput {
            reason: error.to_string(),
        }
    })?;
    let codes = table
        .categorical_values(&mark_id)
        .ok_or(CompartmentInterfaceError::MissingCompartmentMark)?;
    let levels = table
        .categorical_levels(&mark_id)
        .ok_or(CompartmentInterfaceError::MissingCompartmentMark)?;
    let measurement_status = table
        .measurement_status(&mark_id)
        .ok_or(CompartmentInterfaceError::MissingCompartmentMark)?;
    let negative_code = resolve_level(levels, &descriptor.negative_compartment_id)?;
    let positive_code = resolve_level(levels, &descriptor.positive_compartment_id)?;
    let pattern = input.pattern();
    if pattern.len() > limits.maximum_points {
        return Err(CompartmentInterfaceError::PointLimitExceeded {
            observed: pattern.len(),
            maximum: limits.maximum_points,
        });
    }
    if pattern.len() > limits.maximum_distance_queries {
        return Err(CompartmentInterfaceError::DistanceQueryLimitExceeded {
            required: pattern.len(),
            maximum: limits.maximum_distance_queries,
        });
    }
    if pattern.x_um.len() != pattern.len()
        || pattern.y_um.len() != pattern.len()
        || pattern.valid.len() != pattern.len()
        || pattern.valid.iter().any(|valid| *valid != 1)
        || codes.len() != pattern.len()
        || input.cell_ids().len() != pattern.len()
    {
        return Err(CompartmentInterfaceError::InvalidDeclaredInput {
            reason: "pattern, cell, validity, and categorical rows are not aligned".into(),
        });
    }
    let estimated_storage_bytes = retained_bytes(
        input,
        &descriptor.negative_compartment_id,
        &descriptor.positive_compartment_id,
    )?;
    if estimated_storage_bytes > limits.maximum_retained_bytes {
        return Err(CompartmentInterfaceError::RetainedByteLimitExceeded {
            required: estimated_storage_bytes,
            maximum: limits.maximum_retained_bytes,
        });
    }

    let mut rows = Vec::new();
    rows.try_reserve_exact(pattern.len())
        .map_err(|_| CompartmentInterfaceError::AllocationFailed)?;
    let mut negative = SummaryAccumulator::new(&descriptor.negative_compartment_id);
    let mut positive = SummaryAccumulator::new(&descriptor.positive_compartment_id);
    for (row, code) in codes.iter().copied().enumerate() {
        let (compartment_id, accumulator, expects_positive) = if code == negative_code {
            (
                descriptor.negative_compartment_id.as_str(),
                &mut negative,
                false,
            )
        } else if code == positive_code {
            (
                descriptor.positive_compartment_id.as_str(),
                &mut positive,
                true,
            )
        } else {
            return Err(CompartmentInterfaceError::UnsupportedCompartmentCode { row, code });
        };
        let distance = partition
            .signed_interface_distance_um(pattern.x_um[row], pattern.y_um[row])
            .map_err(|error| CompartmentInterfaceError::GeometryQuery {
                row,
                reason: error.to_string(),
            })?;
        if (expects_positive && distance < 0.0) || (!expects_positive && distance > 0.0) {
            return Err(CompartmentInterfaceError::SpatialLabelMismatch {
                row,
                label: compartment_id.into(),
                distance_um: distance,
            });
        }
        accumulator.push(distance)?;
        rows.push(CompartmentInterfaceCell {
            row,
            cell_id: input.cell_ids()[row].as_str().into(),
            compartment_id: compartment_id.into(),
            signed_interface_distance_um: canonical_zero(distance),
        });
    }
    Ok(CompartmentInterfaceProfile {
        case_id: pattern.meta.case_id.clone(),
        timepoint: pattern.meta.timepoint.clone(),
        mark_id: mark_id.as_str().into(),
        measurement_status: measurement_status_name(measurement_status).into(),
        negative_compartment_id: descriptor.negative_compartment_id.clone(),
        positive_compartment_id: descriptor.positive_compartment_id.clone(),
        coordinate_frame_id: descriptor.coordinate_frame_id.as_str().into(),
        interface_length_um: descriptor.interface_length_um,
        partition_digest: descriptor.logical_digest.to_string(),
        configuration_digest: configuration_digest.to_string(),
        query_count: pattern.len(),
        estimated_storage_bytes,
        limits: *limits,
        rows,
        summaries: [negative.finish()?, positive.finish()?],
    })
}

fn resolve_level(levels: &[String], expected: &str) -> Result<u32, CompartmentInterfaceError> {
    let mut matches = levels
        .iter()
        .enumerate()
        .filter(|(_, level)| level.as_str() == expected);
    let Some((index, _)) = matches.next() else {
        return Err(CompartmentInterfaceError::UnresolvedCompartmentLevel {
            compartment_id: expected.into(),
        });
    };
    if matches.next().is_some() {
        return Err(CompartmentInterfaceError::UnresolvedCompartmentLevel {
            compartment_id: expected.into(),
        });
    }
    u32::try_from(index).map_err(|_| CompartmentInterfaceError::SizeOverflow)
}

pub(crate) fn retained_bytes(
    input: &DeclaredScalarPatternInput<'_>,
    negative_id: &str,
    positive_id: &str,
) -> Result<usize, CompartmentInterfaceError> {
    let rows = input
        .pattern()
        .len()
        .checked_mul(std::mem::size_of::<CompartmentInterfaceCell>())
        .ok_or(CompartmentInterfaceError::SizeOverflow)?;
    let cell_text = input
        .cell_ids()
        .iter()
        .try_fold(0_usize, |total, cell| {
            total.checked_add(cell.as_str().len())
        })
        .ok_or(CompartmentInterfaceError::SizeOverflow)?;
    let compartment_text = input
        .pattern()
        .len()
        .checked_mul(negative_id.len().max(positive_id.len()))
        .and_then(|value| value.checked_add(2 * (negative_id.len() + positive_id.len())))
        .ok_or(CompartmentInterfaceError::SizeOverflow)?;
    rows.checked_add(cell_text)
        .and_then(|value| value.checked_add(compartment_text))
        .and_then(|value| value.checked_add(2 * std::mem::size_of::<CompartmentInterfaceSummary>()))
        .ok_or(CompartmentInterfaceError::SizeOverflow)
}

pub(crate) fn measurement_status_name(status: MeasurementStatus) -> &'static str {
    match status {
        MeasurementStatus::Measured => "measured",
        MeasurementStatus::ImportedPrediction => "imported_prediction",
        MeasurementStatus::MorphologyPrediction => "morphology_prediction",
        MeasurementStatus::DerivedSummary => "derived_summary",
    }
}

pub(crate) fn configuration_digest(
    input: &DeclaredScalarPatternInput<'_>,
    partition: &BinaryCompartmentPartition2D,
    limits: &CompartmentInterfaceLimits,
) -> Result<ContentDigest, CompartmentInterfaceError> {
    let declared_ref = input.declared_artifact_ref().map_err(|error| {
        CompartmentInterfaceError::InvalidDeclaredInput {
            reason: error.to_string(),
        }
    })?;
    Ok(ContentDigest::from_framed([
        b"marklab-compartment-interface-profile-v1".as_slice(),
        declared_ref.digest().as_bytes(),
        partition.descriptor().logical_digest.as_bytes(),
        &(limits.maximum_points as u128).to_be_bytes(),
        &(limits.maximum_distance_queries as u128).to_be_bytes(),
        &(limits.maximum_retained_bytes as u128).to_be_bytes(),
    ]))
}
