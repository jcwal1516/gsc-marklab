use std::{
    io::{Read, Seek, SeekFrom},
    str::FromStr,
};

use arrow_ipc::{
    convert::fb_to_schema, root_as_footer_with_opts, root_as_message_with_opts, MessageHeader,
    MetadataVersion,
};
use marklab_project::ContentDigest;

use crate::ExpectedCellSet;

use super::super::{
    ArrowIpcFailure, CellEmbeddingTablePhysicalBindings, EmbeddingColumnarBudgets,
    EmbeddingColumnarError,
};
use super::{
    preflight::{validate_record_batch_message, CellEmbeddingArrowPreflight},
    profile::{
        arrow_failure, component_bytes, embedding_schema, enforce_decoded_budget,
        enforce_retained_budget, enforce_row_group_budget, footer_verifier_options,
        nonnegative_i32_usize, nonnegative_usize, schema_message_verifier_options,
        validate_flatbuffer_schema, validate_shape, ALIGNMENT, ARROW_MAGIC, CONTINUATION_MARKER,
        EOS_BYTES, HEADER_BYTES, MAXIMUM_FOOTER_BYTES, MAXIMUM_MESSAGE_BYTES, METADATA_KEYS,
        RECORD_BATCH_ROWS, TRAILER_BYTES,
    },
};

pub(super) fn preflight_cell_embedding_table_arrow_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    content_digest: ContentDigest,
    expected: &ExpectedCellSet,
    bindings: CellEmbeddingTablePhysicalBindings,
    budgets: EmbeddingColumnarBudgets,
) -> Result<CellEmbeddingArrowPreflight, EmbeddingColumnarError> {
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
    let dimension = validate_flatbuffer_schema(footer_schema, bindings)?;
    if fb_to_schema(footer_schema) != embedding_schema(dimension, bindings)? {
        return Err(arrow_failure(ArrowIpcFailure::InvalidSchema));
    }
    validate_shape(expected.cells().len(), dimension)?;
    enforce_decoded_budget(component_bytes(expected.cells().len(), dimension)?, budgets)?;

    let schema_end =
        validate_initial_schema_message(reader, file_len, footer_start, dimension, bindings)?;
    let record_batches = footer
        .recordBatches()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidFooter))?;
    let record_batch_count = record_batches.len();
    if record_batch_count != expected.cells().len().div_ceil(RECORD_BATCH_ROWS) {
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
    for (batch_index, block) in record_batches.iter().enumerate() {
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
        if block_end > footer_start - EOS_BYTES {
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
        let batch_start = batch_index
            .checked_mul(RECORD_BATCH_ROWS)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let expected_rows = expected
            .cells()
            .len()
            .saturating_sub(batch_start)
            .min(RECORD_BATCH_ROWS);
        let batch_end = batch_start
            .checked_add(expected_rows)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        let expected_cells = expected
            .cells()
            .get(batch_start..batch_end)
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidRowCount))?;
        validate_record_batch_message(
            message_bytes,
            body_bytes,
            block.bodyLength(),
            expected_rows,
            expected_cells,
            dimension,
            &mut decoded_bytes,
        )?;
        aggregate_rows = aggregate_rows
            .checked_add(expected_rows)
            .ok_or(EmbeddingColumnarError::SizeOverflow)?;
        next_offset = block_end;
    }
    if next_offset != footer_start - EOS_BYTES || aggregate_rows != expected.cells().len() {
        return Err(arrow_failure(ArrowIpcFailure::InvalidRowCount));
    }
    if decoded_bytes > budgets.maximum_decoded_bytes() {
        return Err(EmbeddingColumnarError::DecodedByteBudgetExceeded {
            required: decoded_bytes,
            maximum: budgets.maximum_decoded_bytes(),
        });
    }

    Ok(CellEmbeddingArrowPreflight::new(
        u64::try_from(aggregate_rows).map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
        dimension,
        u32::try_from(record_batch_count).map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
        encoded_byte_len,
        content_digest,
        retained_required,
    ))
}

pub(super) fn declared_table_logical_digest_reader<R: Read + Seek + ?Sized>(
    reader: &mut R,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ContentDigest, EmbeddingColumnarError> {
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
    let (_, _, footer_bytes) = read_footer(reader, file_len, budgets)?;
    let footer = root_as_footer_with_opts(&footer_verifier_options(), &footer_bytes)
        .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidFooter))?;
    let schema = footer
        .schema()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidSchema))?;
    let metadata = schema
        .custom_metadata()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata))?;
    if metadata.len() != METADATA_KEYS.len() {
        return Err(arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata));
    }
    let entry = metadata.get(2);
    if entry.key() != Some("marklab.logical_digest") {
        return Err(arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata));
    }
    ContentDigest::from_str(
        entry
            .value()
            .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata))?,
    )
    .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidApplicationMetadata))
}

fn read_footer<R: Read + Seek + ?Sized>(
    reader: &mut R,
    file_len: usize,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(usize, usize, Vec<u8>), EmbeddingColumnarError> {
    if file_len < HEADER_BYTES + EOS_BYTES + TRAILER_BYTES {
        return Err(arrow_failure(ArrowIpcFailure::InvalidMagic));
    }
    let mut header = [0_u8; HEADER_BYTES];
    read_exact_at(
        reader,
        file_len,
        0,
        &mut header,
        ArrowIpcFailure::InvalidMagic,
    )?;
    let trailer_start = file_len
        .checked_sub(TRAILER_BYTES)
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidFooterLength))?;
    let mut trailer = [0_u8; TRAILER_BYTES];
    read_exact_at(
        reader,
        file_len,
        trailer_start,
        &mut trailer,
        ArrowIpcFailure::InvalidFooterLength,
    )?;
    if header.get(..ARROW_MAGIC.len()) != Some(ARROW_MAGIC)
        || header[ARROW_MAGIC.len()..].iter().any(|byte| *byte != 0)
        || trailer.get(4..) != Some(ARROW_MAGIC)
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidMagic));
    }
    let signed_footer_length = i32::from_le_bytes(
        trailer[..4]
            .try_into()
            .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidFooterLength))?,
    );
    if signed_footer_length <= 0 {
        return Err(arrow_failure(ArrowIpcFailure::InvalidFooterLength));
    }
    let footer_length = usize::try_from(signed_footer_length)
        .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidFooterLength))?;
    if footer_length > MAXIMUM_FOOTER_BYTES
        || footer_length > file_len.saturating_sub(TRAILER_BYTES)
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidFooterLength));
    }
    let footer_start = trailer_start
        .checked_sub(footer_length)
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidFooterLength))?;
    if footer_start < HEADER_BYTES + EOS_BYTES {
        return Err(arrow_failure(ArrowIpcFailure::InvalidFooterLength));
    }
    let mut eos = [0_u8; EOS_BYTES];
    read_exact_at(
        reader,
        file_len,
        footer_start - EOS_BYTES,
        &mut eos,
        ArrowIpcFailure::InvalidFooterLength,
    )?;
    if &eos[..4] != CONTINUATION_MARKER || eos[4..] != [0, 0, 0, 0] {
        return Err(arrow_failure(ArrowIpcFailure::InvalidFooterLength));
    }
    let retained_required = footer_length
        .checked_mul(2)
        .and_then(|value| value.checked_add(MAXIMUM_MESSAGE_BYTES))
        .ok_or(EmbeddingColumnarError::SizeOverflow)?;
    enforce_retained_budget(retained_required, budgets)?;
    let footer_bytes = read_vec_at(
        reader,
        file_len,
        footer_start,
        footer_length,
        ArrowIpcFailure::InvalidFooter,
    )?;
    Ok((footer_start, footer_length, footer_bytes))
}

fn validate_initial_schema_message<R: Read + Seek + ?Sized>(
    reader: &mut R,
    file_len: usize,
    footer_start: usize,
    dimension: u32,
    bindings: CellEmbeddingTablePhysicalBindings,
) -> Result<usize, EmbeddingColumnarError> {
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
    let declared = i32::from_le_bytes(
        prefix[4..]
            .try_into()
            .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidMessage))?,
    );
    let declared = nonnegative_i32_usize(declared, ArrowIpcFailure::InvalidMessage)?;
    let metadata_length = declared
        .checked_add(8)
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidMessage))?;
    let end = HEADER_BYTES
        .checked_add(metadata_length)
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidMessage))?;
    if declared == 0
        || metadata_length > MAXIMUM_MESSAGE_BYTES
        || metadata_length % ALIGNMENT != 0
        || end > footer_start - EOS_BYTES
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidMessage));
    }
    let message_bytes = read_vec_at(
        reader,
        file_len,
        HEADER_BYTES,
        metadata_length,
        ArrowIpcFailure::InvalidMessage,
    )?;
    let message =
        root_as_message_with_opts(&schema_message_verifier_options(), &message_bytes[8..])
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
    if validate_flatbuffer_schema(schema, bindings)? != dimension
        || fb_to_schema(schema) != embedding_schema(dimension, bindings)?
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidSchema));
    }
    Ok(end)
}

fn read_exact_at<R: Read + Seek + ?Sized>(
    reader: &mut R,
    file_len: usize,
    offset: usize,
    buffer: &mut [u8],
    range_failure: ArrowIpcFailure,
) -> Result<(), EmbeddingColumnarError> {
    if offset
        .checked_add(buffer.len())
        .is_none_or(|end| end > file_len)
    {
        return Err(arrow_failure(range_failure));
    }
    reader
        .seek(SeekFrom::Start(
            u64::try_from(offset).map_err(|_| EmbeddingColumnarError::SizeOverflow)?,
        ))
        .map_err(|_| arrow_failure(ArrowIpcFailure::ArtifactRead))?;
    reader
        .read_exact(buffer)
        .map_err(|_| arrow_failure(ArrowIpcFailure::ArtifactRead))
}

fn read_vec_at<R: Read + Seek + ?Sized>(
    reader: &mut R,
    file_len: usize,
    offset: usize,
    length: usize,
    range_failure: ArrowIpcFailure,
) -> Result<Vec<u8>, EmbeddingColumnarError> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| EmbeddingColumnarError::AllocationFailed { requested: length })?;
    bytes.resize(length, 0);
    read_exact_at(reader, file_len, offset, &mut bytes, range_failure)?;
    Ok(bytes)
}
