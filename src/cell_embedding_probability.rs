use std::io::Write;

use marklab_embeddings::{
    CellEmbeddingArtifact, CellEmbeddingTable, EmbeddingQcSummary, EmbeddingStatus,
};
use marklab_workflow::{ArtifactId, ContentDigest, ContentDigestWriter};
use thiserror::Error;

use crate::{
    cell_embedding_cross_covariance::{
        energy as scalar_embedding_cross_covariance_energy,
        requirements as scalar_embedding_cross_covariance_requirements,
        ScalarEmbeddingCrossCovarianceError,
    },
    BinaryMarkDeclaration, DeclaredScalarIdentity, DeclaredScalarPatternInput,
    ProbabilityMarkDeclaration,
};

const PROBABILITY_VALUES_DIGEST_DOMAIN: &[u8] =
    b"marklab-declared-probability-cell-embedding-values-v1";

/// Availability of declared probability–cell-embedding cross-covariance energy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclaredProbabilityCellEmbeddingCrossCovarianceStatus {
    /// At least two present rows carry nonconstant declared probabilities.
    Available,
    /// Fewer than two embedding rows are present.
    InsufficientPresentRows,
    /// At least two rows are present but their declared probabilities are equal.
    NoProbabilityVariation,
}

/// Descriptive probability–embedding cross-covariance energy over exact aligned rows.
#[derive(Clone, Debug, PartialEq)]
pub struct DeclaredProbabilityCellEmbeddingCrossCovarianceEnergy {
    status: DeclaredProbabilityCellEmbeddingCrossCovarianceStatus,
    present_row_count: u64,
    present_probability_mean: Option<f64>,
    dimension: u32,
    cross_covariance_energy: Option<f64>,
    scalar_identity: DeclaredScalarIdentity,
    binary_mark: BinaryMarkDeclaration,
    probability_mark: ProbabilityMarkDeclaration,
    probability_values_logical_digest: ContentDigest,
    embedding_artifact_id: ArtifactId,
    row_link_artifact_id: ArtifactId,
    embedding_provenance_artifact_id: ArtifactId,
    embedding_qc_summary: EmbeddingQcSummary,
    table_logical_digest: ContentDigest,
}

impl DeclaredProbabilityCellEmbeddingCrossCovarianceEnergy {
    /// Whether the numeric energy is available and, if not, why.
    pub fn status(&self) -> DeclaredProbabilityCellEmbeddingCrossCovarianceStatus {
        self.status
    }

    /// Exact number of embedding rows whose vectors are present.
    pub fn present_row_count(&self) -> u64 {
        self.present_row_count
    }

    /// Fixed-order mean declared probability among present embedding rows.
    pub fn present_probability_mean(&self) -> Option<f64> {
        self.present_probability_mean
    }

    /// Exact positive embedding dimension.
    pub fn dimension(&self) -> u32 {
        self.dimension
    }

    /// Mean squared component population cross-covariance, when available.
    pub fn cross_covariance_energy(&self) -> Option<f64> {
        self.cross_covariance_energy
    }

    /// Exact declared row, slide, frame, and declaration identity.
    pub fn scalar_identity(&self) -> &DeclaredScalarIdentity {
        &self.scalar_identity
    }

    /// Required binary declaration retained as context but unused by the estimand.
    pub fn binary_mark(&self) -> &BinaryMarkDeclaration {
        &self.binary_mark
    }

    /// Exact probability declaration whose dense values enter the estimand.
    pub fn probability_mark(&self) -> &ProbabilityMarkDeclaration {
        &self.probability_mark
    }

    /// Domain-separated identity of all ordered CellId-bound probability `f32` bits.
    pub fn probability_values_logical_digest(&self) -> ContentDigest {
        self.probability_values_logical_digest
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

/// Missing probability modality, invalid binding, caller limit, or bounded allocation.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum DeclaredProbabilityCellEmbeddingCrossCovarianceError {
    /// The declared input has no dense probability mark and declaration.
    #[error("declared probability–cell-embedding covariance requires a probability mark")]
    ProbabilityMarkRequired,
    /// The materialized table does not match the verified embedding artifact.
    #[error("declared probability cell-embedding table and verified artifact disagree")]
    EmbeddingArtifactBindingMismatch,
    /// A typed vector-artifact mark references a different verified embedding artifact.
    #[error("declared vector mark and supplied verified embedding artifact disagree")]
    VectorArtifactReferenceMismatch,
    /// Declared and embedding CellIds disagree at one row.
    #[error("declared probability and cell-embedding identities disagree at row {row}")]
    CellIdBindingMismatch {
        /// First mismatched or absent row.
        row: usize,
    },
    /// The admitted row traversal exceeds the caller maximum.
    #[error("declared probability cell-embedding rows {required} exceed caller maximum {maximum}")]
    RowCountBudgetExceeded {
        /// Exact table row count.
        required: usize,
        /// Caller-provided row maximum.
        maximum: usize,
    },
    /// Conservative component visits exceed the caller maximum.
    #[error(
        "declared probability cell-embedding component operations {required} exceed caller maximum {maximum}"
    )]
    ComponentOperationBudgetExceeded {
        /// Checked two present-row passes plus two dimension passes.
        required: u64,
        /// Caller-provided component-operation maximum.
        maximum: u64,
    },
    /// Exact two-accumulator storage exceeds the caller maximum.
    #[error(
        "declared probability cell-embedding working bytes {required} exceed caller maximum {maximum}"
    )]
    WorkingByteBudgetExceeded {
        /// Exact two-accumulator byte count.
        required: usize,
        /// Caller-provided working-byte maximum.
        maximum: usize,
    },
    /// A checked count or byte computation overflowed.
    #[error("declared probability cell-embedding size computation overflowed")]
    SizeOverflow,
    /// Exact bounded accumulator allocation failed.
    #[error(
        "declared probability cell-embedding accumulator allocation failed for {requested} bytes"
    )]
    AllocationFailed {
        /// Exact requested working bytes.
        requested: usize,
    },
}

impl From<ScalarEmbeddingCrossCovarianceError>
    for DeclaredProbabilityCellEmbeddingCrossCovarianceError
{
    fn from(error: ScalarEmbeddingCrossCovarianceError) -> Self {
        match error {
            ScalarEmbeddingCrossCovarianceError::RowUnavailable { row } => {
                Self::CellIdBindingMismatch { row }
            }
            ScalarEmbeddingCrossCovarianceError::SizeOverflow => Self::SizeOverflow,
            ScalarEmbeddingCrossCovarianceError::AllocationFailed { requested } => {
                Self::AllocationFailed { requested }
            }
        }
    }
}

/// Measure population cross-covariance energy for a declared probability and cell embeddings.
///
/// The probability modality is required, the materialized table is bound to its existing verified
/// artifact, and every ordered CellId is bound before explicit row/component/working limits admit
/// arithmetic. Present rows alone enter a fixed-order two-pass `f64` population covariance; binary
/// rows are retained only as declared context. Runtime is `O(N*D)` when available with exactly two
/// `D`-component `f64` accumulators. The result is descriptive only and proves no correlation,
/// spatial association, classification, calibration, embedding quality, patient effect, inference,
/// real-source result, or biological meaning.
///
/// # Errors
///
/// Returns a typed category when the probability modality is absent, the verified artifact/table
/// or ordered CellIds disagree, a caller limit is insufficient, a checked size overflows, or the
/// bounded accumulator allocation fails.
pub fn declared_probability_cell_embedding_cross_covariance_energy(
    input: &DeclaredScalarPatternInput<'_>,
    table: &CellEmbeddingTable,
    artifact: CellEmbeddingArtifact,
    maximum_rows: usize,
    maximum_component_operations: u64,
    maximum_working_bytes: usize,
) -> Result<
    DeclaredProbabilityCellEmbeddingCrossCovarianceEnergy,
    DeclaredProbabilityCellEmbeddingCrossCovarianceError,
> {
    let probability_mark = input
        .probability_mark()
        .ok_or(DeclaredProbabilityCellEmbeddingCrossCovarianceError::ProbabilityMarkRequired)?;
    let probabilities = input
        .pattern()
        .mark_prob
        .as_deref()
        .ok_or(DeclaredProbabilityCellEmbeddingCrossCovarianceError::ProbabilityMarkRequired)?;

    let embedding_qc_summary = table.qc_summary();
    if embedding_qc_summary != artifact.qc_summary() {
        return Err(
            DeclaredProbabilityCellEmbeddingCrossCovarianceError::EmbeddingArtifactBindingMismatch,
        );
    }
    if input
        .vector_artifact_ref()
        .is_some_and(|declared_artifact| declared_artifact != artifact)
    {
        return Err(
            DeclaredProbabilityCellEmbeddingCrossCovarianceError::VectorArtifactReferenceMismatch,
        );
    }

    let row_count = table.row_count();
    if row_count > maximum_rows {
        return Err(
            DeclaredProbabilityCellEmbeddingCrossCovarianceError::RowCountBudgetExceeded {
                required: row_count,
                maximum: maximum_rows,
            },
        );
    }
    if input.cell_ids().len() != row_count || probabilities.len() != row_count {
        return Err(
            DeclaredProbabilityCellEmbeddingCrossCovarianceError::CellIdBindingMismatch {
                row: input
                    .cell_ids()
                    .len()
                    .min(probabilities.len())
                    .min(row_count),
            },
        );
    }

    let mut probability_digest = ContentDigest::builder();
    write_probability_part(&mut probability_digest, PROBABILITY_VALUES_DIGEST_DOMAIN)?;
    write_probability_part(
        &mut probability_digest,
        input.scalar_identity().cell_ids_logical_digest().as_bytes(),
    )?;
    write_probability_part(
        &mut probability_digest,
        input
            .scalar_identity()
            .declared_input_logical_digest()
            .as_bytes(),
    )?;
    let row_count_u64 = u64::try_from(row_count)
        .map_err(|_| DeclaredProbabilityCellEmbeddingCrossCovarianceError::SizeOverflow)?;
    write_probability_part(&mut probability_digest, &row_count_u64.to_be_bytes())?;

    let mut present_probability_count = 0_u64;
    let mut present_probability_sum = 0.0_f64;
    let mut first_present_probability = None;
    let mut has_probability_variation = false;
    for (row_index, ((cell_id, &probability), embedding_row)) in input
        .cell_ids()
        .iter()
        .zip(probabilities)
        .zip((0..row_count).map(|index| table.row(index)))
        .enumerate()
    {
        let embedding_row = embedding_row.map_err(|_| {
            DeclaredProbabilityCellEmbeddingCrossCovarianceError::CellIdBindingMismatch {
                row: row_index,
            }
        })?;
        if embedding_row.cell_id() != cell_id {
            return Err(
                DeclaredProbabilityCellEmbeddingCrossCovarianceError::CellIdBindingMismatch {
                    row: row_index,
                },
            );
        }
        probability_digest
            .write_all(&probability.to_bits().to_be_bytes())
            .map_err(|_| DeclaredProbabilityCellEmbeddingCrossCovarianceError::SizeOverflow)?;
        if embedding_row.status() == EmbeddingStatus::Present {
            present_probability_count = present_probability_count
                .checked_add(1)
                .ok_or(DeclaredProbabilityCellEmbeddingCrossCovarianceError::SizeOverflow)?;
            present_probability_sum += f64::from(probability);
            if let Some(first) = first_present_probability {
                has_probability_variation |= probability != first;
            } else {
                first_present_probability = Some(probability);
            }
        }
    }
    if present_probability_count != embedding_qc_summary.present_count() {
        return Err(
            DeclaredProbabilityCellEmbeddingCrossCovarianceError::EmbeddingArtifactBindingMismatch,
        );
    }
    let (probability_values_logical_digest, _) = probability_digest.finish();

    let dimension = embedding_qc_summary.dimension();
    let covariance_requirements =
        scalar_embedding_cross_covariance_requirements(present_probability_count, dimension)
            .map_err(DeclaredProbabilityCellEmbeddingCrossCovarianceError::from)?;
    if covariance_requirements.component_operations > maximum_component_operations {
        return Err(
            DeclaredProbabilityCellEmbeddingCrossCovarianceError::ComponentOperationBudgetExceeded {
                required: covariance_requirements.component_operations,
                maximum: maximum_component_operations,
            },
        );
    }
    if covariance_requirements.working_bytes > maximum_working_bytes {
        return Err(
            DeclaredProbabilityCellEmbeddingCrossCovarianceError::WorkingByteBudgetExceeded {
                required: covariance_requirements.working_bytes,
                maximum: maximum_working_bytes,
            },
        );
    }

    let present_probability_mean = (present_probability_count > 0).then(|| {
        let mean = present_probability_sum / present_probability_count as f64;
        if mean == 0.0 {
            0.0
        } else {
            mean
        }
    });
    let status = if present_probability_count < 2 {
        DeclaredProbabilityCellEmbeddingCrossCovarianceStatus::InsufficientPresentRows
    } else if !has_probability_variation {
        DeclaredProbabilityCellEmbeddingCrossCovarianceStatus::NoProbabilityVariation
    } else {
        DeclaredProbabilityCellEmbeddingCrossCovarianceStatus::Available
    };

    let cross_covariance_energy =
        if status == DeclaredProbabilityCellEmbeddingCrossCovarianceStatus::Available {
            Some(
                scalar_embedding_cross_covariance_energy(
                    table,
                    probabilities,
                    present_probability_count,
                    present_probability_mean.expect("available mean must exist"),
                    covariance_requirements,
                )
                .map_err(DeclaredProbabilityCellEmbeddingCrossCovarianceError::from)?,
            )
        } else {
            None
        };

    Ok(DeclaredProbabilityCellEmbeddingCrossCovarianceEnergy {
        status,
        present_row_count: present_probability_count,
        present_probability_mean,
        dimension,
        cross_covariance_energy,
        scalar_identity: input.scalar_identity().clone(),
        binary_mark: input.binary_mark().clone(),
        probability_mark: probability_mark.clone(),
        probability_values_logical_digest,
        embedding_artifact_id: artifact.embedding_artifact_id(),
        row_link_artifact_id: artifact.row_link_artifact_id(),
        embedding_provenance_artifact_id: artifact.provenance_artifact_id(),
        embedding_qc_summary,
        table_logical_digest: artifact.logical_digest(),
    })
}

fn write_probability_part(
    writer: &mut ContentDigestWriter,
    bytes: &[u8],
) -> Result<(), DeclaredProbabilityCellEmbeddingCrossCovarianceError> {
    writer
        .write_all(&(bytes.len() as u128).to_be_bytes())
        .and_then(|()| writer.write_all(bytes))
        .map_err(|_| DeclaredProbabilityCellEmbeddingCrossCovarianceError::SizeOverflow)
}
