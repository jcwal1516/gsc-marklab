use std::io::{Cursor, Read, Seek, SeekFrom};

use arrow_ipc::convert::fb_to_schema;
use marklab_project::ContentDigest;

use crate::columnar::enforce_file_budget;
use crate::{CellEmbeddingRowLink, ExpectedCellSet};

use super::{
    super::{
        super::super::{ArrowIpcFailure, ColumnarError, EmbeddingColumnarBudgets},
        physical::{
            decode_footer, decode_record_batch_message, read_footer, read_record_batch_block,
            validate_schema_message,
        },
        profile::{
            align, arrow_failure, enforce_decoded_budget, enforce_retained_budget,
            nonnegative_usize, validate_all_valid_bitmap, validity_bytes, ALIGNMENT, EOS_BYTES,
            MAXIMUM_MESSAGE_BYTES, RECORD_BATCH_ROWS,
        },
    },
    profile::{row_link_schema, validate_flatbuffer_schema, validate_nullable_bitmap},
};

/// Validated structural and logical declaration for one canonical Arrow row-link table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellEmbeddingRowLinkArrowPreflight {
    row_count: u64,
    record_batch_count: u32,
    encoded_byte_len: u64,
    content_digest: ContentDigest,
    pub(super) retained_preflight_bytes: usize,
    pub(super) maximum_batch_decoded_bytes: usize,
}

impl CellEmbeddingRowLinkArrowPreflight {
    /// Declared canonical row-link rows.
    pub fn row_count(self) -> u64 {
        self.row_count
    }

    /// Number of canonical record batches.
    pub fn record_batch_count(self) -> u32 {
        self.record_batch_count
    }

    /// Exact encoded Arrow file length.
    pub fn encoded_byte_len(self) -> u64 {
        self.encoded_byte_len
    }

    /// SHA-256 of the exact preflighted bytes.
    pub fn content_digest(self) -> ContentDigest {
        self.content_digest
    }
}

/// Validate hostile row-link Arrow bytes before any stock Arrow decoder sees them.
pub fn preflight_cell_embedding_row_link_arrow_bytes(
    bytes: &[u8],
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellEmbeddingRowLinkArrowPreflight, ColumnarError> {
    let encoded_byte_len = u64::try_from(bytes.len()).map_err(|_| ColumnarError::SizeOverflow)?;
    enforce_file_budget(encoded_byte_len, budgets)?;
    preflight_cell_embedding_row_link_arrow_reader(
        &mut Cursor::new(bytes),
        ContentDigest::from_bytes(bytes),
        expected,
        row_link,
        budgets,
    )
}

pub(super) fn preflight_cell_embedding_row_link_arrow_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    content_digest: ContentDigest,
    expected: &ExpectedCellSet,
    row_link: &CellEmbeddingRowLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellEmbeddingRowLinkArrowPreflight, ColumnarError> {
    if row_link.expected_cells_logical_digest() != expected.logical_digest()
        || row_link.entries().len() != expected.cells().len()
        || row_link
            .entries()
            .iter()
            .zip(expected.cells())
            .any(|(entry, expected)| entry.cell_id() != expected)
    {
        return Err(ColumnarError::ArtifactBindingMismatch);
    }
    let encoded_byte_len = reader
        .seek(SeekFrom::End(0))
        .map_err(|_| arrow_failure(ArrowIpcFailure::ArtifactRead))?;
    enforce_file_budget(encoded_byte_len, budgets)?;
    let file_len = usize::try_from(encoded_byte_len).map_err(|_| ColumnarError::SizeOverflow)?;
    let (footer_start, footer_length, footer_bytes) = read_footer(reader, file_len, budgets)?;
    let data_end = footer_start
        .checked_sub(EOS_BYTES)
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidFooterLength))?;
    let footer = decode_footer(&footer_bytes)?;
    if footer
        .custom_metadata()
        .is_some_and(|metadata| !metadata.is_empty())
    {
        return Err(arrow_failure(ArrowIpcFailure::FileCustomMetadata));
    }
    if footer
        .dictionaries()
        .is_some_and(|dictionaries| !dictionaries.is_empty())
    {
        return Err(arrow_failure(ArrowIpcFailure::DictionaryBatch));
    }
    let footer_schema = footer
        .schema()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidSchema))?;
    validate_flatbuffer_schema(footer_schema, row_link)?;
    if fb_to_schema(footer_schema) != row_link_schema(row_link)? {
        return Err(arrow_failure(ArrowIpcFailure::InvalidSchema));
    }
    enforce_retained_budget(MAXIMUM_MESSAGE_BYTES, budgets)?;
    let expected_schema = row_link_schema(row_link)?;
    let schema_end = validate_schema_message(
        reader,
        file_len,
        footer_start,
        &expected_schema,
        |schema| validate_flatbuffer_schema(schema, row_link),
        |_bytes| Ok(()),
    )?;
    let batches = footer
        .recordBatches()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidFooter))?;
    if batches.len() != row_link.entries().len().div_ceil(RECORD_BATCH_ROWS) {
        return Err(arrow_failure(ArrowIpcFailure::InvalidRowCount));
    }
    let footer_retained = footer_length
        .checked_mul(2)
        .and_then(|value| value.checked_add(MAXIMUM_MESSAGE_BYTES))
        .ok_or(ColumnarError::SizeOverflow)?;
    let mut retained_required = footer_retained;
    let mut next_offset = schema_end;
    let mut aggregate_rows = 0_usize;
    let mut decoded_bytes = 0_u64;
    let mut maximum_batch_decoded_bytes = 0_u64;
    for (batch_index, block) in batches.iter().enumerate() {
        let physical = read_record_batch_block(
            reader,
            file_len,
            footer_start,
            footer_length,
            next_offset,
            block.offset(),
            block.metaDataLength(),
            block.bodyLength(),
            budgets,
        )?;
        retained_required = retained_required.max(physical.retained_preflight_bytes);
        let (message_bytes, body_bytes) = physical.message_and_body();
        let start = batch_index
            .checked_mul(RECORD_BATCH_ROWS)
            .ok_or(ColumnarError::SizeOverflow)?;
        let rows = row_link
            .entries()
            .len()
            .checked_sub(start)
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidRowCount))?
            .min(RECORD_BATCH_ROWS);
        let decoded_before = decoded_bytes;
        validate_record_batch(
            message_bytes,
            body_bytes,
            block.bodyLength(),
            row_link,
            start,
            rows,
            &mut decoded_bytes,
        )?;
        maximum_batch_decoded_bytes = maximum_batch_decoded_bytes.max(
            decoded_bytes
                .checked_sub(decoded_before)
                .ok_or(ColumnarError::SizeOverflow)?,
        );
        aggregate_rows = aggregate_rows
            .checked_add(rows)
            .ok_or(ColumnarError::SizeOverflow)?;
        next_offset = physical.end;
    }
    if next_offset != data_end || aggregate_rows != row_link.entries().len() {
        return Err(arrow_failure(ArrowIpcFailure::InvalidRowCount));
    }
    enforce_decoded_budget(decoded_bytes, budgets)?;
    Ok(CellEmbeddingRowLinkArrowPreflight {
        row_count: u64::try_from(aggregate_rows).map_err(|_| ColumnarError::SizeOverflow)?,
        record_batch_count: u32::try_from(batches.len())
            .map_err(|_| ColumnarError::SizeOverflow)?,
        encoded_byte_len,
        content_digest,
        retained_preflight_bytes: retained_required,
        maximum_batch_decoded_bytes: usize::try_from(maximum_batch_decoded_bytes)
            .map_err(|_| ColumnarError::SizeOverflow)?,
    })
}

fn validate_record_batch(
    message_bytes: &[u8],
    body_bytes: &[u8],
    declared_body_length: i64,
    row_link: &CellEmbeddingRowLink,
    start: usize,
    rows: usize,
    aggregate_decoded_bytes: &mut u64,
) -> Result<(), ColumnarError> {
    let batch = decode_record_batch_message(message_bytes, body_bytes, declared_body_length)?;
    if batch.compression().is_some()
        || batch
            .variadicBufferCounts()
            .is_some_and(|counts| !counts.is_empty())
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidMessage));
    }
    if nonnegative_usize(batch.length(), ArrowIpcFailure::InvalidRowCount)? != rows
        || rows == 0
        || rows > RECORD_BATCH_ROWS
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidRowCount));
    }
    let nodes = batch
        .nodes()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidFieldNodes))?;
    if nodes.len() != 3 {
        return Err(arrow_failure(ArrowIpcFailure::InvalidFieldNodes));
    }
    let end = start.checked_add(rows).ok_or(ColumnarError::SizeOverflow)?;
    let entries = row_link
        .entries()
        .get(start..end)
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidRowCount))?;
    let missing = entries
        .iter()
        .filter(|entry| entry.source_embedding_row().is_none())
        .count();
    for (node, null_count) in nodes.iter().zip([0_usize, 0, missing]) {
        if nonnegative_usize(node.length(), ArrowIpcFailure::InvalidFieldNodes)? != rows
            || nonnegative_usize(node.null_count(), ArrowIpcFailure::InvalidFieldNodes)?
                != null_count
        {
            return Err(arrow_failure(ArrowIpcFailure::InvalidFieldNodes));
        }
    }
    let buffers = batch
        .buffers()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidBuffers))?;
    if buffers.len() != 7 {
        return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
    }
    let validity = validity_bytes(rows)?;
    let offsets = rows
        .checked_add(1)
        .and_then(|value| value.checked_mul(size_of::<i32>()))
        .ok_or(ColumnarError::SizeOverflow)?;
    let cell_bytes = entries.iter().try_fold(0_usize, |total, entry| {
        total
            .checked_add(entry.cell_id().as_str().len())
            .ok_or(ColumnarError::SizeOverflow)
    })?;
    let values = rows
        .checked_mul(size_of::<u64>())
        .ok_or(ColumnarError::SizeOverflow)?;
    let exact_lengths = [
        validity, offsets, cell_bytes, validity, values, validity, values,
    ];
    let mut ranges = [(0_usize, 0_usize); 7];
    let mut next_offset = 0_usize;
    let mut decoded_bytes = 0_u64;
    for (index, (buffer, exact_length)) in buffers.iter().zip(exact_lengths).enumerate() {
        let offset = nonnegative_usize(buffer.offset(), ArrowIpcFailure::InvalidBuffers)?;
        let length = nonnegative_usize(buffer.length(), ArrowIpcFailure::InvalidBuffers)?;
        if offset != align(next_offset)? || offset % ALIGNMENT != 0 || length != exact_length {
            return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
        }
        let end = offset
            .checked_add(length)
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidBuffers))?;
        if end > body_bytes.len() {
            return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
        }
        ranges[index] = (offset, end);
        next_offset = end;
        decoded_bytes = decoded_bytes
            .checked_add(u64::try_from(length).map_err(|_| ColumnarError::SizeOverflow)?)
            .ok_or(ColumnarError::SizeOverflow)?;
    }
    if align(next_offset)? != body_bytes.len() {
        return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
    }
    for index in [0, 3] {
        let bitmap = body_bytes
            .get(ranges[index].0..ranges[index].1)
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidBuffers))?;
        validate_all_valid_bitmap(bitmap, rows)?;
    }
    let nullable_bitmap = body_bytes
        .get(ranges[5].0..ranges[5].1)
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidBuffers))?;
    validate_nullable_bitmap(nullable_bitmap, row_link, start, rows)?;
    validate_values(body_bytes, ranges, row_link, start, rows)?;
    *aggregate_decoded_bytes = aggregate_decoded_bytes
        .checked_add(decoded_bytes)
        .ok_or(ColumnarError::SizeOverflow)?;
    Ok(())
}

fn validate_values(
    body: &[u8],
    ranges: [(usize, usize); 7],
    row_link: &CellEmbeddingRowLink,
    start: usize,
    rows: usize,
) -> Result<(), ColumnarError> {
    let end = start.checked_add(rows).ok_or(ColumnarError::SizeOverflow)?;
    let entries = row_link
        .entries()
        .get(start..end)
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidRowCount))?;
    let offsets = body
        .get(ranges[1].0..ranges[1].1)
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidBuffers))?;
    let cell_data = body
        .get(ranges[2].0..ranges[2].1)
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidBuffers))?;
    let mut expected_offset = 0_usize;
    for local_row in 0..=rows {
        let byte_offset = local_row
            .checked_mul(size_of::<i32>())
            .ok_or(ColumnarError::SizeOverflow)?;
        let byte_end = byte_offset
            .checked_add(size_of::<i32>())
            .ok_or(ColumnarError::SizeOverflow)?;
        let offset = i32::from_le_bytes(
            offsets
                .get(byte_offset..byte_end)
                .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidBuffers))?
                .try_into()
                .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidBuffers))?,
        );
        if usize::try_from(offset).ok() != Some(expected_offset) {
            return Err(arrow_failure(ArrowIpcFailure::InvalidBuffers));
        }
        if local_row < rows {
            expected_offset = expected_offset
                .checked_add(
                    entries
                        .get(local_row)
                        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidRowCount))?
                        .cell_id()
                        .as_str()
                        .len(),
                )
                .ok_or(ColumnarError::SizeOverflow)?;
        }
    }
    let mut cursor = 0_usize;
    for entry in entries {
        let value = entry.cell_id().as_str().as_bytes();
        let value_end = cursor
            .checked_add(value.len())
            .ok_or(ColumnarError::SizeOverflow)?;
        if cell_data.get(cursor..value_end) != Some(value) {
            return Err(arrow_failure(ArrowIpcFailure::InvalidCellOrder));
        }
        cursor = value_end;
    }
    let source_cells = body
        .get(ranges[4].0..ranges[4].1)
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidBuffers))?;
    let source_embeddings = body
        .get(ranges[6].0..ranges[6].1)
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidBuffers))?;
    for (local_row, entry) in entries.iter().enumerate() {
        let offset = local_row
            .checked_mul(size_of::<u64>())
            .ok_or(ColumnarError::SizeOverflow)?;
        let value_end = offset
            .checked_add(size_of::<u64>())
            .ok_or(ColumnarError::SizeOverflow)?;
        let source_cell = u64::from_le_bytes(
            source_cells
                .get(offset..value_end)
                .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidBuffers))?
                .try_into()
                .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidBuffers))?,
        );
        let source_embedding = u64::from_le_bytes(
            source_embeddings
                .get(offset..value_end)
                .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidBuffers))?
                .try_into()
                .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidBuffers))?,
        );
        if source_cell != entry.source_cell_row()
            || source_embedding != entry.source_embedding_row().unwrap_or(0)
        {
            return Err(arrow_failure(ArrowIpcFailure::InvalidComponent));
        }
    }
    Ok(())
}
