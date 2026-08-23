use std::{fmt, mem::size_of};

use marklab_data::SlideId;
use marklab_project::{ArtifactId, ContentDigest};

use crate::{digest::canonical_positive_zero, EmbeddingStatus};

#[cfg(feature = "parquet")]
use super::physical::MatrixPhysicalProfile;
use super::{
    entity::{EmbeddingEntityKind, EntitySpec},
    error::MultiscaleEmbeddingError,
};

mod scan;
mod wrappers;
pub(crate) use scan::MatrixSummaryAccumulator;
use scan::{MatrixBlock, MatrixView};
pub use wrappers::*;

const MAX_ROWS: usize = 100_000_000;
const MAX_DIMENSION: u32 = 65_536;

/// Factual integrity summary for one typed multiscale embedding table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MultiscaleEmbeddingQcSummary {
    entity_kind: EmbeddingEntityKind,
    row_count: u64,
    present_count: u64,
    missing_vector_count: u64,
    extraction_failed_count: u64,
    qc_rejected_count: u64,
    all_zero_present_count: u64,
    dimension: u32,
    logical_digest: ContentDigest,
}

impl MultiscaleEmbeddingQcSummary {
    /// Typed entity family summarized by these counts.
    pub fn entity_kind(self) -> EmbeddingEntityKind {
        self.entity_kind
    }

    /// Total canonical row count.
    pub fn row_count(self) -> u64 {
        self.row_count
    }

    /// Rows carrying meaningful present vectors.
    pub fn present_count(self) -> u64 {
        self.present_count
    }

    /// Rows explicitly missing a vector.
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

    /// Present vectors whose canonical components are all positive zero.
    pub fn all_zero_present_count(self) -> u64 {
        self.all_zero_present_count
    }

    /// Fixed vector dimension.
    pub fn dimension(self) -> u32 {
        self.dimension
    }

    /// Format-independent logical identity.
    pub fn logical_digest(self) -> ContentDigest {
        self.logical_digest
    }
}

trait OwnedRow<I> {
    fn id(&self) -> &I;
    fn vector_capacity(&self) -> usize;
    fn into_parts(self) -> (I, EmbeddingStatus, Option<Vec<f32>>);
}

#[cfg(feature = "parquet")]
#[derive(Clone, Copy)]
pub(crate) struct MultiscaleMatrixRow<'a> {
    id: &'a str,
    status: EmbeddingStatus,
    vector: Option<&'a [f32]>,
}

#[cfg(feature = "parquet")]
impl<'a> MultiscaleMatrixRow<'a> {
    pub(crate) fn id(self) -> &'a str {
        self.id
    }

    pub(crate) fn status(self) -> EmbeddingStatus {
        self.status
    }

    pub(crate) fn vector(self) -> Option<&'a [f32]> {
        self.vector
    }
}

#[cfg(feature = "parquet")]
pub(crate) trait MultiscaleMatrixTable {
    fn profile(&self) -> MatrixPhysicalProfile;
    fn entity_kind(&self) -> EmbeddingEntityKind;
    fn owning_slide_id(&self) -> &SlideId;
    fn row_count(&self) -> usize;
    fn dimension(&self) -> u32;
    fn row(&self, index: usize) -> Result<MultiscaleMatrixRow<'_>, MultiscaleEmbeddingError>;
    fn qc_summary(&self) -> MultiscaleEmbeddingQcSummary;
    fn expected_entities_artifact_id(&self) -> ArtifactId;
    fn expected_entities_logical_digest(&self) -> ContentDigest;
    fn support_artifact_id(&self) -> ArtifactId;
    fn support_logical_digest(&self) -> ContentDigest;
    fn provenance_artifact_id(&self) -> ArtifactId;
    fn provenance_logical_digest(&self) -> ContentDigest;

    fn logical_digest(&self) -> ContentDigest {
        self.qc_summary().logical_digest()
    }
}

struct MatrixCore<I> {
    owning_slide_id: SlideId,
    ids: Box<[I]>,
    statuses: Box<[EmbeddingStatus]>,
    values: Vec<f32>,
    dimension: u32,
    qc_summary: MultiscaleEmbeddingQcSummary,
    expected_entities_artifact_id: ArtifactId,
    expected_entities_logical_digest: ContentDigest,
    support_artifact_id: ArtifactId,
    support_logical_digest: ContentDigest,
    provenance_artifact_id: ArtifactId,
    provenance_logical_digest: ContentDigest,
}

impl<I> fmt::Debug for MatrixCore<I> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MatrixCore")
            .field("row_count", &self.ids.len())
            .field("dimension", &self.dimension)
            .field("qc_summary", &self.qc_summary)
            .field(
                "expected_entities_artifact_id",
                &self.expected_entities_artifact_id,
            )
            .field("support_artifact_id", &self.support_artifact_id)
            .field("provenance_artifact_id", &self.provenance_artifact_id)
            .finish()
    }
}

impl<I: EntitySpec> MatrixCore<I> {
    #[allow(clippy::too_many_arguments)]
    fn from_rows<R: OwnedRow<I>>(
        dimension: u32,
        owning_slide_id: &SlideId,
        expected_ids: &[I],
        expected_entities_artifact_id: ArtifactId,
        expected_entities_logical_digest: ContentDigest,
        support_artifact_id: ArtifactId,
        support_logical_digest: ContentDigest,
        provenance_artifact_id: ArtifactId,
        provenance_logical_digest: ContentDigest,
        rows: Vec<R>,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        if expected_entities_artifact_id == support_artifact_id
            || expected_entities_artifact_id == provenance_artifact_id
            || support_artifact_id == provenance_artifact_id
        {
            return Err(MultiscaleEmbeddingError::DuplicateMultiscaleTableArtifactDependency);
        }
        validate_shape(dimension, rows.len())?;
        if rows.len() != expected_ids.len()
            || rows
                .iter()
                .zip(expected_ids)
                .any(|(row, expected)| row.id() != expected)
        {
            return Err(MultiscaleEmbeddingError::RowSetMismatch);
        }
        let required = retained_bytes(dimension, owning_slide_id, expected_ids, &rows)?;
        if required > maximum_retained_bytes {
            return Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded {
                required,
                maximum: maximum_retained_bytes,
            });
        }
        let dimension_usize =
            usize::try_from(dimension).map_err(|_| MultiscaleEmbeddingError::SizeOverflow)?;
        let component_count = rows
            .len()
            .checked_mul(dimension_usize)
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        let value_bytes = component_count
            .checked_mul(size_of::<f32>())
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        let mut values = Vec::new();
        values.try_reserve_exact(component_count).map_err(|_| {
            MultiscaleEmbeddingError::AllocationFailed {
                requested: value_bytes,
            }
        })?;
        let mut ids = Vec::new();
        ids.try_reserve_exact(rows.len()).map_err(|_| {
            MultiscaleEmbeddingError::AllocationFailed {
                requested: rows.len().saturating_mul(size_of::<I>()),
            }
        })?;
        let mut statuses = Vec::new();
        statuses.try_reserve_exact(rows.len()).map_err(|_| {
            MultiscaleEmbeddingError::AllocationFailed {
                requested: rows.len().saturating_mul(size_of::<EmbeddingStatus>()),
            }
        })?;

        let mut accumulator = MatrixSummaryAccumulator::new(
            I::KIND,
            I::TABLE_DOMAIN,
            owning_slide_id,
            expected_entities_artifact_id,
            expected_entities_logical_digest,
            support_artifact_id,
            support_logical_digest,
            provenance_artifact_id,
            provenance_logical_digest,
            dimension,
            rows.len(),
        )?;
        for (row_index, row) in rows.into_iter().enumerate() {
            let (id, status, vector) = row.into_parts();
            match (status, vector) {
                (EmbeddingStatus::Present, Some(vector)) => {
                    if vector.len() != dimension_usize {
                        return Err(MultiscaleEmbeddingError::DimensionMismatch {
                            expected: dimension_usize,
                            observed: vector.len(),
                        });
                    }
                    let start = values.len();
                    for (column, value) in vector.into_iter().enumerate() {
                        if !value.is_finite() {
                            return Err(MultiscaleEmbeddingError::NonFiniteComponent {
                                row: row_index,
                                column,
                            });
                        }
                        let value = canonical_positive_zero(value);
                        values.push(value);
                    }
                    accumulator.push(id.as_str(), status, Some(&values[start..]))?;
                }
                (EmbeddingStatus::MissingVector, None) => {
                    values.resize(values.len() + dimension_usize, 0.0);
                    accumulator.push(id.as_str(), status, None)?;
                }
                (EmbeddingStatus::ExtractionFailed, None) => {
                    values.resize(values.len() + dimension_usize, 0.0);
                    accumulator.push(id.as_str(), status, None)?;
                }
                (EmbeddingStatus::QcRejected, None) => {
                    values.resize(values.len() + dimension_usize, 0.0);
                    accumulator.push(id.as_str(), status, None)?;
                }
                _ => return Err(MultiscaleEmbeddingError::StatusVectorMismatch),
            }
            ids.push(id);
            statuses.push(status);
        }
        let qc_summary = accumulator.finish()?;
        Ok(Self {
            owning_slide_id: owning_slide_id.clone(),
            ids: ids.into_boxed_slice(),
            statuses: statuses.into_boxed_slice(),
            values,
            dimension,
            qc_summary,
            expected_entities_artifact_id,
            expected_entities_logical_digest,
            support_artifact_id,
            support_logical_digest,
            provenance_artifact_id,
            provenance_logical_digest,
        })
    }

    fn row(&self, index: usize) -> Result<MatrixView<'_, I>, MultiscaleEmbeddingError> {
        let id = self
            .ids
            .get(index)
            .ok_or(MultiscaleEmbeddingError::RowOutOfBounds {
                index,
                row_count: self.ids.len(),
            })?;
        let status = self.statuses[index];
        let vector = if status == EmbeddingStatus::Present {
            let dimension = self.dimension as usize;
            let start = index * dimension;
            Some(&self.values[start..start + dimension])
        } else {
            None
        };
        Ok(MatrixView { id, status, vector })
    }

    fn block(
        &self,
        start: usize,
        row_count: usize,
    ) -> Result<MatrixBlock<'_, I>, MultiscaleEmbeddingError> {
        let end =
            start
                .checked_add(row_count)
                .ok_or(MultiscaleEmbeddingError::RowBlockOutOfBounds {
                    start,
                    row_count,
                    table_row_count: self.ids.len(),
                })?;
        if end > self.ids.len() {
            return Err(MultiscaleEmbeddingError::RowBlockOutOfBounds {
                start,
                row_count,
                table_row_count: self.ids.len(),
            });
        }
        let dimension = self.dimension as usize;
        let value_start = start
            .checked_mul(dimension)
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        let value_end = end
            .checked_mul(dimension)
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        Ok(MatrixBlock {
            ids: &self.ids[start..end],
            statuses: &self.statuses[start..end],
            values: &self.values[value_start..value_end],
            dimension: self.dimension,
        })
    }

    fn scan_qc(
        &self,
        maximum_block_rows: usize,
    ) -> Result<MultiscaleEmbeddingQcSummary, MultiscaleEmbeddingError> {
        if maximum_block_rows == 0 {
            return Err(MultiscaleEmbeddingError::ZeroScanBlockRows);
        }
        let mut accumulator = MatrixSummaryAccumulator::new(
            I::KIND,
            I::TABLE_DOMAIN,
            &self.owning_slide_id,
            self.expected_entities_artifact_id,
            self.expected_entities_logical_digest,
            self.support_artifact_id,
            self.support_logical_digest,
            self.provenance_artifact_id,
            self.provenance_logical_digest,
            self.dimension,
            self.ids.len(),
        )?;
        let mut start = 0_usize;
        while start < self.ids.len() {
            let rows = self.ids.len().saturating_sub(start).min(maximum_block_rows);
            for view in self.block(start, rows)?.rows() {
                accumulator.push(view.id.as_str(), view.status, view.vector)?;
            }
            start = start
                .checked_add(rows)
                .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        }
        accumulator.finish()
    }
}

fn validate_shape(dimension: u32, rows: usize) -> Result<(), MultiscaleEmbeddingError> {
    if dimension == 0 {
        return Err(MultiscaleEmbeddingError::ZeroDimension);
    }
    if dimension > MAX_DIMENSION {
        return Err(MultiscaleEmbeddingError::DimensionExceeded {
            observed: dimension,
            maximum: MAX_DIMENSION,
        });
    }
    if rows > MAX_ROWS {
        return Err(MultiscaleEmbeddingError::RowCountExceeded {
            observed: rows,
            maximum: MAX_ROWS,
        });
    }
    Ok(())
}

fn retained_bytes<I: EntitySpec, R: OwnedRow<I>>(
    dimension: u32,
    owning_slide_id: &SlideId,
    ids: &[I],
    rows: &Vec<R>,
) -> Result<usize, MultiscaleEmbeddingError> {
    let dimension =
        usize::try_from(dimension).map_err(|_| MultiscaleEmbeddingError::SizeOverflow)?;
    let component_count = ids
        .len()
        .checked_mul(dimension)
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    let values = component_count
        .checked_mul(size_of::<f32>())
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    let result_rows = ids
        .len()
        .checked_mul(size_of::<I>() + size_of::<EmbeddingStatus>())
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    let construction_rows = rows
        .capacity()
        .checked_mul(size_of::<R>())
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
    let construction_vectors = rows.iter().try_fold(0_usize, |total, row| {
        let bytes = row
            .vector_capacity()
            .checked_mul(size_of::<f32>())
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        total
            .checked_add(bytes)
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)
    })?;
    ids.iter().try_fold(
        size_of::<MatrixCore<I>>()
            .checked_add(owning_slide_id.as_str().len())
            .and_then(|value| value.checked_add(values))
            .and_then(|value| value.checked_add(result_rows))
            .and_then(|value| value.checked_add(construction_rows))
            .and_then(|value| value.checked_add(construction_vectors))
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?,
        |total, id| {
            total
                .checked_add(id.as_str().len())
                .ok_or(MultiscaleEmbeddingError::SizeOverflow)
        },
    )
}
