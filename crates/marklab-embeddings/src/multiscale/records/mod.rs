mod artifact_graph;
mod binding;
pub(super) mod codec;
mod derivation;
mod normalization;
mod producer;
mod provenance;
mod source;
mod support;

#[cfg(feature = "parquet")]
pub use artifact_graph::{
    CellPatchInputArtifactGraphError, CellPatchInputArtifactRole,
    PatchRegionInputArtifactGraphError, PatchRegionInputArtifactRole,
    VerifiedCellPatchInputArtifactGraph, VerifiedPatchRegionInputArtifactGraph,
};
pub use artifact_graph::{
    MultiscaleEmbeddingArtifactGraphError, MultiscaleEmbeddingArtifactRole,
    VerifiedDerivedRegionEmbeddingArtifactGraph, VerifiedDerivedSlideEmbeddingArtifactGraph,
    VerifiedDirectPatchEmbeddingArtifactGraph,
};
pub use binding::MultiscaleArtifactBinding;
pub use derivation::MultiscaleEmbeddingDerivationContract;
pub use normalization::{PatchEmbeddingInputNormalization, PatchNormalizationDecimal};
pub use producer::CellPatchLinkProducer;
pub use provenance::{
    MultiscaleDirectPatchInputArtifacts, MultiscaleDirectPatchModelProvenance,
    MultiscaleEmbeddingExecutionProvenance, MultiscaleEmbeddingProvenance,
    MultiscaleEmbeddingProvenanceVariant,
};
pub use source::{
    PatchEmbeddingSourceRowLink, PatchEmbeddingSourceRowLinkEntry, PatchIdentityMap,
    PatchIdentityMapEntry, PatchSourceEntityEntry, PatchSourceEntitySet,
};
pub use support::{MultiscaleEmbeddingSupport, MultiscaleEmbeddingSupportVariant};
