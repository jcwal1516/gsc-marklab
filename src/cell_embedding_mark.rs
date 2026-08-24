use std::{io::Write, mem::size_of};

use marklab_embeddings::{
    CellEmbeddingArtifact, CellEmbeddingTable, EmbeddingQcSummary, EmbeddingStatus,
};
use marklab_workflow::{ArtifactId, ContentDigest, ContentDigestWriter};
use thiserror::Error;

use crate::{
    BinaryMarkDeclaration, DeclaredScalarIdentity, DeclaredScalarPatternInput,
    ProbabilityMarkDeclaration,
};

const BINARY_GROUPING_DIGEST_DOMAIN: &[u8] = b"marklab-declared-binary-cell-embedding-groups-v1";

/// Extraction-status counts for one exact declared binary-mark group.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DeclaredBinaryCellEmbeddingGroupCounts {
    row_count: u64,
    present_count: u64,
    missing_vector_count: u64,
    extraction_failed_count: u64,
    qc_rejected_count: u64,
}

impl DeclaredBinaryCellEmbeddingGroupCounts {
    /// Total declared rows in this binary group.
    pub fn row_count(self) -> u64 {
        self.row_count
    }

    /// Rows carrying a present embedding vector.
    pub fn present_count(self) -> u64 {
        self.present_count
    }

    /// Rows explicitly missing an embedding vector.
    pub fn missing_vector_count(self) -> u64 {
        self.missing_vector_count
    }

    /// Rows whose embedding extraction failed.
    pub fn extraction_failed_count(self) -> u64 {
        self.extraction_failed_count
    }

    /// Rows whose embedding vector was rejected by declared QC.
    pub fn qc_rejected_count(self) -> u64 {
        self.qc_rejected_count
    }

    fn record(
        &mut self,
        status: EmbeddingStatus,
    ) -> Result<(), DeclaredBinaryCellEmbeddingCentroidDiscrepancyError> {
        self.row_count = self
            .row_count
            .checked_add(1)
            .ok_or(DeclaredBinaryCellEmbeddingCentroidDiscrepancyError::SizeOverflow)?;
        let count = match status {
            EmbeddingStatus::Present => &mut self.present_count,
            EmbeddingStatus::MissingVector => &mut self.missing_vector_count,
            EmbeddingStatus::ExtractionFailed => &mut self.extraction_failed_count,
            EmbeddingStatus::QcRejected => &mut self.qc_rejected_count,
        };
        *count = count
            .checked_add(1)
            .ok_or(DeclaredBinaryCellEmbeddingCentroidDiscrepancyError::SizeOverflow)?;
        Ok(())
    }
}

/// Availability of a declared binary-group cell-embedding centroid discrepancy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclaredBinaryCellEmbeddingCentroidDiscrepancyStatus {
    /// Both binary groups contain at least one present embedding vector.
    Available,
    /// At least one binary group contains no present embedding vector.
    InsufficientGroups,
}

/// Descriptive centroid discrepancy over exact declared binary and verified embedding rows.
#[derive(Clone, Debug, PartialEq)]
pub struct DeclaredBinaryCellEmbeddingCentroidDiscrepancy {
    status: DeclaredBinaryCellEmbeddingCentroidDiscrepancyStatus,
    marked_counts: DeclaredBinaryCellEmbeddingGroupCounts,
    unmarked_counts: DeclaredBinaryCellEmbeddingGroupCounts,
    dimension: u32,
    mean_squared_component_difference: Option<f64>,
    scalar_identity: DeclaredScalarIdentity,
    binary_mark: BinaryMarkDeclaration,
    probability_mark: Option<ProbabilityMarkDeclaration>,
    binary_grouping_logical_digest: ContentDigest,
    embedding_artifact_id: ArtifactId,
    row_link_artifact_id: ArtifactId,
    embedding_provenance_artifact_id: ArtifactId,
    embedding_qc_summary: EmbeddingQcSummary,
    table_logical_digest: ContentDigest,
}

impl DeclaredBinaryCellEmbeddingCentroidDiscrepancy {
    /// Whether the numeric discrepancy is available.
    pub fn status(&self) -> DeclaredBinaryCellEmbeddingCentroidDiscrepancyStatus {
        self.status
    }

    /// Exact extraction-status counts for declared binary-marked rows.
    pub fn marked_counts(&self) -> DeclaredBinaryCellEmbeddingGroupCounts {
        self.marked_counts
    }

    /// Exact extraction-status counts for declared binary-unmarked rows.
    pub fn unmarked_counts(&self) -> DeclaredBinaryCellEmbeddingGroupCounts {
        self.unmarked_counts
    }

    /// Exact positive embedding dimension.
    pub fn dimension(&self) -> u32 {
        self.dimension
    }

    /// Mean squared component difference between group centroids, when available.
    pub fn mean_squared_component_difference(&self) -> Option<f64> {
        self.mean_squared_component_difference
    }

    /// Exact declared row, slide, frame, and declaration identity.
    pub fn scalar_identity(&self) -> &DeclaredScalarIdentity {
        &self.scalar_identity
    }

    /// Required binary declaration whose exact rows select the groups.
    pub fn binary_mark(&self) -> &BinaryMarkDeclaration {
        &self.binary_mark
    }

    /// Optional probability declaration retained without selecting groups.
    pub fn probability_mark(&self) -> Option<&ProbabilityMarkDeclaration> {
        self.probability_mark.as_ref()
    }

    /// Domain-separated identity of the exact ordered CellId-to-binary-group assignments.
    pub fn binary_grouping_logical_digest(&self) -> ContentDigest {
        self.binary_grouping_logical_digest
    }

    /// Exact verified physical embedding-table artifact identity.
    pub fn embedding_artifact_id(&self) -> ArtifactId {
        self.embedding_artifact_id
    }

    /// Exact verified physical row-link artifact identity.
    pub fn row_link_artifact_id(&self) -> ArtifactId {
        self.row_link_artifact_id
    }

    /// Exact cell-embedding model/provenance artifact identity.
    pub fn embedding_provenance_artifact_id(&self) -> ArtifactId {
        self.embedding_provenance_artifact_id
    }

    /// Factual verified embedding status, shape, zero-vector, and logical summary.
    pub fn embedding_qc_summary(&self) -> EmbeddingQcSummary {
        self.embedding_qc_summary
    }

    /// Format-independent identity of the exact materialized embedding table.
    pub fn table_logical_digest(&self) -> ContentDigest {
        self.table_logical_digest
    }
}

/// Invalid artifact/row binding, caller resource limit, or bounded allocation.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum DeclaredBinaryCellEmbeddingCentroidDiscrepancyError {
    /// The materialized table does not match the verified embedding artifact.
    #[error("declared binary cell-embedding table and verified artifact disagree")]
    EmbeddingArtifactBindingMismatch,
    /// Declared and embedding CellIds disagree at one row.
    #[error("declared binary and cell-embedding identities disagree at row {row}")]
    CellIdBindingMismatch {
        /// First mismatched or absent row.
        row: usize,
    },
    /// The admitted row traversal exceeds the caller maximum.
    #[error("declared binary cell-embedding rows {required} exceed caller maximum {maximum}")]
    RowCountBudgetExceeded {
        /// Exact table row count.
        required: usize,
        /// Caller-provided row maximum.
        maximum: usize,
    },
    /// Conservative component visits exceed the caller maximum.
    #[error(
        "declared binary cell-embedding component operations {required} exceed caller maximum {maximum}"
    )]
    ComponentOperationBudgetExceeded {
        /// Checked present-row accumulation visits plus one final dimension pass.
        required: u64,
        /// Caller-provided component-operation maximum.
        maximum: u64,
    },
    /// Exact two-accumulator storage exceeds the caller maximum.
    #[error(
        "declared binary cell-embedding working bytes {required} exceed caller maximum {maximum}"
    )]
    WorkingByteBudgetExceeded {
        /// Exact two-accumulator byte count.
        required: usize,
        /// Caller-provided working-byte maximum.
        maximum: usize,
    },
    /// A checked count or byte computation overflowed.
    #[error("declared binary cell-embedding size computation overflowed")]
    SizeOverflow,
    /// Exact bounded accumulator allocation failed.
    #[error("declared binary cell-embedding accumulator allocation failed for {requested} bytes")]
    AllocationFailed {
        /// Exact requested working bytes.
        requested: usize,
    },
}

/// Narrow immutable S7 input binding reused only by its concrete scheduler node.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DeclaredBinaryCellEmbeddingCentroidBinding {
    marked_counts: DeclaredBinaryCellEmbeddingGroupCounts,
    unmarked_counts: DeclaredBinaryCellEmbeddingGroupCounts,
    dimension: u32,
    scalar_identity: DeclaredScalarIdentity,
    binary_mark: BinaryMarkDeclaration,
    probability_mark: Option<ProbabilityMarkDeclaration>,
    binary_grouping_logical_digest: ContentDigest,
    embedding_artifact_id: ArtifactId,
    row_link_artifact_id: ArtifactId,
    embedding_provenance_artifact_id: ArtifactId,
    embedding_qc_summary: EmbeddingQcSummary,
    table_logical_digest: ContentDigest,
}

impl DeclaredBinaryCellEmbeddingCentroidBinding {
    pub(crate) fn status(&self) -> DeclaredBinaryCellEmbeddingCentroidDiscrepancyStatus {
        if self.marked_counts.present_count() == 0 || self.unmarked_counts.present_count() == 0 {
            DeclaredBinaryCellEmbeddingCentroidDiscrepancyStatus::InsufficientGroups
        } else {
            DeclaredBinaryCellEmbeddingCentroidDiscrepancyStatus::Available
        }
    }

    pub(crate) fn attach_value(
        &self,
        value: Option<f64>,
    ) -> Option<DeclaredBinaryCellEmbeddingCentroidDiscrepancy> {
        let status = self.status();
        let valid = match (status, value) {
            (DeclaredBinaryCellEmbeddingCentroidDiscrepancyStatus::Available, Some(value)) => {
                value.is_finite()
                    && value >= 0.0
                    && (value != 0.0 || value.to_bits() == 0.0_f64.to_bits())
            }
            (DeclaredBinaryCellEmbeddingCentroidDiscrepancyStatus::InsufficientGroups, None) => {
                true
            }
            _ => false,
        };
        valid.then(|| DeclaredBinaryCellEmbeddingCentroidDiscrepancy {
            status,
            marked_counts: self.marked_counts,
            unmarked_counts: self.unmarked_counts,
            dimension: self.dimension,
            mean_squared_component_difference: value,
            scalar_identity: self.scalar_identity.clone(),
            binary_mark: self.binary_mark.clone(),
            probability_mark: self.probability_mark.clone(),
            binary_grouping_logical_digest: self.binary_grouping_logical_digest,
            embedding_artifact_id: self.embedding_artifact_id,
            row_link_artifact_id: self.row_link_artifact_id,
            embedding_provenance_artifact_id: self.embedding_provenance_artifact_id,
            embedding_qc_summary: self.embedding_qc_summary,
            table_logical_digest: self.table_logical_digest,
        })
    }

    pub(crate) fn matches_result(
        &self,
        result: &DeclaredBinaryCellEmbeddingCentroidDiscrepancy,
    ) -> bool {
        result.status == self.status()
            && result.marked_counts == self.marked_counts
            && result.unmarked_counts == self.unmarked_counts
            && result.dimension == self.dimension
            && result.scalar_identity == self.scalar_identity
            && result.binary_mark == self.binary_mark
            && result.probability_mark == self.probability_mark
            && result.binary_grouping_logical_digest == self.binary_grouping_logical_digest
            && result.embedding_artifact_id == self.embedding_artifact_id
            && result.row_link_artifact_id == self.row_link_artifact_id
            && result.embedding_provenance_artifact_id == self.embedding_provenance_artifact_id
            && result.embedding_qc_summary == self.embedding_qc_summary
            && result.table_logical_digest == self.table_logical_digest
            && self
                .attach_value(result.mean_squared_component_difference)
                .is_some()
    }
}

/// Bind every non-numeric S7 input and resource limit without component arithmetic.
pub(crate) fn bind_declared_binary_cell_embedding_centroid(
    input: &DeclaredScalarPatternInput<'_>,
    table: &CellEmbeddingTable,
    artifact: CellEmbeddingArtifact,
    maximum_rows: usize,
    maximum_component_operations: u64,
    maximum_working_bytes: usize,
) -> Result<
    DeclaredBinaryCellEmbeddingCentroidBinding,
    DeclaredBinaryCellEmbeddingCentroidDiscrepancyError,
> {
    let embedding_qc_summary = table.qc_summary();
    if embedding_qc_summary != artifact.qc_summary() {
        return Err(
            DeclaredBinaryCellEmbeddingCentroidDiscrepancyError::EmbeddingArtifactBindingMismatch,
        );
    }

    let row_count = table.row_count();
    if row_count > maximum_rows {
        return Err(
            DeclaredBinaryCellEmbeddingCentroidDiscrepancyError::RowCountBudgetExceeded {
                required: row_count,
                maximum: maximum_rows,
            },
        );
    }
    if input.cell_ids().len() != row_count {
        return Err(
            DeclaredBinaryCellEmbeddingCentroidDiscrepancyError::CellIdBindingMismatch {
                row: input.cell_ids().len().min(row_count),
            },
        );
    }

    let mut marked_counts = DeclaredBinaryCellEmbeddingGroupCounts::default();
    let mut unmarked_counts = DeclaredBinaryCellEmbeddingGroupCounts::default();
    let mut grouping_digest = ContentDigest::builder();
    write_grouping_part(&mut grouping_digest, BINARY_GROUPING_DIGEST_DOMAIN)?;
    write_grouping_part(
        &mut grouping_digest,
        input.scalar_identity().cell_ids_logical_digest().as_bytes(),
    )?;
    write_grouping_part(
        &mut grouping_digest,
        input
            .scalar_identity()
            .declared_input_logical_digest()
            .as_bytes(),
    )?;
    let row_count_u64 = u64::try_from(row_count)
        .map_err(|_| DeclaredBinaryCellEmbeddingCentroidDiscrepancyError::SizeOverflow)?;
    write_grouping_part(&mut grouping_digest, &row_count_u64.to_be_bytes())?;
    for (row_index, (cell_id, &mark)) in input
        .cell_ids()
        .iter()
        .zip(input.pattern().mark.iter())
        .enumerate()
    {
        let embedding_row = table.row(row_index).map_err(|_| {
            DeclaredBinaryCellEmbeddingCentroidDiscrepancyError::CellIdBindingMismatch {
                row: row_index,
            }
        })?;
        if embedding_row.cell_id() != cell_id {
            return Err(
                DeclaredBinaryCellEmbeddingCentroidDiscrepancyError::CellIdBindingMismatch {
                    row: row_index,
                },
            );
        }
        grouping_digest
            .write_all(&[mark])
            .map_err(|_| DeclaredBinaryCellEmbeddingCentroidDiscrepancyError::SizeOverflow)?;
        if mark == 1 {
            marked_counts.record(embedding_row.status())?;
        } else {
            unmarked_counts.record(embedding_row.status())?;
        }
    }
    let (binary_grouping_logical_digest, _) = grouping_digest.finish();

    let dimension = embedding_qc_summary.dimension();
    let component_operations = embedding_qc_summary
        .present_count()
        .checked_mul(u64::from(dimension))
        .and_then(|value| value.checked_add(u64::from(dimension)))
        .ok_or(DeclaredBinaryCellEmbeddingCentroidDiscrepancyError::SizeOverflow)?;
    if component_operations > maximum_component_operations {
        return Err(
            DeclaredBinaryCellEmbeddingCentroidDiscrepancyError::ComponentOperationBudgetExceeded {
                required: component_operations,
                maximum: maximum_component_operations,
            },
        );
    }
    let dimension_usize = usize::try_from(dimension)
        .map_err(|_| DeclaredBinaryCellEmbeddingCentroidDiscrepancyError::SizeOverflow)?;
    let working_bytes = dimension_usize
        .checked_mul(size_of::<f64>())
        .and_then(|value| value.checked_mul(2))
        .ok_or(DeclaredBinaryCellEmbeddingCentroidDiscrepancyError::SizeOverflow)?;
    if working_bytes > maximum_working_bytes {
        return Err(
            DeclaredBinaryCellEmbeddingCentroidDiscrepancyError::WorkingByteBudgetExceeded {
                required: working_bytes,
                maximum: maximum_working_bytes,
            },
        );
    }

    Ok(DeclaredBinaryCellEmbeddingCentroidBinding {
        marked_counts,
        unmarked_counts,
        dimension,
        scalar_identity: input.scalar_identity().clone(),
        binary_mark: input.binary_mark().clone(),
        probability_mark: input.probability_mark().cloned(),
        binary_grouping_logical_digest,
        embedding_artifact_id: artifact.embedding_artifact_id(),
        row_link_artifact_id: artifact.row_link_artifact_id(),
        embedding_provenance_artifact_id: artifact.provenance_artifact_id(),
        embedding_qc_summary,
        table_logical_digest: artifact.logical_digest(),
    })
}

/// Compare mean present cell-embedding vectors for exact declared binary groups.
///
/// The materialized table is bound to its existing verified artifact, row identities are bound to
/// the declared input, and explicit row/component/working limits are admitted before allocation or
/// component arithmetic. Present components are accumulated as `f64` in canonical row/component
/// order; all non-present statuses are counted but excluded. Runtime is `O(N*D)` when available,
/// with exactly two `D`-component `f64` accumulators. The result is descriptive only and proves no
/// spatial association, classification/separability, embedding quality, independent validation,
/// patient effect, inference, real-source result, or biological meaning.
///
/// # Errors
///
/// Returns a typed category when the verified artifact/table or ordered CellIds disagree, a caller
/// limit is insufficient, a checked size overflows, or the bounded accumulator allocation fails.
pub fn declared_binary_cell_embedding_centroid_discrepancy(
    input: &DeclaredScalarPatternInput<'_>,
    table: &CellEmbeddingTable,
    artifact: CellEmbeddingArtifact,
    maximum_rows: usize,
    maximum_component_operations: u64,
    maximum_working_bytes: usize,
) -> Result<
    DeclaredBinaryCellEmbeddingCentroidDiscrepancy,
    DeclaredBinaryCellEmbeddingCentroidDiscrepancyError,
> {
    let binding = bind_declared_binary_cell_embedding_centroid(
        input,
        table,
        artifact,
        maximum_rows,
        maximum_component_operations,
        maximum_working_bytes,
    )?;
    let dimension_usize = usize::try_from(binding.dimension)
        .map_err(|_| DeclaredBinaryCellEmbeddingCentroidDiscrepancyError::SizeOverflow)?;
    let working_bytes = dimension_usize
        .checked_mul(size_of::<f64>())
        .and_then(|value| value.checked_mul(2))
        .ok_or(DeclaredBinaryCellEmbeddingCentroidDiscrepancyError::SizeOverflow)?;

    let mean_squared_component_difference =
        if binding.status() == DeclaredBinaryCellEmbeddingCentroidDiscrepancyStatus::Available {
            let mut marked_sums = zeroed_accumulator(dimension_usize, working_bytes)?;
            let mut unmarked_sums = zeroed_accumulator(dimension_usize, working_bytes)?;
            for (row_index, &mark) in input.pattern().mark.iter().enumerate() {
                let embedding_row = table.row(row_index).map_err(|_| {
                    DeclaredBinaryCellEmbeddingCentroidDiscrepancyError::CellIdBindingMismatch {
                        row: row_index,
                    }
                })?;
                let Some(vector) = embedding_row.vector() else {
                    continue;
                };
                let sums = if mark == 1 {
                    &mut marked_sums
                } else {
                    &mut unmarked_sums
                };
                for (sum, &component) in sums.iter_mut().zip(vector) {
                    *sum += f64::from(component);
                }
            }
            let marked_denominator = binding.marked_counts.present_count() as f64;
            let unmarked_denominator = binding.unmarked_counts.present_count() as f64;
            let mut squared_difference = 0.0_f64;
            for (&marked_sum, &unmarked_sum) in marked_sums.iter().zip(&unmarked_sums) {
                let marked_mean = marked_sum / marked_denominator;
                let unmarked_mean = unmarked_sum / unmarked_denominator;
                let difference = marked_mean - unmarked_mean;
                squared_difference += difference * difference;
            }
            let mean = squared_difference / f64::from(binding.dimension);
            Some(if mean == 0.0 { 0.0 } else { mean })
        } else {
            None
        };

    Ok(binding
        .attach_value(mean_squared_component_difference)
        .expect("S7 arithmetic must match its bound availability"))
}

fn write_grouping_part(
    writer: &mut ContentDigestWriter,
    bytes: &[u8],
) -> Result<(), DeclaredBinaryCellEmbeddingCentroidDiscrepancyError> {
    writer
        .write_all(&(bytes.len() as u128).to_be_bytes())
        .and_then(|()| writer.write_all(bytes))
        .map_err(|_| DeclaredBinaryCellEmbeddingCentroidDiscrepancyError::SizeOverflow)
}

fn zeroed_accumulator(
    dimension: usize,
    requested: usize,
) -> Result<Vec<f64>, DeclaredBinaryCellEmbeddingCentroidDiscrepancyError> {
    let mut values = Vec::new();
    values.try_reserve_exact(dimension).map_err(|_| {
        DeclaredBinaryCellEmbeddingCentroidDiscrepancyError::AllocationFailed { requested }
    })?;
    values.resize(dimension, 0.0);
    Ok(values)
}
