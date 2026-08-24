use std::io::Write;

use marklab_data::{CellId, CoordinateFrameId, SlideId};
use marklab_workflow::{ContentDigest, ContentDigestWriter};

use super::{
    declaration::{
        measurement_status_name, BinaryMarkDeclaration, BinaryMarkOrigin, DeclaredMarkUse,
        ProbabilityMarkDeclaration,
    },
    DeclaredScalarInputError,
};

const CELL_IDS_DIGEST_DOMAIN: &[u8] = b"marklab-declared-scalar-cell-ids-v1";
const INPUT_DIGEST_DOMAIN: &[u8] = b"marklab-declared-scalar-pattern-v1";

/// Compact runtime identity for one declared scalar-pattern binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredScalarIdentity {
    row_count: usize,
    cell_ids_logical_digest: ContentDigest,
    owning_slide_id: SlideId,
    coordinate_frame_id: CoordinateFrameId,
    declared_input_logical_digest: ContentDigest,
}

impl DeclaredScalarIdentity {
    pub(super) fn new(
        row_count: usize,
        cell_ids_logical_digest: ContentDigest,
        owning_slide_id: SlideId,
        coordinate_frame_id: CoordinateFrameId,
        declared_input_logical_digest: ContentDigest,
    ) -> Self {
        Self {
            row_count,
            cell_ids_logical_digest,
            owning_slide_id,
            coordinate_frame_id,
            declared_input_logical_digest,
        }
    }

    /// Number of ordered CellId rows in the declared binding.
    pub fn row_count(&self) -> usize {
        self.row_count
    }

    /// Format-independent digest of the exact ordered CellId sequence.
    pub fn cell_ids_logical_digest(&self) -> ContentDigest {
        self.cell_ids_logical_digest
    }

    /// Explicit slide that owns every declared CellId.
    pub fn owning_slide_id(&self) -> &SlideId {
        &self.owning_slide_id
    }

    /// Explicit physical `[X,Y]` micrometre frame of the compatibility coordinates.
    pub fn coordinate_frame_id(&self) -> &CoordinateFrameId {
        &self.coordinate_frame_id
    }

    /// Digest of row, slide, frame, mark, status, threshold, and provenance identity.
    pub fn declared_input_logical_digest(&self) -> ContentDigest {
        self.declared_input_logical_digest
    }

    pub(crate) fn matches_mark_use(&self, mark_use: &DeclaredMarkUse) -> bool {
        declared_identity_from_cell_digest(
            self.cell_ids_logical_digest,
            &self.owning_slide_id,
            &self.coordinate_frame_id,
            mark_use.binary_mark(),
            mark_use.probability_mark(),
        )
        .is_ok_and(|(digest, _)| digest == self.declared_input_logical_digest)
    }
}

pub(super) fn cell_ids_identity(
    cell_ids: &[CellId],
) -> Result<(ContentDigest, u64), DeclaredScalarInputError> {
    let mut writer = ContentDigest::builder();
    write_part(&mut writer, CELL_IDS_DIGEST_DOMAIN)?;
    write_part(&mut writer, &usize_to_u64(cell_ids.len())?.to_be_bytes())?;
    for cell_id in cell_ids {
        write_part(&mut writer, cell_id.as_str().as_bytes())?;
    }
    Ok(writer.finish())
}

pub(super) fn declared_identity(
    cell_ids: &[CellId],
    slide_id: &SlideId,
    frame_id: &CoordinateFrameId,
    binary: &BinaryMarkDeclaration,
    probability: Option<&ProbabilityMarkDeclaration>,
) -> Result<(ContentDigest, u64), DeclaredScalarInputError> {
    let (cell_digest, _) = cell_ids_identity(cell_ids)?;
    declared_identity_from_cell_digest(cell_digest, slide_id, frame_id, binary, probability)
}

fn declared_identity_from_cell_digest(
    cell_digest: ContentDigest,
    slide_id: &SlideId,
    frame_id: &CoordinateFrameId,
    binary: &BinaryMarkDeclaration,
    probability: Option<&ProbabilityMarkDeclaration>,
) -> Result<(ContentDigest, u64), DeclaredScalarInputError> {
    let mut writer = ContentDigest::builder();
    write_part(&mut writer, INPUT_DIGEST_DOMAIN)?;
    write_part(&mut writer, cell_digest.as_bytes())?;
    write_part(&mut writer, slide_id.as_str().as_bytes())?;
    write_part(&mut writer, frame_id.as_str().as_bytes())?;
    write_part(&mut writer, binary.mark_id.as_str().as_bytes())?;
    write_part(&mut writer, binary.label.as_bytes())?;
    write_part(
        &mut writer,
        measurement_status_name(binary.measurement_status).as_bytes(),
    )?;
    write_part(
        &mut writer,
        binary.provenance_artifact_id.digest().as_bytes(),
    )?;
    write_part(&mut writer, binary.origin.wire_name().as_bytes())?;
    if let BinaryMarkOrigin::Thresholded {
        probability_mark_id,
        comparator,
        threshold,
        threshold_provenance_artifact_id,
    } = &binary.origin
    {
        write_part(&mut writer, probability_mark_id.as_str().as_bytes())?;
        write_part(&mut writer, comparator.wire_name().as_bytes())?;
        write_part(&mut writer, &threshold.to_bits().to_be_bytes())?;
        write_part(
            &mut writer,
            threshold_provenance_artifact_id.digest().as_bytes(),
        )?;
    }
    match probability {
        Some(probability) => {
            write_part(&mut writer, b"probability-present")?;
            write_part(&mut writer, probability.mark_id.as_str().as_bytes())?;
            write_part(
                &mut writer,
                measurement_status_name(probability.measurement_status).as_bytes(),
            )?;
            write_part(
                &mut writer,
                probability.provenance_artifact_id.digest().as_bytes(),
            )?;
        }
        None => write_part(&mut writer, b"probability-absent")?,
    }
    Ok(writer.finish())
}

fn write_part(
    writer: &mut ContentDigestWriter,
    bytes: &[u8],
) -> Result<(), DeclaredScalarInputError> {
    writer
        .write_all(&(bytes.len() as u128).to_be_bytes())
        .and_then(|()| writer.write_all(bytes))
        .map_err(|_| DeclaredScalarInputError::LogicalIdentityOverflow)
}

fn usize_to_u64(value: usize) -> Result<u64, DeclaredScalarInputError> {
    u64::try_from(value).map_err(|_| DeclaredScalarInputError::LogicalIdentityOverflow)
}
