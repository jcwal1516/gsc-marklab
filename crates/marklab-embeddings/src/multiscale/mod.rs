mod allocation;
mod artifact;
mod cell_patch;
mod cell_patch_context;
#[cfg(feature = "parquet")]
mod contained_cell_patch_dispersion;
mod context;
mod digest;
mod entity;
mod error;
mod expected;
mod finalization;
mod footprint;
mod json;
mod matrix_artifact;
mod overlap;
mod overlap_dispersion;
mod patch_dependency;
mod patch_region;
pub(crate) mod physical;
mod records;
mod region_dispersion;
mod slide_finalization;
mod slide_path_discrepancy;
mod table;

pub use artifact::{
    VerifiedCellPatchAssignmentArtifact, VerifiedCellPatchEdgeArtifact,
    VerifiedCellPatchLinkArtifact, VerifiedPatchFootprintArtifact, VerifiedPatchOverlapArtifact,
    VerifiedPatchRegionLinkArtifact,
};
pub use cell_patch::{
    CellPatchAnchor, CellPatchAssignment, CellPatchAssignmentMode, CellPatchAssignmentStatus,
    CellPatchContributor, CellPatchEdge, CellPatchLink, CellPatchLinkBindings, CellPatchWeight,
    DeclaredCellPatchAssignment,
};
pub use cell_patch_context::{cell_patch_context, CellPatchContextError, CellPatchContextResult};
#[cfg(feature = "parquet")]
pub use contained_cell_patch_dispersion::{
    contained_cell_patch_embedding_dispersion, ContainedCellPatchEmbeddingDispersion,
    ContainedCellPatchEmbeddingDispersionError, ContainedCellPatchEmbeddingDispersionStatus,
};
pub use context::{EffectiveReceptiveField, PatchEmbeddingContext};
pub use entity::EmbeddingEntityKind;
pub use error::MultiscaleEmbeddingError;
pub use expected::{ExpectedPatchSet, ExpectedRegionSet, ExpectedSlideSet};
pub use finalization::{
    finalize_region_embedding_table_from_patches, DerivedRegionEmbeddingTableCandidate,
    EmbeddingFinalizationBudgets,
};
pub use footprint::{PatchFootprint, PatchFootprintSet};
pub use matrix_artifact::{
    VerifiedPatchEmbeddingSupportArtifact, VerifiedPatchEmbeddingTableArtifact,
    VerifiedRegionEmbeddingSupportArtifact, VerifiedRegionEmbeddingTableArtifact,
    VerifiedSlideEmbeddingSupportArtifact, VerifiedSlideEmbeddingTableArtifact,
};
pub use overlap::{PatchOverlapEdge, PatchOverlapGraph};
pub use overlap_dispersion::{
    patch_overlap_embedding_dispersion, PatchOverlapEmbeddingDispersion,
    PatchOverlapEmbeddingDispersionError, PatchOverlapEmbeddingDispersionStatus,
};
pub use patch_dependency::{
    patch_dependency_weighting, PatchDependencyWeight, PatchDependencyWeighting,
    PatchDependencyWeightingError,
};
pub use patch_region::{
    PatchRegionAssessment, PatchRegionAssessmentBindings, PatchRegionDeclaration, PatchRegionLink,
    PatchRegionRelation,
};
#[cfg(feature = "parquet")]
pub use records::{
    CellPatchInputArtifactGraphError, CellPatchInputArtifactRole,
    PatchRegionInputArtifactGraphError, PatchRegionInputArtifactRole,
    VerifiedCellPatchInputArtifactGraph, VerifiedPatchRegionInputArtifactGraph,
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
    VerifiedDerivedRegionEmbeddingArtifactGraph, VerifiedDerivedSlideEmbeddingArtifactGraph,
    VerifiedDirectPatchEmbeddingArtifactGraph,
};
pub use region_dispersion::{
    patch_region_embedding_dispersion, PatchRegionEmbeddingDispersion,
    PatchRegionEmbeddingDispersionError, PatchRegionEmbeddingDispersionStatus,
};
pub use slide_finalization::{
    finalize_slide_embedding_table_from_patches, finalize_slide_embedding_table_from_regions,
    DerivedSlideEmbeddingTableCandidate,
};
pub use slide_path_discrepancy::{
    slide_embedding_aggregation_path_discrepancy, SlideEmbeddingAggregationPathDiscrepancy,
    SlideEmbeddingAggregationPathDiscrepancyError, SlideEmbeddingAggregationPathDiscrepancyStatus,
};
#[cfg(feature = "parquet")]
pub(crate) use table::{MatrixSummaryAccumulator, MultiscaleMatrixTable};
pub use table::{
    MultiscaleEmbeddingQcSummary, PatchEmbeddingBlock, PatchEmbeddingRow, PatchEmbeddingTable,
    PatchEmbeddingView, RegionEmbeddingBlock, RegionEmbeddingRow, RegionEmbeddingTable,
    RegionEmbeddingView, SlideEmbeddingBlock, SlideEmbeddingRow, SlideEmbeddingTable,
    SlideEmbeddingView,
};
