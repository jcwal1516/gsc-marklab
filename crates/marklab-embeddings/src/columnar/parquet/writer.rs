use std::io::{self, Write};

use bytes::Bytes;
use marklab_project::{ContentDigest, ContentDigestWriter};
use parquet::{
    arrow::arrow_writer::{ArrowWriter, ArrowWriterOptions},
    file::metadata::{ColumnChunkMetaData, ParquetMetaData, RowGroupMetaData},
    format::{ColumnChunk, ColumnMetaData, Encoding, FileMetaData, PageEncodingStats, RowGroup},
};

use crate::{CellEmbeddingTable, EmbeddingStatus};

use super::{
    super::{
        arrow::{build_embedding_record_batch, validate_embedding_table_bindings},
        CellEmbeddingTablePhysicalBindings, ColumnarWriteSummary, EmbeddingColumnarBudgets,
        EmbeddingColumnarError,
    },
    compact::canonical_compact_len,
    profile::{
        embedding_decoded_bytes, embedding_schema, writer_properties,
        MAXIMUM_APPLICATION_METADATA_BYTES, MAXIMUM_DIMENSION, MAXIMUM_FOOTER_BYTES, MAXIMUM_ROWS,
        PUBLIC_BATCH_ROWS, ROOT_NAME, ROW_GROUP_ROWS,
    },
};

pub(super) const MAXIMUM_ENCODED_FOOTER_BASE_BYTES: usize = 64 * 1024;
pub(super) const MAXIMUM_ENCODED_FOOTER_BYTES_PER_ROW_GROUP: usize = 2 * 1024;
pub(super) const WRITER_FIXED_RETAINED_BYTES: usize = 64 * 1024;

/// Write the exact deterministic Parquet profile for a canonical embedding table.
///
/// The retained-byte budget covers Marklab and locked Parquet allocations, but not buffering owned
/// by the caller-provided output sink.
pub fn write_cell_embedding_table_parquet(
    output: &mut (dyn Write + Send),
    table: &CellEmbeddingTable,
    bindings: CellEmbeddingTablePhysicalBindings,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ColumnarWriteSummary, EmbeddingColumnarError> {
    validate_embedding_table_bindings(table, bindings)?;
    if table.dimension() > MAXIMUM_DIMENSION || table.row_count() > MAXIMUM_ROWS {
        return Err(EmbeddingColumnarError::ParquetWriter);
    }
    let (identifier_bytes, status_bytes) = total_text_bytes(table)?;
    let decoded_bytes = embedding_decoded_bytes(
        table.row_count(),
        table.dimension(),
        identifier_bytes,
        status_bytes,
    )?;
    enforce_decoded_budget(decoded_bytes, budgets)?;
    let estimates = estimate_writer_memory(table)?;
    enforce_row_group_budget(estimates.row_group_bytes, budgets)?;
    enforce_retained_budget(estimates.retained_bytes, budgets)?;

    let schema = embedding_schema(table.dimension())?;
    let options = ArrowWriterOptions::new()
        .with_properties(writer_properties(bindings))
        .with_skip_arrow_metadata(true)
        .with_schema_root(ROOT_NAME.to_owned());
    let mut sink = ParquetDigestingWriter::new(output, budgets.maximum_file_bytes());
    let write_result = (|| {
        let mut writer = ArrowWriter::try_new_with_options(
            &mut sink,
            std::sync::Arc::new(schema.clone()),
            options,
        )
        .map_err(|_| EmbeddingColumnarError::ParquetWriter)?;
        let mut start = 0_usize;
        while start < table.row_count() {
            let end = start
                .checked_add(PUBLIC_BATCH_ROWS)
                .map(|value| value.min(table.row_count()))
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
            let batch =
                build_embedding_record_batch(table, &schema, start, end, table.dimension())?;
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
    Ok(ColumnarWriteSummary::new(
        content_digest,
        encoded_byte_len,
        u64::try_from(table.row_count()).map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
        table.dimension(),
    ))
}

#[derive(Clone, Copy)]
struct WriterMemoryEstimate {
    row_group_bytes: usize,
    retained_bytes: usize,
}

fn estimate_writer_memory(
    table: &CellEmbeddingTable,
) -> Result<WriterMemoryEstimate, EmbeddingColumnarError> {
    let row_group_count = table.row_count().div_ceil(ROW_GROUP_ROWS);
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

    let group_rows = table.row_count().min(ROW_GROUP_ROWS);
    let batch_rows = table.row_count().min(PUBLIC_BATCH_ROWS);
    let dimension =
        usize::try_from(table.dimension()).map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
    let group_components = group_rows
        .checked_mul(dimension)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let batch_components = batch_rows
        .checked_mul(dimension)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let (group_identifier_bytes, group_status_bytes) = maximum_text_bytes(table, ROW_GROUP_ROWS)?;
    let (batch_identifier_bytes, _) = maximum_text_bytes(table, PUBLIC_BATCH_ROWS)?;
    let batch_status_bytes = batch_rows
        .checked_mul(EmbeddingStatus::ExtractionFailed.wire_name().len())
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;

    let uncompressed_value_bytes = group_components
        .checked_mul(size_of::<f32>())
        .and_then(|value| value.checked_add(group_rows.checked_mul(size_of::<i32>())?))
        .and_then(|value| value.checked_add(group_identifier_bytes))
        .and_then(|value| value.checked_add(group_rows.checked_mul(size_of::<i32>())?))
        .and_then(|value| value.checked_add(group_status_bytes))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let level_bytes = validity_bytes(group_components)?
        .checked_mul(4)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let page_count_bound = group_rows
        .checked_mul(3)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let page_header_capacity = page_count_bound
        .checked_mul(1024)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let page_vector_capacity = page_count_bound
        .checked_mul(4)
        .and_then(|value| value.checked_mul(size_of::<Bytes>()))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let retained_row_group_chunks = uncompressed_value_bytes
        .checked_add(level_bytes)
        .and_then(|value| value.checked_mul(2))
        .and_then(|value| value.checked_add(page_header_capacity))
        .and_then(|value| value.checked_add(page_vector_capacity))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;

    let batch_input_bytes = batch_components
        .checked_mul(size_of::<f32>())
        .and_then(|value| value.checked_add(batch_identifier_bytes))
        .and_then(|value| value.checked_add(batch_status_bytes))
        .and_then(|value| {
            value.checked_add(
                batch_rows
                    .checked_add(1)?
                    .checked_mul(size_of::<i32>())?
                    .checked_mul(2)?,
            )
        })
        .and_then(|value| value.checked_add(validity_bytes(batch_rows).ok()?.checked_mul(3)?))
        .and_then(|value| value.checked_add(validity_bytes(batch_components).ok()?))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let fixed_list_level_workspaces = batch_components
        .checked_mul(16)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let page_assembly_transient = uncompressed_value_bytes
        .checked_add(level_bytes)
        .and_then(|value| value.checked_mul(4))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let metadata_retained = estimate_metadata_retained_bytes(row_group_count)?;
    let row_group_local_peak = batch_input_bytes
        .checked_add(fixed_list_level_workspaces)
        .and_then(|value| value.checked_add(retained_row_group_chunks))
        .and_then(|value| value.checked_add(page_assembly_transient))
        .and_then(|value| value.checked_add(WRITER_FIXED_RETAINED_BYTES))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let row_group_peak = row_group_local_peak
        .checked_add(metadata_retained)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let footer_peak = metadata_retained
        .checked_add(WRITER_FIXED_RETAINED_BYTES)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    Ok(WriterMemoryEstimate {
        row_group_bytes: row_group_local_peak,
        retained_bytes: row_group_peak.max(footer_peak),
    })
}

fn maximum_text_bytes(
    table: &CellEmbeddingTable,
    chunk_rows: usize,
) -> Result<(usize, usize), EmbeddingColumnarError> {
    let mut maximum_identifiers = 0_usize;
    let mut maximum_statuses = 0_usize;
    let mut identifiers = 0_usize;
    let mut statuses = 0_usize;
    for index in 0..table.row_count() {
        if index != 0 && index % chunk_rows == 0 {
            maximum_identifiers = maximum_identifiers.max(identifiers);
            maximum_statuses = maximum_statuses.max(statuses);
            identifiers = 0;
            statuses = 0;
        }
        let row = table
            .row(index)
            .map_err(|_| EmbeddingColumnarError::ParquetWriter)?;
        identifiers = identifiers
            .checked_add(row.cell_id().as_str().len())
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        statuses = statuses
            .checked_add(row.status().wire_name().len())
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    }
    Ok((
        maximum_identifiers.max(identifiers),
        maximum_statuses.max(statuses),
    ))
}

fn total_text_bytes(table: &CellEmbeddingTable) -> Result<(usize, usize), EmbeddingColumnarError> {
    let mut identifiers = 0_usize;
    let mut statuses = 0_usize;
    for index in 0..table.row_count() {
        let row = table
            .row(index)
            .map_err(|_| EmbeddingColumnarError::ParquetWriter)?;
        identifiers = identifiers
            .checked_add(row.cell_id().as_str().len())
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        statuses = statuses
            .checked_add(row.status().wire_name().len())
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    }
    Ok((identifiers, statuses))
}

pub(super) fn estimate_metadata_retained_bytes(
    row_group_count: usize,
) -> Result<usize, EmbeddingColumnarError> {
    let raw_group_inline = size_of::<RowGroup>()
        .checked_add(3 * size_of::<ColumnChunk>())
        .and_then(|value| value.checked_add(3 * size_of::<ColumnMetaData>()))
        .and_then(|value| value.checked_add(6 * size_of::<Encoding>()))
        .and_then(|value| value.checked_add(5 * size_of::<String>()))
        .and_then(|value| value.checked_add(3 * size_of::<PageEncodingStats>()))
        .and_then(|value| value.checked_add(128))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let high_level_group_inline = size_of::<RowGroupMetaData>()
        .checked_add(3 * size_of::<ColumnChunkMetaData>())
        .and_then(|value| value.checked_add(256))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let per_group = raw_group_inline
        .checked_add(high_level_group_inline)
        .and_then(|value| value.checked_add(MAXIMUM_ENCODED_FOOTER_BYTES_PER_ROW_GROUP))
        .and_then(|value| value.checked_mul(2))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let base = MAXIMUM_APPLICATION_METADATA_BYTES
        .checked_add(MAXIMUM_ENCODED_FOOTER_BASE_BYTES)
        .and_then(|value| value.checked_add(size_of::<FileMetaData>()))
        .and_then(|value| value.checked_add(size_of::<ParquetMetaData>()))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    base.checked_add(
        row_group_count
            .checked_mul(per_group)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?,
    )
    .ok_or(EmbeddingColumnarError::SizeOverflow)
}

pub(super) fn validity_bytes(values: usize) -> Result<usize, EmbeddingColumnarError> {
    values
        .checked_add(7)
        .map(|value| value / 8)
        .ok_or(EmbeddingColumnarError::SizeOverflow)
}

fn enforce_decoded_budget(
    required: u64,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), EmbeddingColumnarError> {
    if required > budgets.maximum_decoded_bytes() {
        return Err(EmbeddingColumnarError::DecodedByteBudgetExceeded {
            required,
            maximum: budgets.maximum_decoded_bytes(),
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

pub(super) struct ParquetDigestingWriter<'a> {
    output: &'a mut (dyn Write + Send),
    digest: ContentDigestWriter,
    byte_len: u64,
    maximum_file_bytes: u64,
    budget_exceeded: Option<u64>,
}

impl<'a> ParquetDigestingWriter<'a> {
    pub(super) fn new(output: &'a mut (dyn Write + Send), maximum_file_bytes: u64) -> Self {
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

impl Write for ParquetDigestingWriter<'_> {
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
