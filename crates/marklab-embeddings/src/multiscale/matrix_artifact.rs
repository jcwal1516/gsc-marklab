use std::fmt;

use marklab_data::SlideId;
use marklab_project::{ArtifactId, ContentDigest};

#[cfg(feature = "parquet")]
use super::PatchRegionLink;
use super::{digest::LogicalDigest, MultiscaleEmbeddingQcSummary};
#[cfg(feature = "parquet")]
use super::{
    DerivedRegionEmbeddingTableCandidate, DerivedSlideEmbeddingTableCandidate, EmbeddingEntityKind,
    PatchEmbeddingSourceRowLink, PatchEmbeddingTable, VerifiedDirectPatchEmbeddingArtifactGraph,
    VerifiedPatchFootprintArtifact, VerifiedPatchOverlapArtifact,
};

mod support;
mod table_artifact;

#[cfg(feature = "parquet")]
#[allow(unused_imports)]
pub(in crate::multiscale) use support::{
    slide_lineage_digest, VerifiedPatchEmbeddingSupportBindings,
    VerifiedRegionEmbeddingSupportBindings, VerifiedSlideEmbeddingSupportBindings,
};
pub use support::{
    VerifiedPatchEmbeddingSupportArtifact, VerifiedRegionEmbeddingSupportArtifact,
    VerifiedSlideEmbeddingSupportArtifact,
};
pub use table_artifact::{
    VerifiedPatchEmbeddingTableArtifact, VerifiedRegionEmbeddingTableArtifact,
    VerifiedSlideEmbeddingTableArtifact,
};
#[cfg(feature = "parquet")]
pub(in crate::multiscale) use table_artifact::{
    VerifiedPatchEmbeddingTableBindings, VerifiedRegionEmbeddingTableBindings,
};
