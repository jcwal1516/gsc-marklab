use std::{io::Write, sync::Arc};

use arrow::{
    array::{ArrayRef, StringBuilder, UInt64Array, UInt64Builder},
    buffer::{BooleanBuffer, NullBuffer},
    datatypes::Schema,
    record_batch::RecordBatch,
};
use arrow_ipc::{
    writer::{FileWriter, IpcWriteOptions},
    MetadataVersion,
};

use crate::CellEmbeddingRowLink;

use super::{
    super::{
        super::super::{ColumnarError, EmbeddingColumnarBudgets, RowLinkColumnarWriteSummary},
        profile::{
            enforce_decoded_budget, enforce_retained_budget, enforce_row_group_budget,
            validity_bytes, ALIGNMENT, MAXIMUM_MESSAGE_BYTES, RECORD_BATCH_ROWS,
        },
        writer::DigestingWriter,
    },
    profile::row_link_schema,
};

/// Write the exact deterministic Arrow IPC profile for a canonical row link.
pub fn write_cell_embedding_row_link_arrow(
    output: &mut dyn Write,
    row_link: &CellEmbeddingRowLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<RowLinkColumnarWriteSummary, ColumnarError> {
    let row_count = row_link.entries().len();
    let decoded_bytes = estimate_decoded_bytes(row_link)?;
    enforce_decoded_budget(decoded_bytes, budgets)?;
    let maximum_batch_bytes = estimate_batch_bytes(row_link)?;
    enforce_row_group_budget(maximum_batch_bytes, budgets)?;

    let record_batch_count = row_count.div_ceil(RECORD_BATCH_ROWS);
    let record_block_bytes = record_batch_count
        .checked_mul(size_of::<arrow_ipc::Block>())
        .ok_or(ColumnarError::SizeOverflow)?;
    let record_block_capacity_bytes = record_block_bytes
        .checked_mul(2)
        .and_then(|value| value.checked_add(size_of::<Vec<arrow_ipc::Block>>()))
        .ok_or(ColumnarError::SizeOverflow)?;
    let batch_peak_bytes = maximum_batch_bytes
        .checked_mul(2)
        .and_then(|value| value.checked_add(record_block_capacity_bytes))
        .ok_or(ColumnarError::SizeOverflow)?;
    let footer_builder_bytes = record_block_bytes
        .checked_add(MAXIMUM_MESSAGE_BYTES)
        .and_then(|value| value.checked_mul(2))
        .ok_or(ColumnarError::SizeOverflow)?;
    let footer_peak_bytes = record_block_capacity_bytes
        .checked_add(footer_builder_bytes)
        .and_then(|value| value.checked_add(MAXIMUM_MESSAGE_BYTES))
        .ok_or(ColumnarError::SizeOverflow)?;
    enforce_retained_budget(batch_peak_bytes.max(footer_peak_bytes), budgets)?;

    let schema = row_link_schema(row_link)?;
    let options = IpcWriteOptions::try_new(ALIGNMENT, false, MetadataVersion::V5)
        .map_err(|_| ColumnarError::ArrowWriter)?;
    let mut sink = DigestingWriter::new(output, budgets.maximum_file_bytes());
    let write_result = (|| {
        let mut writer = FileWriter::try_new_with_options(&mut sink, &schema, options)
            .map_err(|_| ColumnarError::ArrowWriter)?;
        let mut start = 0_usize;
        while start < row_count {
            let end = start
                .checked_add(RECORD_BATCH_ROWS)
                .map(|value| value.min(row_count))
                .ok_or(ColumnarError::SizeOverflow)?;
            let batch = build_batch(row_link, &schema, start, end)?;
            writer
                .write(&batch)
                .map_err(|_| ColumnarError::ArrowWriter)?;
            start = end;
        }
        writer.finish().map_err(|_| ColumnarError::ArrowWriter)
    })();
    if let Some(observed) = sink.budget_exceeded() {
        return Err(ColumnarError::FileByteBudgetExceeded {
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
    schema: &Schema,
    start: usize,
    end: usize,
) -> Result<RecordBatch, ColumnarError> {
    let rows = end.checked_sub(start).ok_or(ColumnarError::SizeOverflow)?;
    let identifier_bytes =
        row_link.entries()[start..end]
            .iter()
            .try_fold(0_usize, |total, entry| {
                total
                    .checked_add(entry.cell_id().as_str().len())
                    .ok_or(ColumnarError::SizeOverflow)
            })?;
    let mut cells = StringBuilder::with_capacity(rows, identifier_bytes);
    let mut source_cells = UInt64Builder::with_capacity(rows);
    let mut source_embedding_values = Vec::with_capacity(rows);
    let entries = &row_link.entries()[start..end];
    for entry in entries {
        cells.append_value(entry.cell_id().as_str());
        source_cells.append_value(entry.source_cell_row());
        source_embedding_values.push(entry.source_embedding_row().unwrap_or_default());
    }
    // The frozen profile requires a physical validity bitmap even for all-valid batches.
    // Constructing the nullable column from a builder would elide that bitmap when its
    // null count is zero and make equivalent logical inputs encode differently.
    let source_embedding_validity = NullBuffer::new(BooleanBuffer::collect_bool(rows, |row| {
        entries[row].source_embedding_row().is_some()
    }));
    let source_embeddings = UInt64Array::new(
        source_embedding_values.into(),
        Some(source_embedding_validity),
    );
    let columns: Vec<ArrayRef> = vec![
        Arc::new(cells.finish()),
        Arc::new(source_cells.finish()),
        Arc::new(source_embeddings),
    ];
    RecordBatch::try_new(Arc::new(schema.clone()), columns).map_err(|_| ColumnarError::ArrowWriter)
}

fn estimate_decoded_bytes(row_link: &CellEmbeddingRowLink) -> Result<u64, ColumnarError> {
    let mut required = 0_u64;
    for batch in row_link.entries().chunks(RECORD_BATCH_ROWS) {
        let rows = batch.len();
        let identifier_bytes = batch.iter().try_fold(0_u64, |total, entry| {
            total
                .checked_add(
                    u64::try_from(entry.cell_id().as_str().len())
                        .map_err(|_| ColumnarError::SizeOverflow)?,
                )
                .ok_or(ColumnarError::SizeOverflow)
        })?;
        let offsets = rows
            .checked_add(1)
            .and_then(|value| value.checked_mul(size_of::<i32>()))
            .ok_or(ColumnarError::SizeOverflow)?;
        let values = rows
            .checked_mul(2)
            .and_then(|value| value.checked_mul(size_of::<u64>()))
            .ok_or(ColumnarError::SizeOverflow)?;
        let validity = validity_bytes(rows)?
            .checked_mul(3)
            .ok_or(ColumnarError::SizeOverflow)?;
        let batch_bytes = identifier_bytes
            .checked_add(u64::try_from(offsets).map_err(|_| ColumnarError::SizeOverflow)?)
            .and_then(|value| value.checked_add(u64::try_from(values).ok()?))
            .and_then(|value| value.checked_add(u64::try_from(validity).ok()?))
            .ok_or(ColumnarError::SizeOverflow)?;
        required = required
            .checked_add(batch_bytes)
            .ok_or(ColumnarError::SizeOverflow)?;
    }
    Ok(required)
}

fn estimate_batch_bytes(row_link: &CellEmbeddingRowLink) -> Result<usize, ColumnarError> {
    let rows = row_link.entries().len().min(RECORD_BATCH_ROWS);
    let identifier_bytes = row_link
        .entries()
        .chunks(RECORD_BATCH_ROWS)
        .map(|batch| {
            batch.iter().try_fold(0_usize, |total, entry| {
                total
                    .checked_add(entry.cell_id().as_str().len())
                    .ok_or(ColumnarError::SizeOverflow)
            })
        })
        .try_fold(0_usize, |maximum, bytes| {
            bytes.map(|bytes| maximum.max(bytes))
        })?;
    let offsets = rows
        .checked_add(1)
        .and_then(|value| value.checked_mul(size_of::<i32>()))
        .ok_or(ColumnarError::SizeOverflow)?;
    let values = rows
        .checked_mul(2)
        .and_then(|value| value.checked_mul(size_of::<u64>()))
        .ok_or(ColumnarError::SizeOverflow)?;
    let validity = validity_bytes(rows)?
        .checked_mul(3)
        .ok_or(ColumnarError::SizeOverflow)?;
    identifier_bytes
        .checked_add(offsets)
        .and_then(|value| value.checked_add(values))
        .and_then(|value| value.checked_add(validity))
        .and_then(|value| value.checked_add(7 * (ALIGNMENT - 1)))
        .and_then(|value| value.checked_add(MAXIMUM_MESSAGE_BYTES))
        .ok_or(ColumnarError::SizeOverflow)
}
