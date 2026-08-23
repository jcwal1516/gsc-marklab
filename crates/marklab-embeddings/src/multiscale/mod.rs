mod artifact;
mod cell_patch;
mod context;
mod digest;
mod entity;
mod error;
mod expected;
mod footprint;
mod json;
mod overlap;
mod patch_region;
pub(crate) mod physical;
mod records;
mod table;

pub use artifact::{VerifiedPatchFootprintArtifact, VerifiedPatchOverlapArtifact};
pub use cell_patch::{
    CellPatchAnchor, CellPatchAssignment, CellPatchAssignmentMode, CellPatchAssignmentStatus,
    CellPatchContributor, CellPatchEdge, CellPatchLink, CellPatchLinkBindings, CellPatchWeight,
    DeclaredCellPatchAssignment,
};
pub use context::{EffectiveReceptiveField, PatchEmbeddingContext};
pub use entity::EmbeddingEntityKind;
pub use error::MultiscaleEmbeddingError;
pub use expected::{ExpectedPatchSet, ExpectedRegionSet, ExpectedSlideSet};
pub use footprint::{PatchFootprint, PatchFootprintSet};
pub use overlap::{PatchOverlapEdge, PatchOverlapGraph};
pub use patch_region::{
    PatchRegionAssessment, PatchRegionAssessmentBindings, PatchRegionDeclaration, PatchRegionLink,
    PatchRegionRelation,
};
pub use records::{
    CellPatchLinkProducer, MultiscaleArtifactBinding, MultiscaleDirectPatchInputArtifacts,
    MultiscaleDirectPatchModelProvenance, MultiscaleEmbeddingArtifactGraphError,
    MultiscaleEmbeddingArtifactRole, MultiscaleEmbeddingDerivationContract,
    MultiscaleEmbeddingExecutionProvenance, MultiscaleEmbeddingProvenance,
    MultiscaleEmbeddingProvenanceVariant, MultiscaleEmbeddingSupport,
    MultiscaleEmbeddingSupportVariant, PatchEmbeddingInputNormalization,
    PatchEmbeddingSourceRowLink, PatchEmbeddingSourceRowLinkEntry, PatchIdentityMap,
    PatchIdentityMapEntry, PatchNormalizationDecimal, PatchSourceEntityEntry, PatchSourceEntitySet,
    VerifiedDirectPatchEmbeddingArtifactGraph,
};
pub use table::{
    MultiscaleEmbeddingQcSummary, PatchEmbeddingBlock, PatchEmbeddingRow, PatchEmbeddingTable,
    PatchEmbeddingView, RegionEmbeddingBlock, RegionEmbeddingRow, RegionEmbeddingTable,
    RegionEmbeddingView, SlideEmbeddingBlock, SlideEmbeddingRow, SlideEmbeddingTable,
    SlideEmbeddingView,
};
