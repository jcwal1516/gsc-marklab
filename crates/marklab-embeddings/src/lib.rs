#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Canonical typed embedding values and bounded artifact encodings.

mod artifact;
#[cfg(feature = "parquet")]
mod columnar;
mod context;
mod digest;
mod error;
mod expected;
mod identity_map;
mod multiscale;
mod provenance;
mod row_link;
mod source;
mod table;

pub use artifact::{
    CellEmbeddingArtifact, EmbeddingDtype, VerifiedCellEmbeddingRowLinkArtifact,
    VerifiedCellEmbeddingTableArtifact,
};
#[cfg(feature = "parquet")]
pub use columnar::{
    preflight_cell_embedding_row_link_arrow_bytes, preflight_cell_embedding_row_link_parquet_bytes,
    preflight_cell_embedding_table_arrow_bytes, preflight_cell_embedding_table_parquet_bytes,
    preflight_patch_footprint_set_arrow_bytes, preflight_patch_footprint_set_parquet_bytes,
    preflight_patch_overlap_graph_arrow_bytes, preflight_patch_overlap_graph_parquet_bytes,
    publish_cell_embedding_row_link_arrow, publish_cell_embedding_row_link_parquet,
    publish_cell_embedding_table_arrow, publish_cell_embedding_table_parquet,
    publish_patch_footprint_set_arrow, publish_patch_footprint_set_parquet,
    publish_patch_overlap_graph_arrow, publish_patch_overlap_graph_parquet,
    read_cell_embedding_table_arrow_bytes, read_cell_embedding_table_arrow_from_store,
    read_cell_embedding_table_parquet_bytes, read_cell_embedding_table_parquet_from_store,
    scan_cell_embedding_table_arrow_bytes, scan_cell_embedding_table_arrow_from_store,
    scan_cell_embedding_table_parquet_bytes, scan_cell_embedding_table_parquet_from_store,
    validate_cell_embedding_row_link_arrow_bytes,
    validate_cell_embedding_row_link_arrow_from_store,
    validate_cell_embedding_row_link_parquet_bytes,
    validate_cell_embedding_row_link_parquet_from_store, validate_patch_footprint_set_arrow_bytes,
    validate_patch_footprint_set_arrow_from_store, validate_patch_footprint_set_parquet_bytes,
    validate_patch_footprint_set_parquet_from_store, validate_patch_overlap_graph_arrow_bytes,
    validate_patch_overlap_graph_arrow_from_store, validate_patch_overlap_graph_parquet_bytes,
    validate_patch_overlap_graph_parquet_from_store, verify_cell_embedding_row_link_arrow_bytes,
    verify_cell_embedding_row_link_arrow_from_store, verify_cell_embedding_row_link_parquet_bytes,
    verify_cell_embedding_row_link_parquet_from_store, verify_cell_embedding_table_arrow_bytes,
    verify_cell_embedding_table_arrow_from_store, verify_cell_embedding_table_parquet_bytes,
    verify_cell_embedding_table_parquet_from_store, verify_patch_footprint_set_arrow_bytes,
    verify_patch_footprint_set_arrow_from_store, verify_patch_footprint_set_parquet_bytes,
    verify_patch_footprint_set_parquet_from_store, verify_patch_overlap_graph_arrow_bytes,
    verify_patch_overlap_graph_arrow_from_store, verify_patch_overlap_graph_parquet_bytes,
    verify_patch_overlap_graph_parquet_from_store, write_cell_embedding_row_link_arrow,
    write_cell_embedding_row_link_parquet, write_cell_embedding_table_arrow,
    write_cell_embedding_table_parquet, write_patch_footprint_set_arrow,
    write_patch_footprint_set_parquet, write_patch_overlap_graph_arrow,
    write_patch_overlap_graph_parquet, ArrowIpcFailure, CellEmbeddingArrowPreflight,
    CellEmbeddingParquetPreflight, CellEmbeddingRowLinkArrowPreflight,
    CellEmbeddingRowLinkParquetPreflight, CellEmbeddingTablePhysicalBindings, ColumnarWriteSummary,
    EmbeddingColumnarBudgets, EmbeddingColumnarError, EmbeddingColumnarPublicationError,
    MultiscaleColumnarError, MultiscaleColumnarPublicationError, ParquetFailure,
    PatchFootprintArrowPreflight, PatchFootprintParquetPreflight, PatchOverlapArrowPreflight,
    PatchOverlapParquetPreflight, RowLinkColumnarWriteSummary, SpatialArrowFailure,
    SpatialColumnarWriteSummary, SpatialParquetFailure,
};
pub use context::{EmbeddingSpatialContext, PatchBoundaryPolicy, PositiveRational};
pub use error::EmbeddingError;
pub use expected::ExpectedCellSet;
pub use identity_map::{CellIdentityMap, CellIdentityMapEntry};
pub use multiscale::{
    CellPatchAnchor, CellPatchAssignment, CellPatchAssignmentMode, CellPatchAssignmentStatus,
    CellPatchContributor, CellPatchEdge, CellPatchLink, CellPatchLinkBindings,
    CellPatchLinkProducer, CellPatchWeight, DeclaredCellPatchAssignment, EffectiveReceptiveField,
    EmbeddingEntityKind, ExpectedPatchSet, ExpectedRegionSet, ExpectedSlideSet,
    MultiscaleArtifactBinding, MultiscaleDirectPatchInputArtifacts,
    MultiscaleDirectPatchModelProvenance, MultiscaleEmbeddingArtifactGraphError,
    MultiscaleEmbeddingArtifactRole, MultiscaleEmbeddingDerivationContract,
    MultiscaleEmbeddingError, MultiscaleEmbeddingExecutionProvenance,
    MultiscaleEmbeddingProvenance, MultiscaleEmbeddingProvenanceVariant,
    MultiscaleEmbeddingQcSummary, MultiscaleEmbeddingSupport, MultiscaleEmbeddingSupportVariant,
    PatchEmbeddingBlock, PatchEmbeddingContext, PatchEmbeddingInputNormalization,
    PatchEmbeddingRow, PatchEmbeddingSourceRowLink, PatchEmbeddingSourceRowLinkEntry,
    PatchEmbeddingTable, PatchEmbeddingView, PatchFootprint, PatchFootprintSet, PatchIdentityMap,
    PatchIdentityMapEntry, PatchNormalizationDecimal, PatchOverlapEdge, PatchOverlapGraph,
    PatchRegionAssessment, PatchRegionAssessmentBindings, PatchRegionDeclaration, PatchRegionLink,
    PatchRegionRelation, PatchSourceEntityEntry, PatchSourceEntitySet, RegionEmbeddingBlock,
    RegionEmbeddingRow, RegionEmbeddingTable, RegionEmbeddingView, SlideEmbeddingBlock,
    SlideEmbeddingRow, SlideEmbeddingTable, SlideEmbeddingView,
    VerifiedDirectPatchEmbeddingArtifactGraph, VerifiedPatchFootprintArtifact,
    VerifiedPatchOverlapArtifact,
};
#[cfg(feature = "parquet")]
pub use multiscale::{
    CellPatchInputArtifactGraphError, CellPatchInputArtifactRole,
    VerifiedCellPatchInputArtifactGraph,
};
pub use provenance::{
    ArtifactAvailabilityFailure, CanonicalDecimal, CellEmbeddingArtifactRole,
    CellEmbeddingExecutionProvenance, CellEmbeddingInputArtifacts, CellEmbeddingModelProvenance,
    CellEmbeddingProvenance, CellEmbeddingTensorContract, EmbeddingArtifactGraphError,
    VerifiedCellEmbeddingArtifactGraph,
};
pub use row_link::{CellEmbeddingRowLink, CellEmbeddingRowLinkEntry};
#[cfg(feature = "csv")]
pub use source::{
    import_cellvit_he_bundle_bytes, import_cellvit_he_bundle_from_store,
    import_cellvit_he_bundle_readers, CellVitCsvSummary, CellVitHeArtifactBindings,
    CellVitHeImportCandidate, CellVitHeImportRequest, ImportedCellVitHeBundle,
    MissingPromotionField, SourceBundleReconciler, SourceBundleReconciliation,
};
pub use source::{
    CellVitCsvField, CellVitNpyMatrix, CellVitNpySummary, CsvFailure, ImportFailure,
    ManifestFailure, NpyFailure, NpyVersion, ReconciliationFailure, SourceBundleBudgets,
    SourceBundleError, SourceFileKind, SourceIoFailure, SourceIoOperation,
};
pub use table::{
    CellEmbeddingBlock, CellEmbeddingRow, CellEmbeddingTable, CellEmbeddingView,
    EmbeddingQcSummary, EmbeddingStatus,
};
