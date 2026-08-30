use std::{io::Write, sync::Arc};

use arrow::{
    array::{ArrayRef, StringBuilder, UInt64Builder},
    record_batch::RecordBatch,
};
use parquet::arrow::arrow_writer::{ArrowWriter, ArrowWriterOptions};

use crate::CellEmbeddingRowLink;

use super::super::super::{
    enforce_decoded_budget, enforce_retained_budget, enforce_row_group_budget,
    EmbeddingColumnarBudgets, EmbeddingColumnarError, RowLinkColumnarWriteSummary,
};
use super::{
    super::{
        compact::canonical_compact_len,
        profile::{MAXIMUM_FOOTER_BYTES, MAXIMUM_ROWS, PUBLIC_BATCH_ROWS, ROW_GROUP_ROWS},
        writer::{
            estimate_metadata_retained_bytes, validity_bytes, ParquetDigestingWriter,
            MAXIMUM_ENCODED_FOOTER_BASE_BYTES, MAXIMUM_ENCODED_FOOTER_BYTES_PER_ROW_GROUP,
            WRITER_FIXED_RETAINED_BYTES,
        },
    },
    profile::{row_link_schema, writer_properties, ROOT_NAME},
};

/// Write the exact deterministic Parquet profile for a canonical row link.
///
/// The retained-byte budget excludes buffering owned by the caller-provided output sink.
pub fn write_cell_embedding_row_link_parquet(
    output: &mut (dyn Write + Send),
    row_link: &CellEmbeddingRowLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<RowLinkColumnarWriteSummary, EmbeddingColumnarError> {
    if row_link.entries().len() > MAXIMUM_ROWS {
        return Err(EmbeddingColumnarError::ParquetWriter);
    }
    let decoded_bytes = estimate_decoded_bytes(row_link)?;
    enforce_decoded_budget(decoded_bytes, budgets)?;
    let estimates = estimate_writer_memory(row_link)?;
    enforce_row_group_budget(estimates.row_group_bytes, budgets)?;
    enforce_retained_budget(estimates.retained_bytes, budgets)?;

    let schema = row_link_schema();
    let options = ArrowWriterOptions::new()
        .with_properties(writer_properties(row_link))
        .with_skip_arrow_metadata(true)
        .with_schema_root(ROOT_NAME.to_owned());
    let mut sink = ParquetDigestingWriter::new(output, budgets.maximum_file_bytes());
    let write_result = (|| {
        let mut writer =
            ArrowWriter::try_new_with_options(&mut sink, Arc::new(schema.clone()), options)
                .map_err(|_| EmbeddingColumnarError::ParquetWriter)?;
        let mut start = 0_usize;
        while start < row_link.entries().len() {
            let end = start
                .checked_add(PUBLIC_BATCH_ROWS)
                .map(|value| value.min(row_link.entries().len()))
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
            let batch = build_batch(row_link, &schema, start, end)?;
            writer
                .write(&batch)
                .map_err(|_| EmbeddingColumnarError::ParquetWriter)?;
            start = end;
        }
        let metadata = writer
            .close()
            .map_err(|_| EmbeddingColumnarError::ParquetWriter)?;
        canonical_compact_len(&metadata, MAXIMUM_FOOTER_BYTES)
            .map_err(|_| EmbeddingColumnarError::ParquetWriter)?;
        Ok(())
    })();
    if let Some(observed) = sink.budget_exceeded() {
        return Err(EmbeddingColumnarError::FileByteBudgetExceeded {
            observed,
            maximum: budgets.maximum_file_bytes(),
        });
    }
    write_result?;
    let (content_digest, encoded_byte_len) = sink.finish();
    Ok(RowLinkColumnarWriteSummary::new(
        content_digest,
        encoded_byte_len,
        row_link.row_count(),
    ))
}

fn build_batch(
    row_link: &CellEmbeddingRowLink,
    schema: &arrow::datatypes::Schema,
    start: usize,
    end: usize,
) -> Result<RecordBatch, EmbeddingColumnarError> {
    let entries = row_link
        .entries()
        .get(start..end)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let identifier_bytes = entries.iter().try_fold(0_usize, |total, entry| {
        total
            .checked_add(entry.cell_id().as_str().len())
            .ok_or(EmbeddingColumnarError::SizeOverflow)
    })?;
    let mut cells = StringBuilder::with_capacity(entries.len(), identifier_bytes);
    let mut source_cells = UInt64Builder::with_capacity(entries.len());
    let mut source_embeddings = UInt64Builder::with_capacity(entries.len());
    for entry in entries {
        cells.append_value(entry.cell_id().as_str());
        source_cells.append_value(entry.source_cell_row());
        if let Some(row) = entry.source_embedding_row() {
            source_embeddings.append_value(row);
        } else {
            source_embeddings.append_null();
        }
    }
    let columns: Vec<ArrayRef> = vec![
        Arc::new(cells.finish()),
        Arc::new(source_cells.finish()),
        Arc::new(source_embeddings.finish()),
    ];
    RecordBatch::try_new(Arc::new(schema.clone()), columns)
        .map_err(|_| EmbeddingColumnarError::ParquetWriter)
}

#[derive(Clone, Copy)]
struct WriterMemoryEstimate {
    row_group_bytes: usize,
    retained_bytes: usize,
}

fn estimate_writer_memory(
    row_link: &CellEmbeddingRowLink,
) -> Result<WriterMemoryEstimate, EmbeddingColumnarError> {
    let row_count = row_link.entries().len();
    let row_group_count = row_count.div_ceil(ROW_GROUP_ROWS);
    let encoded_footer_bytes = MAXIMUM_ENCODED_FOOTER_BASE_BYTES
        .checked_add(
            row_group_count
                .checked_mul(MAXIMUM_ENCODED_FOOTER_BYTES_PER_ROW_GROUP)
                .ok_or(EmbeddingColumnarError::SizeOverflow)?,
        )
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    if encoded_footer_bytes > MAXIMUM_FOOTER_BYTES {
        return Err(EmbeddingColumnarError::ParquetWriter);
    }
    let group_rows = row_count.min(ROW_GROUP_ROWS);
    let batch_rows = row_count.min(PUBLIC_BATCH_ROWS);
    let group_identifiers = maximum_identifier_bytes(row_link, ROW_GROUP_ROWS)?;
    let batch_identifiers = maximum_identifier_bytes(row_link, PUBLIC_BATCH_ROWS)?;
    let uncompressed_values = group_rows
        .checked_mul(size_of::<i32>())
        .and_then(|value| value.checked_add(group_identifiers))
        .and_then(|value| value.checked_add(group_rows.checked_mul(size_of::<u64>())?))
        .and_then(|value| value.checked_add(group_rows.checked_mul(size_of::<u64>())?))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let level_bytes = validity_bytes(group_rows)?
        .checked_mul(2)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let pages = group_rows
        .checked_mul(3)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let page_overhead = pages
        .checked_mul(1024 + 4 * size_of::<bytes::Bytes>())
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let row_group_bytes = uncompressed_values
        .checked_add(level_bytes)
        .and_then(|value| value.checked_mul(2))
        .and_then(|value| value.checked_add(page_overhead))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let input_batch = batch_rows
        .checked_mul(size_of::<u64>())
        .and_then(|value| value.checked_mul(2))
        .and_then(|value| value.checked_add(batch_identifiers))
        .and_then(|value| value.checked_add(batch_rows.checked_mul(size_of::<i32>())?))
        .and_then(|value| value.checked_add(validity_bytes(batch_rows).ok()?))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let level_workspaces = batch_rows
        .checked_mul(16)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let page_transient = uncompressed_values
        .checked_add(level_bytes)
        .and_then(|value| value.checked_mul(4))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let metadata = estimate_metadata_retained_bytes(row_group_count)?;
    let row_group_local_peak = input_batch
        .checked_add(level_workspaces)
        .and_then(|value| value.checked_add(row_group_bytes))
        .and_then(|value| value.checked_add(page_transient))
        .and_then(|value| value.checked_add(WRITER_FIXED_RETAINED_BYTES))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let retained_bytes = row_group_local_peak
        .checked_add(metadata)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    Ok(WriterMemoryEstimate {
        row_group_bytes: row_group_local_peak,
        retained_bytes,
    })
}

fn maximum_identifier_bytes(
    row_link: &CellEmbeddingRowLink,
    chunk_rows: usize,
) -> Result<usize, EmbeddingColumnarError> {
    let mut maximum = 0_usize;
    let mut current = 0_usize;
    for (index, entry) in row_link.entries().iter().enumerate() {
        if index != 0 && index % chunk_rows == 0 {
            maximum = maximum.max(current);
            current = 0;
        }
        current = current
            .checked_add(entry.cell_id().as_str().len())
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    }
    Ok(maximum.max(current))
}

pub(super) fn estimate_decoded_bytes(
    row_link: &CellEmbeddingRowLink,
) -> Result<u64, EmbeddingColumnarError> {
    let row_count = row_link.entries().len();
    let identifiers = row_link.entries().iter().try_fold(0_u64, |total, entry| {
        total
            .checked_add(
                u64::try_from(entry.cell_id().as_str().len())
                    .map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
            )
            .ok_or(EmbeddingColumnarError::SizeOverflow)
    })?;
    let fixed = row_count
        .checked_mul(size_of::<u64>() * 2)
        .and_then(|value| value.checked_add((row_count + 1).checked_mul(size_of::<i32>())?))
        .and_then(|value| value.checked_add(validity_bytes(row_count).ok()?))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    identifiers
        .checked_add(u64::try_from(fixed).map_err(|_| EmbeddingColumnarError::SizeOverflow)?)
        .ok_or(EmbeddingColumnarError::SizeOverflow)
}
