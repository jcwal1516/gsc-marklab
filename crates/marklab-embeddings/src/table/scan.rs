use std::fmt;

use marklab_data::CellId;
use marklab_project::{ArtifactId, ContentDigest};

use crate::{digest::FramedDigest, EmbeddingError};

use super::{
    CellEmbeddingTable, CellEmbeddingView, EmbeddingQcSummary, EmbeddingStatus,
    LOGICAL_DIGEST_DOMAIN,
};

/// Borrowed contiguous block whose status-aware rows never expose private fillers.
#[derive(Clone, Copy)]
pub struct CellEmbeddingBlock<'a> {
    cells: &'a [CellId],
    statuses: &'a [EmbeddingStatus],
    values: &'a [f32],
    dimension: u32,
}

impl fmt::Debug for CellEmbeddingBlock<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CellEmbeddingBlock")
            .field("row_count", &self.cells.len())
            .field("dimension", &self.dimension)
            .finish()
    }
}

impl<'a> CellEmbeddingBlock<'a> {
    /// Number of rows in this block.
    pub fn row_count(self) -> usize {
        self.cells.len()
    }

    /// Fixed vector dimension shared by every row.
    pub fn dimension(self) -> u32 {
        self.dimension
    }

    /// Return one status-aware row relative to this block.
    pub fn row(self, index: usize) -> Result<CellEmbeddingView<'a>, EmbeddingError> {
        let cell_id = self
            .cells
            .get(index)
            .ok_or(EmbeddingError::RowOutOfBounds {
                index,
                row_count: self.cells.len(),
            })?;
        Ok(self.view(index, cell_id))
    }

    /// Iterate every status-aware row in canonical order.
    pub fn rows(self) -> impl ExactSizeIterator<Item = CellEmbeddingView<'a>> + 'a {
        self.cells
            .iter()
            .enumerate()
            .map(move |(index, cell_id)| self.view(index, cell_id))
    }

    fn view(self, index: usize, cell_id: &'a CellId) -> CellEmbeddingView<'a> {
        let status = self.statuses[index];
        let vector = if status == EmbeddingStatus::Present {
            let dimension = self.dimension as usize;
            let start = index * dimension;
            Some(&self.values[start..start + dimension])
        } else {
            None
        };
        CellEmbeddingView {
            cell_id,
            status,
            vector,
        }
    }
}

impl CellEmbeddingTable {
    /// Borrow a checked contiguous row block without exposing non-present fillers.
    pub fn block(
        &self,
        start: usize,
        row_count: usize,
    ) -> Result<CellEmbeddingBlock<'_>, EmbeddingError> {
        let end = start
            .checked_add(row_count)
            .ok_or(EmbeddingError::RowBlockOutOfBounds {
                start,
                row_count,
                table_row_count: self.cells.len(),
            })?;
        if end > self.cells.len() {
            return Err(EmbeddingError::RowBlockOutOfBounds {
                start,
                row_count,
                table_row_count: self.cells.len(),
            });
        }
        let dimension = self.dimension as usize;
        let value_start = start
            .checked_mul(dimension)
            .ok_or(EmbeddingError::SizeOverflow)?;
        let value_end = end
            .checked_mul(dimension)
            .ok_or(EmbeddingError::SizeOverflow)?;
        Ok(CellEmbeddingBlock {
            cells: &self.cells[start..end],
            statuses: &self.statuses[start..end],
            values: &self.values[value_start..value_end],
            dimension: self.dimension,
        })
    }

    /// Recompute factual QC and logical identity by scanning bounded borrowed blocks.
    ///
    /// This scans an already materialized table. Physical Arrow and Parquet streaming
    /// scans are separate APIs and do not retain the complete table.
    pub fn scan_qc(&self, maximum_block_rows: usize) -> Result<EmbeddingQcSummary, EmbeddingError> {
        if maximum_block_rows == 0 {
            return Err(EmbeddingError::ZeroScanBlockRows);
        }
        let mut accumulator = EmbeddingQcAccumulator::new(
            self.expected_cells_artifact_id,
            self.provenance_artifact_id,
            self.row_link_digest,
            self.dimension,
            self.cells.len(),
        )?;
        let mut start = 0_usize;
        while start < self.cells.len() {
            let rows = self
                .cells
                .len()
                .saturating_sub(start)
                .min(maximum_block_rows);
            for row in self.block(start, rows)?.rows() {
                accumulator.push_view(row)?;
            }
            start = start
                .checked_add(rows)
                .ok_or(EmbeddingError::SizeOverflow)?;
        }
        accumulator.finish()
    }
}

pub(crate) struct EmbeddingQcAccumulator {
    dimension: usize,
    expected_rows: usize,
    next_row: usize,
    present_count: u64,
    missing_vector_count: u64,
    extraction_failed_count: u64,
    qc_rejected_count: u64,
    all_zero_present_count: u64,
    digest: FramedDigest,
}

impl EmbeddingQcAccumulator {
    pub(crate) fn new(
        expected_cells_artifact_id: ArtifactId,
        provenance_artifact_id: ArtifactId,
        row_link_digest: ContentDigest,
        dimension: u32,
        expected_rows: usize,
    ) -> Result<Self, EmbeddingError> {
        if dimension == 0 {
            return Err(EmbeddingError::ZeroDimension);
        }
        let row_count = u64::try_from(expected_rows).map_err(|_| EmbeddingError::SizeOverflow)?;
        let mut digest = FramedDigest::new();
        digest.field(LOGICAL_DIGEST_DOMAIN);
        digest.field(provenance_artifact_id.digest().as_bytes());
        digest.field(row_link_digest.as_bytes());
        digest.field(expected_cells_artifact_id.digest().as_bytes());
        digest.field(&dimension.to_be_bytes());
        digest.field(&row_count.to_be_bytes());
        Ok(Self {
            dimension: usize::try_from(dimension).map_err(|_| EmbeddingError::SizeOverflow)?,
            expected_rows,
            next_row: 0,
            present_count: 0,
            missing_vector_count: 0,
            extraction_failed_count: 0,
            qc_rejected_count: 0,
            all_zero_present_count: 0,
            digest,
        })
    }

    pub(crate) fn push_view(&mut self, view: CellEmbeddingView<'_>) -> Result<(), EmbeddingError> {
        self.push(view.cell_id(), view.status(), view.vector())
    }

    pub(crate) fn push(
        &mut self,
        cell_id: &CellId,
        status: EmbeddingStatus,
        vector: Option<&[f32]>,
    ) -> Result<(), EmbeddingError> {
        if self.next_row >= self.expected_rows {
            return Err(EmbeddingError::RowSetMismatch);
        }
        self.digest.field(cell_id.as_str().as_bytes());
        self.digest.field(status.wire_name().as_bytes());
        match (status, vector) {
            (EmbeddingStatus::Present, Some(vector)) => {
                if vector.len() != self.dimension {
                    return Err(EmbeddingError::DimensionMismatch {
                        expected: self.dimension,
                        observed: vector.len(),
                    });
                }
                let mut all_zero = true;
                for (column, value) in vector.iter().copied().enumerate() {
                    if !value.is_finite() {
                        return Err(EmbeddingError::NonFiniteComponent {
                            row: self.next_row,
                            column,
                        });
                    }
                    if value == 0.0 && value.to_bits() != 0 {
                        return Err(EmbeddingError::StatusVectorMismatch);
                    }
                    all_zero &= value.to_bits() == 0;
                    self.digest.field(&value.to_bits().to_be_bytes());
                }
                self.present_count = increment(self.present_count)?;
                if all_zero {
                    self.all_zero_present_count = increment(self.all_zero_present_count)?;
                }
            }
            (EmbeddingStatus::MissingVector, None) => {
                self.missing_vector_count = increment(self.missing_vector_count)?;
            }
            (EmbeddingStatus::ExtractionFailed, None) => {
                self.extraction_failed_count = increment(self.extraction_failed_count)?;
            }
            (EmbeddingStatus::QcRejected, None) => {
                self.qc_rejected_count = increment(self.qc_rejected_count)?;
            }
            _ => return Err(EmbeddingError::StatusVectorMismatch),
        }
        self.next_row = self
            .next_row
            .checked_add(1)
            .ok_or(EmbeddingError::SizeOverflow)?;
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<EmbeddingQcSummary, EmbeddingError> {
        if self.next_row != self.expected_rows {
            return Err(EmbeddingError::RowSetMismatch);
        }
        let row_count =
            u64::try_from(self.expected_rows).map_err(|_| EmbeddingError::SizeOverflow)?;
        let status_total = self
            .present_count
            .checked_add(self.missing_vector_count)
            .and_then(|value| value.checked_add(self.extraction_failed_count))
            .and_then(|value| value.checked_add(self.qc_rejected_count))
            .ok_or(EmbeddingError::SizeOverflow)?;
        if status_total != row_count {
            return Err(EmbeddingError::RowSetMismatch);
        }
        Ok(EmbeddingQcSummary {
            row_count,
            present_count: self.present_count,
            missing_vector_count: self.missing_vector_count,
            extraction_failed_count: self.extraction_failed_count,
            qc_rejected_count: self.qc_rejected_count,
            all_zero_present_count: self.all_zero_present_count,
            dimension: u32::try_from(self.dimension).map_err(|_| EmbeddingError::SizeOverflow)?,
            logical_digest: self.digest.finish(),
        })
    }
}

fn increment(value: u64) -> Result<u64, EmbeddingError> {
    value.checked_add(1).ok_or(EmbeddingError::SizeOverflow)
}
