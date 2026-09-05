use std::io::Write;

use marklab_data::{CoordinateFrameId, SlideId};
use marklab_workflow::{ArtifactRef, ContentDigest, ContentDigestWriter};

use super::{AssayMarkValues, MarkTable, ScalarMarkColumn, ScalarMarkColumnValues};
use crate::scalar_mark::{declaration::measurement_status_name, DeclaredScalarInputError};

const MARK_TABLE_KIND: &str = "application/vnd.marklab.scalar-mark-table;version=1";
const MARK_TABLE_DIGEST_DOMAIN: &[u8] = b"marklab-scalar-mark-table-v1";

pub(super) fn declared_artifact_ref(
    table: &MarkTable,
    slide_id: &SlideId,
    frame_id: &CoordinateFrameId,
) -> Result<ArtifactRef, DeclaredScalarInputError> {
    let mut writer = ContentDigest::builder();
    let panel = table
        .columns
        .iter()
        .any(|column| matches!(&column.values, ScalarMarkColumnValues::Assay { .. }));
    let kind = if panel {
        "application/vnd.marklab.scalar-mark-table;version=2"
    } else {
        MARK_TABLE_KIND
    };
    write_part(
        &mut writer,
        if panel {
            b"marklab-scalar-mark-table-v2"
        } else {
            MARK_TABLE_DIGEST_DOMAIN
        },
    )?;
    if panel {
        write_part(&mut writer, &(table.cell_ids.len() as u128).to_be_bytes())?;
        write_part(&mut writer, &(table.columns.len() as u128).to_be_bytes())?;
    }
    write_part(&mut writer, slide_id.as_str().as_bytes())?;
    write_part(&mut writer, frame_id.as_str().as_bytes())?;
    for cell_id in &table.cell_ids {
        write_part(&mut writer, cell_id.as_str().as_bytes())?;
    }
    for column in &table.columns {
        write_column(&mut writer, column)?;
    }
    let (digest, byte_len) = writer.finish();
    ArtifactRef::new(kind, digest, byte_len)
        .map_err(|_| DeclaredScalarInputError::LogicalIdentityOverflow)
}

fn write_column(
    writer: &mut ContentDigestWriter,
    column: &ScalarMarkColumn,
) -> Result<(), DeclaredScalarInputError> {
    let (kind, label, status, provenance) = match &column.values {
        ScalarMarkColumnValues::Assay { declaration, .. } => (
            "assay",
            declaration.label(),
            declaration.measurement_status(),
            declaration.provenance_artifact_id(),
        ),
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
        ScalarMarkColumnValues::Ordinal { declaration, .. } => (
            "ordinal",
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
    let (modality, unit) = match column.compatibility_semantics {
        Some((modality, unit)) => (modality.wire_name(), unit.wire_name()),
        None => ("multiplex_protein", "assay_defined"),
    };
    write_part(writer, modality.as_bytes())?;
    write_part(writer, unit.as_bytes())?;
    write_part(writer, column.missingness.wire_name().as_bytes())?;
    write_part(writer, provenance.digest().as_bytes())?;
    match &column.values {
        ScalarMarkColumnValues::Assay {
            declaration,
            values,
        } => {
            write_part(writer, declaration.unit().as_bytes())?;
            match values {
                AssayMarkValues::Continuous(values) => {
                    write_part(writer, b"continuous_f64")?;
                    for value in values {
                        if let Some(value) = value {
                            let mut bytes = [0; 9];
                            bytes[0] = 1;
                            bytes[1..].copy_from_slice(&value.to_bits().to_be_bytes());
                            write_part(writer, &bytes)?;
                        } else {
                            write_part(writer, &[0])?;
                        }
                    }
                }
                AssayMarkValues::Binary(values) => {
                    write_part(writer, b"binary")?;
                    for value in values {
                        if let Some(value) = value {
                            write_part(writer, &[1, u8::from(*value)])?;
                        } else {
                            write_part(writer, &[0])?;
                        }
                    }
                }
                AssayMarkValues::Categorical { levels, values } => {
                    write_part(writer, b"categorical")?;
                    write_part(writer, &(levels.len() as u128).to_be_bytes())?;
                    for level in levels {
                        write_part(writer, level.as_bytes())?;
                    }
                    for value in values {
                        if let Some(value) = value {
                            let mut bytes = [0; 5];
                            bytes[0] = 1;
                            bytes[1..].copy_from_slice(&value.to_be_bytes());
                            write_part(writer, &bytes)?;
                        } else {
                            write_part(writer, &[0])?;
                        }
                    }
                }
            }
        }
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
        ScalarMarkColumnValues::Ordinal {
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
