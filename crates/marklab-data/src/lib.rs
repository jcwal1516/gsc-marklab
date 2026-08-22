#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! In-memory cohort hierarchy and data contracts for Marklab.

mod hierarchy;
mod node;

pub use hierarchy::{
    BiologicalSourceError, CohortDesignSummary, CohortHierarchy, HierarchyError,
    RepeatedMeasureLinkError,
};
pub use marklab_core::{
    BlockId, CellId, CoreId, HierarchyId, HierarchyKind, IdentityError, PatchId, PatientId,
    RegionId, SectionId, SiteId, SlideId, SpecimenId, TimepointId,
};
pub use node::{
    HierarchyNode, RepeatedMeasureError, RepeatedMeasureSet, ReplicationRole, ReplicationRoleKind,
};
