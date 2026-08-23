use std::io::{Cursor, Read, Seek, SeekFrom};

use arrow_ipc::{
    convert::fb_to_schema, root_as_footer_with_opts, root_as_message_with_opts, MessageHeader,
    MetadataVersion,
};
use marklab_project::ContentDigest;

use crate::{CellEmbeddingRowLink, ExpectedCellSet};

use super::{
    super::{
        super::super::{ArrowIpcFailure, EmbeddingColumnarBudgets, EmbeddingColumnarError},
        preflight_reader::{read_exact_at, read_footer, read_vec_at},
        profile::{
            align, arrow_failure, enforce_decoded_budget, enforce_retained_budget,
            enforce_row_group_budget, footer_verifier_options, message_verifier_options,
            nonnegative_i32_usize, nonnegative_usize, schema_message_verifier_options,
            validate_all_valid_bitmap, validity_bytes, ALIGNMENT, CONTINUATION_MARKER, EOS_BYTES,
            HEADER_BYTES, MAXIMUM_MESSAGE_BYTES, RECORD_BATCH_ROWS,
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
) -> Result<CellEmbeddingRowLinkArrowPreflight, EmbeddingColumnarError> {
    let encoded_byte_len =
        u64::try_from(bytes.len()).map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
    if encoded_byte_len > budgets.maximum_file_bytes() {
        return Err(EmbeddingColumnarError::FileByteBudgetExceeded {
            observed: encoded_byte_len,
            maximum: budgets.maximum_file_bytes(),
        });
    }
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
) -> Result<CellEmbeddingRowLinkArrowPreflight, EmbeddingColumnarError> {
    if row_link.expected_cells_logical_digest() != expected.logical_digest()
        || row_link.entries().len() != expected.cells().len()
        || row_link
            .entries()
            .iter()
            .zip(expected.cells())
            .any(|(entry, expected)| entry.cell_id() != expected)
    {
        return Err(EmbeddingColumnarError::ArtifactBindingMismatch);
    }
    let encoded_byte_len = reader
        .seek(SeekFrom::End(0))
        .map_err(|_| arrow_failure(ArrowIpcFailure::ArtifactRead))?;
    if encoded_byte_len > budgets.maximum_file_bytes() {
        return Err(EmbeddingColumnarError::FileByteBudgetExceeded {
            observed: encoded_byte_len,
            maximum: budgets.maximum_file_bytes(),
        });
    }
    let file_len =
        usize::try_from(encoded_byte_len).map_err(|_| EmbeddingColumnarError::SizeOverflow)?;
    let (footer_start, footer_length, footer_bytes) = read_footer(reader, file_len, budgets)?;
    let data_end = footer_start
        .checked_sub(EOS_BYTES)
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidFooterLength))?;
    let footer = root_as_footer_with_opts(&footer_verifier_options(), &footer_bytes)
        .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidFooter))?;
    if footer.version() != MetadataVersion::V5 {
        return Err(arrow_failure(ArrowIpcFailure::UnsupportedVersion));
    }
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
    let schema_end = validate_initial_schema(reader, file_len, footer_start, row_link, budgets)?;
    let batches = footer
        .recordBatches()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidFooter))?;
    if batches.len() != row_link.entries().len().div_ceil(RECORD_BATCH_ROWS) {
        return Err(arrow_failure(ArrowIpcFailure::InvalidRowCount));
    }
    let footer_retained = footer_length
        .checked_mul(2)
        .and_then(|value| value.checked_add(MAXIMUM_MESSAGE_BYTES))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let mut retained_required = footer_retained;
    let mut next_offset = schema_end;
    let mut aggregate_rows = 0_usize;
    let mut decoded_bytes = 0_u64;
    for (batch_index, block) in batches.iter().enumerate() {
        let offset = nonnegative_usize(block.offset(), ArrowIpcFailure::InvalidBlock)?;
        let metadata_length =
            nonnegative_i32_usize(block.metaDataLength(), ArrowIpcFailure::InvalidBlock)?;
        let body_length = nonnegative_usize(block.bodyLength(), ArrowIpcFailure::InvalidBlock)?;
        if offset != next_offset
            || offset % ALIGNMENT != 0
            || !(8..=MAXIMUM_MESSAGE_BYTES).contains(&metadata_length)
            || metadata_length % ALIGNMENT != 0
            || body_length % ALIGNMENT != 0
        {
            return Err(arrow_failure(ArrowIpcFailure::InvalidBlock));
        }
        let block_bytes = metadata_length
            .checked_add(body_length)
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidBlock))?;
        enforce_row_group_budget(block_bytes, budgets)?;
        let block_retained = footer_length
            .checked_mul(2)
            .and_then(|value| value.checked_add(block_bytes))
            .and_then(|value| value.checked_add(MAXIMUM_MESSAGE_BYTES))
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        retained_required = retained_required.max(block_retained);
        enforce_retained_budget(retained_required, budgets)?;
        let block_end = offset
            .checked_add(block_bytes)
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidBlock))?;
        if block_end > data_end {
            return Err(arrow_failure(ArrowIpcFailure::InvalidBlock));
        }
        let physical_block = read_vec_at(
            reader,
            file_len,
            offset,
            block_bytes,
            ArrowIpcFailure::InvalidBlock,
        )?;
        let (message_bytes, body_bytes) = physical_block.split_at(metadata_length);
        let start = batch_index
            .checked_mul(RECORD_BATCH_ROWS)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let rows = row_link
            .entries()
            .len()
            .checked_sub(start)
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidRowCount))?
            .min(RECORD_BATCH_ROWS);
        validate_record_batch(
            message_bytes,
            body_bytes,
            block.bodyLength(),
            row_link,
            start,
            rows,
            &mut decoded_bytes,
        )?;
        aggregate_rows = aggregate_rows
            .checked_add(rows)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        next_offset = block_end;
    }
    if next_offset != data_end || aggregate_rows != row_link.entries().len() {
        return Err(arrow_failure(ArrowIpcFailure::InvalidRowCount));
    }
    enforce_decoded_budget(decoded_bytes, budgets)?;
    Ok(CellEmbeddingRowLinkArrowPreflight {
        row_count: u64::try_from(aggregate_rows)
            .map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
        record_batch_count: u32::try_from(batches.len())
            .map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
        encoded_byte_len,
        content_digest,
        retained_preflight_bytes: retained_required,
    })
}

fn validate_initial_schema<R: Read + Seek + ?Sized>(
    reader: &mut R,
    file_len: usize,
    footer_start: usize,
    row_link: &CellEmbeddingRowLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<usize, EmbeddingColumnarError> {
    let data_end = footer_start
        .checked_sub(EOS_BYTES)
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidFooterLength))?;
    let mut prefix = [0_u8; 8];
    read_exact_at(
        reader,
        file_len,
        HEADER_BYTES,
        &mut prefix,
        ArrowIpcFailure::InvalidMessage,
    )?;
    if &prefix[..4] != CONTINUATION_MARKER {
        return Err(arrow_failure(ArrowIpcFailure::InvalidMessage));
    }
    let declared = nonnegative_i32_usize(
        i32::from_le_bytes(
            prefix[4..]
                .try_into()
                .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidMessage))?,
        ),
        ArrowIpcFailure::InvalidMessage,
    )?;
    let metadata_length = declared
        .checked_add(8)
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidMessage))?;
    let end = HEADER_BYTES
        .checked_add(metadata_length)
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidMessage))?;
    if declared == 0
        || metadata_length > MAXIMUM_MESSAGE_BYTES
        || metadata_length % ALIGNMENT != 0
        || end > data_end
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidMessage));
    }
    enforce_retained_budget(MAXIMUM_MESSAGE_BYTES, budgets)?;
    let bytes = read_vec_at(
        reader,
        file_len,
        HEADER_BYTES,
        metadata_length,
        ArrowIpcFailure::InvalidMessage,
    )?;
    let message = root_as_message_with_opts(&schema_message_verifier_options(), &bytes[8..])
        .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidMessage))?;
    if message.version() != MetadataVersion::V5
        || message.header_type() != MessageHeader::Schema
        || message.bodyLength() != 0
        || message
            .custom_metadata()
            .is_some_and(|metadata| !metadata.is_empty())
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidMessage));
    }
    let schema = message
        .header_as_schema()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidSchema))?;
    validate_flatbuffer_schema(schema, row_link)?;
    if fb_to_schema(schema) != row_link_schema(row_link)? {
        return Err(arrow_failure(ArrowIpcFailure::InvalidSchema));
    }
    Ok(end)
}

#[allow(clippy::too_many_arguments)]
fn validate_record_batch(
    message_bytes: &[u8],
    body_bytes: &[u8],
    declared_body_length: i64,
    row_link: &CellEmbeddingRowLink,
    start: usize,
    rows: usize,
    aggregate_decoded_bytes: &mut u64,
) -> Result<(), EmbeddingColumnarError> {
    if message_bytes.len() < 8 || &message_bytes[..4] != CONTINUATION_MARKER {
        return Err(arrow_failure(ArrowIpcFailure::InvalidMessage));
    }
    let declared = nonnegative_i32_usize(
        i32::from_le_bytes(
            message_bytes[4..8]
                .try_into()
                .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidMessage))?,
        ),
        ArrowIpcFailure::InvalidMessage,
    )?;
    if declared == 0
        || declared.checked_add(8) != Some(message_bytes.len())
        || declared > MAXIMUM_MESSAGE_BYTES - 8
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidMessage));
    }
    let message = root_as_message_with_opts(&message_verifier_options(), &message_bytes[8..])
        .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidMessage))?;
    if message.version() != MetadataVersion::V5
        || message.header_type() != MessageHeader::RecordBatch
        || message.bodyLength() != declared_body_length
        || message.bodyLength()
            != i64::try_from(body_bytes.len()).map_err(|_| EmbeddingColumnarError::SizeOverflow)?
        || message
            .custom_metadata()
            .is_some_and(|metadata| !metadata.is_empty())
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidMessage));
    }
    let batch = message
        .header_as_record_batch()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidMessage))?;
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
    let end = start
        .checked_add(rows)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
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
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    let cell_bytes = entries.iter().try_fold(0_usize, |total, entry| {
        total
            .checked_add(entry.cell_id().as_str().len())
            .ok_or(EmbeddingColumnarError::SizeOverflow)
    })?;
    let values = rows
        .checked_mul(size_of::<u64>())
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
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
            .checked_add(u64::try_from(length).map_err(|_| EmbeddingColumnarError::SizeOverflow)?)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
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
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    Ok(())
}

fn validate_values(
    body: &[u8],
    ranges: [(usize, usize); 7],
    row_link: &CellEmbeddingRowLink,
    start: usize,
    rows: usize,
) -> Result<(), EmbeddingColumnarError> {
    let end = start
        .checked_add(rows)
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
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
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let byte_end = byte_offset
            .checked_add(size_of::<i32>())
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
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
                .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        }
    }
    let mut cursor = 0_usize;
    for entry in entries {
        let value = entry.cell_id().as_str().as_bytes();
        let value_end = cursor
            .checked_add(value.len())
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
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
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let value_end = offset
            .checked_add(size_of::<u64>())
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
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
