use marklab_project::{ArtifactRecord, TableColumnType, TableFormat, TableScalarType};

use crate::{CellEmbeddingRowLink, ExpectedCellSet, VerifiedCellEmbeddingArtifactGraph};

use super::ColumnarError;

pub(in crate::columnar) struct CellEmbeddingRecordProfile<'a> {
    pub content_kind: &'a str,
    pub encoding_version: &'a str,
    pub schema_id: &'a str,
    pub table_format: TableFormat,
}

/// Admit the encoding-independent artifact graph and table contract.
///
/// Format readers retain their physical shape limits and error mapping.
pub(in crate::columnar) fn admit_cell_embedding_record(
    record: &ArtifactRecord,
    observed_byte_len: u64,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    graph: VerifiedCellEmbeddingArtifactGraph,
    profile: CellEmbeddingRecordProfile<'_>,
) -> Result<u32, ColumnarError> {
    if record.content().kind() != profile.content_kind
        || record.content().byte_len() != observed_byte_len
        || record.schema().id() != profile.schema_id
        || record.schema().version() != 1
        || !record.semantic_metadata().is_empty()
        || graph.dependency_count != 13
        || graph.expected_cells_logical_digest != expected.logical_digest()
        || graph.row_link_logical_digest != row_link.logical_digest()
        || graph.expected_cells_artifact_id != row_link.expected_cells_artifact_id()
        || graph.source_cells_artifact_id != row_link.source_cells_artifact_id()
        || graph.source_vectors_artifact_id != row_link.source_vectors_artifact_id()
        || graph.identity_map_artifact_id != row_link.identity_map_artifact_id()
        || graph.converter_artifact_id != row_link.converter_artifact_id()
        || row_link.expected_cells_logical_digest() != expected.logical_digest()
        || row_link.entries().len() != expected.cells().len()
    {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    let mut expected_dependencies = vec![
        graph.expected_cells_artifact_id,
        graph.provenance_artifact_id,
        graph.row_link_artifact_id,
    ];
    expected_dependencies.sort_unstable();
    if record.dependencies() != expected_dependencies {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    let manifest = record
        .table()
        .ok_or(ColumnarError::ArtifactBindingMismatch)?;
    if manifest.format() != profile.table_format
        || manifest.encoding_version() != profile.encoding_version
        || manifest.row_count()
            != u64::try_from(expected.cells().len()).map_err(|_| ColumnarError::SizeOverflow)?
        || manifest.columns().len() != 3
        || manifest.columns()[0].name() != "cell_id"
        || manifest.columns()[0].column_type() != &TableColumnType::Scalar(TableScalarType::Utf8)
        || manifest.columns()[0].nullable()
        || manifest.columns()[1].name() != "embedding"
        || manifest.columns()[1].nullable()
        || manifest.columns()[2].name() != "embedding_status"
        || manifest.columns()[2].column_type() != &TableColumnType::Scalar(TableScalarType::Utf8)
        || manifest.columns()[2].nullable()
        || manifest.primary_key() != ["cell_id"]
    {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    let dimension = match manifest.columns()[1].column_type() {
        TableColumnType::FixedSizeList {
            element: TableScalarType::F32,
            length,
        } => *length,
        _ => return Err(ColumnarError::ArtifactBindingMismatch),
    };
    if dimension != graph.output_dimension {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    Ok(dimension)
}
