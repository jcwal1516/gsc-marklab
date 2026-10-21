#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Identity types for Marklab analysis bundles, deterministic simulation-seed derivation,
//! and the [`backend`] contracts that bind Python worker results to their environment.

pub mod backend;

mod coordinate_identity;
mod identity;
pub mod vocabulary;

pub use coordinate_identity::{
    CoordinateFrameId, CoordinateIdentityError, CoordinateIdentityKind, TransformId, UncertaintyId,
};

pub use identity::{
    BlockId, CellId, CoreId, HierarchyId, HierarchyKind, IdentityError, PatchId, PatientId,
    RegionId, SectionId, SiteId, SlideId, SpecimenId, TimepointId,
};

/// Derive the existing 256-bit simulation seed from a namespace and little-endian seed.
///
/// The namespace bytes, including any trailing NUL, are part of the deterministic contract.
pub fn derive_simulation_seed(namespace: &[u8], seed: u64) -> [u8; 32] {
    use sha2::{Digest, Sha256};

    let mut hash = Sha256::new();
    hash.update(namespace);
    hash.update(seed.to_le_bytes());
    hash.finalize().into()
}
