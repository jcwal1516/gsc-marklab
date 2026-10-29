use marklab_project::{ArtifactRecord, TableFormat};

use crate::{
    CellEmbeddingRowLink, EmbeddingError, EmbeddingStatus, ExpectedCellSet,
    VerifiedCellEmbeddingArtifactGraph,
};

use super::super::profile::{
    CONTENT_KIND, ENCODING_VERSION, MAXIMUM_DIMENSION, MAXIMUM_ROWS, SCHEMA_ID,
};
use crate::columnar::{
    admission::{admit_cell_embedding_record, CellEmbeddingRecordProfile},
    ColumnarError, ParquetFailure,
};

pub(super) fn validate_embedding_record(
    record: &ArtifactRecord,
    observed_byte_len: u64,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    graph: VerifiedCellEmbeddingArtifactGraph,
) -> Result<u32, ColumnarError> {
    let dimension = admit_cell_embedding_record(
        record,
        observed_byte_len,
        expected,
        row_link,
        graph,
        CellEmbeddingRecordProfile {
            content_kind: CONTENT_KIND,
            encoding_version: ENCODING_VERSION,
            schema_id: SCHEMA_ID,
            table_format: TableFormat::ParquetFile,
        },
    )?;
    if dimension == 0 || dimension > MAXIMUM_DIMENSION || expected.cells().len() > MAXIMUM_ROWS {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    Ok(dimension)
}

pub(super) fn parse_embedding_status(value: &str) -> Result<EmbeddingStatus, ColumnarError> {
    match value {
        "present" => Ok(EmbeddingStatus::Present),
        "missing_vector" => Ok(EmbeddingStatus::MissingVector),
        "extraction_failed" => Ok(EmbeddingStatus::ExtractionFailed),
        "qc_rejected" => Ok(EmbeddingStatus::QcRejected),
        _ => Err(parquet_failure(ParquetFailure::InvalidStatus)),
    }
}

pub(super) fn map_table_construction_error(error: EmbeddingError) -> ColumnarError {
    match error {
        EmbeddingError::SizeOverflow => ColumnarError::SizeOverflow,
        EmbeddingError::RetainedByteBudgetExceeded { required, maximum } => {
            ColumnarError::RetainedByteBudgetExceeded { required, maximum }
        }
        EmbeddingError::AllocationFailed { requested } => {
            ColumnarError::AllocationFailed { requested }
        }
        _ => parquet_failure(ParquetFailure::InvalidComponent),
    }
}

pub(super) fn parquet_failure(reason: ParquetFailure) -> ColumnarError {
    ColumnarError::Parquet { reason }
}
