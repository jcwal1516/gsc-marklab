use std::io::Write;

use marklab_workflow::{ContentDigest, ContentDigestWriter, MarklabProject};
use thiserror::Error;

use crate::scalar_mark::{
    validate_nucleus_area_um2_provenance, BinaryMarkDeclaration, DeclaredScalarIdentity,
    DeclaredScalarInputError, DeclaredScalarPatternInput, NucleusAreaUm2MarkDeclaration,
    ProbabilityMarkDeclaration,
};

const PAIRED_VALUES_DIGEST_DOMAIN: &[u8] =
    b"marklab-declared-binary-group-nucleus-area-um2-values-v1";

/// Availability of the declared binary-group nucleus-area contrast.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclaredBinaryGroupNucleusAreaContrastStatus {
    /// Both exact binary groups contain at least one row.
    Available,
    /// At least one exact binary group contains no rows.
    InsufficientGroups,
}

/// Descriptive within-input nucleus-area means and their signed binary-group contrast.
#[derive(Clone, Debug, PartialEq)]
pub struct DeclaredBinaryGroupNucleusAreaContrast {
    status: DeclaredBinaryGroupNucleusAreaContrastStatus,
    row_count: usize,
    marked_count: usize,
    unmarked_count: usize,
    marked_mean_nucleus_area_um2: Option<f64>,
    unmarked_mean_nucleus_area_um2: Option<f64>,
    marked_minus_unmarked_mean_nucleus_area_um2: Option<f64>,
    scalar_identity: DeclaredScalarIdentity,
    binary_mark: BinaryMarkDeclaration,
    probability_mark: Option<ProbabilityMarkDeclaration>,
    nucleus_area_mark: NucleusAreaUm2MarkDeclaration,
    paired_values_logical_digest: ContentDigest,
}

impl DeclaredBinaryGroupNucleusAreaContrast {
    /// Whether both binary groups support the signed contrast.
    pub fn status(&self) -> DeclaredBinaryGroupNucleusAreaContrastStatus {
        self.status
    }

    /// Exact dense input row count.
    pub fn row_count(&self) -> usize {
        self.row_count
    }

    /// Exact number of binary-marked rows.
    pub fn marked_count(&self) -> usize {
        self.marked_count
    }

    /// Exact number of binary-unmarked rows.
    pub fn unmarked_count(&self) -> usize {
        self.unmarked_count
    }

    /// Mean declared nucleus area among marked rows, when that group is nonempty.
    pub fn marked_mean_nucleus_area_um2(&self) -> Option<f64> {
        self.marked_mean_nucleus_area_um2
    }

    /// Mean declared nucleus area among unmarked rows, when that group is nonempty.
    pub fn unmarked_mean_nucleus_area_um2(&self) -> Option<f64> {
        self.unmarked_mean_nucleus_area_um2
    }

    /// Signed marked-minus-unmarked mean area, when both groups are nonempty.
    pub fn marked_minus_unmarked_mean_nucleus_area_um2(&self) -> Option<f64> {
        self.marked_minus_unmarked_mean_nucleus_area_um2
    }

    /// Exact declared row, slide, frame, and binary/probability identity.
    pub fn scalar_identity(&self) -> &DeclaredScalarIdentity {
        &self.scalar_identity
    }

    /// Required binary declaration that selects the two groups.
    pub fn binary_mark(&self) -> &BinaryMarkDeclaration {
        &self.binary_mark
    }

    /// Optional probability declaration retained as unused context.
    pub fn probability_mark(&self) -> Option<&ProbabilityMarkDeclaration> {
        self.probability_mark.as_ref()
    }

    /// Exact fixed nucleus-area declaration whose values are summarized.
    pub fn nucleus_area_mark(&self) -> &NucleusAreaUm2MarkDeclaration {
        &self.nucleus_area_mark
    }

    /// Domain-separated identity of every ordered binary/area value pair.
    pub fn paired_values_logical_digest(&self) -> ContentDigest {
        self.paired_values_logical_digest
    }
}

/// Invalid declaration/value binding or caller row limit.
#[derive(Debug, Error)]
pub enum DeclaredBinaryGroupNucleusAreaContrastError {
    /// Existing declared scalar input or exact nucleus-area provenance is invalid.
    #[error(transparent)]
    ScalarInput(#[from] DeclaredScalarInputError),
    /// The compatibility Pattern has no dense nucleus-area column.
    #[error("declared binary nucleus-area contrast requires Pattern::nucleus_area_um2")]
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
    /// Row traversal exceeds the caller maximum.
    #[error("declared binary nucleus-area rows {required} exceed caller maximum {maximum}")]
    RowCountBudgetExceeded {
        /// Exact input row count.
        required: usize,
        /// Caller-provided row maximum.
        maximum: usize,
    },
    /// A checked count or digest write overflowed.
    #[error("declared binary nucleus-area size computation overflowed")]
    SizeOverflow,
}

/// Compare declared nucleus-area means across exact binary-marked and unmarked rows.
///
/// The target project and fixed nucleus-area provenance are revalidated before a bounded stored-
/// order pass over dense positive square-micrometre values. Optional probabilities are retained as
/// context but never select groups. Runtime is `O(N)` with fixed incremental storage. The result is
/// descriptive only and proves no patient/specimen effect, segmentation accuracy, classification,
/// spatial association, inference, real-source result, or biological meaning.
///
/// # Errors
///
/// Returns a typed category when project/provenance validation fails, the nucleus-area column is
/// absent or invalid, the row limit is insufficient, or a checked count/digest operation fails.
pub fn declared_binary_group_nucleus_area_contrast(
    project: &MarklabProject,
    input: &DeclaredScalarPatternInput<'_>,
    nucleus_area_mark: NucleusAreaUm2MarkDeclaration,
    maximum_rows: usize,
) -> Result<DeclaredBinaryGroupNucleusAreaContrast, DeclaredBinaryGroupNucleusAreaContrastError> {
    input.revalidate_project(project)?;
    validate_nucleus_area_um2_provenance(project, &nucleus_area_mark)?;
    let nucleus_areas = input
        .pattern()
        .nucleus_area_um2
        .as_deref()
        .ok_or(DeclaredBinaryGroupNucleusAreaContrastError::NucleusAreaColumnRequired)?;
    let row_count = input.pattern().len();
    if nucleus_areas.len() != row_count {
        return Err(
            DeclaredBinaryGroupNucleusAreaContrastError::NucleusAreaLengthMismatch {
                expected: row_count,
                observed: nucleus_areas.len(),
            },
        );
    }
    if row_count > maximum_rows {
        return Err(
            DeclaredBinaryGroupNucleusAreaContrastError::RowCountBudgetExceeded {
                required: row_count,
                maximum: maximum_rows,
            },
        );
    }

    let row_count_u64 = u64::try_from(row_count)
        .map_err(|_| DeclaredBinaryGroupNucleusAreaContrastError::SizeOverflow)?;
    let mut paired_digest = ContentDigest::builder();
    write_digest_part(&mut paired_digest, PAIRED_VALUES_DIGEST_DOMAIN)?;
    write_digest_part(
        &mut paired_digest,
        input.scalar_identity().cell_ids_logical_digest().as_bytes(),
    )?;
    write_digest_part(
        &mut paired_digest,
        input
            .scalar_identity()
            .declared_input_logical_digest()
            .as_bytes(),
    )?;
    write_digest_part(
        &mut paired_digest,
        nucleus_area_mark
            .provenance_artifact_id()
            .digest()
            .as_bytes(),
    )?;
    write_digest_part(&mut paired_digest, &row_count_u64.to_be_bytes())?;

    let mut marked_count = 0_usize;
    let mut unmarked_count = 0_usize;
    let mut marked_sum = 0.0_f64;
    let mut unmarked_sum = 0.0_f64;
    for (row, (&binary, &area)) in input.pattern().mark.iter().zip(nucleus_areas).enumerate() {
        if !area.is_finite() || area <= 0.0 {
            return Err(
                DeclaredBinaryGroupNucleusAreaContrastError::InvalidNucleusAreaValue { row },
            );
        }
        paired_digest
            .write_all(&[binary])
            .and_then(|()| paired_digest.write_all(&area.to_bits().to_be_bytes()))
            .map_err(|_| DeclaredBinaryGroupNucleusAreaContrastError::SizeOverflow)?;
        if binary == 1 {
            marked_count = marked_count
                .checked_add(1)
                .ok_or(DeclaredBinaryGroupNucleusAreaContrastError::SizeOverflow)?;
            marked_sum += f64::from(area);
        } else {
            unmarked_count = unmarked_count
                .checked_add(1)
                .ok_or(DeclaredBinaryGroupNucleusAreaContrastError::SizeOverflow)?;
            unmarked_sum += f64::from(area);
        }
    }
    let (paired_values_logical_digest, _) = paired_digest.finish();
    let marked_mean_nucleus_area_um2 = group_mean(marked_sum, marked_count);
    let unmarked_mean_nucleus_area_um2 = group_mean(unmarked_sum, unmarked_count);
    let status = if marked_count > 0 && unmarked_count > 0 {
        DeclaredBinaryGroupNucleusAreaContrastStatus::Available
    } else {
        DeclaredBinaryGroupNucleusAreaContrastStatus::InsufficientGroups
    };
    let marked_minus_unmarked_mean_nucleus_area_um2 =
        match (marked_mean_nucleus_area_um2, unmarked_mean_nucleus_area_um2) {
            (Some(marked), Some(unmarked)) => {
                let difference = marked - unmarked;
                Some(if difference == 0.0 { 0.0 } else { difference })
            }
            _ => None,
        };

    Ok(DeclaredBinaryGroupNucleusAreaContrast {
        status,
        row_count,
        marked_count,
        unmarked_count,
        marked_mean_nucleus_area_um2,
        unmarked_mean_nucleus_area_um2,
        marked_minus_unmarked_mean_nucleus_area_um2,
        scalar_identity: input.scalar_identity().clone(),
        binary_mark: input.binary_mark().clone(),
        probability_mark: input.probability_mark().cloned(),
        nucleus_area_mark,
        paired_values_logical_digest,
    })
}

fn group_mean(sum: f64, count: usize) -> Option<f64> {
    (count > 0).then(|| {
        let mean = sum / count as f64;
        if mean == 0.0 {
            0.0
        } else {
            mean
        }
    })
}

fn write_digest_part(
    writer: &mut ContentDigestWriter,
    bytes: &[u8],
) -> Result<(), DeclaredBinaryGroupNucleusAreaContrastError> {
    writer
        .write_all(&(bytes.len() as u128).to_be_bytes())
        .and_then(|()| writer.write_all(bytes))
        .map_err(|_| DeclaredBinaryGroupNucleusAreaContrastError::SizeOverflow)
}
