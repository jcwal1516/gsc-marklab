use std::collections::BTreeSet;

use marklab_data::CoordinateFrameId;
use marklab_workflow::ContentDigest;
use rstar::RTree;
use thiserror::Error;

use crate::common::finite::canonical_zero;

use super::{topology::BoundarySegment, ObservationWindow2D};

/// Fixed work ceiling for validating one exact binary compartment partition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompartmentPartitionLimits {
    /// Maximum summed boundary-segment count across domain and both compartments.
    pub maximum_boundary_segments: usize,
}

impl CompartmentPartitionLimits {
    /// Require a positive explicit boundary-segment ceiling.
    pub fn new(maximum_boundary_segments: usize) -> Result<Self, CompartmentPartitionError> {
        if maximum_boundary_segments == 0 {
            return Err(CompartmentPartitionError::InvalidResourceLimit);
        }
        Ok(Self {
            maximum_boundary_segments,
        })
    }
}

/// Canonical identity and physical summary of one oriented binary compartment partition.
#[derive(Clone, Debug, PartialEq)]
pub struct CompartmentPartitionDescriptor {
    /// Compartment whose interface distances are negative.
    pub negative_compartment_id: String,
    /// Compartment whose interface distances are positive.
    pub positive_compartment_id: String,
    /// Exact shared physical coordinate frame.
    pub coordinate_frame_id: CoordinateFrameId,
    /// Analyzed-domain area in square micrometres.
    pub observation_area_um2: f64,
    /// Negative-compartment area in square micrometres.
    pub negative_area_um2: f64,
    /// Positive-compartment area in square micrometres.
    pub positive_area_um2: f64,
    /// Number of exactly matched internal interface segments.
    pub interface_segment_count: usize,
    /// Total shared-interface length in micrometres.
    pub interface_length_um: f64,
    /// Complete negative-compartment polygon boundary length.
    pub negative_boundary_length_um: f64,
    /// Negative-compartment boundary coincident with the analyzed tissue edge.
    pub negative_outer_boundary_length_um: f64,
    /// Complete positive-compartment polygon boundary length.
    pub positive_boundary_length_um: f64,
    /// Positive-compartment boundary coincident with the analyzed tissue edge.
    pub positive_outer_boundary_length_um: f64,
    /// Number of disconnected negative-compartment polygon components.
    pub negative_component_count: usize,
    /// Number of holes inside negative-compartment components.
    pub negative_hole_count: usize,
    /// Number of disconnected positive-compartment polygon components.
    pub positive_component_count: usize,
    /// Number of holes inside positive-compartment components.
    pub positive_hole_count: usize,
    /// Summed boundary-segment count validated against the caller ceiling.
    pub validated_boundary_segment_count: usize,
    /// Exact partition identity including role order and aligned interface geometry.
    pub logical_digest: ContentDigest,
}

/// Exact two-compartment tessellation with a declared negative-to-positive interface orientation.
#[derive(Clone, Debug)]
pub struct BinaryCompartmentPartition2D {
    observation: ObservationWindow2D,
    negative: ObservationWindow2D,
    positive: ObservationWindow2D,
    interface: RTree<BoundarySegment>,
    negative_component_areas_um2: Box<[f64]>,
    positive_component_areas_um2: Box<[f64]>,
    descriptor: CompartmentPartitionDescriptor,
}

impl BinaryCompartmentPartition2D {
    /// Validate exact segment-aligned coverage and construct the shared-interface index.
    pub fn new(
        observation: ObservationWindow2D,
        negative_compartment_id: impl Into<String>,
        negative: ObservationWindow2D,
        positive_compartment_id: impl Into<String>,
        positive: ObservationWindow2D,
        limits: CompartmentPartitionLimits,
    ) -> Result<Self, CompartmentPartitionError> {
        let negative_compartment_id = negative_compartment_id.into();
        let positive_compartment_id = positive_compartment_id.into();
        validate_compartment_id(&negative_compartment_id)?;
        validate_compartment_id(&positive_compartment_id)?;
        if negative_compartment_id == positive_compartment_id {
            return Err(CompartmentPartitionError::DuplicateCompartmentId);
        }
        let coordinate_frame_id = observation
            .coordinate_frame_id()
            .cloned()
            .ok_or(CompartmentPartitionError::MissingCoordinateFrame)?;
        if negative.coordinate_frame_id() != Some(&coordinate_frame_id)
            || positive.coordinate_frame_id() != Some(&coordinate_frame_id)
        {
            return Err(CompartmentPartitionError::CoordinateFrameMismatch);
        }

        let validated_boundary_segment_count = observation
            .boundary
            .size()
            .checked_add(negative.boundary.size())
            .and_then(|value| value.checked_add(positive.boundary.size()))
            .ok_or(CompartmentPartitionError::SizeOverflow)?;
        if validated_boundary_segment_count > limits.maximum_boundary_segments {
            return Err(CompartmentPartitionError::BoundarySegmentLimitExceeded {
                observed: validated_boundary_segment_count,
                maximum: limits.maximum_boundary_segments,
            });
        }

        let observation_segments = segment_keys(&observation);
        let negative_segments = segment_keys(&negative);
        let positive_segments = segment_keys(&positive);
        validate_exact_tessellation(
            &observation_segments,
            &negative_segments,
            &positive_segments,
        )?;
        let area_sum = negative.area_um2() + positive.area_um2();
        if !area_sum.is_finite()
            || area_sum
                .to_bits()
                .abs_diff(observation.area_um2().to_bits())
                > 16
        {
            return Err(not_exact(
                "compartment areas do not cover the analyzed domain",
            ));
        }

        let interface_keys = negative_segments
            .intersection(&positive_segments)
            .filter(|key| !observation_segments.contains(*key))
            .copied()
            .collect::<BTreeSet<_>>();
        if interface_keys.is_empty() {
            return Err(CompartmentPartitionError::NoSharedInterface);
        }
        let interface_segments = negative
            .boundary
            .iter()
            .filter(|segment| interface_keys.contains(&segment.canonical_key()))
            .cloned()
            .collect::<Vec<_>>();
        if interface_segments.len() != interface_keys.len() {
            return Err(not_exact("shared interface segment identity is ambiguous"));
        }
        let interface_length_um = segment_key_length_sum(interface_keys.iter());
        if !interface_length_um.is_finite() || interface_length_um <= 0.0 {
            return Err(not_exact(
                "shared interface length is not finite and positive",
            ));
        }
        let negative_outer_boundary_length_um =
            boundary_length_for_keys(&negative, &observation_segments);
        let positive_outer_boundary_length_um =
            boundary_length_for_keys(&positive, &observation_segments);
        validate_boundary_decomposition(
            negative.perimeter_um(),
            negative_outer_boundary_length_um,
            interface_length_um,
        )?;
        let negative_component_areas_um2 = negative.component_areas_um2();
        let positive_component_areas_um2 = positive.component_areas_um2();
        validate_boundary_decomposition(
            positive.perimeter_um(),
            positive_outer_boundary_length_um,
            interface_length_um,
        )?;
        let logical_digest = partition_digest(
            &observation,
            &negative_compartment_id,
            &negative,
            &positive_compartment_id,
            &positive,
            &interface_keys,
            limits,
        );
        let descriptor = CompartmentPartitionDescriptor {
            negative_compartment_id,
            positive_compartment_id,
            coordinate_frame_id,
            observation_area_um2: observation.area_um2(),
            negative_area_um2: negative.area_um2(),
            positive_area_um2: positive.area_um2(),
            interface_segment_count: interface_segments.len(),
            interface_length_um,
            negative_boundary_length_um: negative.perimeter_um(),
            negative_outer_boundary_length_um,
            positive_boundary_length_um: positive.perimeter_um(),
            positive_outer_boundary_length_um,
            negative_component_count: negative.descriptor().component_count,
            negative_hole_count: negative.descriptor().hole_count,
            positive_component_count: positive.descriptor().component_count,
            positive_hole_count: positive.descriptor().hole_count,
            validated_boundary_segment_count,
            logical_digest,
        };
        Ok(Self {
            observation,
            negative,
            positive,
            interface: RTree::bulk_load(interface_segments),
            negative_component_areas_um2: negative_component_areas_um2.into_boxed_slice(),
            positive_component_areas_um2: positive_component_areas_um2.into_boxed_slice(),
            descriptor,
        })
    }

    /// Signed Euclidean distance to the shared interface.
    ///
    /// Values are positive in the declared positive compartment, negative in
    /// the negative compartment, and exact zero on the interface. Points
    /// outside the analyzed domain fail rather than receiving a sign.
    pub fn signed_interface_distance_um(
        &self,
        x_um: f64,
        y_um: f64,
    ) -> Result<f64, CompartmentPartitionError> {
        if !x_um.is_finite() || !y_um.is_finite() {
            return Err(CompartmentPartitionError::NonFiniteQueryPoint);
        }
        if !self.observation.contains(x_um, y_um) {
            return Err(CompartmentPartitionError::OutsideObservationWindow);
        }
        let point = [canonical_zero(x_um), canonical_zero(y_um)];
        let distance = self
            .interface
            .nearest_neighbor(point)
            .map(|segment| segment.distance_2_to(point).sqrt())
            .ok_or(CompartmentPartitionError::NoSharedInterface)?;
        if distance == 0.0 {
            return Ok(0.0);
        }
        match (
            self.negative.contains(x_um, y_um),
            self.positive.contains(x_um, y_um),
        ) {
            (true, false) => Ok(-distance),
            (false, true) => Ok(distance),
            _ => Err(CompartmentPartitionError::AmbiguousCompartmentMembership),
        }
    }

    /// Canonical partition identity and physical summary.
    pub fn descriptor(&self) -> &CompartmentPartitionDescriptor {
        &self.descriptor
    }

    pub(crate) fn negative_component_areas_um2(&self) -> &[f64] {
        &self.negative_component_areas_um2
    }

    pub(crate) fn positive_component_areas_um2(&self) -> &[f64] {
        &self.positive_component_areas_um2
    }

    pub(crate) fn observation_window(&self) -> &ObservationWindow2D {
        &self.observation
    }

    pub(crate) fn negative_window(&self) -> &ObservationWindow2D {
        &self.negative
    }

    pub(crate) fn positive_window(&self) -> &ObservationWindow2D {
        &self.positive
    }
}

type SegmentKey = [u64; 4];

fn segment_keys(window: &ObservationWindow2D) -> BTreeSet<SegmentKey> {
    window
        .boundary
        .iter()
        .map(BoundarySegment::canonical_key)
        .collect()
}

fn boundary_length_for_keys(window: &ObservationWindow2D, keys: &BTreeSet<SegmentKey>) -> f64 {
    let window_keys = segment_keys(window);
    segment_key_length_sum(window_keys.intersection(keys))
}

fn segment_key_length_sum<'a>(keys: impl Iterator<Item = &'a SegmentKey>) -> f64 {
    let mut sum = 0.0;
    let mut correction = 0.0;
    for key in keys {
        let start = [f64::from_bits(key[0]), f64::from_bits(key[1])];
        let end = [f64::from_bits(key[2]), f64::from_bits(key[3])];
        let length = (end[0] - start[0]).hypot(end[1] - start[1]);
        let corrected = length - correction;
        let next = sum + corrected;
        correction = (next - sum) - corrected;
        sum = next;
    }
    sum + correction
}

fn validate_boundary_decomposition(
    perimeter: f64,
    outer: f64,
    interface: f64,
) -> Result<(), CompartmentPartitionError> {
    let decomposed = outer + interface;
    if !outer.is_finite()
        || outer < 0.0
        || !decomposed.is_finite()
        || decomposed.to_bits().abs_diff(perimeter.to_bits()) > 16
    {
        return Err(not_exact(
            "shared and outer segments do not reconstruct compartment perimeter",
        ));
    }
    Ok(())
}

fn validate_exact_tessellation(
    observation: &BTreeSet<SegmentKey>,
    negative: &BTreeSet<SegmentKey>,
    positive: &BTreeSet<SegmentKey>,
) -> Result<(), CompartmentPartitionError> {
    for segment in observation {
        if negative.contains(segment) == positive.contains(segment) {
            return Err(not_exact(
                "each analyzed outer segment must belong to exactly one compartment",
            ));
        }
    }
    for segment in negative {
        if !observation.contains(segment) && !positive.contains(segment) {
            return Err(not_exact(
                "each negative internal segment must have one positive mate",
            ));
        }
    }
    for segment in positive {
        if !observation.contains(segment) && !negative.contains(segment) {
            return Err(not_exact(
                "each positive internal segment must have one negative mate",
            ));
        }
    }
    Ok(())
}

fn partition_digest(
    observation: &ObservationWindow2D,
    negative_id: &str,
    negative: &ObservationWindow2D,
    positive_id: &str,
    positive: &ObservationWindow2D,
    interface: &BTreeSet<SegmentKey>,
    limits: CompartmentPartitionLimits,
) -> ContentDigest {
    let mut fields = vec![
        b"marklab-binary-compartment-partition-2d-v1".to_vec(),
        observation.descriptor().logical_digest.as_bytes().to_vec(),
        b"negative".to_vec(),
        negative_id.as_bytes().to_vec(),
        negative.descriptor().logical_digest.as_bytes().to_vec(),
        b"positive".to_vec(),
        positive_id.as_bytes().to_vec(),
        positive.descriptor().logical_digest.as_bytes().to_vec(),
        (limits.maximum_boundary_segments as u128)
            .to_be_bytes()
            .to_vec(),
        (interface.len() as u128).to_be_bytes().to_vec(),
    ];
    for segment in interface {
        for coordinate in segment {
            fields.push(coordinate.to_be_bytes().to_vec());
        }
    }
    ContentDigest::from_framed(fields.iter().map(Vec::as_slice))
}

fn validate_compartment_id(value: &str) -> Result<(), CompartmentPartitionError> {
    if value.is_empty()
        || value.len() > 255
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(CompartmentPartitionError::InvalidCompartmentId);
    }
    Ok(())
}

fn not_exact(reason: &'static str) -> CompartmentPartitionError {
    CompartmentPartitionError::NotExactPartition {
        reason: reason.into(),
    }
}

/// Failure to validate or query an exact binary compartment partition.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum CompartmentPartitionError {
    /// The boundary-segment ceiling is zero.
    #[error("compartment-partition resource limits must be positive")]
    InvalidResourceLimit,
    /// A compartment ID is blank, padded, over 255 bytes, or contains control characters.
    #[error("compartment ID is invalid")]
    InvalidCompartmentId,
    /// Positive and negative roles use the same compartment ID.
    #[error("positive and negative compartment IDs must differ")]
    DuplicateCompartmentId,
    /// The analyzed domain has no exact physical coordinate frame.
    #[error("compartment partition requires a bound physical coordinate frame")]
    MissingCoordinateFrame,
    /// Domain and compartment coordinate frames differ.
    #[error("compartment partition coordinate frames differ")]
    CoordinateFrameMismatch,
    /// Total boundary validation work exceeds the caller ceiling.
    #[error("compartment partition has {observed} boundary segments; maximum is {maximum}")]
    BoundarySegmentLimitExceeded {
        /// Summed domain and compartment segment count.
        observed: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Compartment boundaries do not form the required exact aligned tessellation.
    #[error("compartment windows are not an exact partition: {reason}")]
    NotExactPartition {
        /// Deterministic structural reason.
        reason: String,
    },
    /// The two compartments have no internal matched boundary.
    #[error("compartment partition has no shared internal interface")]
    NoSharedInterface,
    /// Query coordinates are non-finite.
    #[error("compartment-interface query point must be finite")]
    NonFiniteQueryPoint,
    /// A query is outside the analyzed tissue domain.
    #[error("compartment-interface query point is outside the observation window")]
    OutsideObservationWindow,
    /// A non-interface point belongs to both or neither compartment.
    #[error("compartment-interface query has ambiguous compartment membership")]
    AmbiguousCompartmentMembership,
    /// Checked size arithmetic overflowed.
    #[error("compartment-partition size arithmetic overflow")]
    SizeOverflow,
}
