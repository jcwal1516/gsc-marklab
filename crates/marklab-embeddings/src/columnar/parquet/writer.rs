use std::io::{self, Write};

use bytes::Bytes;
use marklab_project::{ContentDigest, ContentDigestWriter};
use parquet::{
    arrow::arrow_writer::{ArrowWriter, ArrowWriterOptions},
    file::metadata::{ColumnChunkMetaData, ParquetMetaData, RowGroupMetaData},
};

use crate::columnar::parquet::format::{
    ColumnChunk, ColumnMetaData, Encoding, FileMetaData, PageEncodingStats, RowGroup,
};

use crate::{CellEmbeddingTable, EmbeddingStatus};

use super::{
    super::{
        arrow::{build_embedding_record_batch, validate_embedding_table_bindings},
        enforce_decoded_budget, enforce_retained_budget, enforce_row_group_budget,
        CellEmbeddingTablePhysicalBindings, ColumnarError, ColumnarWriteSummary,
        EmbeddingColumnarBudgets,
    },
    profile::{
        embedding_decoded_bytes, embedding_schema, writer_properties,
        MAXIMUM_APPLICATION_METADATA_BYTES, MAXIMUM_DIMENSION, MAXIMUM_FOOTER_BYTES, MAXIMUM_ROWS,
        PARQUET_MAGIC, PUBLIC_BATCH_ROWS, ROOT_NAME, ROW_GROUP_ROWS, TRAILER_BYTES,
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
) -> Result<ColumnarWriteSummary, ColumnarError> {
    validate_embedding_table_bindings(table, bindings)?;
    if table.dimension() > MAXIMUM_DIMENSION || table.row_count() > MAXIMUM_ROWS {
        return Err(ColumnarError::ParquetWriter);
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
        .map_err(|_| ColumnarError::ParquetWriter)?;
        let mut start = 0_usize;
        while start < table.row_count() {
            let end = start
                .checked_add(PUBLIC_BATCH_ROWS)
                .map(|value| value.min(table.row_count()))
                .ok_or(ColumnarError::SizeOverflow)?;
            let batch =
                build_embedding_record_batch(table, &schema, start, end, table.dimension())?;
            writer
                .write(&batch)
                .map_err(|_| ColumnarError::ParquetWriter)?;
            start = end;
        }
        writer.close().map_err(|_| ColumnarError::ParquetWriter)?;
        sink.require_footer_within(MAXIMUM_FOOTER_BYTES)
    })();
    if let Some(observed) = sink.budget_exceeded() {
        return Err(ColumnarError::FileByteBudgetExceeded {
            observed,
            maximum: budgets.maximum_file_bytes(),
        });
    }
    write_result?;
    let (content_digest, encoded_byte_len) = sink.finish();
    Ok(ColumnarWriteSummary::new(
        content_digest,
        encoded_byte_len,
        u64::try_from(table.row_count()).map_err(|_| ColumnarError::SizeOverflow)?,
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
) -> Result<WriterMemoryEstimate, ColumnarError> {
    let row_group_count = table.row_count().div_ceil(ROW_GROUP_ROWS);
    let encoded_footer_bytes = MAXIMUM_ENCODED_FOOTER_BASE_BYTES
        .checked_add(
            row_group_count
                .checked_mul(MAXIMUM_ENCODED_FOOTER_BYTES_PER_ROW_GROUP)
                .ok_or(ColumnarError::SizeOverflow)?,
        )
        .ok_or(ColumnarError::SizeOverflow)?;
    if encoded_footer_bytes > MAXIMUM_FOOTER_BYTES {
        return Err(ColumnarError::ParquetWriter);
    }

    let group_rows = table.row_count().min(ROW_GROUP_ROWS);
    let batch_rows = table.row_count().min(PUBLIC_BATCH_ROWS);
    let dimension = usize::try_from(table.dimension()).map_err(|_| ColumnarError::SizeOverflow)?;
    let group_components = group_rows
        .checked_mul(dimension)
        .ok_or(ColumnarError::SizeOverflow)?;
    let batch_components = batch_rows
        .checked_mul(dimension)
        .ok_or(ColumnarError::SizeOverflow)?;
    let (group_identifier_bytes, group_status_bytes) = maximum_text_bytes(table, ROW_GROUP_ROWS)?;
    let (batch_identifier_bytes, _) = maximum_text_bytes(table, PUBLIC_BATCH_ROWS)?;
    let batch_status_bytes = batch_rows
        .checked_mul(EmbeddingStatus::ExtractionFailed.wire_name().len())
        .ok_or(ColumnarError::SizeOverflow)?;

    let uncompressed_value_bytes = group_components
        .checked_mul(size_of::<f32>())
        .and_then(|value| value.checked_add(group_rows.checked_mul(size_of::<i32>())?))
        .and_then(|value| value.checked_add(group_identifier_bytes))
        .and_then(|value| value.checked_add(group_rows.checked_mul(size_of::<i32>())?))
        .and_then(|value| value.checked_add(group_status_bytes))
        .ok_or(ColumnarError::SizeOverflow)?;
    let level_bytes = validity_bytes(group_components)?
        .checked_mul(4)
        .ok_or(ColumnarError::SizeOverflow)?;
    let page_count_bound = group_rows
        .checked_mul(3)
        .ok_or(ColumnarError::SizeOverflow)?;
    let page_header_capacity = page_count_bound
        .checked_mul(1024)
        .ok_or(ColumnarError::SizeOverflow)?;
    let page_vector_capacity = page_count_bound
        .checked_mul(4)
        .and_then(|value| value.checked_mul(size_of::<Bytes>()))
        .ok_or(ColumnarError::SizeOverflow)?;
    let retained_row_group_chunks = uncompressed_value_bytes
        .checked_add(level_bytes)
        .and_then(|value| value.checked_mul(2))
        .and_then(|value| value.checked_add(page_header_capacity))
        .and_then(|value| value.checked_add(page_vector_capacity))
        .ok_or(ColumnarError::SizeOverflow)?;

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
        .ok_or(ColumnarError::SizeOverflow)?;
    let fixed_list_level_workspaces = batch_components
        .checked_mul(16)
        .ok_or(ColumnarError::SizeOverflow)?;
    let page_assembly_transient = uncompressed_value_bytes
        .checked_add(level_bytes)
        .and_then(|value| value.checked_mul(4))
        .ok_or(ColumnarError::SizeOverflow)?;
    let metadata_retained = estimate_metadata_retained_bytes(row_group_count)?;
    let row_group_local_peak = batch_input_bytes
        .checked_add(fixed_list_level_workspaces)
        .and_then(|value| value.checked_add(retained_row_group_chunks))
        .and_then(|value| value.checked_add(page_assembly_transient))
        .and_then(|value| value.checked_add(WRITER_FIXED_RETAINED_BYTES))
        .ok_or(ColumnarError::SizeOverflow)?;
    let row_group_peak = row_group_local_peak
        .checked_add(metadata_retained)
        .ok_or(ColumnarError::SizeOverflow)?;
    let footer_peak = metadata_retained
        .checked_add(WRITER_FIXED_RETAINED_BYTES)
        .ok_or(ColumnarError::SizeOverflow)?;
    Ok(WriterMemoryEstimate {
        row_group_bytes: row_group_local_peak,
        retained_bytes: row_group_peak.max(footer_peak),
    })
}

fn maximum_text_bytes(
    table: &CellEmbeddingTable,
    chunk_rows: usize,
) -> Result<(usize, usize), ColumnarError> {
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
        let row = table.row(index).map_err(|_| ColumnarError::ParquetWriter)?;
        identifiers = identifiers
            .checked_add(row.cell_id().as_str().len())
            .ok_or(ColumnarError::SizeOverflow)?;
        statuses = statuses
            .checked_add(row.status().wire_name().len())
            .ok_or(ColumnarError::SizeOverflow)?;
    }
    Ok((
        maximum_identifiers.max(identifiers),
        maximum_statuses.max(statuses),
    ))
}

fn total_text_bytes(table: &CellEmbeddingTable) -> Result<(usize, usize), ColumnarError> {
    let mut identifiers = 0_usize;
    let mut statuses = 0_usize;
    for index in 0..table.row_count() {
        let row = table.row(index).map_err(|_| ColumnarError::ParquetWriter)?;
        identifiers = identifiers
            .checked_add(row.cell_id().as_str().len())
            .ok_or(ColumnarError::SizeOverflow)?;
        statuses = statuses
            .checked_add(row.status().wire_name().len())
            .ok_or(ColumnarError::SizeOverflow)?;
    }
    Ok((identifiers, statuses))
}

pub(super) fn estimate_metadata_retained_bytes(
    row_group_count: usize,
) -> Result<usize, ColumnarError> {
    estimate_metadata_retained_bytes_for_columns(row_group_count, 3)
}

pub(super) fn estimate_metadata_retained_bytes_for_columns(
    row_group_count: usize,
    column_count: usize,
) -> Result<usize, ColumnarError> {
    let raw_group_inline = size_of::<RowGroup>()
        .checked_add(
            column_count
                .checked_mul(size_of::<ColumnChunk>())
                .ok_or(ColumnarError::SizeOverflow)?,
        )
        .and_then(|value| value.checked_add(column_count.checked_mul(size_of::<ColumnMetaData>())?))
        .and_then(|value| value.checked_add(column_count.checked_mul(2 * size_of::<Encoding>())?))
        .and_then(|value| {
            value.checked_add(
                column_count
                    .checked_add(2)?
                    .checked_mul(size_of::<String>())?,
            )
        })
        .and_then(|value| {
            value.checked_add(column_count.checked_mul(size_of::<PageEncodingStats>())?)
        })
        .and_then(|value| value.checked_add(128))
        .ok_or(ColumnarError::SizeOverflow)?;
    let high_level_group_inline = size_of::<RowGroupMetaData>()
        .checked_add(
            column_count
                .checked_mul(size_of::<ColumnChunkMetaData>())
                .ok_or(ColumnarError::SizeOverflow)?,
        )
        .and_then(|value| value.checked_add(256))
        .ok_or(ColumnarError::SizeOverflow)?;
    let per_group = raw_group_inline
        .checked_add(high_level_group_inline)
        .and_then(|value| value.checked_add(MAXIMUM_ENCODED_FOOTER_BYTES_PER_ROW_GROUP))
        .and_then(|value| value.checked_mul(2))
        .ok_or(ColumnarError::SizeOverflow)?;
    let base = MAXIMUM_APPLICATION_METADATA_BYTES
        .checked_add(MAXIMUM_ENCODED_FOOTER_BASE_BYTES)
        .and_then(|value| value.checked_add(size_of::<FileMetaData>()))
        .and_then(|value| value.checked_add(size_of::<ParquetMetaData>()))
        .ok_or(ColumnarError::SizeOverflow)?;
    base.checked_add(
        row_group_count
            .checked_mul(per_group)
            .ok_or(ColumnarError::SizeOverflow)?,
    )
    .ok_or(ColumnarError::SizeOverflow)
}

pub(super) fn validity_bytes(values: usize) -> Result<usize, ColumnarError> {
    values
        .checked_add(7)
        .map(|value| value / 8)
        .ok_or(ColumnarError::SizeOverflow)
}

pub(super) struct ParquetDigestingWriter<'a> {
    output: &'a mut (dyn Write + Send),
    digest: ContentDigestWriter,
    byte_len: u64,
    maximum_file_bytes: u64,
    budget_exceeded: Option<u64>,
    trailer: [u8; TRAILER_BYTES],
}

impl<'a> ParquetDigestingWriter<'a> {
    pub(super) fn new(output: &'a mut (dyn Write + Send), maximum_file_bytes: u64) -> Self {
        Self {
            output,
            digest: ContentDigestWriter::default(),
            byte_len: 0,
            maximum_file_bytes,
            budget_exceeded: None,
            trailer: [0; TRAILER_BYTES],
        }
    }

    pub(super) fn budget_exceeded(&self) -> Option<u64> {
        self.budget_exceeded
    }

    pub(super) fn finish(self) -> (ContentDigest, u64) {
        self.digest.finish()
    }

    /// Require the trailer written last to declare a footer of at most `maximum_bytes`, so that
    /// preflight accepts the file.
    pub(super) fn require_footer_within(&self, maximum_bytes: usize) -> Result<(), ColumnarError> {
        let (length, magic) = self.trailer.split_at(TRAILER_BYTES - PARQUET_MAGIC.len());
        let complete = self.byte_len >= TRAILER_BYTES as u64 && magic == PARQUET_MAGIC;
        let footer_bytes = length
            .try_into()
            .map(u32::from_le_bytes)
            .map_err(|_| ColumnarError::ParquetWriter)?;
        match usize::try_from(footer_bytes) {
            Ok(bytes) if complete && bytes <= maximum_bytes => Ok(()),
            _ => Err(ColumnarError::ParquetWriter),
        }
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
        let kept = written.min(TRAILER_BYTES);
        self.trailer.copy_within(kept.., 0);
        self.trailer[TRAILER_BYTES - kept..].copy_from_slice(&buffer[written - kept..written]);
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

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::ParquetDigestingWriter;

    #[test]
    fn footer_check_reads_the_trailer_across_split_writes() {
        let mut output = Vec::new();
        let mut sink = ParquetDigestingWriter::new(&mut output, 1024);
        sink.write_all(b"PAR1footer").expect("body");
        sink.write_all(&6_u32.to_le_bytes()[..3])
            .expect("split length");
        assert!(sink.require_footer_within(6).is_err(), "no trailer yet");
        sink.write_all(&6_u32.to_le_bytes()[3..])
            .expect("split length");
        sink.write_all(b"PA").expect("split magic");
        sink.write_all(b"R1").expect("split magic");
        assert!(sink.require_footer_within(6).is_ok());
        assert!(sink.require_footer_within(5).is_err());
    }
}
