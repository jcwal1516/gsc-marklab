use marklab_data::{CellId, CohortHierarchy, HierarchyId};
use marklab_project::{ArtifactId, ContentDigest};

use crate::{digest::FramedDigest, EmbeddingError, EmbeddingStatus, ExpectedCellSet};

const DIGEST_DOMAIN: &[u8] = b"marklab-cell-embedding-row-link-v1";
const PRESENT_MARKER: &[u8] = b"embedding-row-present";
const ABSENT_MARKER: &[u8] = b"embedding-row-absent";
const MAX_ROW_COUNT: usize = 100_000_000;

/// One canonical cell's explicit source-row correspondence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CellEmbeddingRowLinkEntry {
    cell_id: CellId,
    status: EmbeddingStatus,
    source_cell_row: u64,
    source_embedding_row: Option<u64>,
}

impl CellEmbeddingRowLinkEntry {
    /// Validate one source-row link against the closed status/index rule.
    pub fn new(
        cell_id: CellId,
        status: EmbeddingStatus,
        source_cell_row: u64,
        source_embedding_row: Option<u64>,
    ) -> Result<Self, EmbeddingError> {
        if matches!(
            status,
            EmbeddingStatus::Present | EmbeddingStatus::QcRejected
        ) != source_embedding_row.is_some()
        {
            return Err(EmbeddingError::StatusEmbeddingRowMismatch);
        }
        Ok(Self {
            cell_id,
            status,
            source_cell_row,
            source_embedding_row,
        })
    }

    /// Link a cell to a source row carrying an accepted vector.
    pub fn present(cell_id: CellId, source_cell_row: u64, source_embedding_row: u64) -> Self {
        Self {
            cell_id,
            status: EmbeddingStatus::Present,
            source_cell_row,
            source_embedding_row: Some(source_embedding_row),
        }
    }

    /// Link a cell whose source explicitly has no vector row.
    pub fn missing_vector(cell_id: CellId, source_cell_row: u64) -> Self {
        Self {
            cell_id,
            status: EmbeddingStatus::MissingVector,
            source_cell_row,
            source_embedding_row: None,
        }
    }

    /// Link a cell whose extraction failed before producing a vector row.
    pub fn extraction_failed(cell_id: CellId, source_cell_row: u64) -> Self {
        Self {
            cell_id,
            status: EmbeddingStatus::ExtractionFailed,
            source_cell_row,
            source_embedding_row: None,
        }
    }

    /// Link a cell to a source vector row that explicit QC rejected.
    pub fn qc_rejected(cell_id: CellId, source_cell_row: u64, source_embedding_row: u64) -> Self {
        Self {
            cell_id,
            status: EmbeddingStatus::QcRejected,
            source_cell_row,
            source_embedding_row: Some(source_embedding_row),
        }
    }

    /// Canonical cell identity.
    pub fn cell_id(&self) -> &CellId {
        &self.cell_id
    }

    /// Closed extraction status governing embedding-row presence.
    pub fn status(&self) -> EmbeddingStatus {
        self.status
    }

    /// Explicit zero-based source-cell row.
    pub fn source_cell_row(&self) -> u64 {
        self.source_cell_row
    }

    /// Explicit zero-based source-vector row, when one existed.
    pub fn source_embedding_row(&self) -> Option<u64> {
        self.source_embedding_row
    }
}

/// Immutable exact correspondence between canonical cells and source rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CellEmbeddingRowLink {
    source_cells_artifact_id: ArtifactId,
    source_vectors_artifact_id: ArtifactId,
    expected_cells_artifact_id: ArtifactId,
    identity_map_artifact_id: ArtifactId,
    converter_artifact_id: ArtifactId,
    expected_cells_logical_digest: ContentDigest,
    entries: Box<[CellEmbeddingRowLinkEntry]>,
    row_count: u64,
    logical_digest: ContentDigest,
}

impl CellEmbeddingRowLink {
    /// Validate exact cells, hierarchy membership, row domains, dependencies, and retained bytes.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_cells_artifact_id: ArtifactId,
        source_vectors_artifact_id: ArtifactId,
        expected_cells_artifact_id: ArtifactId,
        identity_map_artifact_id: ArtifactId,
        converter_artifact_id: ArtifactId,
        expected: &ExpectedCellSet,
        hierarchy: &CohortHierarchy,
        mut entries: Vec<CellEmbeddingRowLinkEntry>,
        maximum_retained_bytes: usize,
    ) -> Result<Self, EmbeddingError> {
        if entries.len() > MAX_ROW_COUNT {
            return Err(EmbeddingError::RowCountExceeded {
                observed: entries.len(),
                maximum: MAX_ROW_COUNT,
            });
        }
        let entry_bytes = entries
            .len()
            .checked_mul(size_of::<CellEmbeddingRowLinkEntry>())
            .ok_or(EmbeddingError::SizeOverflow)?;
        let identifier_bytes = entries.iter().try_fold(0_usize, |total, entry| {
            total
                .checked_add(entry.cell_id.as_str().len())
                .ok_or(EmbeddingError::SizeOverflow)
        })?;
        let required = entry_bytes
            .checked_add(identifier_bytes)
            .ok_or(EmbeddingError::SizeOverflow)?;
        if required > maximum_retained_bytes {
            return Err(EmbeddingError::RetainedByteBudgetExceeded {
                required,
                maximum: maximum_retained_bytes,
            });
        }
        if entries.len() != expected.cells().len()
            || entries
                .iter()
                .zip(expected.cells())
                .any(|(entry, expected_cell)| &entry.cell_id != expected_cell)
        {
            return Err(EmbeddingError::RowLinkSetMismatch);
        }
        if entries.iter().any(|entry| {
            hierarchy
                .node(&HierarchyId::from(entry.cell_id.clone()))
                .is_none()
        }) {
            return Err(EmbeddingError::RowLinkCellMissingFromHierarchy);
        }
        if entries.iter().any(|entry| {
            matches!(
                entry.status,
                EmbeddingStatus::Present | EmbeddingStatus::QcRejected
            ) != entry.source_embedding_row.is_some()
        }) {
            return Err(EmbeddingError::StatusEmbeddingRowMismatch);
        }
        entries.sort_unstable_by_key(|entry| entry.source_cell_row);
        if entries.iter().enumerate().any(|(expected_row, entry)| {
            u64::try_from(expected_row).ok() != Some(entry.source_cell_row)
        }) {
            return Err(EmbeddingError::InvalidSourceCellRows);
        }
        entries.sort_unstable_by_key(|entry| entry.source_embedding_row);
        if entries
            .iter()
            .filter_map(|entry| entry.source_embedding_row)
            .enumerate()
            .any(|(expected_row, observed_row)| {
                u64::try_from(expected_row).ok() != Some(observed_row)
            })
        {
            return Err(EmbeddingError::InvalidSourceEmbeddingRows);
        }
        entries.sort_unstable_by(|left, right| left.cell_id.cmp(&right.cell_id));
        let dependencies = [
            source_cells_artifact_id,
            source_vectors_artifact_id,
            expected_cells_artifact_id,
            identity_map_artifact_id,
            converter_artifact_id,
        ];
        if !all_distinct(dependencies) {
            return Err(EmbeddingError::DuplicateArtifactDependency);
        }
        let count = u64::try_from(entries.len()).map_err(|_| EmbeddingError::SizeOverflow)?;
        let logical_digest = digest(dependencies, count, &entries);
        Ok(Self {
            source_cells_artifact_id,
            source_vectors_artifact_id,
            expected_cells_artifact_id,
            identity_map_artifact_id,
            converter_artifact_id,
            expected_cells_logical_digest: expected.logical_digest(),
            entries: entries.into_boxed_slice(),
            row_count: count,
            logical_digest,
        })
    }

    /// Source-cell artifact dependency.
    pub fn source_cells_artifact_id(&self) -> ArtifactId {
        self.source_cells_artifact_id
    }

    /// Source-vector artifact dependency.
    pub fn source_vectors_artifact_id(&self) -> ArtifactId {
        self.source_vectors_artifact_id
    }

    /// Expected-cell artifact dependency.
    pub fn expected_cells_artifact_id(&self) -> ArtifactId {
        self.expected_cells_artifact_id
    }

    /// Explicit source-to-canonical identity-map dependency.
    pub fn identity_map_artifact_id(&self) -> ArtifactId {
        self.identity_map_artifact_id
    }

    /// Converter-manifest artifact dependency.
    pub fn converter_artifact_id(&self) -> ArtifactId {
        self.converter_artifact_id
    }

    /// Logical expected-set identity validated when this link was constructed.
    pub fn expected_cells_logical_digest(&self) -> ContentDigest {
        self.expected_cells_logical_digest
    }

    /// Canonically ordered linkage entries.
    pub fn entries(&self) -> &[CellEmbeddingRowLinkEntry] {
        &self.entries
    }

    /// Number of linked canonical cells.
    pub fn row_count(&self) -> u64 {
        self.row_count
    }

    /// Sorted exact direct dependency set.
    pub fn direct_dependencies(&self) -> [ArtifactId; 5] {
        let mut dependencies = [
            self.source_cells_artifact_id,
            self.source_vectors_artifact_id,
            self.expected_cells_artifact_id,
            self.identity_map_artifact_id,
            self.converter_artifact_id,
        ];
        dependencies.sort_unstable();
        dependencies
    }

    /// Domain-separated logical digest over dependencies and exact row correspondence.
    pub fn logical_digest(&self) -> ContentDigest {
        self.logical_digest
    }
}

fn all_distinct<const N: usize>(mut values: [ArtifactId; N]) -> bool {
    values.sort_unstable();
    values.windows(2).all(|pair| pair[0] != pair[1])
}

fn digest(
    dependencies: [ArtifactId; 5],
    count: u64,
    entries: &[CellEmbeddingRowLinkEntry],
) -> ContentDigest {
    let mut digest = FramedDigest::new();
    digest.field(DIGEST_DOMAIN);
    for dependency in dependencies {
        digest.field(dependency.digest().as_bytes());
    }
    digest.field(&count.to_be_bytes());
    for entry in entries {
        digest.field(entry.cell_id.as_str().as_bytes());
        digest.field(&entry.source_cell_row.to_be_bytes());
        match entry.source_embedding_row {
            Some(row) => {
                digest.field(PRESENT_MARKER);
                digest.field(&row.to_be_bytes());
            }
            None => digest.field(ABSENT_MARKER),
        }
    }
    digest.finish()
}
