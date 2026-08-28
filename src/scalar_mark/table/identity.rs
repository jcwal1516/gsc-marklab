use std::io::Write;

use marklab_data::{CoordinateFrameId, SlideId};
use marklab_workflow::{ArtifactRef, ContentDigest, ContentDigestWriter};

use super::{MarkTable, ScalarMarkColumn, ScalarMarkColumnValues};
use crate::scalar_mark::{declaration::measurement_status_name, DeclaredScalarInputError};

const MARK_TABLE_KIND: &str = "application/vnd.marklab.scalar-mark-table;version=1";
const MARK_TABLE_DIGEST_DOMAIN: &[u8] = b"marklab-scalar-mark-table-v1";

pub(super) fn declared_artifact_ref(
    table: &MarkTable,
    slide_id: &SlideId,
    frame_id: &CoordinateFrameId,
) -> Result<ArtifactRef, DeclaredScalarInputError> {
    let mut writer = ContentDigest::builder();
    write_part(&mut writer, MARK_TABLE_DIGEST_DOMAIN)?;
    write_part(&mut writer, slide_id.as_str().as_bytes())?;
    write_part(&mut writer, frame_id.as_str().as_bytes())?;
    for cell_id in &table.cell_ids {
        write_part(&mut writer, cell_id.as_str().as_bytes())?;
    }
    for column in &table.columns {
        write_column(&mut writer, column)?;
    }
    let (digest, byte_len) = writer.finish();
    ArtifactRef::new(MARK_TABLE_KIND, digest, byte_len)
        .map_err(|_| DeclaredScalarInputError::LogicalIdentityOverflow)
}

fn write_column(
    writer: &mut ContentDigestWriter,
    column: &ScalarMarkColumn,
) -> Result<(), DeclaredScalarInputError> {
    let (kind, label, status, provenance) = match &column.values {
        ScalarMarkColumnValues::Binary { declaration, .. } => (
            "binary",
            declaration.label(),
            declaration.measurement_status(),
            declaration.provenance_artifact_id(),
        ),
        ScalarMarkColumnValues::Probability { declaration, .. } => (
            "probability",
            declaration.mark_id().as_str(),
            declaration.measurement_status(),
            declaration.provenance_artifact_id(),
        ),
        ScalarMarkColumnValues::Continuous { declaration, .. } => (
            "continuous",
            declaration.label(),
            declaration.measurement_status(),
            declaration.provenance_artifact_id(),
        ),
        ScalarMarkColumnValues::Categorical { declaration, .. } => (
            "categorical",
            declaration.label(),
            declaration.measurement_status(),
            declaration.provenance_artifact_id(),
        ),
        ScalarMarkColumnValues::ProbabilitySimplex { declaration, .. } => (
            "probability_simplex",
            declaration.label(),
            declaration.measurement_status(),
            declaration.provenance_artifact_id(),
        ),
        ScalarMarkColumnValues::VectorArtifactRef {
            declaration,
            artifact,
            ..
        } => (
            "vector_artifact_ref",
            declaration.label(),
            declaration.measurement_status(),
            artifact.provenance_artifact_id(),
        ),
    };
    write_part(writer, kind.as_bytes())?;
    write_part(writer, column.mark_id().as_str().as_bytes())?;
    write_part(writer, label.as_bytes())?;
    write_part(writer, measurement_status_name(status).as_bytes())?;
    write_part(writer, column.modality.wire_name().as_bytes())?;
    write_part(writer, column.unit.wire_name().as_bytes())?;
    write_part(writer, column.missingness.wire_name().as_bytes())?;
    write_part(writer, provenance.digest().as_bytes())?;
    match &column.values {
        ScalarMarkColumnValues::Binary {
            declaration,
            values,
        } => {
            write_part(writer, declaration.origin().wire_name().as_bytes())?;
            for value in values {
                write_part(writer, &[*value])?;
            }
        }
        ScalarMarkColumnValues::Probability { values, .. }
        | ScalarMarkColumnValues::Continuous { values, .. } => {
            for value in values {
                write_part(writer, &value.to_bits().to_be_bytes())?;
            }
        }
        ScalarMarkColumnValues::Categorical {
            declaration,
            values,
        } => {
            write_part(writer, &(declaration.levels().len() as u128).to_be_bytes())?;
            for level in declaration.levels() {
                write_part(writer, level.as_bytes())?;
            }
            for value in values {
                write_part(writer, &value.to_be_bytes())?;
            }
        }
        ScalarMarkColumnValues::ProbabilitySimplex {
            declaration,
            row_count,
            values,
        } => {
            write_part(writer, &(*row_count as u128).to_be_bytes())?;
            write_part(writer, &(declaration.levels().len() as u128).to_be_bytes())?;
            for level in declaration.levels() {
                write_part(writer, level.as_bytes())?;
            }
            for value in values {
                write_part(writer, &value.to_bits().to_be_bytes())?;
            }
        }
        ScalarMarkColumnValues::VectorArtifactRef {
            artifact,
            row_count,
            cell_ids_logical_digest,
            ..
        } => {
            write_part(writer, &(*row_count as u128).to_be_bytes())?;
            write_part(writer, &(artifact.dimension() as u128).to_be_bytes())?;
            write_part(writer, b"f32")?;
            write_part(writer, artifact.embedding_artifact_id().digest().as_bytes())?;
            write_part(
                writer,
                artifact.expected_cells_artifact_id().digest().as_bytes(),
            )?;
            write_part(writer, artifact.row_link_artifact_id().digest().as_bytes())?;
            write_part(
                writer,
                artifact.provenance_artifact_id().digest().as_bytes(),
            )?;
            write_part(writer, artifact.logical_digest().as_bytes())?;
            write_part(writer, cell_ids_logical_digest.as_bytes())?;
            let qc = artifact.qc_summary();
            for count in [
                qc.row_count(),
                qc.present_count(),
                qc.missing_vector_count(),
                qc.extraction_failed_count(),
                qc.qc_rejected_count(),
                qc.all_zero_present_count(),
            ] {
                write_part(writer, &(count as u128).to_be_bytes())?;
            }
        }
    }
    Ok(())
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
