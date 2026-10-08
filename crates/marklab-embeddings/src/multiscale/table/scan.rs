use std::fmt;

use marklab_data::SlideId;
use marklab_project::{ArtifactId, ContentDigest};

use crate::EmbeddingStatus;

use super::MultiscaleEmbeddingQcSummary;
use crate::multiscale::{
    digest::LogicalDigest, entity::EmbeddingEntityKind, error::MultiscaleEmbeddingError,
};

pub(super) struct MatrixView<'a, I> {
    pub(super) id: &'a I,
    pub(super) status: EmbeddingStatus,
    pub(super) vector: Option<&'a [f32]>,
}

impl<I> Copy for MatrixView<'_, I> {}

impl<I> Clone for MatrixView<'_, I> {
    fn clone(&self) -> Self {
        *self
    }
}

pub(super) struct MatrixBlock<'a, I> {
    pub(super) ids: &'a [I],
    pub(super) statuses: &'a [EmbeddingStatus],
    pub(super) values: &'a [f32],
    pub(super) dimension: u32,
}

impl<I> Copy for MatrixBlock<'_, I> {}

impl<I> Clone for MatrixBlock<'_, I> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<I> fmt::Debug for MatrixBlock<'_, I> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MatrixBlock")
            .field("row_count", &self.ids.len())
            .field("dimension", &self.dimension)
            .finish()
    }
}

impl<'a, I> MatrixBlock<'a, I> {
    pub(super) fn row_count(self) -> usize {
        self.ids.len()
    }

    pub(super) fn row(self, index: usize) -> Result<MatrixView<'a, I>, MultiscaleEmbeddingError> {
        let id = self
            .ids
            .get(index)
            .ok_or(MultiscaleEmbeddingError::RowOutOfBounds {
                index,
                row_count: self.ids.len(),
            })?;
        Ok(self.view(index, id))
    }

    pub(super) fn rows(self) -> impl ExactSizeIterator<Item = MatrixView<'a, I>> + 'a {
        self.ids
            .iter()
            .enumerate()
            .map(move |(index, id)| self.view(index, id))
    }

    fn view(self, index: usize, id: &'a I) -> MatrixView<'a, I> {
        let status = self.statuses[index];
        let vector = if status == EmbeddingStatus::Present {
            let dimension = self.dimension as usize;
            let start = index * dimension;
            Some(&self.values[start..start + dimension])
        } else {
            None
        };
        MatrixView { id, status, vector }
    }
}

#[derive(Default)]
struct StatusCounts {
    present: u64,
    missing: u64,
    extraction_failed: u64,
    qc_rejected: u64,
    all_zero_present: u64,
}

pub(crate) struct MatrixSummaryAccumulator {
    entity_kind: EmbeddingEntityKind,
    dimension: usize,
    expected_rows: usize,
    next_row: usize,
    counts: StatusCounts,
    digest: LogicalDigest,
}

impl MatrixSummaryAccumulator {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        entity_kind: EmbeddingEntityKind,
        logical_domain: &'static [u8],
        owning_slide_id: &SlideId,
        expected_entities_artifact_id: ArtifactId,
        expected_entities_logical_digest: ContentDigest,
        support_artifact_id: ArtifactId,
        support_logical_digest: ContentDigest,
        provenance_artifact_id: ArtifactId,
        provenance_logical_digest: ContentDigest,
        dimension: u32,
        expected_rows: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        let row_count =
            u64::try_from(expected_rows).map_err(|_| MultiscaleEmbeddingError::SizeOverflow)?;
        let mut digest = LogicalDigest::new(logical_domain);
        digest.text(entity_kind.wire_name());
        digest.text(owning_slide_id.as_str());
        digest.artifact_id(expected_entities_artifact_id);
        digest.content_digest(expected_entities_logical_digest);
        digest.artifact_id(support_artifact_id);
        digest.content_digest(support_logical_digest);
        digest.artifact_id(provenance_artifact_id);
        digest.content_digest(provenance_logical_digest);
        digest.u32(dimension);
        digest.u64(row_count);
        Ok(Self {
            entity_kind,
            dimension: usize::try_from(dimension)
                .map_err(|_| MultiscaleEmbeddingError::SizeOverflow)?,
            expected_rows,
            next_row: 0,
            counts: StatusCounts::default(),
            digest,
        })
    }

    pub(crate) fn push(
        &mut self,
        id: &str,
        status: EmbeddingStatus,
        vector: Option<&[f32]>,
    ) -> Result<(), MultiscaleEmbeddingError> {
        if self.next_row >= self.expected_rows {
            return Err(MultiscaleEmbeddingError::RowSetMismatch);
        }
        self.digest.text(id);
        self.digest.text(status.wire_name());
        match (status, vector) {
            (EmbeddingStatus::Present, Some(vector)) => {
                if vector.len() != self.dimension {
                    return Err(MultiscaleEmbeddingError::DimensionMismatch {
                        expected: self.dimension,
                        observed: vector.len(),
                    });
                }
                let mut all_zero = true;
                for (column, value) in vector.iter().copied().enumerate() {
                    if !value.is_finite() {
                        return Err(MultiscaleEmbeddingError::NonFiniteComponent {
                            row: self.next_row,
                            column,
                        });
                    }
                    if value == 0.0 && value.to_bits() != 0 {
                        return Err(MultiscaleEmbeddingError::StatusVectorMismatch);
                    }
                    all_zero &= value.to_bits() == 0;
                    self.digest.f32(value);
                }
                self.counts.present = increment(self.counts.present)?;
                if all_zero {
                    self.counts.all_zero_present = increment(self.counts.all_zero_present)?;
                }
            }
            (EmbeddingStatus::MissingVector, None) => {
                self.counts.missing = increment(self.counts.missing)?;
            }
            (EmbeddingStatus::ExtractionFailed, None) => {
                self.counts.extraction_failed = increment(self.counts.extraction_failed)?;
            }
            (EmbeddingStatus::QcRejected, None) => {
                self.counts.qc_rejected = increment(self.counts.qc_rejected)?;
            }
            _ => return Err(MultiscaleEmbeddingError::StatusVectorMismatch),
        }
        self.next_row = self
            .next_row
            .checked_add(1)
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<MultiscaleEmbeddingQcSummary, MultiscaleEmbeddingError> {
        if self.next_row != self.expected_rows {
            return Err(MultiscaleEmbeddingError::RowSetMismatch);
        }
        let row_count = u64::try_from(self.expected_rows)
            .map_err(|_| MultiscaleEmbeddingError::SizeOverflow)?;
        let status_total = self
            .counts
            .present
            .checked_add(self.counts.missing)
            .and_then(|value| value.checked_add(self.counts.extraction_failed))
            .and_then(|value| value.checked_add(self.counts.qc_rejected))
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        if status_total != row_count {
            return Err(MultiscaleEmbeddingError::RowSetMismatch);
        }
        Ok(MultiscaleEmbeddingQcSummary {
            entity_kind: self.entity_kind,
            row_count,
            present_count: self.counts.present,
            missing_vector_count: self.counts.missing,
            extraction_failed_count: self.counts.extraction_failed,
            qc_rejected_count: self.counts.qc_rejected,
            all_zero_present_count: self.counts.all_zero_present,
            dimension: u32::try_from(self.dimension)
                .map_err(|_| MultiscaleEmbeddingError::SizeOverflow)?,
            logical_digest: self.digest.finish(),
        })
    }
}

fn increment(value: u64) -> Result<u64, MultiscaleEmbeddingError> {
    value
        .checked_add(1)
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)
}
