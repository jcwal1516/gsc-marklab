#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Foundational typed identities for Marklab analysis bundles.

mod coordinate_identity;
mod identity;

pub use coordinate_identity::{
    CoordinateFrameId, CoordinateIdentityError, CoordinateIdentityKind, TransformId, UncertaintyId,
};

pub use identity::{
    BlockId, CellId, CoreId, HierarchyId, HierarchyKind, IdentityError, PatchId, PatientId,
    RegionId, SectionId, SiteId, SlideId, SpecimenId, TimepointId,
};
