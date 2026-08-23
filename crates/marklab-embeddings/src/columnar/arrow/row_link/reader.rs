use std::io::{Cursor, Read, Seek};

use arrow::array::{Array, StringArray, UInt64Array};
use arrow_ipc::reader::FileReaderBuilder;
use marklab_project::{
    ArtifactRecord, LocalArtifactStore, TableColumnType, TableFormat, TableScalarType,
    VerifiedReaderError,
};

use crate::{CellEmbeddingRowLink, ExpectedCellSet};

use super::{
    super::{
        super::super::{ArrowIpcFailure, EmbeddingColumnarBudgets, EmbeddingColumnarError},
        profile::arrow_failure,
    },
    preflight::{
        preflight_cell_embedding_row_link_arrow_bytes,
        preflight_cell_embedding_row_link_arrow_reader, CellEmbeddingRowLinkArrowPreflight,
    },
    profile::{row_link_schema, CONTENT_KIND, ENCODING_VERSION, SCHEMA_ID},
};

/// Fully decode and validate borrowed canonical Arrow row-link bytes.
pub fn validate_cell_embedding_row_link_arrow_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellEmbeddingRowLinkArrowPreflight, EmbeddingColumnarError> {
    let encoded_byte_len =
        u64::try_from(bytes.len()).map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
    if encoded_byte_len > budgets.maximum_file_bytes() {
        return Err(EmbeddingColumnarError::FileByteBudgetExceeded {
            observed: encoded_byte_len,
            maximum: budgets.maximum_file_bytes(),
        });
    }
    validate_record(record, encoded_byte_len, row_link)?;
    let preflight =
        preflight_cell_embedding_row_link_arrow_bytes(bytes, expected, row_link, budgets)?;
    validate_preflight_identity(preflight, record)?;
    decode_row_link_arrow(Cursor::new(bytes), row_link, preflight, budgets)
}

/// Fully decode a managed row link through one pre/post-verified store descriptor.
pub fn validate_cell_embedding_row_link_arrow_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellEmbeddingRowLinkArrowPreflight, VerifiedReaderError<EmbeddingColumnarError>> {
    let encoded_byte_len = record.content().byte_len();
    if encoded_byte_len > budgets.maximum_file_bytes() {
        return Err(VerifiedReaderError::Callback(
            EmbeddingColumnarError::FileByteBudgetExceeded {
                observed: encoded_byte_len,
                maximum: budgets.maximum_file_bytes(),
            },
        ));
    }
    validate_record(record, encoded_byte_len, row_link).map_err(VerifiedReaderError::Callback)?;
    store.with_verified_reader(record, |reader| {
        let preflight = preflight_cell_embedding_row_link_arrow_reader(
            reader,
            record.content().digest(),
            expected,
            row_link,
            budgets,
        )?;
        validate_preflight_identity(preflight, record)?;
        decode_row_link_arrow(reader, row_link, preflight, budgets)
    })
}

fn validate_preflight_identity(
    preflight: CellEmbeddingRowLinkArrowPreflight,
    record: &ArtifactRecord,
) -> Result<(), EmbeddingColumnarError> {
    if preflight.content_digest() != record.content().digest()
        || preflight.encoded_byte_len() != record.content().byte_len()
    {
        return Err(EmbeddingColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

fn decode_row_link_arrow<R: Read + Seek>(
    source: R,
    row_link: &CellEmbeddingRowLink,
    preflight: CellEmbeddingRowLinkArrowPreflight,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellEmbeddingRowLinkArrowPreflight, EmbeddingColumnarError> {
    if preflight.retained_preflight_bytes > budgets.maximum_retained_bytes() {
        return Err(EmbeddingColumnarError::RetainedByteBudgetExceeded {
            required: preflight.retained_preflight_bytes,
            maximum: budgets.maximum_retained_bytes(),
        });
    }
    let mut reader = FileReaderBuilder::new()
        .with_max_footer_fb_depth(8)
        .with_max_footer_fb_tables(32)
        .build(source)
        .map_err(|_| arrow_failure(ArrowIpcFailure::StockDecode))?;
    if reader.schema().as_ref() != &row_link_schema(row_link)?
        || !reader.custom_metadata().is_empty()
    {
        return Err(arrow_failure(ArrowIpcFailure::StockDecode));
    }
    let mut global_row = 0_usize;
    for decoded in &mut reader {
        let batch = decoded.map_err(|_| arrow_failure(ArrowIpcFailure::StockDecode))?;
        if batch.num_columns() != 3 {
            return Err(arrow_failure(ArrowIpcFailure::StockDecode));
        }
        for column in batch.columns() {
            column
                .to_data()
                .validate_full()
                .map_err(|_| arrow_failure(ArrowIpcFailure::StockDecode))?;
        }
        let cells = batch
            .column(0)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::StockDecode))?;
        let source_cells = batch
            .column(1)
            .as_any()
            .downcast_ref::<UInt64Array>()
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::StockDecode))?;
        let source_embeddings = batch
            .column(2)
            .as_any()
            .downcast_ref::<UInt64Array>()
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::StockDecode))?;
        if cells.null_count() != 0 || source_cells.null_count() != 0 {
            return Err(arrow_failure(ArrowIpcFailure::StockDecode));
        }
        for local_row in 0..batch.num_rows() {
            let entry = row_link
                .entries()
                .get(global_row)
                .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidRowCount))?;
            if cells.value(local_row) != entry.cell_id().as_str() {
                return Err(arrow_failure(ArrowIpcFailure::InvalidCellOrder));
            }
            if source_cells.value(local_row) != entry.source_cell_row() {
                return Err(arrow_failure(ArrowIpcFailure::InvalidComponent));
            }
            match entry.source_embedding_row() {
                Some(expected_row)
                    if !source_embeddings.is_null(local_row)
                        && source_embeddings.value(local_row) == expected_row => {}
                None if source_embeddings.is_null(local_row) => {}
                _ => return Err(arrow_failure(ArrowIpcFailure::InvalidStatus)),
            }
            global_row = global_row
                .checked_add(1)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        }
    }
    if global_row != row_link.entries().len() {
        return Err(arrow_failure(ArrowIpcFailure::InvalidRowCount));
    }
    Ok(preflight)
}

fn validate_record(
    record: &ArtifactRecord,
    observed_byte_len: u64,
    row_link: &CellEmbeddingRowLink,
) -> Result<(), EmbeddingColumnarError> {
    if record.content().kind() != CONTENT_KIND
        || record.content().byte_len() != observed_byte_len
        || record.schema().id() != SCHEMA_ID
        || record.schema().version() != 1
        || !record.semantic_metadata().is_empty()
        || record.dependencies() != row_link.direct_dependencies()
    {
        return Err(EmbeddingColumnarError::ArtifactBindingMismatch);
    }
    let manifest = record
        .table()
        .ok_or(EmbeddingColumnarError::ArtifactBindingMismatch)?;
    if manifest.format() != TableFormat::ArrowIpcFile
        || manifest.encoding_version() != ENCODING_VERSION
        || manifest.row_count() != row_link.row_count()
        || manifest.columns().len() != 3
        || manifest.columns()[0].name() != "cell_id"
        || manifest.columns()[0].column_type() != &TableColumnType::Scalar(TableScalarType::Utf8)
        || manifest.columns()[0].nullable()
        || manifest.columns()[1].name() != "source_cell_row"
        || manifest.columns()[1].column_type() != &TableColumnType::Scalar(TableScalarType::U64)
        || manifest.columns()[1].nullable()
        || manifest.columns()[2].name() != "source_embedding_row"
        || manifest.columns()[2].column_type() != &TableColumnType::Scalar(TableScalarType::U64)
        || !manifest.columns()[2].nullable()
        || manifest.primary_key() != ["cell_id"]
    {
        return Err(EmbeddingColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}
