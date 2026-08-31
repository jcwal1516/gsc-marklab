use marklab_project::{ArtifactRecord, TableColumnType, TableFormat, TableScalarType};

use crate::{
    CellEmbeddingRowLink, EmbeddingError, EmbeddingStatus, ExpectedCellSet,
    VerifiedCellEmbeddingArtifactGraph,
};

use super::super::profile::{
    CONTENT_KIND, ENCODING_VERSION, MAXIMUM_DIMENSION, MAXIMUM_ROWS, SCHEMA_ID,
};
use crate::columnar::{EmbeddingColumnarError, ParquetFailure};

pub(super) fn validate_embedding_record(
    record: &ArtifactRecord,
    observed_byte_len: u64,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    graph: VerifiedCellEmbeddingArtifactGraph,
) -> Result<u32, EmbeddingColumnarError> {
    if record.content().kind() != CONTENT_KIND
        || record.content().byte_len() != observed_byte_len
        || record.schema().id() != SCHEMA_ID
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
        return Err(EmbeddingColumnarError::ArtifactBindingMismatch);
    }
    let mut expected_dependencies = vec![
        graph.expected_cells_artifact_id,
        graph.provenance_artifact_id,
        graph.row_link_artifact_id,
    ];
    expected_dependencies.sort_unstable();
    if record.dependencies() != expected_dependencies {
        return Err(EmbeddingColumnarError::ArtifactBindingMismatch);
    }
    let manifest = record
        .table()
        .ok_or(EmbeddingColumnarError::ArtifactBindingMismatch)?;
    if manifest.format() != TableFormat::ParquetFile
        || manifest.encoding_version() != ENCODING_VERSION
        || manifest.row_count()
            != u64::try_from(expected.cells().len())
                .map_err(|_| EmbeddingColumnarError::SizeOverflow)?
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
        return Err(EmbeddingColumnarError::ArtifactBindingMismatch);
    }
    let dimension = match manifest.columns()[1].column_type() {
        TableColumnType::FixedSizeList {
            element: TableScalarType::F32,
            length,
        } => *length,
        _ => return Err(EmbeddingColumnarError::ArtifactBindingMismatch),
    };
    if dimension != graph.output_dimension
        || dimension == 0
        || dimension > MAXIMUM_DIMENSION
        || expected.cells().len() > MAXIMUM_ROWS
    {
        return Err(EmbeddingColumnarError::ArtifactBindingMismatch);
    }
    Ok(dimension)
}

pub(super) fn parse_embedding_status(
    value: &str,
) -> Result<EmbeddingStatus, EmbeddingColumnarError> {
    match value {
        "present" => Ok(EmbeddingStatus::Present),
        "missing_vector" => Ok(EmbeddingStatus::MissingVector),
        "extraction_failed" => Ok(EmbeddingStatus::ExtractionFailed),
        "qc_rejected" => Ok(EmbeddingStatus::QcRejected),
        _ => Err(parquet_failure(ParquetFailure::InvalidStatus)),
    }
}

pub(super) fn map_table_construction_error(error: EmbeddingError) -> EmbeddingColumnarError {
    match error {
        EmbeddingError::SizeOverflow => EmbeddingColumnarError::SizeOverflow,
        EmbeddingError::RetainedByteBudgetExceeded { required, maximum } => {
            EmbeddingColumnarError::RetainedByteBudgetExceeded { required, maximum }
        }
        EmbeddingError::AllocationFailed { requested } => {
            EmbeddingColumnarError::AllocationFailed { requested }
        }
        _ => parquet_failure(ParquetFailure::InvalidComponent),
    }
}

pub(super) fn parquet_failure(reason: ParquetFailure) -> EmbeddingColumnarError {
    EmbeddingColumnarError::Parquet { reason }
}
