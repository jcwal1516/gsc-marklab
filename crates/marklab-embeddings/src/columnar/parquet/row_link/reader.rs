use std::{
    io::{Cursor, Read, Seek, SeekFrom},
    sync::Arc,
};

use arrow::array::{Array, RecordBatchReader, StringArray, UInt64Array};
use bytes::Bytes;
use marklab_project::{
    ArtifactRecord, LocalArtifactStore, TableColumnType, TableFormat, TableScalarType,
    VerifiedReaderError,
};
use parquet::{
    arrow::arrow_reader::{
        ArrowReaderMetadata, ArrowReaderOptions, ParquetRecordBatchReaderBuilder,
    },
    file::metadata::RowGroupMetaData,
};

use crate::{CellEmbeddingRowLink, ExpectedCellSet};

use super::{
    super::{profile::ROW_GROUP_ROWS, reader::RowGroupWindow},
    preflight::{
        prepare_cell_embedding_row_link_parquet_reader, PreparedCellEmbeddingRowLinkParquet,
    },
    profile::{row_link_schema, CONTENT_KIND, ENCODING_VERSION, SCHEMA_ID},
};
use crate::columnar::{EmbeddingColumnarBudgets, EmbeddingColumnarError, ParquetFailure};

/// Fully decode and validate borrowed canonical row-link Parquet bytes.
pub fn validate_cell_embedding_row_link_parquet_bytes(
    bytes: &[u8],
    record: &ArtifactRecord,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<super::CellEmbeddingRowLinkParquetPreflight, EmbeddingColumnarError> {
    let encoded_byte_len =
        u64::try_from(bytes.len()).map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
    if encoded_byte_len > budgets.maximum_file_bytes() {
        return Err(EmbeddingColumnarError::FileByteBudgetExceeded {
            observed: encoded_byte_len,
            maximum: budgets.maximum_file_bytes(),
        });
    }
    validate_record(record, encoded_byte_len, row_link)?;
    let mut source = Cursor::new(bytes);
    let prepared = prepare_cell_embedding_row_link_parquet_reader(
        &mut source,
        encoded_byte_len,
        marklab_project::ContentDigest::from_bytes(bytes),
        expected,
        row_link,
        budgets,
    )?;
    validate_preflight_identity(&prepared, record)?;
    decode_row_link_parquet(&mut source, row_link, prepared, budgets)
}

/// Fully decode a managed row link through one pre/post-verified store descriptor.
pub fn validate_cell_embedding_row_link_parquet_from_store(
    store: &LocalArtifactStore,
    record: &ArtifactRecord,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<super::CellEmbeddingRowLinkParquetPreflight, VerifiedReaderError<EmbeddingColumnarError>>
{
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
        let prepared = prepare_cell_embedding_row_link_parquet_reader(
            reader,
            encoded_byte_len,
            record.content().digest(),
            expected,
            row_link,
            budgets,
        )?;
        validate_preflight_identity(&prepared, record)?;
        decode_row_link_parquet(reader, row_link, prepared, budgets)
    })
}

fn validate_preflight_identity(
    prepared: &PreparedCellEmbeddingRowLinkParquet,
    record: &ArtifactRecord,
) -> Result<(), EmbeddingColumnarError> {
    if prepared.summary.content_digest() != record.content().digest()
        || prepared.summary.encoded_byte_len() != record.content().byte_len()
    {
        return Err(EmbeddingColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

fn decode_row_link_parquet<R: Read + Seek + ?Sized>(
    source: &mut R,
    row_link: &CellEmbeddingRowLink,
    prepared: PreparedCellEmbeddingRowLinkParquet,
    budgets: EmbeddingColumnarBudgets,
) -> Result<super::CellEmbeddingRowLinkParquetPreflight, EmbeddingColumnarError> {
    let maximum_group_peak = prepared
        .metadata
        .row_groups()
        .iter()
        .try_fold(0_usize, |maximum, group| {
            estimate_group_peak(group).map(|required| maximum.max(required))
        })?;
    let retained = prepared
        .metadata
        .memory_size()
        .checked_add(maximum_group_peak)
        .and_then(|value| value.checked_add(64 * 1024))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?
        .max(prepared.summary.retained_preflight_bytes);
    enforce_retained_budget(retained, budgets)?;

    let PreparedCellEmbeddingRowLinkParquet { summary, metadata } = prepared;
    let metadata = Arc::new(metadata);
    let expected_schema = row_link_schema();
    let reader_metadata = ArrowReaderMetadata::try_new(
        Arc::clone(&metadata),
        ArrowReaderOptions::new().with_schema(Arc::new(expected_schema.clone())),
    )
    .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
    let mut global_row = 0_usize;
    for (group_index, group) in metadata.row_groups().iter().enumerate() {
        let rows = usize::try_from(group.num_rows())
            .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
        let (start, length) = validated_group_range(group)?;
        let group_peak = estimate_group_peak(group)?;
        enforce_row_group_budget(group_peak, budgets)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| EmbeddingColumnarError::AllocationFailed { requested: length })?;
        bytes.resize(length, 0);
        source
            .seek(SeekFrom::Start(start))
            .and_then(|_| source.read_exact(&mut bytes))
            .map_err(|_| parquet_failure(ParquetFailure::ArtifactRead))?;
        let end = start
            .checked_add(u64::try_from(length).map_err(|_| EmbeddingColumnarError::SizeOverflow)?)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let window = RowGroupWindow::new(start, end, Bytes::from(bytes));
        let mut reader =
            ParquetRecordBatchReaderBuilder::new_with_metadata(window, reader_metadata.clone())
                .with_row_groups(vec![group_index])
                .with_batch_size(ROW_GROUP_ROWS)
                .build()
                .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
        if reader.schema().as_ref() != &expected_schema {
            return Err(parquet_failure(ParquetFailure::StockDecode));
        }
        let group_start = global_row;
        for decoded in &mut reader {
            let batch = decoded.map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
            validate_batch(&batch, row_link, &mut global_row)?;
        }
        if global_row
            .checked_sub(group_start)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?
            != rows
        {
            return Err(parquet_failure(ParquetFailure::InvalidRowCount));
        }
    }
    if global_row != row_link.entries().len() {
        return Err(parquet_failure(ParquetFailure::InvalidRowCount));
    }
    Ok(summary)
}

fn validate_batch(
    batch: &arrow::record_batch::RecordBatch,
    row_link: &CellEmbeddingRowLink,
    global_row: &mut usize,
) -> Result<(), EmbeddingColumnarError> {
    if batch.num_columns() != 3 {
        return Err(parquet_failure(ParquetFailure::StockDecode));
    }
    for column in batch.columns() {
        column
            .to_data()
            .validate_full()
            .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
    }
    let cells = batch
        .column(0)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| parquet_failure(ParquetFailure::StockDecode))?;
    let source_cells = batch
        .column(1)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .ok_or_else(|| parquet_failure(ParquetFailure::StockDecode))?;
    let source_embeddings = batch
        .column(2)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .ok_or_else(|| parquet_failure(ParquetFailure::StockDecode))?;
    if cells.null_count() != 0 || source_cells.null_count() != 0 {
        return Err(parquet_failure(ParquetFailure::StockDecode));
    }
    for local_row in 0..batch.num_rows() {
        let entry = row_link
            .entries()
            .get(*global_row)
            .ok_or_else(|| parquet_failure(ParquetFailure::InvalidRowCount))?;
        if cells.value(local_row) != entry.cell_id().as_str() {
            return Err(parquet_failure(ParquetFailure::InvalidCellOrder));
        }
        if source_cells.value(local_row) != entry.source_cell_row() {
            return Err(parquet_failure(ParquetFailure::InvalidComponent));
        }
        match entry.source_embedding_row() {
            Some(expected_row)
                if !source_embeddings.is_null(local_row)
                    && source_embeddings.value(local_row) == expected_row => {}
            None if source_embeddings.is_null(local_row) => {}
            _ => return Err(parquet_failure(ParquetFailure::InvalidStatus)),
        }
        *global_row = global_row
            .checked_add(1)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    }
    Ok(())
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
    if manifest.format() != TableFormat::ParquetFile
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

fn validated_group_range(group: &RowGroupMetaData) -> Result<(u64, usize), EmbeddingColumnarError> {
    if group.num_columns() != 3 {
        return Err(parquet_failure(ParquetFailure::StockDecode));
    }
    let start = u64::try_from(group.column(0).data_page_offset())
        .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
    let mut next = start;
    for column in group.columns() {
        let offset = u64::try_from(column.data_page_offset())
            .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
        let length = u64::try_from(column.compressed_size())
            .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
        if offset != next || length == 0 {
            return Err(parquet_failure(ParquetFailure::StockDecode));
        }
        next = next
            .checked_add(length)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    }
    Ok((
        start,
        usize::try_from(next - start).map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
    ))
}

fn estimate_group_peak(group: &RowGroupMetaData) -> Result<usize, EmbeddingColumnarError> {
    let (_, encoded) = validated_group_range(group)?;
    let rows = usize::try_from(group.num_rows())
        .map_err(|_| parquet_failure(ParquetFailure::StockDecode))?;
    let arrow_output = encoded
        .checked_add(
            rows.checked_mul(size_of::<u64>() * 2)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?,
        )
        .and_then(|value| value.checked_add((rows + 1).checked_mul(size_of::<i32>())?))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    encoded
        .checked_mul(2)
        .and_then(|value| value.checked_add(arrow_output.checked_mul(2)?))
        .and_then(|value| value.checked_add(rows.checked_mul(4)?))
        .ok_or(EmbeddingColumnarError::SizeOverflow)
}

fn enforce_retained_budget(
    required: usize,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), EmbeddingColumnarError> {
    if required > budgets.maximum_retained_bytes() {
        return Err(EmbeddingColumnarError::RetainedByteBudgetExceeded {
            required,
            maximum: budgets.maximum_retained_bytes(),
        });
    }
    Ok(())
}

fn enforce_row_group_budget(
    required: usize,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), EmbeddingColumnarError> {
    if required > budgets.maximum_row_group_bytes() {
        return Err(EmbeddingColumnarError::RowGroupByteBudgetExceeded {
            required,
            maximum: budgets.maximum_row_group_bytes(),
        });
    }
    Ok(())
}

fn parquet_failure(reason: ParquetFailure) -> EmbeddingColumnarError {
    EmbeddingColumnarError::Parquet { reason }
}
