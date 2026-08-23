use marklab_project::{ArtifactId, ContentDigest};

use crate::{EmbeddingError, EmbeddingQcSummary, VerifiedCellEmbeddingArtifactGraph};

/// Exact scalar dtype for a canonical cell-embedding matrix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmbeddingDtype {
    /// IEEE-754 single-precision components.
    F32,
}

/// Unforgeable receipt for one fully decoded and validated physical embedding table.
///
/// Values can be obtained only from the format-specific `verify_*` APIs after raw
/// preflight, stock decoding, logical-digest validation, and artifact binding succeed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedCellEmbeddingTableArtifact {
    embedding_artifact_id: ArtifactId,
    expected_cells_artifact_id: ArtifactId,
    row_link_artifact_id: ArtifactId,
    row_link_logical_digest: ContentDigest,
    provenance_artifact_id: ArtifactId,
    qc_summary: EmbeddingQcSummary,
}

impl VerifiedCellEmbeddingTableArtifact {
    #[cfg(feature = "parquet")]
    pub(crate) fn new(
        embedding_artifact_id: ArtifactId,
        graph: VerifiedCellEmbeddingArtifactGraph,
        qc_summary: EmbeddingQcSummary,
    ) -> Self {
        Self {
            embedding_artifact_id,
            expected_cells_artifact_id: graph.expected_cells_artifact_id,
            row_link_artifact_id: graph.row_link_artifact_id,
            row_link_logical_digest: graph.row_link_logical_digest,
            provenance_artifact_id: graph.provenance_artifact_id,
            qc_summary,
        }
    }

    /// Exact embedding-table artifact identity whose physical content was validated.
    pub fn embedding_artifact_id(self) -> ArtifactId {
        self.embedding_artifact_id
    }

    /// Recomputed factual QC and format-independent logical identity.
    pub fn qc_summary(self) -> EmbeddingQcSummary {
        self.qc_summary
    }
}

/// Unforgeable receipt for one fully decoded and validated physical row-link table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedCellEmbeddingRowLinkArtifact {
    row_link_artifact_id: ArtifactId,
    expected_cells_artifact_id: ArtifactId,
    expected_cells_logical_digest: ContentDigest,
    row_link_logical_digest: ContentDigest,
    row_count: u64,
}

impl VerifiedCellEmbeddingRowLinkArtifact {
    #[cfg(feature = "parquet")]
    pub(crate) fn new(
        row_link_artifact_id: ArtifactId,
        row_link: &crate::CellEmbeddingRowLink,
    ) -> Self {
        Self {
            row_link_artifact_id,
            expected_cells_artifact_id: row_link.expected_cells_artifact_id(),
            expected_cells_logical_digest: row_link.expected_cells_logical_digest(),
            row_link_logical_digest: row_link.logical_digest(),
            row_count: row_link.row_count(),
        }
    }

    /// Exact row-link artifact identity whose physical content was validated.
    pub fn row_link_artifact_id(self) -> ArtifactId {
        self.row_link_artifact_id
    }

    /// Exact validated row count.
    pub fn row_count(self) -> u64 {
        self.row_count
    }
}

/// Compact in-memory binding for one verified physical embedding artifact.
///
/// This contains project metadata only; it never owns the matrix or per-row vectors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellEmbeddingArtifact {
    embedding_artifact_id: ArtifactId,
    row_link_artifact_id: ArtifactId,
    provenance_artifact_id: ArtifactId,
    row_count: u64,
    dimension: u32,
    dtype: EmbeddingDtype,
    logical_digest: ContentDigest,
    qc_summary: EmbeddingQcSummary,
}

impl CellEmbeddingArtifact {
    /// Bind exact verified embedding and row-link receipts to a verified provenance graph.
    pub fn new(
        embedding: VerifiedCellEmbeddingTableArtifact,
        row_link: VerifiedCellEmbeddingRowLinkArtifact,
        graph: VerifiedCellEmbeddingArtifactGraph,
    ) -> Result<Self, EmbeddingError> {
        let qc_summary = embedding.qc_summary;
        let status_count = qc_summary
            .present_count()
            .checked_add(qc_summary.missing_vector_count())
            .and_then(|count| count.checked_add(qc_summary.extraction_failed_count()))
            .and_then(|count| count.checked_add(qc_summary.qc_rejected_count()))
            .ok_or(EmbeddingError::SizeOverflow)?;
        if graph.dependency_count != 13
            || embedding.embedding_artifact_id == embedding.expected_cells_artifact_id
            || embedding.embedding_artifact_id == embedding.row_link_artifact_id
            || embedding.embedding_artifact_id == embedding.provenance_artifact_id
            || embedding.expected_cells_artifact_id != graph.expected_cells_artifact_id
            || embedding.row_link_artifact_id != graph.row_link_artifact_id
            || embedding.row_link_logical_digest != graph.row_link_logical_digest
            || embedding.provenance_artifact_id != graph.provenance_artifact_id
            || row_link.row_link_artifact_id != graph.row_link_artifact_id
            || row_link.expected_cells_artifact_id != graph.expected_cells_artifact_id
            || row_link.expected_cells_logical_digest != graph.expected_cells_logical_digest
            || row_link.row_link_logical_digest != graph.row_link_logical_digest
            || row_link.row_count != qc_summary.row_count()
            || qc_summary.dimension() != graph.output_dimension
            || status_count != qc_summary.row_count()
        {
            return Err(EmbeddingError::ArtifactBindingMismatch);
        }
        Ok(Self {
            embedding_artifact_id: embedding.embedding_artifact_id,
            row_link_artifact_id: row_link.row_link_artifact_id,
            provenance_artifact_id: graph.provenance_artifact_id,
            row_count: qc_summary.row_count(),
            dimension: qc_summary.dimension(),
            dtype: EmbeddingDtype::F32,
            logical_digest: qc_summary.logical_digest(),
            qc_summary,
        })
    }

    /// Physical embedding artifact identity.
    pub fn embedding_artifact_id(self) -> ArtifactId {
        self.embedding_artifact_id
    }

    /// Physical row-link artifact identity.
    pub fn row_link_artifact_id(self) -> ArtifactId {
        self.row_link_artifact_id
    }

    /// Canonical provenance artifact identity.
    pub fn provenance_artifact_id(self) -> ArtifactId {
        self.provenance_artifact_id
    }

    /// Exact canonical row count.
    pub fn row_count(self) -> u64 {
        self.row_count
    }

    /// Exact fixed vector dimension.
    pub fn dimension(self) -> u32 {
        self.dimension
    }

    /// Exact component dtype.
    pub fn dtype(self) -> EmbeddingDtype {
        self.dtype
    }

    /// Format-independent logical table identity.
    pub fn logical_digest(self) -> ContentDigest {
        self.logical_digest
    }

    /// Factual QC summary bound into this metadata.
    pub fn qc_summary(self) -> EmbeddingQcSummary {
        self.qc_summary
    }
}
