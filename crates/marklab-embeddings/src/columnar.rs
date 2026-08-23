mod arrow;
mod error;
mod multiscale;
mod parquet;

pub use arrow::{
    preflight_cell_embedding_row_link_arrow_bytes, preflight_cell_embedding_table_arrow_bytes,
    preflight_cell_patch_assignment_table_arrow_bytes, preflight_cell_patch_edge_table_arrow_bytes,
    preflight_patch_footprint_set_arrow_bytes, preflight_patch_overlap_graph_arrow_bytes,
    preflight_patch_region_link_arrow_bytes, publish_cell_embedding_row_link_arrow,
    publish_cell_embedding_table_arrow, publish_cell_patch_assignment_table_arrow,
    publish_cell_patch_edge_table_arrow, publish_patch_footprint_set_arrow,
    publish_patch_overlap_graph_arrow, publish_patch_region_link_arrow,
    read_cell_embedding_table_arrow_bytes, read_cell_embedding_table_arrow_from_store,
    scan_cell_embedding_table_arrow_bytes, scan_cell_embedding_table_arrow_from_store,
    validate_cell_embedding_row_link_arrow_bytes,
    validate_cell_embedding_row_link_arrow_from_store,
    validate_cell_patch_assignment_table_arrow_bytes,
    validate_cell_patch_assignment_table_arrow_from_store,
    validate_cell_patch_edge_table_arrow_bytes, validate_cell_patch_edge_table_arrow_from_store,
    validate_patch_footprint_set_arrow_bytes, validate_patch_footprint_set_arrow_from_store,
    validate_patch_overlap_graph_arrow_bytes, validate_patch_overlap_graph_arrow_from_store,
    validate_patch_region_link_arrow_bytes, validate_patch_region_link_arrow_from_store,
    verify_cell_embedding_row_link_arrow_bytes, verify_cell_embedding_row_link_arrow_from_store,
    verify_cell_embedding_table_arrow_bytes, verify_cell_embedding_table_arrow_from_store,
    verify_cell_patch_assignment_table_arrow_bytes,
    verify_cell_patch_assignment_table_arrow_from_store, verify_cell_patch_edge_table_arrow_bytes,
    verify_cell_patch_edge_table_arrow_from_store, verify_patch_footprint_set_arrow_bytes,
    verify_patch_footprint_set_arrow_from_store, verify_patch_overlap_graph_arrow_bytes,
    verify_patch_overlap_graph_arrow_from_store, verify_patch_region_link_arrow_bytes,
    verify_patch_region_link_arrow_from_store, write_cell_embedding_row_link_arrow,
    write_cell_embedding_table_arrow, write_cell_patch_assignment_table_arrow,
    write_cell_patch_edge_table_arrow, write_patch_footprint_set_arrow,
    write_patch_overlap_graph_arrow, write_patch_region_link_arrow, CellEmbeddingArrowPreflight,
    CellEmbeddingRowLinkArrowPreflight, CellPatchAssignmentArrowPreflight,
    CellPatchEdgeArrowPreflight, EmbeddingColumnarPublicationError,
    MultiscaleColumnarPublicationError, PatchFootprintArrowPreflight, PatchOverlapArrowPreflight,
    PatchRegionArrowPreflight,
};
pub use error::{ArrowIpcFailure, EmbeddingColumnarError, ParquetFailure};
pub use multiscale::{
    MultiscaleColumnarError, SpatialArrowFailure, SpatialColumnarWriteSummary,
    SpatialParquetFailure,
};
pub use parquet::{
    preflight_cell_embedding_row_link_parquet_bytes, preflight_cell_embedding_table_parquet_bytes,
    preflight_cell_patch_assignment_table_parquet_bytes,
    preflight_cell_patch_edge_table_parquet_bytes, preflight_patch_footprint_set_parquet_bytes,
    preflight_patch_overlap_graph_parquet_bytes, preflight_patch_region_link_parquet_bytes,
    publish_cell_embedding_row_link_parquet, publish_cell_embedding_table_parquet,
    publish_cell_patch_assignment_table_parquet, publish_cell_patch_edge_table_parquet,
    publish_patch_footprint_set_parquet, publish_patch_overlap_graph_parquet,
    publish_patch_region_link_parquet, read_cell_embedding_table_parquet_bytes,
    read_cell_embedding_table_parquet_from_store, scan_cell_embedding_table_parquet_bytes,
    scan_cell_embedding_table_parquet_from_store, validate_cell_embedding_row_link_parquet_bytes,
    validate_cell_embedding_row_link_parquet_from_store,
    validate_cell_patch_assignment_table_parquet_bytes,
    validate_cell_patch_assignment_table_parquet_from_store,
    validate_cell_patch_edge_table_parquet_bytes,
    validate_cell_patch_edge_table_parquet_from_store, validate_patch_footprint_set_parquet_bytes,
    validate_patch_footprint_set_parquet_from_store, validate_patch_overlap_graph_parquet_bytes,
    validate_patch_overlap_graph_parquet_from_store, validate_patch_region_link_parquet_bytes,
    validate_patch_region_link_parquet_from_store, verify_cell_embedding_row_link_parquet_bytes,
    verify_cell_embedding_row_link_parquet_from_store, verify_cell_embedding_table_parquet_bytes,
    verify_cell_embedding_table_parquet_from_store,
    verify_cell_patch_assignment_table_parquet_bytes,
    verify_cell_patch_assignment_table_parquet_from_store,
    verify_cell_patch_edge_table_parquet_bytes, verify_cell_patch_edge_table_parquet_from_store,
    verify_patch_footprint_set_parquet_bytes, verify_patch_footprint_set_parquet_from_store,
    verify_patch_overlap_graph_parquet_bytes, verify_patch_overlap_graph_parquet_from_store,
    verify_patch_region_link_parquet_bytes, verify_patch_region_link_parquet_from_store,
    write_cell_embedding_row_link_parquet, write_cell_embedding_table_parquet,
    write_cell_patch_assignment_table_parquet, write_cell_patch_edge_table_parquet,
    write_patch_footprint_set_parquet, write_patch_overlap_graph_parquet,
    write_patch_region_link_parquet, CellEmbeddingParquetPreflight,
    CellEmbeddingRowLinkParquetPreflight, CellPatchAssignmentParquetPreflight,
    CellPatchEdgeParquetPreflight, PatchFootprintParquetPreflight, PatchOverlapParquetPreflight,
    PatchRegionParquetPreflight,
};

use marklab_project::{ArtifactId, ContentDigest};

/// Caller-provided physical file, retained, row-group, and decoded-byte maxima.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmbeddingColumnarBudgets {
    maximum_file_bytes: u64,
    maximum_retained_bytes: usize,
    maximum_row_group_bytes: usize,
    maximum_decoded_bytes: u64,
}

impl EmbeddingColumnarBudgets {
    /// Declare maximum encoded file, retained, row-group, and decoded bytes.
    pub fn new(
        maximum_file_bytes: u64,
        maximum_retained_bytes: usize,
        maximum_row_group_bytes: usize,
        maximum_decoded_bytes: u64,
    ) -> Self {
        Self {
            maximum_file_bytes,
            maximum_retained_bytes,
            maximum_row_group_bytes,
            maximum_decoded_bytes,
        }
    }

    /// Maximum accepted or emitted encoded bytes.
    pub fn maximum_file_bytes(self) -> u64 {
        self.maximum_file_bytes
    }

    /// Maximum retained bytes for one physical operation.
    pub fn maximum_retained_bytes(self) -> usize {
        self.maximum_retained_bytes
    }

    /// Maximum retained bytes for one Arrow record batch or Parquet row group.
    pub fn maximum_row_group_bytes(self) -> usize {
        self.maximum_row_group_bytes
    }

    /// Maximum decoded value bytes for one operation.
    pub fn maximum_decoded_bytes(self) -> u64 {
        self.maximum_decoded_bytes
    }
}

/// Exact embedding-table artifact IDs and logical identities carried by physical metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellEmbeddingTablePhysicalBindings {
    expected_cells_artifact_id: ArtifactId,
    provenance_artifact_id: ArtifactId,
    row_link_artifact_id: ArtifactId,
    row_link_logical_digest: ContentDigest,
    table_logical_digest: ContentDigest,
}

impl CellEmbeddingTablePhysicalBindings {
    /// Bind distinct expected-cell, provenance, and row-link artifacts to both logical digests.
    pub fn new(
        expected_cells_artifact_id: ArtifactId,
        provenance_artifact_id: ArtifactId,
        row_link_artifact_id: ArtifactId,
        row_link_logical_digest: ContentDigest,
        table_logical_digest: ContentDigest,
    ) -> Result<Self, EmbeddingColumnarError> {
        if expected_cells_artifact_id == provenance_artifact_id
            || expected_cells_artifact_id == row_link_artifact_id
            || provenance_artifact_id == row_link_artifact_id
        {
            return Err(EmbeddingColumnarError::ArtifactBindingMismatch);
        }
        Ok(Self {
            expected_cells_artifact_id,
            provenance_artifact_id,
            row_link_artifact_id,
            row_link_logical_digest,
            table_logical_digest,
        })
    }

    /// Expected-cell artifact ID.
    pub fn expected_cells_artifact_id(self) -> ArtifactId {
        self.expected_cells_artifact_id
    }

    /// Provenance artifact ID.
    pub fn provenance_artifact_id(self) -> ArtifactId {
        self.provenance_artifact_id
    }

    /// Physical row-link artifact ID.
    pub fn row_link_artifact_id(self) -> ArtifactId {
        self.row_link_artifact_id
    }

    /// Logical row-link content identity used by the table digest.
    pub fn row_link_logical_digest(self) -> ContentDigest {
        self.row_link_logical_digest
    }

    /// Expected logical embedding-table identity.
    pub fn table_logical_digest(self) -> ContentDigest {
        self.table_logical_digest
    }
}

/// Exact encoded identity emitted by one canonical physical writer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ColumnarWriteSummary {
    content_digest: ContentDigest,
    encoded_byte_len: u64,
    row_count: u64,
    dimension: u32,
}

/// Exact encoded identity emitted by one canonical row-link writer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RowLinkColumnarWriteSummary {
    content_digest: ContentDigest,
    encoded_byte_len: u64,
    row_count: u64,
}

impl RowLinkColumnarWriteSummary {
    pub(crate) fn new(
        content_digest: ContentDigest,
        encoded_byte_len: u64,
        row_count: u64,
    ) -> Self {
        Self {
            content_digest,
            encoded_byte_len,
            row_count,
        }
    }

    /// SHA-256 of the exact encoded bytes.
    pub fn content_digest(self) -> ContentDigest {
        self.content_digest
    }

    /// Exact encoded byte length.
    pub fn encoded_byte_len(self) -> u64 {
        self.encoded_byte_len
    }

    /// Encoded canonical row-link rows.
    pub fn row_count(self) -> u64 {
        self.row_count
    }
}

impl ColumnarWriteSummary {
    pub(crate) fn new(
        content_digest: ContentDigest,
        encoded_byte_len: u64,
        row_count: u64,
        dimension: u32,
    ) -> Self {
        Self {
            content_digest,
            encoded_byte_len,
            row_count,
            dimension,
        }
    }

    /// SHA-256 of the exact encoded bytes.
    pub fn content_digest(self) -> ContentDigest {
        self.content_digest
    }

    /// Exact encoded byte length.
    pub fn encoded_byte_len(self) -> u64 {
        self.encoded_byte_len
    }

    /// Encoded canonical table rows.
    pub fn row_count(self) -> u64 {
        self.row_count
    }

    /// Encoded fixed vector dimension.
    pub fn dimension(self) -> u32 {
        self.dimension
    }
}
