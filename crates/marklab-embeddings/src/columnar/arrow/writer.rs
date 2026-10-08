use std::{
    io::{self, Write},
    sync::Arc,
};

use arrow::{
    array::{ArrayRef, FixedSizeListBuilder, Float32Builder, StringBuilder},
    datatypes::{DataType, Schema},
    record_batch::RecordBatch,
};
use arrow_ipc::{
    writer::{FileWriter, IpcWriteOptions},
    MetadataVersion,
};
use marklab_project::{ContentDigest, ContentDigestWriter};

use crate::{CellEmbeddingTable, EmbeddingStatus};

use super::{
    super::{
        CellEmbeddingTablePhysicalBindings, ColumnarWriteSummary, EmbeddingColumnarBudgets,
        EmbeddingColumnarError,
    },
    profile::{
        component_bytes, embedding_schema, enforce_decoded_budget, enforce_retained_budget,
        enforce_row_group_budget, validate_shape, validity_bytes, ALIGNMENT, MAXIMUM_MESSAGE_BYTES,
        RECORD_BATCH_ROWS,
    },
};

/// Write the exact deterministic Arrow IPC profile for a canonical embedding table.
pub fn write_cell_embedding_table_arrow(
    output: &mut dyn Write,
    table: &CellEmbeddingTable,
    bindings: CellEmbeddingTablePhysicalBindings,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ColumnarWriteSummary, EmbeddingColumnarError> {
    validate_table_bindings(table, bindings)?;
    let row_count = table.row_count();
    let dimension = table.dimension();
    validate_shape(row_count, dimension)?;
    let decoded_bytes = estimate_writer_decoded_bytes(table, dimension)?;
    enforce_decoded_budget(decoded_bytes, budgets)?;

    let maximum_batch_bytes = estimate_writer_batch_bytes(table, dimension)?;
    enforce_row_group_budget(maximum_batch_bytes, budgets)?;
    let record_batch_count = row_count.div_ceil(RECORD_BATCH_ROWS);
    let record_block_bytes = record_batch_count
        .checked_mul(size_of::<arrow_ipc::Block>())
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let record_block_capacity_bytes = record_block_bytes
        .checked_mul(2)
        .and_then(|value| value.checked_add(size_of::<Vec<arrow_ipc::Block>>()))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let batch_peak_bytes = maximum_batch_bytes
        .checked_mul(2)
        .and_then(|value| value.checked_add(record_block_capacity_bytes))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let footer_builder_bytes = record_block_bytes
        .checked_add(MAXIMUM_MESSAGE_BYTES)
        .and_then(|value| value.checked_mul(2))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let footer_peak_bytes = record_block_capacity_bytes
        .checked_add(footer_builder_bytes)
        .and_then(|value| value.checked_add(MAXIMUM_MESSAGE_BYTES))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let retained_bytes = batch_peak_bytes.max(footer_peak_bytes);
    enforce_retained_budget(retained_bytes, budgets)?;

    let schema = embedding_schema(dimension, bindings)?;
    let options = IpcWriteOptions::try_new(ALIGNMENT, false, MetadataVersion::V5)
        .map_err(|_| EmbeddingColumnarError::ArrowWriter)?;
    let mut sink = DigestingWriter::new(output, budgets.maximum_file_bytes());
    let write_result = (|| {
        let mut writer = FileWriter::try_new_with_options(&mut sink, &schema, options)
            .map_err(|_| EmbeddingColumnarError::ArrowWriter)?;
        let mut start = 0_usize;
        while start < row_count {
            let end = start
                .checked_add(RECORD_BATCH_ROWS)
                .map(|value| value.min(row_count))
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
            let batch = build_record_batch(table, &schema, start, end, dimension)?;
            writer
                .write(&batch)
                .map_err(|_| EmbeddingColumnarError::ArrowWriter)?;
            start = end;
        }
        writer
            .finish()
            .map_err(|_| EmbeddingColumnarError::ArrowWriter)
    })();
    if let Some(observed) = sink.budget_exceeded() {
        return Err(EmbeddingColumnarError::FileByteBudgetExceeded {
            observed,
            maximum: budgets.maximum_file_bytes(),
        });
    }
    write_result?;
    let (content_digest, encoded_byte_len) = sink.finish();
    Ok(ColumnarWriteSummary::new(
        content_digest,
        encoded_byte_len,
        u64::try_from(row_count).map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
        dimension,
    ))
}

/// Validate hostile Arrow bytes through the bounded canonical-profile preflight only.
pub(crate) fn build_record_batch(
    table: &CellEmbeddingTable,
    schema: &Schema,
    start: usize,
    end: usize,
    dimension: u32,
) -> Result<RecordBatch, EmbeddingColumnarError> {
    let row_count = end
        .checked_sub(start)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let dimension_usize =
        usize::try_from(dimension).map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
    let component_count = row_count
        .checked_mul(dimension_usize)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let identifier_bytes = (start..end).try_fold(0_usize, |total, index| {
        let row = table
            .row(index)
            .map_err(|_| EmbeddingColumnarError::ArrowWriter)?;
        total
            .checked_add(row.cell_id().as_str().len())
            .ok_or(EmbeddingColumnarError::SizeOverflow)
    })?;
    let mut cells = StringBuilder::with_capacity(row_count, identifier_bytes);
    let values = Float32Builder::with_capacity(component_count);
    let list_field = match schema.field(1).data_type() {
        DataType::FixedSizeList(field, length)
            if *length
                == i32::try_from(dimension)
                    .map_err(|_| EmbeddingColumnarError::SizeOverflow)? =>
        {
            Arc::clone(field)
        }
        _ => return Err(EmbeddingColumnarError::ArrowWriter),
    };
    let mut embeddings = FixedSizeListBuilder::with_capacity(
        values,
        i32::try_from(dimension).map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
        row_count,
    )
    .with_field(list_field);
    let mut statuses = StringBuilder::with_capacity(
        row_count,
        row_count
            .checked_mul(EmbeddingStatus::ExtractionFailed.wire_name().len())
            .ok_or(EmbeddingColumnarError::SizeOverflow)?,
    );
    for index in start..end {
        let row = table
            .row(index)
            .map_err(|_| EmbeddingColumnarError::ArrowWriter)?;
        cells.append_value(row.cell_id().as_str());
        if let Some(vector) = row.vector() {
            embeddings.values().append_slice(vector);
        } else {
            for _ in 0..dimension_usize {
                embeddings.values().append_value(0.0);
            }
        }
        embeddings.append(true);
        statuses.append_value(row.status().wire_name());
    }
    let columns: Vec<ArrayRef> = vec![
        Arc::new(cells.finish()),
        Arc::new(embeddings.finish()),
        Arc::new(statuses.finish()),
    ];
    RecordBatch::try_new(Arc::new(schema.clone()), columns)
        .map_err(|_| EmbeddingColumnarError::ArrowWriter)
}

pub(crate) fn validate_table_bindings(
    table: &CellEmbeddingTable,
    bindings: CellEmbeddingTablePhysicalBindings,
) -> Result<(), EmbeddingColumnarError> {
    if table.qc_summary().logical_digest() != bindings.table_logical_digest()
        || !table.matches_physical_bindings(
            bindings.expected_cells_artifact_id(),
            bindings.provenance_artifact_id(),
            bindings.row_link_logical_digest(),
        )
    {
        return Err(EmbeddingColumnarError::ArtifactBindingMismatch);
    }
    Ok(())
}

pub(crate) fn estimate_writer_batch_bytes(
    table: &CellEmbeddingTable,
    dimension: u32,
) -> Result<usize, EmbeddingColumnarError> {
    let row_count = table.row_count().min(RECORD_BATCH_ROWS);
    let mut identifier_bytes = 0_usize;
    let mut current_identifier_bytes = 0_usize;
    for index in 0..table.row_count() {
        if index != 0 && index % RECORD_BATCH_ROWS == 0 {
            identifier_bytes = identifier_bytes.max(current_identifier_bytes);
            current_identifier_bytes = 0;
        }
        let row = table
            .row(index)
            .map_err(|_| EmbeddingColumnarError::ArrowWriter)?;
        current_identifier_bytes = current_identifier_bytes
            .checked_add(row.cell_id().as_str().len())
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    }
    identifier_bytes = identifier_bytes.max(current_identifier_bytes);
    let component_bytes = row_count
        .checked_mul(usize::try_from(dimension).map_err(|_| EmbeddingColumnarError::SizeOverflow)?)
        .and_then(|value| value.checked_mul(size_of::<f32>()))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let offsets = row_count
        .checked_add(1)
        .and_then(|value| value.checked_mul(size_of::<i32>()))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let status_bytes = row_count
        .checked_mul(EmbeddingStatus::ExtractionFailed.wire_name().len())
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let row_validity_bytes = validity_bytes(row_count)?;
    let value_validity_bytes = validity_bytes(
        row_count
            .checked_mul(
                usize::try_from(dimension).map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
            )
            .ok_or(EmbeddingColumnarError::SizeOverflow)?,
    )?;
    component_bytes
        .checked_add(identifier_bytes)
        .and_then(|value| value.checked_add(status_bytes))
        .and_then(|value| value.checked_add(offsets.checked_mul(2)?))
        .and_then(|value| value.checked_add(row_validity_bytes.checked_mul(3)?))
        .and_then(|value| value.checked_add(value_validity_bytes))
        .and_then(|value| value.checked_add(9 * (ALIGNMENT - 1)))
        .and_then(|value| value.checked_add(MAXIMUM_MESSAGE_BYTES))
        .ok_or(EmbeddingColumnarError::SizeOverflow)
}

pub(crate) fn estimate_writer_decoded_bytes(
    table: &CellEmbeddingTable,
    dimension: u32,
) -> Result<u64, EmbeddingColumnarError> {
    let dimension_usize =
        usize::try_from(dimension).map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
    let mut required = 0_u64;
    let mut start = 0_usize;
    while start < table.row_count() {
        let end = start
            .checked_add(RECORD_BATCH_ROWS)
            .map(|value| value.min(table.row_count()))
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let rows = end
            .checked_sub(start)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let element_count = rows
            .checked_mul(dimension_usize)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let string_bytes = (start..end).try_fold(0_u64, |total, index| {
            let row = table
                .row(index)
                .map_err(|_| EmbeddingColumnarError::ArrowWriter)?;
            let cell_bytes = u64::try_from(row.cell_id().as_str().len())
                .map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
            let status_bytes = u64::try_from(row.status().wire_name().len())
                .map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
            total
                .checked_add(cell_bytes)
                .and_then(|value| value.checked_add(status_bytes))
                .ok_or(EmbeddingColumnarError::SizeOverflow)
        })?;
        let offset_bytes = rows
            .checked_add(1)
            .and_then(|value| value.checked_mul(size_of::<i32>()))
            .and_then(|value| value.checked_mul(2))
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let row_validity_bytes = validity_bytes(rows)?;
        let element_validity_bytes = validity_bytes(element_count)?;
        let validity_buffer_bytes = row_validity_bytes
            .checked_mul(3)
            .and_then(|value| value.checked_add(element_validity_bytes))
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let offset_bytes =
            u64::try_from(offset_bytes).map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
        let validity_buffer_bytes = u64::try_from(validity_buffer_bytes)
            .map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
        let batch_required = component_bytes(rows, dimension)?
            .checked_add(string_bytes)
            .and_then(|value| value.checked_add(offset_bytes))
            .and_then(|value| value.checked_add(validity_buffer_bytes))
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        required = required
            .checked_add(batch_required)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        start = end;
    }
    Ok(required)
}

pub(super) struct DigestingWriter<'a> {
    output: &'a mut dyn Write,
    digest: ContentDigestWriter,
    byte_len: u64,
    maximum_file_bytes: u64,
    budget_exceeded: Option<u64>,
}

impl<'a> DigestingWriter<'a> {
    pub(super) fn new(output: &'a mut dyn Write, maximum_file_bytes: u64) -> Self {
        Self {
            output,
            digest: ContentDigestWriter::default(),
            byte_len: 0,
            maximum_file_bytes,
            budget_exceeded: None,
        }
    }

    pub(super) fn budget_exceeded(&self) -> Option<u64> {
        self.budget_exceeded
    }

    pub(super) fn finish(self) -> (ContentDigest, u64) {
        self.digest.finish()
    }
}

impl Write for DigestingWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let added =
            u64::try_from(buffer.len()).map_err(|_| io::Error::other("columnar size overflow"))?;
        let attempted = self
            .byte_len
            .checked_add(added)
            .ok_or_else(|| io::Error::other("columnar size overflow"))?;
        if attempted > self.maximum_file_bytes {
            self.budget_exceeded = Some(attempted);
            return Err(io::Error::other("columnar file budget exceeded"));
        }
        let written = self.output.write(buffer)?;
        self.digest.write_all(&buffer[..written])?;
        self.byte_len = self
            .byte_len
            .checked_add(
                u64::try_from(written).map_err(|_| io::Error::other("columnar size overflow"))?,
            )
            .ok_or_else(|| io::Error::other("columnar size overflow"))?;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.output.flush()
    }
}
