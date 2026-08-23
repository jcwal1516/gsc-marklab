use std::fmt;

use marklab_data::CellId;
use marklab_project::{ArtifactId, ContentDigest};

use crate::{digest::canonical_positive_zero, EmbeddingError, ExpectedCellSet};

mod scan;
pub use scan::CellEmbeddingBlock;
pub(crate) use scan::EmbeddingQcAccumulator;

const LOGICAL_DIGEST_DOMAIN: &[u8] = b"marklab-cell-embedding-logical-v1";

/// Closed extraction-validity state for one embedding-entity row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmbeddingStatus {
    /// A finite vector is present and meaningful.
    Present,
    /// The source explicitly has no vector for this embedding entity.
    MissingVector,
    /// Extraction failed and produced no vector.
    ExtractionFailed,
    /// A source vector existed but was rejected by declared QC.
    QcRejected,
}

impl EmbeddingStatus {
    pub(crate) fn wire_name(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::MissingVector => "missing_vector",
            Self::ExtractionFailed => "extraction_failed",
            Self::QcRejected => "qc_rejected",
        }
    }
}

/// Owned construction row for a canonical cell-embedding table.
#[derive(Clone, Debug, PartialEq)]
pub struct CellEmbeddingRow {
    cell_id: CellId,
    status: EmbeddingStatus,
    vector: Option<Vec<f32>>,
}

impl CellEmbeddingRow {
    /// Construct a present row; dimension and finiteness are checked by the table.
    pub fn present(cell_id: CellId, vector: Vec<f32>) -> Self {
        Self {
            cell_id,
            status: EmbeddingStatus::Present,
            vector: Some(vector),
        }
    }

    /// Construct an explicitly non-present row without a usable vector.
    pub fn non_present(cell_id: CellId, status: EmbeddingStatus) -> Result<Self, EmbeddingError> {
        if status == EmbeddingStatus::Present {
            return Err(EmbeddingError::StatusVectorMismatch);
        }
        Ok(Self {
            cell_id,
            status,
            vector: None,
        })
    }
}

/// Borrowed status-aware view that never exposes private filler components.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellEmbeddingView<'a> {
    cell_id: &'a CellId,
    status: EmbeddingStatus,
    vector: Option<&'a [f32]>,
}

impl<'a> CellEmbeddingView<'a> {
    /// Canonical cell identity for this row.
    pub fn cell_id(self) -> &'a CellId {
        self.cell_id
    }

    /// Explicit extraction-validity state.
    pub fn status(self) -> EmbeddingStatus {
        self.status
    }

    /// Present vector, or `None` for every non-present state.
    pub fn vector(self) -> Option<&'a [f32]> {
        self.vector
    }
}

/// Factual integrity summary for one canonical table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmbeddingQcSummary {
    row_count: u64,
    present_count: u64,
    missing_vector_count: u64,
    extraction_failed_count: u64,
    qc_rejected_count: u64,
    all_zero_present_count: u64,
    dimension: u32,
    logical_digest: ContentDigest,
}

impl EmbeddingQcSummary {
    /// Total canonical rows.
    pub fn row_count(self) -> u64 {
        self.row_count
    }

    /// Rows carrying meaningful present vectors.
    pub fn present_count(self) -> u64 {
        self.present_count
    }

    /// Rows explicitly missing a source vector.
    pub fn missing_vector_count(self) -> u64 {
        self.missing_vector_count
    }

    /// Rows whose extraction failed.
    pub fn extraction_failed_count(self) -> u64 {
        self.extraction_failed_count
    }

    /// Rows rejected by declared embedding QC.
    pub fn qc_rejected_count(self) -> u64 {
        self.qc_rejected_count
    }

    /// Present vectors whose every canonical component is positive zero.
    pub fn all_zero_present_count(self) -> u64 {
        self.all_zero_present_count
    }

    /// Fixed vector dimension.
    pub fn dimension(self) -> u32 {
        self.dimension
    }

    /// Logical content digest independent of physical format and chunking.
    pub fn logical_digest(self) -> ContentDigest {
        self.logical_digest
    }
}

/// Canonical row-major cell-embedding table with status-owned validity.
#[derive(Clone, PartialEq)]
pub struct CellEmbeddingTable {
    cells: Box<[CellId]>,
    statuses: Box<[EmbeddingStatus]>,
    values: Vec<f32>,
    dimension: u32,
    qc_summary: EmbeddingQcSummary,
    expected_cells_artifact_id: ArtifactId,
    provenance_artifact_id: ArtifactId,
    row_link_digest: ContentDigest,
}

impl fmt::Debug for CellEmbeddingTable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CellEmbeddingTable")
            .field("row_count", &self.cells.len())
            .field("dimension", &self.dimension)
            .field("qc_summary", &self.qc_summary)
            .field(
                "expected_cells_artifact_id",
                &self.expected_cells_artifact_id,
            )
            .field("provenance_artifact_id", &self.provenance_artifact_id)
            .field("row_link_digest", &self.row_link_digest)
            .finish()
    }
}

impl CellEmbeddingTable {
    /// Validate exact expected rows and materialize one checked contiguous matrix.
    #[allow(clippy::too_many_arguments)]
    pub fn from_rows(
        dimension: u32,
        expected: &ExpectedCellSet,
        expected_cells_artifact_id: ArtifactId,
        provenance_artifact_id: ArtifactId,
        row_link_digest: ContentDigest,
        rows: Vec<CellEmbeddingRow>,
        maximum_retained_bytes: usize,
    ) -> Result<Self, EmbeddingError> {
        if dimension == 0 {
            return Err(EmbeddingError::ZeroDimension);
        }
        if rows.len() != expected.cells().len()
            || rows
                .iter()
                .zip(expected.cells())
                .any(|(row, cell)| &row.cell_id != cell)
        {
            return Err(EmbeddingError::RowSetMismatch);
        }
        let dimension_usize =
            usize::try_from(dimension).map_err(|_| EmbeddingError::SizeOverflow)?;
        let component_count = rows
            .len()
            .checked_mul(dimension_usize)
            .ok_or(EmbeddingError::SizeOverflow)?;
        let value_bytes = component_count
            .checked_mul(size_of::<f32>())
            .ok_or(EmbeddingError::SizeOverflow)?;
        let row_overhead = rows
            .len()
            .checked_mul(size_of::<CellId>() + size_of::<EmbeddingStatus>())
            .ok_or(EmbeddingError::SizeOverflow)?;
        let identifier_bytes = expected.cells().iter().try_fold(0_usize, |total, cell| {
            total
                .checked_add(cell.as_str().len())
                .ok_or(EmbeddingError::SizeOverflow)
        })?;
        let required = value_bytes
            .checked_add(row_overhead)
            .and_then(|value| value.checked_add(identifier_bytes))
            .ok_or(EmbeddingError::SizeOverflow)?;
        if required > maximum_retained_bytes {
            return Err(EmbeddingError::RetainedByteBudgetExceeded {
                required,
                maximum: maximum_retained_bytes,
            });
        }

        let mut values = Vec::new();
        values.try_reserve_exact(component_count).map_err(|_| {
            EmbeddingError::AllocationFailed {
                requested: value_bytes,
            }
        })?;
        let mut statuses = Vec::new();
        statuses
            .try_reserve_exact(rows.len())
            .map_err(|_| EmbeddingError::AllocationFailed {
                requested: rows.len().saturating_mul(size_of::<EmbeddingStatus>()),
            })?;
        let mut accumulator = EmbeddingQcAccumulator::new(
            expected_cells_artifact_id,
            provenance_artifact_id,
            row_link_digest,
            dimension,
            expected.cells().len(),
        )?;
        for (row_index, row) in rows.into_iter().enumerate() {
            let CellEmbeddingRow {
                cell_id,
                status,
                vector,
            } = row;
            statuses.push(status);
            match (status, vector) {
                (EmbeddingStatus::Present, Some(vector)) => {
                    if vector.len() != dimension_usize {
                        return Err(EmbeddingError::DimensionMismatch {
                            expected: dimension_usize,
                            observed: vector.len(),
                        });
                    }
                    let start = values.len();
                    for (column, value) in vector.into_iter().enumerate() {
                        if !value.is_finite() {
                            return Err(EmbeddingError::NonFiniteComponent {
                                row: row_index,
                                column,
                            });
                        }
                        let value = canonical_positive_zero(value);
                        values.push(value);
                    }
                    accumulator.push(&cell_id, status, Some(&values[start..]))?;
                }
                (EmbeddingStatus::MissingVector, None) => {
                    values.resize(values.len() + dimension_usize, 0.0);
                    accumulator.push(&cell_id, status, None)?;
                }
                (EmbeddingStatus::ExtractionFailed, None) => {
                    values.resize(values.len() + dimension_usize, 0.0);
                    accumulator.push(&cell_id, status, None)?;
                }
                (EmbeddingStatus::QcRejected, None) => {
                    values.resize(values.len() + dimension_usize, 0.0);
                    accumulator.push(&cell_id, status, None)?;
                }
                _ => return Err(EmbeddingError::StatusVectorMismatch),
            }
        }
        let qc_summary = accumulator.finish()?;
        Ok(Self {
            cells: clone_cells(expected.cells())?,
            statuses: statuses.into_boxed_slice(),
            values,
            dimension,
            qc_summary,
            expected_cells_artifact_id,
            provenance_artifact_id,
            row_link_digest,
        })
    }

    /// Validate one row-major contiguous matrix whose every expected row is present.
    ///
    /// Values are consumed without constructing per-row vectors. Every component must
    /// be finite; signed zero is canonicalized to positive zero.
    #[allow(clippy::too_many_arguments)]
    pub fn from_present_values(
        dimension: u32,
        expected: &ExpectedCellSet,
        expected_cells_artifact_id: ArtifactId,
        provenance_artifact_id: ArtifactId,
        row_link_digest: ContentDigest,
        mut values: Vec<f32>,
        maximum_retained_bytes: usize,
    ) -> Result<Self, EmbeddingError> {
        if dimension == 0 {
            return Err(EmbeddingError::ZeroDimension);
        }
        let dimension_usize =
            usize::try_from(dimension).map_err(|_| EmbeddingError::SizeOverflow)?;
        let component_count = expected
            .cells()
            .len()
            .checked_mul(dimension_usize)
            .ok_or(EmbeddingError::SizeOverflow)?;
        if values.len() != component_count {
            return Err(EmbeddingError::DimensionMismatch {
                expected: component_count,
                observed: values.len(),
            });
        }
        let value_bytes = component_count
            .checked_mul(size_of::<f32>())
            .ok_or(EmbeddingError::SizeOverflow)?;
        let row_overhead = expected
            .cells()
            .len()
            .checked_mul(size_of::<CellId>() + size_of::<EmbeddingStatus>())
            .ok_or(EmbeddingError::SizeOverflow)?;
        let identifier_bytes = expected.cells().iter().try_fold(0_usize, |total, cell| {
            total
                .checked_add(cell.as_str().len())
                .ok_or(EmbeddingError::SizeOverflow)
        })?;
        let required = value_bytes
            .checked_add(row_overhead)
            .and_then(|value| value.checked_add(identifier_bytes))
            .ok_or(EmbeddingError::SizeOverflow)?;
        if required > maximum_retained_bytes {
            return Err(EmbeddingError::RetainedByteBudgetExceeded {
                required,
                maximum: maximum_retained_bytes,
            });
        }
        let mut accumulator = EmbeddingQcAccumulator::new(
            expected_cells_artifact_id,
            provenance_artifact_id,
            row_link_digest,
            dimension,
            expected.cells().len(),
        )?;
        for (row, (cell_id, vector)) in expected
            .cells()
            .iter()
            .zip(values.chunks_exact_mut(dimension_usize))
            .enumerate()
        {
            for (column, value) in vector.iter_mut().enumerate() {
                if !value.is_finite() {
                    return Err(EmbeddingError::NonFiniteComponent { row, column });
                }
                *value = canonical_positive_zero(*value);
            }
            accumulator.push(cell_id, EmbeddingStatus::Present, Some(vector))?;
        }
        let mut statuses = Vec::new();
        statuses
            .try_reserve_exact(expected.cells().len())
            .map_err(|_| EmbeddingError::AllocationFailed {
                requested: expected
                    .cells()
                    .len()
                    .saturating_mul(size_of::<EmbeddingStatus>()),
            })?;
        statuses.resize(expected.cells().len(), EmbeddingStatus::Present);
        let qc_summary = accumulator.finish()?;
        Ok(Self {
            cells: clone_cells(expected.cells())?,
            statuses: statuses.into_boxed_slice(),
            values,
            dimension,
            qc_summary,
            expected_cells_artifact_id,
            provenance_artifact_id,
            row_link_digest,
        })
    }

    #[cfg(feature = "parquet")]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_physical_values(
        dimension: u32,
        expected: &ExpectedCellSet,
        expected_cells_artifact_id: ArtifactId,
        provenance_artifact_id: ArtifactId,
        row_link_digest: ContentDigest,
        statuses: Vec<EmbeddingStatus>,
        mut values: Vec<f32>,
        maximum_retained_bytes: usize,
    ) -> Result<Self, EmbeddingError> {
        if dimension == 0 {
            return Err(EmbeddingError::ZeroDimension);
        }
        if statuses.len() != expected.cells().len() {
            return Err(EmbeddingError::RowSetMismatch);
        }
        let dimension_usize =
            usize::try_from(dimension).map_err(|_| EmbeddingError::SizeOverflow)?;
        let component_count = expected
            .cells()
            .len()
            .checked_mul(dimension_usize)
            .ok_or(EmbeddingError::SizeOverflow)?;
        if values.len() != component_count {
            return Err(EmbeddingError::DimensionMismatch {
                expected: component_count,
                observed: values.len(),
            });
        }
        let value_bytes = component_count
            .checked_mul(size_of::<f32>())
            .ok_or(EmbeddingError::SizeOverflow)?;
        let row_overhead = expected
            .cells()
            .len()
            .checked_mul(size_of::<CellId>() + size_of::<EmbeddingStatus>())
            .ok_or(EmbeddingError::SizeOverflow)?;
        let identifier_bytes = expected.cells().iter().try_fold(0_usize, |total, cell| {
            total
                .checked_add(cell.as_str().len())
                .ok_or(EmbeddingError::SizeOverflow)
        })?;
        let required = value_bytes
            .checked_add(row_overhead)
            .and_then(|value| value.checked_add(identifier_bytes))
            .ok_or(EmbeddingError::SizeOverflow)?;
        if required > maximum_retained_bytes {
            return Err(EmbeddingError::RetainedByteBudgetExceeded {
                required,
                maximum: maximum_retained_bytes,
            });
        }

        let mut accumulator = EmbeddingQcAccumulator::new(
            expected_cells_artifact_id,
            provenance_artifact_id,
            row_link_digest,
            dimension,
            expected.cells().len(),
        )?;
        for (row, ((cell_id, status), vector)) in expected
            .cells()
            .iter()
            .zip(statuses.iter().copied())
            .zip(values.chunks_exact_mut(dimension_usize))
            .enumerate()
        {
            for (column, value) in vector.iter_mut().enumerate() {
                if !value.is_finite() {
                    return Err(EmbeddingError::NonFiniteComponent { row, column });
                }
                if status == EmbeddingStatus::Present {
                    *value = canonical_positive_zero(*value);
                } else if value.to_bits() != 0 {
                    return Err(EmbeddingError::StatusVectorMismatch);
                }
            }
            accumulator.push(
                cell_id,
                status,
                (status == EmbeddingStatus::Present).then_some(&*vector),
            )?;
        }
        let qc_summary = accumulator.finish()?;
        Ok(Self {
            cells: clone_cells(expected.cells())?,
            statuses: statuses.into_boxed_slice(),
            values,
            dimension,
            qc_summary,
            expected_cells_artifact_id,
            provenance_artifact_id,
            row_link_digest,
        })
    }

    /// Number of canonical rows.
    pub fn row_count(&self) -> usize {
        self.cells.len()
    }

    /// Fixed vector dimension.
    pub fn dimension(&self) -> u32 {
        self.dimension
    }

    /// Status-aware row view that hides every non-present filler.
    pub fn row(&self, index: usize) -> Result<CellEmbeddingView<'_>, EmbeddingError> {
        let cell_id = self
            .cells
            .get(index)
            .ok_or(EmbeddingError::RowOutOfBounds {
                index,
                row_count: self.cells.len(),
            })?;
        let status = self.statuses[index];
        let vector = if status == EmbeddingStatus::Present {
            let dimension = self.dimension as usize;
            let start = index * dimension;
            Some(&self.values[start..start + dimension])
        } else {
            None
        };
        Ok(CellEmbeddingView {
            cell_id,
            status,
            vector,
        })
    }

    /// Factual status counts, shape, zero-vector count, and logical digest.
    pub fn qc_summary(&self) -> EmbeddingQcSummary {
        self.qc_summary
    }

    #[cfg(feature = "parquet")]
    pub(crate) fn matches_physical_bindings(
        &self,
        expected_cells_artifact_id: ArtifactId,
        provenance_artifact_id: ArtifactId,
        row_link_digest: ContentDigest,
    ) -> bool {
        self.expected_cells_artifact_id == expected_cells_artifact_id
            && self.provenance_artifact_id == provenance_artifact_id
            && self.row_link_digest == row_link_digest
    }
}

fn clone_cells(cells: &[CellId]) -> Result<Box<[CellId]>, EmbeddingError> {
    let requested = cells
        .len()
        .checked_mul(size_of::<CellId>())
        .ok_or(EmbeddingError::SizeOverflow)?;
    let mut cloned = Vec::new();
    cloned
        .try_reserve_exact(cells.len())
        .map_err(|_| EmbeddingError::AllocationFailed { requested })?;
    cloned.extend(cells.iter().cloned());
    Ok(cloned.into_boxed_slice())
}
