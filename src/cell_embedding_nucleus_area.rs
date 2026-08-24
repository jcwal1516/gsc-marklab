use std::{io::Write, mem::size_of};

use marklab_embeddings::{
    CellEmbeddingArtifact, CellEmbeddingTable, EmbeddingQcSummary, EmbeddingStatus,
};
use marklab_workflow::{ArtifactId, ContentDigest, ContentDigestWriter, MarklabProject};
use thiserror::Error;

use crate::scalar_mark::{
    validate_nucleus_area_um2_provenance, BinaryMarkDeclaration, DeclaredScalarIdentity,
    DeclaredScalarInputError, DeclaredScalarPatternInput, NucleusAreaUm2MarkDeclaration,
    ProbabilityMarkDeclaration,
};

const NUCLEUS_AREA_VALUES_DIGEST_DOMAIN: &[u8] =
    b"marklab-declared-nucleus-area-um2-cell-embedding-values-v1";

/// Availability of declared nucleus-area–cell-embedding cross-covariance energy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclaredNucleusAreaCellEmbeddingCrossCovarianceStatus {
    /// At least two present rows carry nonconstant declared nucleus areas.
    Available,
    /// Fewer than two embedding rows are present.
    InsufficientPresentRows,
    /// At least two rows are present but their declared nucleus areas are equal.
    NoNucleusAreaVariation,
}

/// Descriptive nucleus-area–embedding cross-covariance energy over exact aligned rows.
#[derive(Clone, Debug, PartialEq)]
pub struct DeclaredNucleusAreaCellEmbeddingCrossCovarianceEnergy {
    status: DeclaredNucleusAreaCellEmbeddingCrossCovarianceStatus,
    present_row_count: u64,
    present_nucleus_area_mean_um2: Option<f64>,
    dimension: u32,
    cross_covariance_energy: Option<f64>,
    scalar_identity: DeclaredScalarIdentity,
    binary_mark: BinaryMarkDeclaration,
    probability_mark: Option<ProbabilityMarkDeclaration>,
    nucleus_area_mark: NucleusAreaUm2MarkDeclaration,
    nucleus_area_values_logical_digest: ContentDigest,
    embedding_artifact_id: ArtifactId,
    row_link_artifact_id: ArtifactId,
    embedding_provenance_artifact_id: ArtifactId,
    embedding_qc_summary: EmbeddingQcSummary,
    table_logical_digest: ContentDigest,
}

impl DeclaredNucleusAreaCellEmbeddingCrossCovarianceEnergy {
    /// Whether the numeric energy is available and, if not, why.
    pub fn status(&self) -> DeclaredNucleusAreaCellEmbeddingCrossCovarianceStatus {
        self.status
    }

    /// Exact number of embedding rows whose vectors are present.
    pub fn present_row_count(&self) -> u64 {
        self.present_row_count
    }

    /// Fixed-order mean declared nucleus area among present embedding rows.
    pub fn present_nucleus_area_mean_um2(&self) -> Option<f64> {
        self.present_nucleus_area_mean_um2
    }

    /// Exact positive embedding dimension.
    pub fn dimension(&self) -> u32 {
        self.dimension
    }

    /// Mean squared component population cross-covariance, when available.
    pub fn cross_covariance_energy(&self) -> Option<f64> {
        self.cross_covariance_energy
    }

    /// Exact declared row, slide, frame, and binary/probability declaration identity.
    pub fn scalar_identity(&self) -> &DeclaredScalarIdentity {
        &self.scalar_identity
    }

    /// Required binary declaration retained as context but unused by the estimand.
    pub fn binary_mark(&self) -> &BinaryMarkDeclaration {
        &self.binary_mark
    }

    /// Optional probability declaration retained as context but unused by the estimand.
    pub fn probability_mark(&self) -> Option<&ProbabilityMarkDeclaration> {
        self.probability_mark.as_ref()
    }

    /// Exact concrete nucleus-area declaration whose dense values enter the estimand.
    pub fn nucleus_area_mark(&self) -> &NucleusAreaUm2MarkDeclaration {
        &self.nucleus_area_mark
    }

    /// Domain-separated identity of all ordered CellId-bound nucleus-area `f32` bits.
    pub fn nucleus_area_values_logical_digest(&self) -> ContentDigest {
        self.nucleus_area_values_logical_digest
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

/// Invalid declaration/value/artifact binding, caller limit, or bounded allocation.
#[derive(Debug, Error)]
pub enum DeclaredNucleusAreaCellEmbeddingCrossCovarianceError {
    /// Existing declared scalar input or exact nucleus-area provenance is invalid.
    #[error(transparent)]
    ScalarInput(#[from] DeclaredScalarInputError),
    /// The compatibility Pattern has no dense nucleus-area column.
    #[error("declared nucleus-area covariance requires Pattern::nucleus_area_um2")]
    NucleusAreaColumnRequired,
    /// The dense nucleus-area column has the wrong number of rows.
    #[error("nucleus-area column has {observed} rows; expected {expected}")]
    NucleusAreaLengthMismatch {
        /// Exact compatibility Pattern row count.
        expected: usize,
        /// Observed nucleus-area row count.
        observed: usize,
    },
    /// A nucleus-area value is non-finite or not strictly positive.
    #[error("nucleus area at row {row} must be finite and strictly positive")]
    InvalidNucleusAreaValue {
        /// First invalid row.
        row: usize,
    },
    /// The materialized table does not match the verified embedding artifact.
    #[error("declared nucleus-area cell-embedding table and verified artifact disagree")]
    EmbeddingArtifactBindingMismatch,
    /// Declared and embedding CellIds disagree at one row.
    #[error("declared nucleus-area and cell-embedding identities disagree at row {row}")]
    CellIdBindingMismatch {
        /// First mismatched or absent row.
        row: usize,
    },
    /// The admitted row traversal exceeds the caller maximum.
    #[error(
        "declared nucleus-area cell-embedding rows {required} exceed caller maximum {maximum}"
    )]
    RowCountBudgetExceeded {
        /// Exact table row count.
        required: usize,
        /// Caller-provided row maximum.
        maximum: usize,
    },
    /// Conservative component visits exceed the caller maximum.
    #[error(
        "declared nucleus-area cell-embedding component operations {required} exceed caller maximum {maximum}"
    )]
    ComponentOperationBudgetExceeded {
        /// Checked two present-row passes plus two dimension passes.
        required: u64,
        /// Caller-provided component-operation maximum.
        maximum: u64,
    },
    /// Exact two-accumulator storage exceeds the caller maximum.
    #[error(
        "declared nucleus-area cell-embedding working bytes {required} exceed caller maximum {maximum}"
    )]
    WorkingByteBudgetExceeded {
        /// Exact two-accumulator byte count.
        required: usize,
        /// Caller-provided working-byte maximum.
        maximum: usize,
    },
    /// A checked count or byte computation overflowed.
    #[error("declared nucleus-area cell-embedding size computation overflowed")]
    SizeOverflow,
    /// Exact bounded accumulator allocation failed.
    #[error(
        "declared nucleus-area cell-embedding accumulator allocation failed for {requested} bytes"
    )]
    AllocationFailed {
        /// Exact requested working bytes.
        requested: usize,
    },
}

/// Measure population cross-covariance energy for declared nucleus area and cell embeddings.
///
/// The target project and exact fixed nucleus-area provenance are revalidated before the dense
/// positive square-micrometre column is bound to every ordered CellId and to the existing verified
/// embedding artifact. Present rows alone enter a fixed-order two-pass `f64` population covariance;
/// binary and optional probability rows are retained only as declared context. Runtime is
/// `O(N*D)` when available with exactly two `D`-component `f64` accumulators. The result is
/// descriptive only and proves no correlation, normalization, segmentation accuracy, spatial
/// association, classification, calibration, embedding quality, patient effect, inference,
/// real-source result, or biological meaning.
///
/// # Errors
///
/// Returns a typed category when target-project/provenance validation fails, the nucleus-area
/// column is absent or invalid, the verified artifact/table or ordered CellIds disagree, a caller
/// limit is insufficient, a checked size overflows, or the bounded allocation fails.
#[allow(clippy::too_many_arguments)]
pub fn declared_nucleus_area_cell_embedding_cross_covariance_energy(
    project: &MarklabProject,
    input: &DeclaredScalarPatternInput<'_>,
    nucleus_area_mark: NucleusAreaUm2MarkDeclaration,
    table: &CellEmbeddingTable,
    artifact: CellEmbeddingArtifact,
    maximum_rows: usize,
    maximum_component_operations: u64,
    maximum_working_bytes: usize,
) -> Result<
    DeclaredNucleusAreaCellEmbeddingCrossCovarianceEnergy,
    DeclaredNucleusAreaCellEmbeddingCrossCovarianceError,
> {
    input.revalidate_project(project)?;
    validate_nucleus_area_um2_provenance(project, &nucleus_area_mark)?;
    let nucleus_areas =
        input.pattern().nucleus_area_um2.as_deref().ok_or(
            DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::NucleusAreaColumnRequired,
        )?;
    let pattern_rows = input.pattern().len();
    if nucleus_areas.len() != pattern_rows {
        return Err(
            DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::NucleusAreaLengthMismatch {
                expected: pattern_rows,
                observed: nucleus_areas.len(),
            },
        );
    }

    let embedding_qc_summary = table.qc_summary();
    if embedding_qc_summary != artifact.qc_summary() {
        return Err(
            DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::EmbeddingArtifactBindingMismatch,
        );
    }
    let row_count = table.row_count();
    if row_count > maximum_rows {
        return Err(
            DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::RowCountBudgetExceeded {
                required: row_count,
                maximum: maximum_rows,
            },
        );
    }
    if input.cell_ids().len() != row_count || nucleus_areas.len() != row_count {
        return Err(
            DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::CellIdBindingMismatch {
                row: input
                    .cell_ids()
                    .len()
                    .min(nucleus_areas.len())
                    .min(row_count),
            },
        );
    }

    let mut values_digest = ContentDigest::builder();
    write_digest_part(&mut values_digest, NUCLEUS_AREA_VALUES_DIGEST_DOMAIN)?;
    write_digest_part(
        &mut values_digest,
        input.scalar_identity().cell_ids_logical_digest().as_bytes(),
    )?;
    write_digest_part(
        &mut values_digest,
        input
            .scalar_identity()
            .declared_input_logical_digest()
            .as_bytes(),
    )?;
    write_digest_part(
        &mut values_digest,
        nucleus_area_mark
            .provenance_artifact_id()
            .digest()
            .as_bytes(),
    )?;
    let row_count_u64 = u64::try_from(row_count)
        .map_err(|_| DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::SizeOverflow)?;
    write_digest_part(&mut values_digest, &row_count_u64.to_be_bytes())?;

    let mut present_area_count = 0_u64;
    let mut present_area_sum = 0.0_f64;
    let mut first_present_area = None;
    let mut has_area_variation = false;
    for (row_index, ((cell_id, &area), embedding_row)) in input
        .cell_ids()
        .iter()
        .zip(nucleus_areas)
        .zip((0..row_count).map(|index| table.row(index)))
        .enumerate()
    {
        let embedding_row = embedding_row.map_err(|_| {
            DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::CellIdBindingMismatch {
                row: row_index,
            }
        })?;
        if embedding_row.cell_id() != cell_id {
            return Err(
                DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::CellIdBindingMismatch {
                    row: row_index,
                },
            );
        }
        if !area.is_finite() || area <= 0.0 {
            return Err(
                DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::InvalidNucleusAreaValue {
                    row: row_index,
                },
            );
        }
        values_digest
            .write_all(&area.to_bits().to_be_bytes())
            .map_err(|_| DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::SizeOverflow)?;
        if embedding_row.status() == EmbeddingStatus::Present {
            present_area_count = present_area_count
                .checked_add(1)
                .ok_or(DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::SizeOverflow)?;
            present_area_sum += f64::from(area);
            if let Some(first) = first_present_area {
                has_area_variation |= area != first;
            } else {
                first_present_area = Some(area);
            }
        }
    }
    if present_area_count != embedding_qc_summary.present_count() {
        return Err(
            DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::EmbeddingArtifactBindingMismatch,
        );
    }
    let (nucleus_area_values_logical_digest, _) = values_digest.finish();

    let dimension = embedding_qc_summary.dimension();
    let component_operations = required_component_operations(present_area_count, dimension)?;
    if component_operations > maximum_component_operations {
        return Err(
            DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::ComponentOperationBudgetExceeded {
                required: component_operations,
                maximum: maximum_component_operations,
            },
        );
    }
    let dimension_usize = usize::try_from(dimension)
        .map_err(|_| DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::SizeOverflow)?;
    let working_bytes = required_working_bytes(dimension_usize)?;
    if working_bytes > maximum_working_bytes {
        return Err(
            DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::WorkingByteBudgetExceeded {
                required: working_bytes,
                maximum: maximum_working_bytes,
            },
        );
    }

    let present_nucleus_area_mean_um2 = (present_area_count > 0).then(|| {
        let mean = present_area_sum / present_area_count as f64;
        if mean == 0.0 {
            0.0
        } else {
            mean
        }
    });
    let status = if present_area_count < 2 {
        DeclaredNucleusAreaCellEmbeddingCrossCovarianceStatus::InsufficientPresentRows
    } else if !has_area_variation {
        DeclaredNucleusAreaCellEmbeddingCrossCovarianceStatus::NoNucleusAreaVariation
    } else {
        DeclaredNucleusAreaCellEmbeddingCrossCovarianceStatus::Available
    };

    let cross_covariance_energy =
        if status == DeclaredNucleusAreaCellEmbeddingCrossCovarianceStatus::Available {
            let denominator = present_area_count as f64;
            let area_mean = present_nucleus_area_mean_um2.expect("available mean must exist");
            let mut embedding_means = zeroed_accumulator(dimension_usize, working_bytes)?;
            let mut covariance_sums = zeroed_accumulator(dimension_usize, working_bytes)?;
            for row_index in 0..row_count {
                let embedding_row = table.row(row_index).map_err(|_| {
                    DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::CellIdBindingMismatch {
                        row: row_index,
                    }
                })?;
                let Some(vector) = embedding_row.vector() else {
                    continue;
                };
                for (sum, &component) in embedding_means.iter_mut().zip(vector) {
                    *sum += f64::from(component);
                }
            }
            for mean in &mut embedding_means {
                *mean /= denominator;
            }
            for (row_index, &area) in nucleus_areas.iter().enumerate() {
                let embedding_row = table.row(row_index).map_err(|_| {
                    DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::CellIdBindingMismatch {
                        row: row_index,
                    }
                })?;
                let Some(vector) = embedding_row.vector() else {
                    continue;
                };
                let centered_area = f64::from(area) - area_mean;
                for ((sum, &component), &embedding_mean) in
                    covariance_sums.iter_mut().zip(vector).zip(&embedding_means)
                {
                    *sum += centered_area * (f64::from(component) - embedding_mean);
                }
            }
            let mut energy = 0.0_f64;
            for covariance_sum in covariance_sums {
                let covariance = covariance_sum / denominator;
                energy += covariance * covariance;
            }
            let energy = energy / f64::from(dimension);
            Some(if energy == 0.0 { 0.0 } else { energy })
        } else {
            None
        };

    Ok(DeclaredNucleusAreaCellEmbeddingCrossCovarianceEnergy {
        status,
        present_row_count: present_area_count,
        present_nucleus_area_mean_um2,
        dimension,
        cross_covariance_energy,
        scalar_identity: input.scalar_identity().clone(),
        binary_mark: input.binary_mark().clone(),
        probability_mark: input.probability_mark().cloned(),
        nucleus_area_mark,
        nucleus_area_values_logical_digest,
        embedding_artifact_id: artifact.embedding_artifact_id(),
        row_link_artifact_id: artifact.row_link_artifact_id(),
        embedding_provenance_artifact_id: artifact.provenance_artifact_id(),
        embedding_qc_summary,
        table_logical_digest: artifact.logical_digest(),
    })
}

fn required_component_operations(
    present_rows: u64,
    dimension: u32,
) -> Result<u64, DeclaredNucleusAreaCellEmbeddingCrossCovarianceError> {
    let dimension = u64::from(dimension);
    present_rows
        .checked_mul(dimension)
        .and_then(|value| value.checked_mul(2))
        .and_then(|value| {
            dimension
                .checked_mul(2)
                .and_then(|tail| value.checked_add(tail))
        })
        .ok_or(DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::SizeOverflow)
}

fn required_working_bytes(
    dimension: usize,
) -> Result<usize, DeclaredNucleusAreaCellEmbeddingCrossCovarianceError> {
    dimension
        .checked_mul(size_of::<f64>())
        .and_then(|value| value.checked_mul(2))
        .ok_or(DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::SizeOverflow)
}

fn write_digest_part(
    writer: &mut ContentDigestWriter,
    bytes: &[u8],
) -> Result<(), DeclaredNucleusAreaCellEmbeddingCrossCovarianceError> {
    writer
        .write_all(&(bytes.len() as u128).to_be_bytes())
        .and_then(|()| writer.write_all(bytes))
        .map_err(|_| DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::SizeOverflow)
}

fn zeroed_accumulator(
    dimension: usize,
    requested: usize,
) -> Result<Vec<f64>, DeclaredNucleusAreaCellEmbeddingCrossCovarianceError> {
    let mut values = Vec::new();
    values.try_reserve_exact(dimension).map_err(|_| {
        DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::AllocationFailed { requested }
    })?;
    values.resize(dimension, 0.0);
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_resource_helpers_expose_overflow_and_allocation_failures() {
        assert!(matches!(
            required_component_operations(u64::MAX, u32::MAX),
            Err(DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::SizeOverflow)
        ));
        assert!(matches!(
            required_working_bytes(usize::MAX),
            Err(DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::SizeOverflow)
        ));
        assert!(matches!(
            zeroed_accumulator(usize::MAX, usize::MAX),
            Err(
                DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::AllocationFailed {
                    requested: usize::MAX
                }
            )
        ));
    }
}
