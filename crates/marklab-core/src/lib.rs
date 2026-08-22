#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Foundational typed identities for Marklab analysis bundles.

mod identity;

pub use identity::{
    BlockId, CellId, CoreId, HierarchyId, HierarchyKind, IdentityError, PatchId, PatientId,
    RegionId, SectionId, SiteId, SlideId, SpecimenId, TimepointId,
};
