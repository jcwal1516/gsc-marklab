#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! In-memory cohort hierarchy and data contracts for Marklab.

mod coordinate;
mod hierarchy;
mod node;

pub use coordinate::{
    Coordinate2D, Coordinate3D, CoordinateError, CoordinateFrame, CoordinateRegistry,
    CoordinateSpace, CoordinateUnit, FrameTransform, ImageCoordinateConvention, SerialSection,
    SerialSectionPlacement, SerialSectionSeries, SerialSectionStatus, SpatialAxis,
    SpatialDimension, TransformMatrix, UncertaintyReference,
};
pub use hierarchy::{
    BiologicalSourceError, CohortDesignSummary, CohortHierarchy, HierarchyError,
    RepeatedMeasureLinkError,
};
pub use marklab_core::{
    BlockId, CellId, CoordinateFrameId, CoordinateIdentityError, CoordinateIdentityKind, CoreId,
    HierarchyId, HierarchyKind, IdentityError, PatchId, PatientId, RegionId, SectionId, SiteId,
    SlideId, SpecimenId, TimepointId, TransformId, UncertaintyId,
};
pub use node::{
    HierarchyNode, RepeatedMeasureError, RepeatedMeasureSet, ReplicationRole, ReplicationRoleKind,
};
