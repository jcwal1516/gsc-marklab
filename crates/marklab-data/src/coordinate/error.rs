use marklab_core::{CoordinateFrameId, SectionId, TransformId, UncertaintyId};
use thiserror::Error;

use super::{
    frame::{CoordinateSpace, CoordinateUnit, SpatialDimension},
    serial::SerialSectionStatus,
};

/// Coordinate frame, transform, serial-section, and operation failures.
#[derive(Clone, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum CoordinateError {
    /// A frame does not declare exactly the X/Y or X/Y/Z spatial axes.
    #[error("coordinate frame {frame} has an invalid spatial-axis set")]
    InvalidAxisSet {
        /// Invalid frame identity.
        frame: CoordinateFrameId,
    },
    /// Image/physical space and the declared coordinate unit disagree.
    #[error("coordinate frame {frame} cannot use {unit:?} in {space:?} space")]
    UnitSpaceMismatch {
        /// Invalid frame identity.
        frame: CoordinateFrameId,
        /// Declared unit.
        unit: CoordinateUnit,
        /// Declared coordinate space.
        space: CoordinateSpace,
    },
    /// A transform matrix contains a non-finite coefficient.
    #[error("transform coefficient at index {index} must be finite")]
    NonFiniteTransformCoefficient {
        /// Zero-based row-major coefficient index.
        index: usize,
    },
    /// An uncertainty reference has a negative or non-finite scalar radius.
    #[error("uncertainty {uncertainty} must have a finite non-negative radius when present")]
    InvalidUncertaintyRadius {
        /// Invalid uncertainty identity.
        uncertainty: UncertaintyId,
    },
    /// More than one frame has the same identity.
    #[error("coordinate frame {frame} is declared more than once")]
    DuplicateFrame {
        /// Repeated frame identity.
        frame: CoordinateFrameId,
    },
    /// More than one uncertainty reference has the same identity.
    #[error("uncertainty {uncertainty} is declared more than once")]
    DuplicateUncertainty {
        /// Repeated uncertainty identity.
        uncertainty: UncertaintyId,
    },
    /// An uncertainty reference names a frame absent from the registry.
    #[error("uncertainty {uncertainty} refers to missing frame {frame}")]
    MissingUncertaintyFrame {
        /// Uncertainty identity.
        uncertainty: UncertaintyId,
        /// Missing frame identity.
        frame: CoordinateFrameId,
    },
    /// More than one transform has the same identity.
    #[error("transform {transform} is declared more than once")]
    DuplicateTransform {
        /// Repeated transform identity.
        transform: TransformId,
    },
    /// A transform names a source frame absent from the registry.
    #[error("transform {transform} refers to missing source frame {frame}")]
    MissingTransformSourceFrame {
        /// Transform identity.
        transform: TransformId,
        /// Missing source-frame identity.
        frame: CoordinateFrameId,
    },
    /// A transform names a target frame absent from the registry.
    #[error("transform {transform} refers to missing target frame {frame}")]
    MissingTransformTargetFrame {
        /// Transform identity.
        transform: TransformId,
        /// Missing target-frame identity.
        frame: CoordinateFrameId,
    },
    /// A transform maps a frame to itself.
    #[error("transform {transform} maps frame {frame} to itself")]
    SelfTransform {
        /// Transform identity.
        transform: TransformId,
        /// Repeated source and target frame.
        frame: CoordinateFrameId,
    },
    /// A transform matrix does not match the source and target dimensions.
    #[error("transform {transform} has incompatible frame or matrix dimensions")]
    TransformDimensionMismatch {
        /// Transform identity.
        transform: TransformId,
    },
    /// A transform names an uncertainty reference absent from the registry.
    #[error("transform {transform} refers to missing uncertainty {uncertainty}")]
    MissingTransformUncertainty {
        /// Transform identity.
        transform: TransformId,
        /// Missing uncertainty identity.
        uncertainty: UncertaintyId,
    },
    /// A transform uncertainty is expressed in a frame other than its target.
    #[error(
        "transform {transform} uncertainty {uncertainty} uses frame {observed}, expected target frame {expected}"
    )]
    TransformUncertaintyFrameMismatch {
        /// Transform identity.
        transform: TransformId,
        /// Uncertainty identity.
        uncertainty: UncertaintyId,
        /// Required target-frame identity.
        expected: CoordinateFrameId,
        /// Declared uncertainty-frame identity.
        observed: CoordinateFrameId,
    },
    /// A directed transform closes a cycle in the registry graph.
    #[error("transform {transform} closes a cycle from {source_frame} to {target_frame}")]
    TransformCycle {
        /// First back-edge transform in declaration-order traversal.
        transform: TransformId,
        /// Back-edge source frame.
        source_frame: CoordinateFrameId,
        /// Back-edge target frame.
        target_frame: CoordinateFrameId,
    },
    /// An operation names a frame absent from the registry.
    #[error("coordinate frame {frame} is not registered")]
    MissingFrame {
        /// Missing frame identity.
        frame: CoordinateFrameId,
    },
    /// An operation requires physical coordinates but received another space.
    #[error("coordinate frame {frame} is not a physical coordinate space")]
    SpaceMismatch {
        /// Incompatible frame identity.
        frame: CoordinateFrameId,
    },
    /// An operation requires a different spatial dimension.
    #[error("coordinate frame {frame} is {observed:?}, expected {expected:?}")]
    DimensionMismatch {
        /// Incompatible frame identity.
        frame: CoordinateFrameId,
        /// Required dimension.
        expected: super::frame::SpatialDimension,
        /// Observed dimension.
        observed: super::frame::SpatialDimension,
    },
    /// A coordinate contains a non-finite value.
    #[error("coordinate in frame {frame} has a non-finite value at index {index}")]
    NonFiniteCoordinate {
        /// Coordinate-frame identity.
        frame: CoordinateFrameId,
        /// Zero-based coordinate index.
        index: usize,
    },
    /// An explicit transform chain names an absent transform.
    #[error("transform {transform} is not registered")]
    MissingTransform {
        /// Missing transform identity.
        transform: TransformId,
    },
    /// A transform chain does not continue from the current coordinate frame.
    #[error(
        "transform {transform} starts at frame {observed}, but the chain is at frame {expected}"
    )]
    ChainSourceMismatch {
        /// Incompatible transform identity.
        transform: TransformId,
        /// Current coordinate-frame identity.
        expected: CoordinateFrameId,
        /// Transform source-frame identity.
        observed: CoordinateFrameId,
    },
    /// A serial-section placement matrix contains a non-finite coefficient.
    #[error("serial-section placement coefficient at index {index} must be finite")]
    NonFiniteSerialSectionPlacementCoefficient {
        /// Zero-based row-major coefficient index.
        index: usize,
    },
    /// A serial section has a non-finite Z center.
    #[error("serial section {section} must have a finite Z center")]
    InvalidSerialSectionZ {
        /// Invalid section identity.
        section: SectionId,
    },
    /// A serial section has a non-positive or non-finite thickness.
    #[error("serial section {section} must have a finite positive thickness")]
    InvalidSerialSectionThickness {
        /// Invalid section identity.
        section: SectionId,
    },
    /// Serial-section status and placement declarations disagree.
    #[error("serial section {section} has an invalid {status:?} placement state")]
    SerialSectionStateMismatch {
        /// Invalid section identity.
        section: SectionId,
        /// Declared observation state.
        status: SerialSectionStatus,
    },
    /// A serial-section series contains no retained entries.
    #[error("serial-section series for volume {volume} must not be empty")]
    EmptySerialSectionSeries {
        /// Series volume-frame identity.
        volume: CoordinateFrameId,
    },
    /// A section identity occurs more than once in one registry.
    #[error("serial section {section} is declared more than once")]
    DuplicateSerialSection {
        /// Repeated section identity.
        section: SectionId,
    },
    /// A retained serial-section ordinal does not follow its predecessor.
    #[error(
        "serial section {section} has ordinal {observed}, expected consecutive ordinal {expected}"
    )]
    NonConsecutiveSerialSectionOrdinal {
        /// Invalid section identity.
        section: SectionId,
        /// Required next ordinal.
        expected: u32,
        /// Declared ordinal.
        observed: u32,
    },
    /// A serial-section ordinal cannot be incremented for the next entry.
    #[error("serial section {section} has the maximum ordinal and cannot have a successor")]
    SerialSectionOrdinalOverflow {
        /// Section carrying the maximum ordinal.
        section: SectionId,
    },
    /// Serial-section Z centers are not strictly increasing.
    #[error(
        "serial section {section} has Z center {observed_z}, which does not follow {previous_z}"
    )]
    NonIncreasingSerialSectionZ {
        /// Invalid section identity.
        section: SectionId,
        /// Prior section-center Z.
        previous_z: f64,
        /// Invalid section-center Z.
        observed_z: f64,
    },
    /// A serial-section series names a volume frame absent from the registry.
    #[error("serial-section series refers to missing volume frame {volume}")]
    MissingSerialSectionVolumeFrame {
        /// Missing volume-frame identity.
        volume: CoordinateFrameId,
    },
    /// A serial-section volume frame is not three-dimensional.
    #[error("serial-section volume frame {volume} is {observed:?}, expected Three")]
    SerialSectionVolumeDimensionMismatch {
        /// Invalid volume-frame identity.
        volume: CoordinateFrameId,
        /// Declared spatial dimension.
        observed: SpatialDimension,
    },
    /// A serial-section volume frame is not physical space.
    #[error("serial-section volume frame {volume} must be physical")]
    SerialSectionVolumeSpaceMismatch {
        /// Invalid volume-frame identity.
        volume: CoordinateFrameId,
    },
    /// More than one serial-section series owns a volume frame.
    #[error("volume frame {volume} has more than one serial-section series")]
    DuplicateSerialSectionSeries {
        /// Repeated volume-frame identity.
        volume: CoordinateFrameId,
    },
    /// A serial-section placement names a source frame absent from the registry.
    #[error("serial section {section} refers to missing source frame {frame}")]
    MissingSerialSectionSourceFrame {
        /// Invalid section identity.
        section: SectionId,
        /// Missing source-frame identity.
        frame: CoordinateFrameId,
    },
    /// A serial-section source frame is not two-dimensional.
    #[error("serial section {section} source frame {frame} is {observed:?}, expected Two")]
    SerialSectionSourceDimensionMismatch {
        /// Invalid section identity.
        section: SectionId,
        /// Invalid source-frame identity.
        frame: CoordinateFrameId,
        /// Declared spatial dimension.
        observed: SpatialDimension,
    },
    /// A serial-section source frame is not physical space.
    #[error("serial section {section} source frame {frame} must be physical")]
    SerialSectionSourceSpaceMismatch {
        /// Invalid section identity.
        section: SectionId,
        /// Invalid source-frame identity.
        frame: CoordinateFrameId,
    },
    /// A serial-section source and target volume use different physical units.
    #[error(
        "serial section {section} source frame {source_frame} uses {source_unit:?}, but volume {volume} uses {volume_unit:?}"
    )]
    SerialSectionUnitMismatch {
        /// Invalid section identity.
        section: SectionId,
        /// Source-frame identity.
        source_frame: CoordinateFrameId,
        /// Target-volume-frame identity.
        volume: CoordinateFrameId,
        /// Source-frame unit.
        source_unit: CoordinateUnit,
        /// Target-volume unit.
        volume_unit: CoordinateUnit,
    },
    /// A placement names an uncertainty reference absent from the registry.
    #[error("serial section {section} refers to missing uncertainty {uncertainty}")]
    MissingSerialSectionUncertainty {
        /// Invalid section identity.
        section: SectionId,
        /// Missing uncertainty identity.
        uncertainty: UncertaintyId,
    },
    /// Placement uncertainty is not expressed in the target volume frame.
    #[error(
        "serial section {section} uncertainty {uncertainty} uses frame {observed}, expected volume {expected}"
    )]
    SerialSectionUncertaintyFrameMismatch {
        /// Invalid section identity.
        section: SectionId,
        /// Uncertainty identity.
        uncertainty: UncertaintyId,
        /// Target-volume-frame identity.
        expected: CoordinateFrameId,
        /// Declared uncertainty-frame identity.
        observed: CoordinateFrameId,
    },
    /// A section identity is absent from the registry.
    #[error("serial section {section} is not registered")]
    MissingSerialSection {
        /// Missing section identity.
        section: SectionId,
    },
    /// A missing section has no coordinate placement to embed.
    #[error("serial section {section} has no placement")]
    SerialSectionNotPlaced {
        /// Unplaced section identity.
        section: SectionId,
    },
    /// A coordinate frame differs from a section placement's source frame.
    #[error("serial section {section} requires coordinate frame {expected}, observed {observed}")]
    SerialSectionCoordinateFrameMismatch {
        /// Section identity.
        section: SectionId,
        /// Placement source-frame identity.
        expected: CoordinateFrameId,
        /// Coordinate-frame identity.
        observed: CoordinateFrameId,
    },
}
