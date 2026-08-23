#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Canonical cell-embedding values and bounded artifact encodings.

#[cfg(feature = "parquet")]
mod columnar;
mod context;
mod digest;
mod error;
mod expected;
mod identity_map;
mod provenance;
mod row_link;
mod source;
mod table;

#[cfg(feature = "parquet")]
pub use columnar::{
    preflight_cell_embedding_row_link_arrow_bytes, preflight_cell_embedding_row_link_parquet_bytes,
    preflight_cell_embedding_table_arrow_bytes, preflight_cell_embedding_table_parquet_bytes,
    publish_cell_embedding_row_link_arrow, publish_cell_embedding_row_link_parquet,
    publish_cell_embedding_table_arrow, publish_cell_embedding_table_parquet,
    read_cell_embedding_table_arrow_bytes, read_cell_embedding_table_arrow_from_store,
    read_cell_embedding_table_parquet_bytes, read_cell_embedding_table_parquet_from_store,
    validate_cell_embedding_row_link_arrow_bytes,
    validate_cell_embedding_row_link_arrow_from_store,
    validate_cell_embedding_row_link_parquet_bytes,
    validate_cell_embedding_row_link_parquet_from_store, write_cell_embedding_row_link_arrow,
    write_cell_embedding_row_link_parquet, write_cell_embedding_table_arrow,
    write_cell_embedding_table_parquet, ArrowIpcFailure, CellEmbeddingArrowPreflight,
    CellEmbeddingParquetPreflight, CellEmbeddingRowLinkArrowPreflight,
    CellEmbeddingRowLinkParquetPreflight, CellEmbeddingTablePhysicalBindings, ColumnarWriteSummary,
    EmbeddingColumnarBudgets, EmbeddingColumnarError, EmbeddingColumnarPublicationError,
    ParquetFailure, RowLinkColumnarWriteSummary,
};
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
    CellEmbeddingRow, CellEmbeddingTable, CellEmbeddingView, EmbeddingQcSummary, EmbeddingStatus,
};
