use std::{
    io::{Read, Seek, SeekFrom, Write},
    mem::size_of,
};

use arrow::{datatypes::Schema, record_batch::RecordBatch};
use arrow_ipc::{
    convert::fb_to_schema,
    root_as_footer_with_opts, root_as_message_with_opts,
    writer::{FileWriter, IpcWriteOptions},
    MessageHeader, MetadataVersion, Type,
};
use marklab_project::ContentDigest;

use crate::columnar::{
    arrow::{
        profile::{
            footer_verifier_options, message_verifier_options, schema_message_verifier_options,
            ALIGNMENT, ARROW_MAGIC, CONTINUATION_MARKER, EOS_BYTES, HEADER_BYTES,
            MAXIMUM_FOOTER_BYTES, MAXIMUM_MESSAGE_BYTES, RECORD_BATCH_ROWS, TRAILER_BYTES,
        },
        writer::DigestingWriter,
    },
    enforce_decoded_budget, enforce_retained_budget, enforce_row_group_budget, ArrowIpcFailure,
    ColumnarError, EmbeddingColumnarBudgets,
};

use super::profile::arrow_failure;

pub(crate) struct EncodedArrow {
    pub(crate) content_digest: ContentDigest,
    pub(crate) encoded_byte_len: u64,
    pub(crate) row_count: u64,
}

pub(crate) fn validate_record_batch_features(
    batch: arrow_ipc::RecordBatch<'_>,
) -> Result<(), ColumnarError> {
    if batch.compression().is_some()
        || batch
            .variadicBufferCounts()
            .is_some_and(|counts| !counts.is_empty())
    {
        return Err(arrow_failure(ArrowIpcFailure::ForbiddenFeature));
    }
    Ok(())
}

pub(crate) fn validate_footer_features(footer: arrow_ipc::Footer<'_>) -> Result<(), ColumnarError> {
    if footer
        .custom_metadata()
        .is_some_and(|metadata| !metadata.is_empty())
        || footer
            .dictionaries()
            .is_some_and(|dictionaries| !dictionaries.is_empty())
    {
        return Err(arrow_failure(ArrowIpcFailure::ForbiddenFeature));
    }
    Ok(())
}

pub(crate) fn decode_footer(bytes: &[u8]) -> Result<arrow_ipc::Footer<'_>, ColumnarError> {
    let footer = root_as_footer_with_opts(&footer_verifier_options(), bytes)
        .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidFooter))?;
    if footer.version() != MetadataVersion::V5 {
        return Err(arrow_failure(ArrowIpcFailure::UnsupportedVersion));
    }
    Ok(footer)
}

/// Admit the schema frame without interpreting its family-specific fields.
pub(crate) fn schema_message_end<R: Read + Seek + ?Sized>(
    reader: &mut R,
    file_len: usize,
    footer_start: usize,
) -> Result<usize, ColumnarError> {
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
    let declared = nonnegative_i32(
        i32::from_le_bytes(
            prefix[4..]
                .try_into()
                .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidMessage))?,
        ),
        ArrowIpcFailure::InvalidMessage,
    )?;
    let metadata_len = declared.checked_add(8).ok_or(ColumnarError::SizeOverflow)?;
    let end = HEADER_BYTES
        .checked_add(metadata_len)
        .ok_or(ColumnarError::SizeOverflow)?;
    if declared == 0
        || metadata_len > MAXIMUM_MESSAGE_BYTES
        || metadata_len % ALIGNMENT != 0
        || end > footer_start.saturating_sub(EOS_BYTES)
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidMessage));
    }
    Ok(end)
}

pub(crate) fn validate_schema_message<R, F, P>(
    reader: &mut R,
    file_len: usize,
    footer_start: usize,
    expected_schema: &Schema,
    validate_schema: F,
    validate_padding: P,
) -> Result<usize, ColumnarError>
where
    R: Read + Seek + ?Sized,
    F: for<'a> FnOnce(arrow_ipc::Schema<'a>) -> Result<(), ColumnarError>,
    P: FnOnce(&[u8]) -> Result<(), ColumnarError>,
{
    let end = schema_message_end(reader, file_len, footer_start)?;
    let metadata_len = end - HEADER_BYTES;
    let bytes = read_vec_at(
        reader,
        file_len,
        HEADER_BYTES,
        metadata_len,
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
    validate_schema(schema)?;
    if fb_to_schema(schema) != *expected_schema {
        return Err(arrow_failure(ArrowIpcFailure::InvalidSchema));
    }
    validate_padding(&bytes)?;
    Ok(end)
}

pub(crate) fn decode_record_batch_message<'a>(
    message_bytes: &'a [u8],
    body: &[u8],
    declared_body_len: i64,
) -> Result<arrow_ipc::RecordBatch<'a>, ColumnarError> {
    if message_bytes.len() < 8 || &message_bytes[..4] != CONTINUATION_MARKER {
        return Err(arrow_failure(ArrowIpcFailure::InvalidMessage));
    }
    let declared = nonnegative_i32(
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
        || message.bodyLength() != declared_body_len
        || usize::try_from(message.bodyLength()).ok() != Some(body.len())
        || message
            .custom_metadata()
            .is_some_and(|metadata| !metadata.is_empty())
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidMessage));
    }
    message
        .header_as_record_batch()
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidMessage))
}

pub(crate) fn validate_required_utf8_field(
    field: arrow_ipc::Field<'_>,
    name: &str,
) -> Result<(), ColumnarError> {
    if field.name() != Some(name)
        || field.nullable()
        || field.dictionary().is_some()
        || field
            .custom_metadata()
            .is_some_and(|metadata| !metadata.is_empty())
        || field
            .children()
            .is_some_and(|children| !children.is_empty())
        || field.type_type() != Type::Utf8
        || field.type_as_utf_8().is_none()
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidSchema));
    }
    Ok(())
}

pub(crate) fn write_record_batches<F>(
    output: &mut dyn Write,
    schema: &Schema,
    row_count: usize,
    budgets: EmbeddingColumnarBudgets,
    mut build_batch: F,
) -> Result<EncodedArrow, ColumnarError>
where
    F: FnMut(&Schema, usize, usize) -> Result<RecordBatch, ColumnarError>,
{
    let options = IpcWriteOptions::try_new(ALIGNMENT, false, MetadataVersion::V5)
        .map_err(|_| ColumnarError::ArrowWriter)?;
    let mut sink = DigestingWriter::new(output, budgets.maximum_file_bytes());
    let write_result = (|| {
        let mut writer = FileWriter::try_new_with_options(&mut sink, schema, options)
            .map_err(|_| ColumnarError::ArrowWriter)?;
        let mut start = 0_usize;
        while start < row_count {
            let end = start
                .checked_add(RECORD_BATCH_ROWS)
                .map(|value| value.min(row_count))
                .ok_or(ColumnarError::SizeOverflow)?;
            writer
                .write(&build_batch(schema, start, end)?)
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
    Ok(EncodedArrow {
        content_digest,
        encoded_byte_len,
        row_count: u64::try_from(row_count).map_err(|_| ColumnarError::SizeOverflow)?,
    })
}

pub(crate) fn enforce_writer_budgets(
    row_count: usize,
    decoded_bytes: u64,
    maximum_batch_bytes: usize,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(), ColumnarError> {
    enforce_decoded_budget(decoded_bytes, budgets)?;
    enforce_row_group_budget(maximum_batch_bytes, budgets)?;
    let record_blocks = row_count
        .div_ceil(RECORD_BATCH_ROWS)
        .checked_mul(size_of::<arrow_ipc::Block>())
        .ok_or(ColumnarError::SizeOverflow)?;
    let block_capacity = record_blocks
        .checked_mul(2)
        .and_then(|value| value.checked_add(size_of::<Vec<arrow_ipc::Block>>()))
        .ok_or(ColumnarError::SizeOverflow)?;
    let batch_peak = maximum_batch_bytes
        .checked_mul(2)
        .and_then(|value| value.checked_add(block_capacity))
        .ok_or(ColumnarError::SizeOverflow)?;
    let footer_peak = record_blocks
        .checked_add(MAXIMUM_MESSAGE_BYTES)
        .and_then(|value| value.checked_mul(2))
        .and_then(|value| value.checked_add(block_capacity))
        .and_then(|value| value.checked_add(MAXIMUM_MESSAGE_BYTES))
        .ok_or(ColumnarError::SizeOverflow)?;
    enforce_retained_budget(batch_peak.max(footer_peak), budgets)
}

pub(crate) fn read_footer<R: Read + Seek + ?Sized>(
    reader: &mut R,
    file_len: usize,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(usize, usize, Vec<u8>), ColumnarError> {
    read_footer_with_eos_policy(reader, file_len, budgets, true)
}

pub(crate) fn read_footer_deferred_eos<R: Read + Seek + ?Sized>(
    reader: &mut R,
    file_len: usize,
    budgets: EmbeddingColumnarBudgets,
) -> Result<(usize, usize, Vec<u8>), ColumnarError> {
    read_footer_with_eos_policy(reader, file_len, budgets, false)
}

fn read_footer_with_eos_policy<R: Read + Seek + ?Sized>(
    reader: &mut R,
    file_len: usize,
    budgets: EmbeddingColumnarBudgets,
    validate_eos: bool,
) -> Result<(usize, usize, Vec<u8>), ColumnarError> {
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
    let signed = i32::from_le_bytes(
        trailer[..4]
            .try_into()
            .map_err(|_| arrow_failure(ArrowIpcFailure::InvalidFooterLength))?,
    );
    if signed <= 0 {
        return Err(arrow_failure(ArrowIpcFailure::InvalidFooterLength));
    }
    let footer_len =
        usize::try_from(signed).map_err(|_| arrow_failure(ArrowIpcFailure::InvalidFooterLength))?;
    if footer_len > MAXIMUM_FOOTER_BYTES || footer_len > trailer_start {
        return Err(arrow_failure(ArrowIpcFailure::InvalidFooterLength));
    }
    let footer_start = trailer_start
        .checked_sub(footer_len)
        .ok_or_else(|| arrow_failure(ArrowIpcFailure::InvalidFooterLength))?;
    if validate_eos {
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
        if &eos[..4] != CONTINUATION_MARKER || eos[4..] != [0; 4] {
            return Err(arrow_failure(ArrowIpcFailure::InvalidFooterLength));
        }
    }
    let retained = footer_len
        .checked_mul(2)
        .and_then(|value| value.checked_add(MAXIMUM_MESSAGE_BYTES))
        .ok_or(ColumnarError::SizeOverflow)?;
    enforce_retained_budget(retained, budgets)?;
    let footer = read_vec_at(
        reader,
        file_len,
        footer_start,
        footer_len,
        ArrowIpcFailure::InvalidFooter,
    )?;
    Ok((footer_start, footer_len, footer))
}

pub(crate) fn read_exact_at<R: Read + Seek + ?Sized>(
    reader: &mut R,
    file_len: usize,
    offset: usize,
    buffer: &mut [u8],
    failure: ArrowIpcFailure,
) -> Result<(), ColumnarError> {
    if offset
        .checked_add(buffer.len())
        .is_none_or(|end| end > file_len)
    {
        return Err(arrow_failure(failure));
    }
    reader
        .seek(SeekFrom::Start(
            u64::try_from(offset).map_err(|_| ColumnarError::SizeOverflow)?,
        ))
        .and_then(|_| reader.read_exact(buffer))
        .map_err(|_| arrow_failure(ArrowIpcFailure::ArtifactRead))
}

pub(crate) fn read_vec_at<R: Read + Seek + ?Sized>(
    reader: &mut R,
    file_len: usize,
    offset: usize,
    length: usize,
    failure: ArrowIpcFailure,
) -> Result<Vec<u8>, ColumnarError> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| ColumnarError::AllocationFailed { requested: length })?;
    bytes.resize(length, 0);
    read_exact_at(reader, file_len, offset, &mut bytes, failure)?;
    Ok(bytes)
}

pub(crate) struct EncodedRecordBatchBlock {
    bytes: Vec<u8>,
    metadata_len: usize,
    pub(crate) end: usize,
    pub(crate) retained_preflight_bytes: usize,
}

pub(crate) struct RecordBatchTraversal {
    pub(crate) retained_preflight_bytes: usize,
    pub(crate) maximum_batch_decoded_bytes: u64,
    pub(crate) decoded_bytes: u64,
    pub(crate) row_count: usize,
    pub(crate) end_offset: usize,
}

impl EncodedRecordBatchBlock {
    pub(crate) fn message_and_body(&self) -> (&[u8], &[u8]) {
        self.bytes.split_at(self.metadata_len)
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn read_record_batch_block<R: Read + Seek + ?Sized>(
    reader: &mut R,
    file_len: usize,
    footer_start: usize,
    footer_len: usize,
    expected_offset: usize,
    offset: i64,
    metadata_len: i32,
    body_len: i64,
    budgets: EmbeddingColumnarBudgets,
) -> Result<EncodedRecordBatchBlock, ColumnarError> {
    let offset = nonnegative_i64(offset, ArrowIpcFailure::InvalidBlock)?;
    let metadata_len = nonnegative_i32(metadata_len, ArrowIpcFailure::InvalidBlock)?;
    let body_len = nonnegative_i64(body_len, ArrowIpcFailure::InvalidBlock)?;
    if offset != expected_offset
        || offset % ALIGNMENT != 0
        || !(8..=MAXIMUM_MESSAGE_BYTES).contains(&metadata_len)
        || metadata_len % ALIGNMENT != 0
        || body_len % ALIGNMENT != 0
    {
        return Err(arrow_failure(ArrowIpcFailure::InvalidBlock));
    }
    let block_len = metadata_len
        .checked_add(body_len)
        .ok_or(ColumnarError::SizeOverflow)?;
    crate::columnar::multiscale::enforce_row_group_budget(block_len, budgets)?;
    let retained_preflight_bytes = footer_len
        .checked_mul(2)
        .and_then(|value| value.checked_add(block_len))
        .and_then(|value| value.checked_add(MAXIMUM_MESSAGE_BYTES))
        .ok_or(ColumnarError::SizeOverflow)?;
    enforce_retained_budget(retained_preflight_bytes, budgets)?;
    let end = offset
        .checked_add(block_len)
        .ok_or(ColumnarError::SizeOverflow)?;
    if end > footer_start.saturating_sub(EOS_BYTES) {
        return Err(arrow_failure(ArrowIpcFailure::InvalidBlock));
    }
    let bytes = read_vec_at(
        reader,
        file_len,
        offset,
        block_len,
        ArrowIpcFailure::InvalidBlock,
    )?;
    Ok(EncodedRecordBatchBlock {
        bytes,
        metadata_len,
        end,
        retained_preflight_bytes,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn traverse_record_batches<'a, R, I, F>(
    reader: &mut R,
    file_len: usize,
    footer_start: usize,
    footer_len: usize,
    schema_end: usize,
    blocks: I,
    row_count: usize,
    budgets: EmbeddingColumnarBudgets,
    mut validate_batch: F,
) -> Result<RecordBatchTraversal, ColumnarError>
where
    R: Read + Seek + ?Sized,
    I: Iterator<Item = &'a arrow_ipc::Block>,
    F: FnMut(&[u8], &[u8], i64, usize, usize) -> Result<u64, ColumnarError>,
{
    let footer_peak = footer_len
        .checked_mul(2)
        .and_then(|value| value.checked_add(MAXIMUM_MESSAGE_BYTES))
        .ok_or(ColumnarError::SizeOverflow)?;
    enforce_retained_budget(footer_peak, budgets)?;
    let mut traversal = RecordBatchTraversal {
        retained_preflight_bytes: footer_peak,
        maximum_batch_decoded_bytes: 0,
        decoded_bytes: 0,
        row_count: 0,
        end_offset: schema_end,
    };
    for (batch_index, block) in blocks.enumerate() {
        let physical = read_record_batch_block(
            reader,
            file_len,
            footer_start,
            footer_len,
            traversal.end_offset,
            block.offset(),
            block.metaDataLength(),
            block.bodyLength(),
            budgets,
        )?;
        traversal.retained_preflight_bytes = traversal
            .retained_preflight_bytes
            .max(physical.retained_preflight_bytes);
        let (message, body) = physical.message_and_body();
        let start = batch_index
            .checked_mul(RECORD_BATCH_ROWS)
            .ok_or(ColumnarError::SizeOverflow)?;
        let rows = row_count.saturating_sub(start).min(RECORD_BATCH_ROWS);
        let decoded = validate_batch(message, body, block.bodyLength(), start, rows)?;
        traversal.decoded_bytes = traversal
            .decoded_bytes
            .checked_add(decoded)
            .ok_or(ColumnarError::SizeOverflow)?;
        traversal.maximum_batch_decoded_bytes = traversal.maximum_batch_decoded_bytes.max(decoded);
        traversal.row_count = traversal
            .row_count
            .checked_add(rows)
            .ok_or(ColumnarError::SizeOverflow)?;
        traversal.end_offset = physical.end;
    }
    Ok(traversal)
}

pub(crate) fn checked_range(
    start: usize,
    rows: usize,
) -> Result<std::ops::Range<usize>, ColumnarError> {
    Ok(start..start.checked_add(rows).ok_or(ColumnarError::SizeOverflow)?)
}

pub(crate) fn validity_bytes(rows: usize) -> Result<usize, ColumnarError> {
    rows.checked_add(7)
        .map(|value| value / 8)
        .ok_or(ColumnarError::SizeOverflow)
}

pub(crate) fn align(value: usize) -> Result<usize, ColumnarError> {
    value
        .checked_add(ALIGNMENT - 1)
        .map(|value| value / ALIGNMENT * ALIGNMENT)
        .ok_or(ColumnarError::SizeOverflow)
}

pub(crate) fn nonnegative_i32(
    value: i32,
    failure: ArrowIpcFailure,
) -> Result<usize, ColumnarError> {
    usize::try_from(value).map_err(|_| arrow_failure(failure))
}

pub(crate) fn nonnegative_i64(
    value: i64,
    failure: ArrowIpcFailure,
) -> Result<usize, ColumnarError> {
    usize::try_from(value).map_err(|_| arrow_failure(failure))
}
