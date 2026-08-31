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
mod rational;
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
    preflight_cell_patch_assignment_table_arrow_bytes,
    preflight_cell_patch_assignment_table_parquet_bytes,
    preflight_cell_patch_edge_table_arrow_bytes, preflight_cell_patch_edge_table_parquet_bytes,
    preflight_patch_footprint_set_arrow_bytes, preflight_patch_footprint_set_parquet_bytes,
    preflight_patch_overlap_graph_arrow_bytes, preflight_patch_overlap_graph_parquet_bytes,
    preflight_patch_region_link_arrow_bytes, preflight_patch_region_link_parquet_bytes,
    publish_cell_embedding_row_link_arrow, publish_cell_embedding_row_link_parquet,
    publish_cell_embedding_table_arrow, publish_cell_embedding_table_parquet,
    publish_cell_patch_assignment_table_arrow, publish_cell_patch_assignment_table_parquet,
    publish_cell_patch_edge_table_arrow, publish_cell_patch_edge_table_parquet,
    publish_patch_footprint_set_arrow, publish_patch_footprint_set_parquet,
    publish_patch_overlap_graph_arrow, publish_patch_overlap_graph_parquet,
    publish_patch_region_link_arrow, publish_patch_region_link_parquet,
    read_cell_embedding_table_arrow_bytes, read_cell_embedding_table_arrow_from_store,
    read_cell_embedding_table_parquet_bytes, read_cell_embedding_table_parquet_from_store,
    scan_cell_embedding_table_arrow_bytes, scan_cell_embedding_table_arrow_from_store,
    scan_cell_embedding_table_parquet_bytes, scan_cell_embedding_table_parquet_from_store,
    validate_cell_embedding_row_link_arrow_bytes,
    validate_cell_embedding_row_link_arrow_from_store,
    validate_cell_embedding_row_link_parquet_bytes,
    validate_cell_embedding_row_link_parquet_from_store,
    validate_cell_patch_assignment_table_arrow_bytes,
    validate_cell_patch_assignment_table_arrow_from_store,
    validate_cell_patch_assignment_table_parquet_bytes,
    validate_cell_patch_assignment_table_parquet_from_store,
    validate_cell_patch_edge_table_arrow_bytes, validate_cell_patch_edge_table_arrow_from_store,
    validate_cell_patch_edge_table_parquet_bytes,
    validate_cell_patch_edge_table_parquet_from_store, validate_patch_footprint_set_arrow_bytes,
    validate_patch_footprint_set_arrow_from_store, validate_patch_footprint_set_parquet_bytes,
    validate_patch_footprint_set_parquet_from_store, validate_patch_overlap_graph_arrow_bytes,
    validate_patch_overlap_graph_arrow_from_store, validate_patch_overlap_graph_parquet_bytes,
    validate_patch_overlap_graph_parquet_from_store, validate_patch_region_link_arrow_bytes,
    validate_patch_region_link_arrow_from_store, validate_patch_region_link_parquet_bytes,
    validate_patch_region_link_parquet_from_store, verify_cell_embedding_row_link_arrow_bytes,
    verify_cell_embedding_row_link_arrow_from_store, verify_cell_embedding_row_link_parquet_bytes,
    verify_cell_embedding_row_link_parquet_from_store, verify_cell_embedding_table_arrow_bytes,
    verify_cell_embedding_table_arrow_from_store, verify_cell_embedding_table_parquet_bytes,
    verify_cell_embedding_table_parquet_from_store, verify_cell_patch_assignment_table_arrow_bytes,
    verify_cell_patch_assignment_table_arrow_from_store,
    verify_cell_patch_assignment_table_parquet_bytes,
    verify_cell_patch_assignment_table_parquet_from_store,
    verify_cell_patch_edge_table_arrow_bytes, verify_cell_patch_edge_table_arrow_from_store,
    verify_cell_patch_edge_table_parquet_bytes, verify_cell_patch_edge_table_parquet_from_store,
    verify_patch_footprint_set_arrow_bytes, verify_patch_footprint_set_arrow_from_store,
    verify_patch_footprint_set_parquet_bytes, verify_patch_footprint_set_parquet_from_store,
    verify_patch_overlap_graph_arrow_bytes, verify_patch_overlap_graph_arrow_from_store,
    verify_patch_overlap_graph_parquet_bytes, verify_patch_overlap_graph_parquet_from_store,
    verify_patch_region_link_arrow_bytes, verify_patch_region_link_arrow_from_store,
    verify_patch_region_link_parquet_bytes, verify_patch_region_link_parquet_from_store,
    write_cell_embedding_row_link_arrow, write_cell_embedding_row_link_parquet,
    write_cell_embedding_table_arrow, write_cell_embedding_table_parquet,
    write_cell_patch_assignment_table_arrow, write_cell_patch_assignment_table_parquet,
    write_cell_patch_edge_table_arrow, write_cell_patch_edge_table_parquet,
    write_patch_footprint_set_arrow, write_patch_footprint_set_parquet,
    write_patch_overlap_graph_arrow, write_patch_overlap_graph_parquet,
    write_patch_region_link_arrow, write_patch_region_link_parquet, ArrowIpcFailure,
    CellEmbeddingArrowPreflight, CellEmbeddingParquetPreflight, CellEmbeddingRowLinkArrowPreflight,
    CellEmbeddingRowLinkParquetPreflight, CellEmbeddingTablePhysicalBindings,
    CellPatchAssignmentArrowPreflight, CellPatchAssignmentParquetPreflight,
    CellPatchEdgeArrowPreflight, CellPatchEdgeParquetPreflight, ColumnarWriteSummary,
    EmbeddingColumnarBudgets, EmbeddingColumnarError, EmbeddingColumnarPublicationError,
    MultiscaleColumnarError, MultiscaleColumnarPublicationError, ParquetFailure,
    PatchFootprintArrowPreflight, PatchFootprintParquetPreflight, PatchOverlapArrowPreflight,
    PatchOverlapParquetPreflight, PatchRegionArrowPreflight, PatchRegionParquetPreflight,
    RowLinkColumnarWriteSummary, SpatialArrowFailure, SpatialColumnarWriteSummary,
    SpatialParquetFailure,
};
#[cfg(feature = "parquet")]
pub use columnar::{
    preflight_patch_embedding_table_arrow_bytes, preflight_patch_embedding_table_parquet_bytes,
    preflight_region_embedding_table_arrow_bytes, preflight_region_embedding_table_parquet_bytes,
    preflight_slide_embedding_table_arrow_bytes, preflight_slide_embedding_table_parquet_bytes,
    publish_patch_embedding_table_arrow, publish_patch_embedding_table_parquet,
    publish_region_embedding_table_arrow, publish_region_embedding_table_parquet,
    publish_slide_embedding_table_arrow, publish_slide_embedding_table_parquet,
    validate_patch_embedding_table_arrow_bytes, validate_patch_embedding_table_arrow_from_store,
    validate_patch_embedding_table_parquet_bytes,
    validate_patch_embedding_table_parquet_from_store, validate_region_embedding_table_arrow_bytes,
    validate_region_embedding_table_arrow_from_store,
    validate_region_embedding_table_parquet_bytes,
    validate_region_embedding_table_parquet_from_store, validate_slide_embedding_table_arrow_bytes,
    validate_slide_embedding_table_arrow_from_store, validate_slide_embedding_table_parquet_bytes,
    validate_slide_embedding_table_parquet_from_store, verify_patch_embedding_table_arrow_bytes,
    verify_patch_embedding_table_arrow_from_store, verify_patch_embedding_table_parquet_bytes,
    verify_patch_embedding_table_parquet_from_store, verify_region_embedding_table_arrow_bytes,
    verify_region_embedding_table_arrow_from_store, verify_region_embedding_table_parquet_bytes,
    verify_region_embedding_table_parquet_from_store, verify_slide_embedding_table_arrow_bytes,
    verify_slide_embedding_table_arrow_from_store, verify_slide_embedding_table_parquet_bytes,
    verify_slide_embedding_table_parquet_from_store, write_patch_embedding_table_arrow,
    write_patch_embedding_table_parquet, write_region_embedding_table_arrow,
    write_region_embedding_table_parquet, write_slide_embedding_table_arrow,
    write_slide_embedding_table_parquet, MultiscaleMatrixArrowPreflight,
    MultiscaleMatrixParquetPreflight,
};
pub use context::{EmbeddingSpatialContext, PatchBoundaryPolicy, PositiveRational};
pub use error::EmbeddingError;
pub use expected::ExpectedCellSet;
pub use identity_map::{CellIdentityMap, CellIdentityMapEntry};
pub use multiscale::{
    cell_patch_context, finalize_region_embedding_table_from_patches,
    finalize_slide_embedding_table_from_patches, finalize_slide_embedding_table_from_regions,
    patch_dependency_weighting, patch_overlap_embedding_dispersion,
    patch_region_embedding_dispersion, slide_embedding_aggregation_path_discrepancy,
    CellPatchAnchor, CellPatchAssignment, CellPatchAssignmentMode, CellPatchAssignmentStatus,
    CellPatchContextError, CellPatchContextResult, CellPatchContributor, CellPatchEdge,
    CellPatchLink, CellPatchLinkBindings, CellPatchLinkProducer, CellPatchWeight,
    DeclaredCellPatchAssignment, DerivedRegionEmbeddingTableCandidate,
    DerivedSlideEmbeddingTableCandidate, EffectiveReceptiveField, EmbeddingEntityKind,
    EmbeddingFinalizationBudgets, ExpectedPatchSet, ExpectedRegionSet, ExpectedSlideSet,
    MultiscaleArtifactBinding, MultiscaleDirectPatchInputArtifacts,
    MultiscaleDirectPatchModelProvenance, MultiscaleEmbeddingArtifactGraphError,
    MultiscaleEmbeddingArtifactRole, MultiscaleEmbeddingDerivationContract,
    MultiscaleEmbeddingError, MultiscaleEmbeddingExecutionProvenance,
    MultiscaleEmbeddingProvenance, MultiscaleEmbeddingProvenanceVariant,
    MultiscaleEmbeddingQcSummary, MultiscaleEmbeddingSupport, MultiscaleEmbeddingSupportVariant,
    PatchDependencyWeight, PatchDependencyWeighting, PatchDependencyWeightingError,
    PatchEmbeddingBlock, PatchEmbeddingContext, PatchEmbeddingInputNormalization,
    PatchEmbeddingRow, PatchEmbeddingSourceRowLink, PatchEmbeddingSourceRowLinkEntry,
    PatchEmbeddingTable, PatchEmbeddingView, PatchFootprint, PatchFootprintSet, PatchIdentityMap,
    PatchIdentityMapEntry, PatchNormalizationDecimal, PatchOverlapEdge,
    PatchOverlapEmbeddingDispersion, PatchOverlapEmbeddingDispersionError,
    PatchOverlapEmbeddingDispersionStatus, PatchOverlapGraph, PatchRegionAssessment,
    PatchRegionAssessmentBindings, PatchRegionDeclaration, PatchRegionEmbeddingDispersion,
    PatchRegionEmbeddingDispersionError, PatchRegionEmbeddingDispersionStatus, PatchRegionLink,
    PatchRegionRelation, PatchSourceEntityEntry, PatchSourceEntitySet, RegionEmbeddingBlock,
    RegionEmbeddingRow, RegionEmbeddingTable, RegionEmbeddingView,
    SlideEmbeddingAggregationPathDiscrepancy, SlideEmbeddingAggregationPathDiscrepancyError,
    SlideEmbeddingAggregationPathDiscrepancyStatus, SlideEmbeddingBlock, SlideEmbeddingRow,
    SlideEmbeddingTable, SlideEmbeddingView, VerifiedCellPatchAssignmentArtifact,
    VerifiedCellPatchEdgeArtifact, VerifiedCellPatchLinkArtifact,
    VerifiedDerivedRegionEmbeddingArtifactGraph, VerifiedDerivedSlideEmbeddingArtifactGraph,
    VerifiedDirectPatchEmbeddingArtifactGraph, VerifiedPatchEmbeddingSupportArtifact,
    VerifiedPatchEmbeddingTableArtifact, VerifiedPatchFootprintArtifact,
    VerifiedPatchOverlapArtifact, VerifiedPatchRegionLinkArtifact,
    VerifiedRegionEmbeddingSupportArtifact, VerifiedRegionEmbeddingTableArtifact,
    VerifiedSlideEmbeddingSupportArtifact, VerifiedSlideEmbeddingTableArtifact,
};
#[cfg(feature = "parquet")]
pub use multiscale::{
    contained_cell_patch_embedding_dispersion, ContainedCellPatchEmbeddingDispersion,
    ContainedCellPatchEmbeddingDispersionError, ContainedCellPatchEmbeddingDispersionStatus,
};
#[cfg(feature = "parquet")]
pub use multiscale::{
    CellPatchInputArtifactGraphError, CellPatchInputArtifactRole,
    PatchRegionInputArtifactGraphError, PatchRegionInputArtifactRole,
    VerifiedCellPatchInputArtifactGraph, VerifiedPatchRegionInputArtifactGraph,
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
