#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Canonical cell-embedding values and bounded artifact encodings.

mod context;
mod digest;
mod error;
mod expected;
mod identity_map;
mod provenance;
mod row_link;
mod table;

pub use context::{EmbeddingSpatialContext, PatchBoundaryPolicy, PositiveRational};
pub use error::EmbeddingError;
pub use expected::ExpectedCellSet;
pub use identity_map::{CellIdentityMap, CellIdentityMapEntry};
pub use provenance::{
    ArtifactAvailabilityFailure, CanonicalDecimal, CellEmbeddingArtifactRole,
    CellEmbeddingExecutionProvenance, CellEmbeddingInputArtifacts, CellEmbeddingModelProvenance,
    CellEmbeddingProvenance, CellEmbeddingTensorContract, EmbeddingArtifactGraphError,
    VerifiedCellEmbeddingArtifactGraph,
};
pub use row_link::{CellEmbeddingRowLink, CellEmbeddingRowLinkEntry};
pub use table::{
    CellEmbeddingRow, CellEmbeddingTable, CellEmbeddingView, EmbeddingQcSummary, EmbeddingStatus,
};
